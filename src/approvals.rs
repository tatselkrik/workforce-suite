//! Temporary operational review permissions, issued by protected server context.
//! A grant never changes account roles, reporting lines or financial approvals.
use crate::{
    access::{Access, Role},
    domain::*,
};
use anyhow::{Result, ensure};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    #[default]
    Leave,
    Attendance,
    Overtime,
}
impl Scope {
    pub const ALL: [Self; 3] = [Self::Leave, Self::Attendance, Self::Overtime];
    pub fn label(self) -> &'static str {
        match self {
            Self::Leave => "Leave",
            Self::Attendance => "Attendance corrections",
            Self::Overtime => "Overtime",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Staff {
    pub user_id: Uuid,
    pub employee_id: Uuid,
    pub name: String,
    pub role: Role,
    #[serde(default)]
    pub staff_roles: Option<crate::staff_roles::StaffRoles>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    pub id: Uuid,
    pub scope: Scope,
    pub principal_id: Uuid,
    pub delegate_id: Uuid,
    pub principal_employee_id: Uuid,
    pub delegate_employee_id: Uuid,
    pub principal_role: Role,
    pub delegate_role: Role,
    #[serde(default)]
    pub principal_staff_roles: Option<crate::staff_roles::StaffRoles>,
    #[serde(default)]
    pub delegate_staff_roles: Option<crate::staff_roles::StaffRoles>,
    pub principal_name: String,
    pub delegate_name: String,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    pub current: bool,
    pub company_revision: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Revocation {
    pub id: Uuid,
    pub delegation_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub company_revision: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Audit {
    pub id: Uuid,
    pub reason: String,
    pub reference: String,
    pub actor_name: String,
    pub created_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Permit {
    pub scope: Scope,
    pub request_id: Uuid,
    pub request_revision: i64,
    pub delegation_id: Uuid,
    pub source_current: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Approve,
    Reject,
    Revoke,
}
impl Decision {
    pub fn label(self) -> &'static str {
        match self {
            Self::Approve => "Approved",
            Self::Reject => "Rejected",
            Self::Revoke => "Approval revoked",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub id: Uuid,
    pub delegation_id: Uuid,
    pub scope: Scope,
    pub request_id: Uuid,
    pub request_revision: i64,
    pub decision: Decision,
    pub principal_id: Uuid,
    pub delegate_id: Uuid,
    pub principal_name: String,
    pub delegate_name: String,
    pub created_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Context {
    pub version: String,
    pub company_id: Uuid,
    pub user_id: Uuid,
    pub employee_id: Option<Uuid>,
    pub role: Role,
    pub as_of: NaiveDate,
    pub revision: i64,
    pub grants: Vec<Grant>,
    pub revocations: Vec<Revocation>,
    pub receipts: Vec<Receipt>,
    pub permits: Vec<Permit>,
    pub audit: Vec<Audit>,
    pub staff: Vec<Staff>,
}
pub fn substitute_role(principal: Role, delegate: Role) -> bool {
    match principal {
        Role::Administrator => delegate == Role::Administrator,
        Role::HrGeneralist => delegate.manages_hr(),
        Role::Manager => delegate.uses_desktop(),
        Role::Employee => false,
    }
}
pub fn substitute_roles(
    scope: Scope,
    principal: &crate::staff_roles::StaffRoles,
    delegate: &crate::staff_roles::StaffRoles,
) -> bool {
    use crate::staff_roles::Capability as C;
    if !principal.uses_desktop() || !delegate.uses_desktop() {
        return false;
    }
    if principal.allows(C::AccountAdministration) {
        return delegate.allows(C::AccountAdministration);
    }
    if principal.allows(C::LeaveAdministration) {
        return delegate.allows(C::LeaveAdministration);
    }
    match scope {
        Scope::Leave => true,
        Scope::Attendance | Scope::Overtime => {
            principal.allows(C::TeamReview)
                && (delegate.allows(C::TeamReview) || delegate.allows(C::AttendanceAdministration))
        }
    }
}
impl Staff {
    pub fn roles(&self) -> crate::staff_roles::StaffRoles {
        self.staff_roles
            .clone()
            .unwrap_or_else(|| crate::staff_roles::StaffRoles::legacy(self.role))
    }
    pub fn label(&self) -> String {
        self.roles()
            .as_slice()
            .iter()
            .map(|r| r.label())
            .collect::<Vec<_>>()
            .join(", ")
    }
}
fn clean(s: &str, min: usize, max: usize) -> bool {
    (min..=max).contains(&s.chars().count()) && !s.chars().any(char::is_control)
}
impl Context {
    pub fn empty(data: &Snapshot) -> Self {
        let mut staff = data
            .accounts
            .iter()
            .filter(|a| a.active && a.role.uses_desktop())
            .filter_map(|a| {
                Some(Staff {
                    user_id: if data.mode == Mode::Demo && a.user_id.is_nil() {
                        Uuid::from_u128(104899)
                    } else {
                        a.user_id
                    },
                    employee_id: a.employee_id?,
                    name: data.employee_name(a.employee_id?).into(),
                    role: a.role,
                    staff_roles: (data.access.staff_contract_version == 1)
                        .then(|| crate::staff_roles::StaffRoles::legacy(a.role)),
                })
            })
            .collect::<Vec<_>>();
        if data.mode == Mode::Demo && staff.is_empty() {
            staff = data
                .employees
                .iter()
                .enumerate()
                .filter(|(_, e)| e.status == EmployeeStatus::Active)
                .map(|(n, e)| Staff {
                    user_id: Uuid::from_u128(104800 + n as u128),
                    employee_id: e.id,
                    name: e.full_name.clone(),
                    role: Role::Manager,
                    staff_roles: (data.access.staff_contract_version == 1)
                        .then(|| crate::staff_roles::StaffRoles::legacy(Role::Manager)),
                })
                .collect();
        }
        Self {
            version: if data.access.staff_contract_version == 1 {
                "approval-delegation-v2"
            } else {
                "approval-delegation-v1"
            }
            .into(),
            company_id: data.company.id,
            user_id: data.access.user_id,
            employee_id: data.access.employee_id,
            role: data.access.role,
            as_of: data.company.today(),
            revision: 0,
            grants: vec![],
            revocations: vec![],
            receipts: vec![],
            permits: vec![],
            audit: vec![],
            staff: if data.access.role.manages_accounts() {
                staff
            } else {
                vec![]
            },
        }
    }
    pub fn validate_access(&self, access: &Access, today: NaiveDate) -> Result<()> {
        ensure!(
            self.version
                == if access.staff_contract_version == 1 {
                    "approval-delegation-v2"
                } else {
                    "approval-delegation-v1"
                }
                && self.company_id == access.company_id
                && self.user_id == access.user_id
                && self.employee_id == access.employee_id
                && self.role == access.role
                && self.as_of == today
                && access.uses_desktop(),
            "Delegation context does not match current staff/date. Refresh."
        );
        ensure!(
            (0..=5000).contains(&self.revision)
                && self.grants.len() <= 5000
                && self.revocations.len() <= 5000
                && self.receipts.len() <= 10000
                && self.permits.len() <= 10000,
            "Delegation context exceeds its supported limits."
        );
        let mut ids = HashSet::new();
        let mut revisions = HashSet::new();
        for g in &self.grants {
            ensure!(
                ids.insert(g.id)
                    && !g.id.is_nil()
                    && !g.principal_id.is_nil()
                    && !g.delegate_id.is_nil()
                    && !g.principal_employee_id.is_nil()
                    && !g.delegate_employee_id.is_nil()
                    && g.principal_id != g.delegate_id
                    && g.principal_employee_id != g.delegate_employee_id
                    && if self.version == "approval-delegation-v2" {
                        match (&g.principal_staff_roles, &g.delegate_staff_roles) {
                            (Some(p), Some(d)) => {
                                p.compatible_base_role() == g.principal_role
                                    && d.compatible_base_role() == g.delegate_role
                                    && substitute_roles(g.scope, p, d)
                            }
                            _ => false,
                        }
                    } else {
                        g.principal_staff_roles.is_none()
                            && g.delegate_staff_roles.is_none()
                            && substitute_role(g.principal_role, g.delegate_role)
                    }
                    && clean(&g.principal_name, 1, 120)
                    && clean(&g.delegate_name, 1, 120)
                    && g.ends_on >= g.starts_on
                    && (g.ends_on - g.starts_on).num_days() < 366
                    && (1..=self.revision).contains(&g.company_revision)
                    && revisions.insert(g.company_revision),
                "Invalid delegation history."
            );
            ensure!(
                access.allows(crate::staff_roles::Capability::LeaveAdministration)
                    || [g.principal_id, g.delegate_id].contains(&access.user_id),
                "Delegation is outside current staff scope."
            );
            ensure!(
                !g.current || (g.starts_on <= today && today <= g.ends_on),
                "Delegation is active outside its dates."
            );
            ensure!(
                !g.current
                    || g.delegate_id != access.user_id
                    || (g.delegate_role == access.role
                        && Some(g.delegate_employee_id) == access.employee_id
                        && (self.version != "approval-delegation-v2"
                            || g.delegate_staff_roles.as_ref() == Some(&access.staff_roles()?))),
                "Delegation substitute binding changed."
            );
            ensure!(
                !g.current
                    || g.principal_id != access.user_id
                    || (g.principal_role == access.role
                        && Some(g.principal_employee_id) == access.employee_id
                        && (self.version != "approval-delegation-v2"
                            || g.principal_staff_roles.as_ref() == Some(&access.staff_roles()?))),
                "Delegation original approver binding changed."
            );
        }
        let mut revoked = HashSet::new();
        for r in &self.revocations {
            ensure!(
                ids.insert(r.id)
                    && !r.id.is_nil()
                    && revoked.insert(r.delegation_id)
                    && self.grants.iter().any(|g| g.id == r.delegation_id
                        && !g.current
                        && g.company_revision < r.company_revision)
                    && (1..=self.revision).contains(&r.company_revision)
                    && revisions.insert(r.company_revision),
                "Invalid delegation revocation."
            );
        }
        if access.allows(crate::staff_roles::Capability::LeaveAdministration) {
            ensure!(
                self.audit.len() == ids.len(),
                "Delegation private history is incomplete."
            );
            let mut audits = HashSet::new();
            for a in &self.audit {
                ensure!(
                    ids.contains(&a.id)
                        && audits.insert(a.id)
                        && clean(&a.reason, 1, 500)
                        && clean(&a.reference, 0, 200)
                        && clean(&a.actor_name, 1, 120),
                    "Invalid delegation private audit."
                );
            }
        } else {
            ensure!(
                self.audit.is_empty(),
                "Delegation includes private HR audit data."
            );
        }
        let mut users = HashSet::new();
        let mut employees = HashSet::new();
        ensure!(
            access.role.manages_accounts() || self.staff.is_empty(),
            "Delegation includes Administrator staff choices."
        );
        for s in &self.staff {
            ensure!(
                !s.user_id.is_nil()
                    && !s.employee_id.is_nil()
                    && s.roles().uses_desktop()
                    && s.roles().compatible_base_role() == s.role
                    && (s.staff_roles.is_some() == (self.version == "approval-delegation-v2"))
                    && clean(&s.name, 1, 120)
                    && users.insert(s.user_id)
                    && employees.insert(s.employee_id),
                "Invalid active delegation staff choice."
            );
        }
        let mut permits = HashSet::new();
        for p in &self.permits {
            ensure!(
                p.request_revision >= 0
                    && !p.request_id.is_nil()
                    && permits.insert((p.scope, p.request_id))
                    && self.grants.iter().any(|g| g.id == p.delegation_id
                        && g.scope == p.scope
                        && g.delegate_id == access.user_id
                        && g.current),
                "Invalid delegated review permission."
            );
        }
        let mut receipt_ids = HashSet::new();
        let mut decisions = HashSet::new();
        for r in &self.receipts {
            ensure!(
                !r.id.is_nil()
                    && receipt_ids.insert(r.id)
                    && !r.request_id.is_nil()
                    && r.request_revision > 0
                    && decisions.insert((r.scope, r.request_id, r.request_revision))
                    && self.grants.iter().any(|g| g.id == r.delegation_id
                        && g.scope == r.scope
                        && g.principal_id == r.principal_id
                        && g.delegate_id == r.delegate_id
                        && g.principal_name == r.principal_name
                        && g.delegate_name == r.delegate_name)
                    && (access.allows(crate::staff_roles::Capability::LeaveAdministration)
                        || [r.principal_id, r.delegate_id].contains(&access.user_id)),
                "Invalid delegated decision receipt."
            );
        }
        Ok(())
    }
    pub fn validate(&self, data: &Snapshot) -> Result<()> {
        self.validate_access(&data.access, data.company.today())?;
        for p in &self.permits {
            let valid = match p.scope {
                Scope::Leave => data.leave.iter().any(|r| {
                    r.id == p.request_id
                        && r.company_id == self.company_id
                        && r.policy.revision == p.request_revision
                        && r.status == LeaveStatus::Pending
                        && Some(r.employee_id) != self.employee_id
                }),
                Scope::Attendance => data.requests.records.iter().any(|r| {
                    r.id == p.request_id
                        && r.company_id == self.company_id
                        && r.revision == p.request_revision
                        && r.status == crate::requests::Status::Pending
                        && Some(r.employee_id) != self.employee_id
                        && r.submitted_by != self.user_id
                }),
                Scope::Overtime => data.overtime.records.iter().any(|r| {
                    r.id == p.request_id
                        && r.company_id == self.company_id
                        && r.revision == p.request_revision
                        && matches!(
                            r.status,
                            crate::overtime::Status::Pending | crate::overtime::Status::Approved
                        )
                        && Some(r.employee_id) != self.employee_id
                        && r.submitted_by != self.user_id
                }),
            };
            ensure!(valid, "Delegated request changed while loading. Refresh.");
        }
        Ok(())
    }
    pub fn permits(
        &self,
        access: &Access,
        today: NaiveDate,
        scope: Scope,
        id: Uuid,
        revision: i64,
    ) -> bool {
        self.company_id == access.company_id
            && self.user_id == access.user_id
            && self.employee_id == access.employee_id
            && self.role == access.role
            && self.as_of == today
            && access.uses_desktop()
            && self.permits.iter().any(|p| {
                p.scope == scope
                    && p.request_id == id
                    && p.request_revision == revision
                    && self.grants.iter().any(|g| {
                        g.id == p.delegation_id
                            && g.current
                            && g.starts_on <= today
                            && today <= g.ends_on
                            && g.delegate_id == access.user_id
                    })
            })
    }
}
pub fn permitted(data: &Snapshot, scope: Scope, id: Uuid, revision: i64) -> bool {
    data.approvals
        .as_ref()
        .is_some_and(|c| c.permits(&data.access, data.company.today(), scope, id, revision))
}
pub fn delegated_source(data: &Snapshot, scope: Scope, id: Uuid, revision: i64) -> Option<bool> {
    if data.mode != Mode::Supabase || !permitted(data, scope, id, revision) {
        return None;
    }
    data.approvals
        .as_ref()?
        .permits
        .iter()
        .find(|p| p.scope == scope && p.request_id == id && p.request_revision == revision)
        .map(|p| p.source_current)
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum Change {
    Grant {
        id: Uuid,
        expected_revision: i64,
        scope: Scope,
        principal_id: Uuid,
        delegate_id: Uuid,
        starts_on: NaiveDate,
        ends_on: NaiveDate,
        reason: String,
        reference: String,
    },
    Revoke {
        id: Uuid,
        expected_revision: i64,
        delegation_id: Uuid,
        reason: String,
    },
}
impl Change {
    pub fn id(&self) -> Uuid {
        match self {
            Self::Grant { id, .. } | Self::Revoke { id, .. } => *id,
        }
    }
    pub fn expected_revision(&self) -> i64 {
        match self {
            Self::Grant {
                expected_revision, ..
            }
            | Self::Revoke {
                expected_revision, ..
            } => *expected_revision,
        }
    }
    pub fn matches_history(&self, c: &Context) -> bool {
        let audit = c.audit.iter().find(|a| a.id == self.id());
        match self {
            Self::Grant {
                id,
                expected_revision,
                scope,
                principal_id,
                delegate_id,
                starts_on,
                ends_on,
                reason,
                reference,
            } => {
                c.grants.iter().any(|g| {
                    g.id == *id
                        && g.company_revision == expected_revision + 1
                        && g.scope == *scope
                        && g.principal_id == *principal_id
                        && g.delegate_id == *delegate_id
                        && g.starts_on == *starts_on
                        && g.ends_on == *ends_on
                }) && audit.is_some_and(|a| a.reason == *reason && a.reference == *reference)
            }
            Self::Revoke {
                id,
                expected_revision,
                delegation_id,
                reason,
            } => {
                c.revocations.iter().any(|r| {
                    r.id == *id
                        && r.company_revision == expected_revision + 1
                        && r.delegation_id == *delegation_id
                }) && audit.is_some_and(|a| a.reason == *reason)
            }
        }
    }
    pub fn payload(&self, data: &Snapshot) -> Result<serde_json::Value> {
        ensure!(
            data.mode != Mode::SignedOut && data.access.role.manages_accounts(),
            "Only Administrators configure approval delegation."
        );
        let c = data
            .approvals
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Refresh to load current delegation settings."))?;
        c.validate(data)?;
        ensure!(!self.id().is_nil(), "Invalid delegation request identity.");
        // A retained exact retry must keep its original revision; the server checks
        // its actor and full payload before considering current configuration.
        let recorded = c.grants.iter().any(|g| g.id == self.id())
            || c.revocations.iter().any(|r| r.id == self.id());
        if recorded {
            ensure!(
                self.matches_history(c),
                "This request identity has different recorded details."
            );
            return Ok(serde_json::to_value(self)?);
        }
        ensure!(
            recorded || self.expected_revision() == c.revision,
            "Delegation settings changed. Refresh and review the retained draft."
        );
        match self {
            Self::Grant {
                scope,
                principal_id,
                delegate_id,
                starts_on,
                ends_on,
                reason,
                reference,
                ..
            } => {
                ensure!(
                    clean(reason, 1, 500) && clean(reference, 0, 200),
                    "Enter a reason of1–500 characters and reference up to200 characters, without control characters."
                );
                ensure!(
                    *starts_on >= c.as_of
                        && ends_on >= starts_on
                        && (*ends_on - *starts_on).num_days() < 366
                        && *ends_on <= c.as_of + chrono::Days::new(366),
                    "Use today or future dates, for at most366 days ending within the next year."
                );
                let p = c
                    .staff
                    .iter()
                    .find(|s| s.user_id == *principal_id)
                    .ok_or_else(|| anyhow::anyhow!("Choose an active original approver."))?;
                let d = c
                    .staff
                    .iter()
                    .find(|s| s.user_id == *delegate_id)
                    .ok_or_else(|| anyhow::anyhow!("Choose an active substitute."))?;
                ensure!(
                    p.user_id != d.user_id
                        && p.employee_id != d.employee_id
                        && substitute_roles(*scope, &p.roles(), &d.roles()),
                    "Choose distinct staff; HR needs HR/Admin, and Administrator needs another Administrator."
                );
                ensure!(
                    recorded
                        || !c.grants.iter().any(|g| g.scope == *scope
                            && g.principal_id == *principal_id
                            && g.starts_on <= *ends_on
                            && g.ends_on >= *starts_on
                            && !c.revocations.iter().any(|r| r.delegation_id == g.id)),
                    "This approver already has an overlapping delegation. Revoke it first."
                );
            }
            Self::Revoke {
                delegation_id,
                reason,
                ..
            } => {
                ensure!(
                    clean(reason, 1, 500) && c.grants.iter().any(|g| g.id == *delegation_id),
                    "Choose a recorded delegation and enter a revocation reason."
                );
                ensure!(
                    recorded
                        || !c
                            .revocations
                            .iter()
                            .any(|r| r.delegation_id == *delegation_id),
                    "This delegation is already revoked."
                );
            }
        }
        Ok(serde_json::to_value(self)?)
    }
}
pub fn apply_demo(data: &mut Snapshot, change: &Change) -> Result<()> {
    change.payload(data)?;
    let c = data.approvals.as_mut().unwrap();
    if c.grants.iter().any(|g| g.id == change.id())
        || c.revocations.iter().any(|r| r.id == change.id())
    {
        return Ok(());
    }
    c.revision += 1;
    let now = Utc::now();
    let (reason, reference) = match change {
        Change::Grant {
            id,
            scope,
            principal_id,
            delegate_id,
            starts_on,
            ends_on,
            reason,
            reference,
            ..
        } => {
            let p = c.staff.iter().find(|s| s.user_id == *principal_id).unwrap();
            let d = c.staff.iter().find(|s| s.user_id == *delegate_id).unwrap();
            c.grants.push(Grant {
                id: *id,
                scope: *scope,
                principal_id: *principal_id,
                delegate_id: *delegate_id,
                principal_employee_id: p.employee_id,
                delegate_employee_id: d.employee_id,
                principal_role: p.role,
                delegate_role: d.role,
                principal_staff_roles: p.staff_roles.clone(),
                delegate_staff_roles: d.staff_roles.clone(),
                principal_name: p.name.clone(),
                delegate_name: d.name.clone(),
                starts_on: *starts_on,
                ends_on: *ends_on,
                current: *starts_on <= c.as_of && c.as_of <= *ends_on,
                company_revision: c.revision,
            });
            (reason.clone(), reference.clone())
        }
        Change::Revoke {
            id,
            delegation_id,
            reason,
            ..
        } => {
            c.grants
                .iter_mut()
                .find(|g| g.id == *delegation_id)
                .unwrap()
                .current = false;
            c.revocations.push(Revocation {
                id: *id,
                delegation_id: *delegation_id,
                created_at: now,
                company_revision: c.revision,
            });
            c.permits.retain(|p| p.delegation_id != *delegation_id);
            (reason.clone(), String::new())
        }
    };
    c.audit.push(Audit {
        id: change.id(),
        reason,
        reference,
        actor_name: "Fictional Administrator".into(),
        created_at: now,
    });
    Ok(())
}
