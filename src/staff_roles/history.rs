//! Protected Administrator audit projection; current names are display context.
use super::{Capability, SCHEMA_VERSION, StaffRoles, admin::Account};
use crate::access::{Access, Role};
use anyhow::{Result, ensure};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;
pub const MAX_EVENTS: usize = 1000;
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    LegacyBackfill,
    AssignmentChanged,
    AccountProvisioned,
}
impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Self::LegacyBackfill => "Recorded baseline",
            Self::AssignmentChanged => "Responsibilities / access changed",
            Self::AccountProvisioned => "Account provisioned",
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RoleEvent {
    pub id: Uuid,
    pub actor_id: Option<Uuid>,
    pub actor_name: Option<String>,
    pub kind: Kind,
    pub previous_roles: Option<StaffRoles>,
    pub roles: StaffRoles,
    pub revision: i64,
    pub reason: String,
    pub previous_active: Option<bool>,
    pub active: Option<bool>,
    pub created_at: DateTime<Utc>,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AccessAction {
    AccountAdded,
    AccessChanged,
}
impl AccessAction {
    pub fn label(self) -> &'static str {
        match self {
            Self::AccountAdded => "Account added",
            Self::AccessChanged => "Access changed",
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AccessEvent {
    pub id: Uuid,
    pub actor_id: Uuid,
    pub actor_name: Option<String>,
    pub action: AccessAction,
    pub previous_role: Option<Role>,
    pub role: Option<Role>,
    pub previous_active: Option<bool>,
    pub active: Option<bool>,
    pub created_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct History {
    pub schema_version: u16,
    pub company_id: Uuid,
    pub actor_id: Uuid,
    pub user_id: Uuid,
    pub employee_id: Uuid,
    pub employee_code: String,
    pub full_name: String,
    pub revision: i64,
    pub current_roles: StaffRoles,
    pub active: bool,
    pub member_updated_at: DateTime<Utc>,
    pub role_event_count: usize,
    pub access_event_count: usize,
    pub role_events: Vec<RoleEvent>,
    pub access_events: Vec<AccessEvent>,
}
fn clean(s: &str, max: usize) -> bool {
    !s.trim().is_empty() && s.chars().count() <= max && !s.chars().any(char::is_control)
}
impl History {
    pub fn validate(&self, actor: &Access, source: &Account) -> Result<()> {
        ensure!(
            actor.role == Role::Administrator
                && actor.staff_contract_version == SCHEMA_VERSION
                && actor.staff_context.is_some()
                && actor.allows(Capability::AccountAdministration),
            "Only a current Administrator reads account history."
        );
        ensure!(
            !self.company_id.is_nil()
                && !self.actor_id.is_nil()
                && !self.user_id.is_nil()
                && !self.employee_id.is_nil()
                && clean(&self.employee_code, 30)
                && clean(&self.full_name, 120)
                && self.revision > 0
                && self.current_roles.compatible_base_role() == source.base_role
                && self.schema_version == SCHEMA_VERSION
                && self.company_id == actor.company_id
                && self.actor_id == actor.user_id
                && self.user_id == source.user_id
                && self.employee_id == source.employee_id
                && self.employee_code == source.employee_code
                && self.full_name == source.full_name
                && self.revision == source.revision
                && self.current_roles == source.roles
                && self.active == source.active
                && self.member_updated_at == source.member_updated_at,
            "Account history does not match the current Administrator and employee source. Refresh."
        );
        ensure!(
            self.role_event_count == self.role_events.len()
                && self.access_event_count == self.access_events.len()
                && self.role_event_count <= MAX_EVENTS
                && self.access_event_count <= MAX_EVENTS,
            "Account history is incomplete or exceeds its complete-view limit."
        );
        let mut ids = HashSet::new();
        let mut previous = 0;
        for e in &self.role_events {
            ensure!(
                !e.id.is_nil()
                    && ids.insert(e.id)
                    && e.revision > previous
                    && e.revision <= self.revision
                    && clean(&e.reason, 500)
                    && e.actor_id.is_none_or(|id| !id.is_nil())
                    && e.actor_name.as_ref().is_none_or(|n| clean(n, 120))
                    && (e.kind == Kind::LegacyBackfill || e.actor_id.is_some()),
                "Invalid immutable staff-role history."
            );
            previous = e.revision;
        }
        ids.clear();
        let mut order = None;
        for e in &self.access_events {
            let at = (e.created_at, e.id);
            ensure!(
                !e.id.is_nil()
                    && ids.insert(e.id)
                    && !e.actor_id.is_nil()
                    && e.actor_name.as_ref().is_none_or(|n| clean(n, 120))
                    && order.is_none_or(|last| last < at),
                "Invalid original access history."
            );
            order = Some(at);
        }
        Ok(())
    }
}
pub fn roles_label(roles: &StaffRoles) -> String {
    roles
        .as_slice()
        .iter()
        .map(|r| r.label())
        .collect::<Vec<_>>()
        .join(", ")
}
pub fn access_label(active: Option<bool>) -> &'static str {
    match active {
        Some(true) => "Enabled",
        Some(false) => "Disabled",
        None => "Not recorded",
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub(crate) fn fixture() -> (Access, Account, History) {
        let mut actor = Access {
            company_id: Uuid::from_u128(1),
            user_id: Uuid::from_u128(2),
            employee_id: Some(Uuid::from_u128(3)),
            role: Role::Administrator,
            staff_contract_version: 1,
            ..Default::default()
        };
        actor
            .bind_staff_context(super::super::Context {
                schema_version: 1,
                company_id: actor.company_id,
                user_id: actor.user_id,
                employee_id: actor.employee_id.unwrap(),
                base_role: actor.role,
                revision: 1,
                runtime_ready: true,
                roles: StaffRoles::legacy(Role::Administrator),
            })
            .unwrap();
        let account = Account {
            user_id: Uuid::from_u128(4),
            employee_id: Uuid::from_u128(5),
            employee_code: "000005".into(),
            full_name: "Fictional staff member with a longer name".into(),
            active: true,
            base_role: Role::Employee,
            member_updated_at: Utc::now(),
            revision: 1,
            roles: StaffRoles::default(),
        };
        let history = History {
            schema_version: 1,
            company_id: actor.company_id,
            actor_id: actor.user_id,
            user_id: account.user_id,
            employee_id: account.employee_id,
            employee_code: account.employee_code.clone(),
            full_name: account.full_name.clone(),
            revision: 1,
            current_roles: account.roles.clone(),
            active: true,
            member_updated_at: account.member_updated_at,
            role_event_count: 1,
            access_event_count: 1,
            role_events: vec![RoleEvent {
                id: Uuid::from_u128(6),
                actor_id: None,
                actor_name: None,
                kind: Kind::LegacyBackfill,
                previous_roles: None,
                roles: StaffRoles::default(),
                revision: 1,
                reason: "Fictional deployment capture; prior values remain unknown".into(),
                previous_active: None,
                active: None,
                created_at: Utc::now(),
            }],
            access_events: vec![AccessEvent {
                id: Uuid::from_u128(7),
                actor_id: actor.user_id,
                actor_name: Some("Fictional current actor name".into()),
                action: AccessAction::AccountAdded,
                previous_role: None,
                role: Some(Role::Employee),
                previous_active: None,
                active: Some(true),
                created_at: Utc::now(),
            }],
        };
        (actor, account, history)
    }
    #[test]
    fn account_history_preserves_unknowns_and_requires_bound_administrator() {
        let (mut actor, source, h) = fixture();
        h.validate(&actor, &source).unwrap();
        assert_eq!(access_label(h.role_events[0].active), "Not recorded");
        actor.staff_context = None;
        assert!(h.validate(&actor, &source).is_err());
        for role in [Role::HrGeneralist, Role::Manager, Role::Employee] {
            actor.role = role;
            assert!(h.validate(&actor, &source).is_err());
        }
    }
    #[test]
    fn history_rejects_foreign_stale_incomplete_or_malformed_records() {
        let (actor, source, h) = fixture();
        for n in 0..13 {
            let mut v = h.clone();
            match n {
                0 => v.company_id = Uuid::new_v4(),
                1 => v.actor_id = Uuid::new_v4(),
                2 => v.user_id = Uuid::new_v4(),
                3 => v.employee_id = Uuid::new_v4(),
                4 => v.revision += 1,
                5 => v.active = false,
                6 => v.role_event_count += 1,
                7 => v.access_event_count += 1,
                8 => v.role_events[0].id = Uuid::nil(),
                9 => v.role_events[0].reason = "private\ncontrol".into(),
                10 => v.access_events.push(v.access_events[0].clone()),
                11 => v.role_events[0].revision = 2,
                _ => v.member_updated_at += chrono::Duration::seconds(1),
            };
            assert!(v.validate(&actor, &source).is_err(), "case{n}");
        }
        let mut json = serde_json::to_value(&h).unwrap();
        json["password"] = serde_json::json!("must not decode");
        assert!(serde_json::from_value::<History>(json).is_err());
    }
    #[test]
    fn history_refresh_never_discards_a_pending_role_draft() {
        use super::super::{
            admin::{AdminContext, Update},
            editor::State,
        };
        let (actor, source, h) = fixture();
        let self_account = Account {
            user_id: actor.user_id,
            employee_id: actor.employee_id.unwrap(),
            base_role: Role::Administrator,
            roles: StaffRoles::legacy(Role::Administrator),
            ..source.clone()
        };
        let context = AdminContext {
            schema_version: 1,
            company_id: actor.company_id,
            user_id: actor.user_id,
            runtime_ready: true,
            account_count: 2,
            accounts: vec![self_account, source.clone()],
        };
        let mut state = State {
            context: Some(context.clone()),
            ..Default::default()
        };
        state.open(&actor, source.user_id).unwrap();
        state.draft.as_mut().unwrap().reason = "Fictional reviewed change".into();
        let request = state.submit(&actor).unwrap();
        state
            .accept(
                &actor,
                Update {
                    context: context.clone(),
                    receipt: None,
                    history: Some(h),
                },
            )
            .unwrap();
        assert_eq!(state.pending, Some(request));
        assert!(state.history.is_some());
        let mut changed = context;
        changed.accounts[1].revision += 1;
        state
            .accept(
                &actor,
                Update {
                    context: changed,
                    receipt: None,
                    history: None,
                },
            )
            .unwrap();
        assert!(state.history.is_none());
        assert!(state.pending.is_some());
        assert!(state.draft.is_some());
    }
}
