//! Typed protected Account Manager views and source-bound role changes.
use super::{Capability, Context, RoleChange, SCHEMA_VERSION, StaffRole, StaffRoles};
use crate::access::{Access, Role};
use anyhow::{Result, ensure};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Account {
    pub user_id: Uuid,
    pub employee_id: Uuid,
    pub employee_code: String,
    pub full_name: String,
    pub active: bool,
    pub base_role: Role,
    pub member_updated_at: DateTime<Utc>,
    pub revision: i64,
    pub roles: StaffRoles,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdminContext {
    pub schema_version: u16,
    pub company_id: Uuid,
    pub user_id: Uuid,
    pub runtime_ready: bool,
    pub account_count: usize,
    pub accounts: Vec<Account>,
}
impl AdminContext {
    pub fn validate(&self, actor: &Access) -> Result<()> {
        ensure!(
            actor.role == Role::Administrator && actor.allows(Capability::AccountAdministration),
            "Only a current Administrator reads staff assignments."
        );
        ensure!(
            self.schema_version == SCHEMA_VERSION
                && !self.company_id.is_nil()
                && self.company_id == actor.company_id
                && !self.user_id.is_nil()
                && self.user_id == actor.user_id,
            "Account role context does not match the current company and Administrator."
        );
        ensure!(
            self.accounts.len() == self.account_count && self.account_count <= 1000,
            "The staff-role view is incomplete or exceeds its supported size."
        );
        let mut users = BTreeSet::new();
        let mut people = BTreeSet::new();
        for account in &self.accounts {
            ensure!(
                !account.user_id.is_nil()
                    && !account.employee_id.is_nil()
                    && account.revision > 0
                    && users.insert(account.user_id)
                    && people.insert(account.employee_id),
                "Role assignments contain an invalid or duplicate employee/account identity."
            );
            ensure!(
                !account.employee_code.trim().is_empty()
                    && account.employee_code.chars().count() <= 30
                    && !account.full_name.trim().is_empty()
                    && account.full_name.chars().count() <= 120,
                "Role assignments contain incomplete employee labels."
            );
            ensure!(
                !self.runtime_ready || account.roles.compatible_base_role() == account.base_role,
                "The saved role projection would grant unrelated legacy access."
            );
        }
        ensure!(
            self.accounts.iter().any(|a| a.user_id == actor.user_id
                && a.active
                && a.base_role == Role::Administrator
                && a.roles.contains(StaffRole::Administrator)),
            "Current Administrator is missing from this role view."
        );
        Ok(())
    }
    pub fn prepare(
        &self,
        actor: &Access,
        user_id: Uuid,
        roles: StaffRoles,
        active: bool,
        reason: String,
        id: Uuid,
    ) -> Result<RoleChange> {
        self.validate(actor)?;
        ensure!(
            self.runtime_ready,
            "Role editing awaits the verified protected workspace rollout."
        );
        let account = self
            .accounts
            .iter()
            .find(|a| a.user_id == user_id)
            .ok_or_else(|| {
                anyhow::anyhow!("Choose an employee account from the current role view.")
            })?;
        let request = RoleChange {
            id,
            user_id,
            active,
            expected_revision: account.revision,
            expected_base_role: account.base_role,
            expected_member_updated_at: account.member_updated_at,
            roles,
            reason,
        };
        let source: &Context = actor.staff_context.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Refresh the protected staff-role context before assigning roles.")
        })?;
        source.validate(actor)?;
        request.validate(source)?;
        Ok(request)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub schema_version: u16,
    pub id: Uuid,
    pub company_id: Uuid,
    pub actor_id: Uuid,
    pub user_id: Uuid,
    pub employee_id: Uuid,
    pub active: bool,
    pub roles: StaffRoles,
    pub revision: i64,
    pub base_role: Role,
    pub member_updated_at: DateTime<Utc>,
}
impl Receipt {
    pub fn validate(&self, actor: &Access, request: &RoleChange, subject: &Account) -> Result<()> {
        ensure!(
            self.schema_version == SCHEMA_VERSION
                && self.id == request.id
                && self.company_id == actor.company_id
                && self.actor_id == actor.user_id
                && self.user_id == request.user_id
                && self.user_id == subject.user_id
                && self.employee_id == subject.employee_id,
            "Staff-role receipt does not match the actor, employee and request."
        );
        ensure!(
            self.active == request.active
                && self.roles == request.roles
                && self.base_role == request.roles.compatible_base_role()
                && request.expected_revision.checked_add(1) == Some(self.revision),
            "Staff-role receipt does not match the reviewed assignment or revision."
        );
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Access, AdminContext) {
        let company_id = Uuid::from_u128(1);
        let user_id = Uuid::from_u128(2);
        let employee_id = Uuid::from_u128(3);
        let timestamp = DateTime::parse_from_rfc3339("2026-10-27T09:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let mut access = Access {
            company_id,
            user_id,
            employee_id: Some(employee_id),
            role: Role::Administrator,
            staff_contract_version: 1,
            ..Default::default()
        };
        access
            .bind_staff_context(Context {
                schema_version: 1,
                company_id,
                user_id,
                employee_id,
                base_role: Role::Administrator,
                revision: 1,
                runtime_ready: true,
                roles: StaffRoles::new(vec![StaffRole::Administrator]).unwrap(),
            })
            .unwrap();
        let accounts = vec![
            Account {
                user_id,
                employee_id,
                employee_code: "EMP-000001".into(),
                full_name: "Fictional Administrator".into(),
                active: true,
                base_role: Role::Administrator,
                member_updated_at: timestamp,
                revision: 1,
                roles: StaffRoles::new(vec![StaffRole::Administrator]).unwrap(),
            },
            Account {
                user_id: Uuid::from_u128(4),
                employee_id: Uuid::from_u128(5),
                employee_code: "000004".into(),
                full_name: "Fictional Employee".into(),
                active: true,
                base_role: Role::Employee,
                member_updated_at: timestamp,
                revision: 1,
                roles: StaffRoles::default(),
            },
        ];
        (
            access,
            AdminContext {
                schema_version: 1,
                company_id,
                user_id,
                runtime_ready: true,
                account_count: 2,
                accounts,
            },
        )
    }
    #[test]
    fn draft_binds_exact_target_revision_base_and_timestamp() {
        let (a, c) = fixture();
        let subject = &c.accounts[1];
        let change = c
            .prepare(
                &a,
                subject.user_id,
                StaffRoles::new(vec![StaffRole::LearningPerformanceOfficer]).unwrap(),
                subject.active,
                "Assign reviewed responsibilities".into(),
                Uuid::from_u128(6),
            )
            .unwrap();
        assert_eq!(change.active, subject.active);
        assert_eq!(change.expected_revision, subject.revision);
        assert_eq!(change.expected_base_role, subject.base_role);
        assert_eq!(change.expected_member_updated_at, subject.member_updated_at);
        assert_eq!(subject.employee_code, "000004");
    }
    #[test]
    fn incomplete_foreign_duplicate_and_non_admin_views_fail_closed() {
        let (a, c) = fixture();
        assert!(c.validate(&a).is_ok());
        let mut bad = c.clone();
        bad.company_id = Uuid::from_u128(99);
        assert!(bad.validate(&a).is_err());
        let mut bad = c.clone();
        bad.user_id = Uuid::from_u128(99);
        assert!(bad.validate(&a).is_err());
        let mut bad = c.clone();
        bad.account_count = 3;
        assert!(bad.validate(&a).is_err());
        let mut bad = c.clone();
        bad.accounts[1].employee_id = bad.accounts[0].employee_id;
        assert!(bad.validate(&a).is_err());
        let mut bad = c.clone();
        bad.accounts[1].user_id = bad.accounts[0].user_id;
        assert!(bad.validate(&a).is_err());
        let mut bad = c.clone();
        bad.accounts[1].base_role = Role::Administrator;
        assert!(bad.validate(&a).is_err());
        let mut other = a.clone();
        other.role = Role::HrGeneralist;
        assert!(c.validate(&other).is_err());
    }
    #[test]
    fn disabled_runtime_self_edit_missing_context_and_unknown_subject_cannot_prepare() {
        let (a, c) = fixture();
        let roles = StaffRoles::new(vec![StaffRole::Recruiter]).unwrap();
        let prepare = |context: &AdminContext, access: &Access, user| {
            context.prepare(
                access,
                user,
                roles.clone(),
                true,
                "Reviewed assignment".into(),
                Uuid::from_u128(6),
            )
        };
        let mut paused = c.clone();
        paused.runtime_ready = false;
        assert!(prepare(&paused, &a, c.accounts[1].user_id).is_err());
        assert!(prepare(&c, &a, a.user_id).is_err());
        assert!(prepare(&c, &a, Uuid::from_u128(99)).is_err());
        let mut missing = a.clone();
        missing.staff_context = None;
        assert!(prepare(&c, &missing, c.accounts[1].user_id).is_err());
    }
    #[test]
    fn receipt_binds_actor_subject_roles_and_exact_next_revision() {
        let (a, c) = fixture();
        let subject = &c.accounts[1];
        let roles = StaffRoles::new(vec![StaffRole::Recruiter]).unwrap();
        let request = c
            .prepare(
                &a,
                subject.user_id,
                roles.clone(),
                true,
                "Reviewed assignment".into(),
                Uuid::from_u128(6),
            )
            .unwrap();
        let receipt = Receipt {
            schema_version: 1,
            id: request.id,
            company_id: a.company_id,
            actor_id: a.user_id,
            user_id: subject.user_id,
            employee_id: subject.employee_id,
            active: request.active,
            roles,
            revision: 2,
            base_role: Role::Employee,
            member_updated_at: subject.member_updated_at,
        };
        assert!(receipt.validate(&a, &request, subject).is_ok());
        let mut wrong = receipt.clone();
        wrong.active = !request.active;
        assert!(wrong.validate(&a, &request, subject).is_err());
        let mut wrong = receipt.clone();
        wrong.actor_id = Uuid::from_u128(99);
        assert!(wrong.validate(&a, &request, subject).is_err());
        let mut wrong = receipt.clone();
        wrong.revision = 3;
        assert!(wrong.validate(&a, &request, subject).is_err());
        let mut wrong = receipt.clone();
        wrong.roles = StaffRoles::new(vec![StaffRole::PayrollOfficer]).unwrap();
        assert!(wrong.validate(&a, &request, subject).is_err());
        let mut wrong = receipt;
        wrong.base_role = Role::HrGeneralist;
        assert!(wrong.validate(&a, &request, subject).is_err());
    }
}
