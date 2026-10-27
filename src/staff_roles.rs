//! Protected staff-role contract for the versioned workspace rollout.
//!
//! This is deliberately separate from the installed four-role `access::Role`.
//! A role set is not authority by itself: the server supplies it, and existing
//! authorization remains in force until the matching runtime contract is ready.
pub mod admin;
pub mod editor;
use anyhow::{Result, ensure};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize, de};
use std::{collections::BTreeSet, fmt};
use uuid::Uuid;

pub const SCHEMA_VERSION: u16 = 1;
pub const MAX_STAFF_ROLES: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StaffRole {
    Administrator,
    HrGeneralist,
    Recruiter,
    LearningPerformanceOfficer,
    PayrollOfficer,
    Manager,
    Employee,
}
impl StaffRole {
    pub const ALL: [Self; 7] = [
        Self::Administrator,
        Self::HrGeneralist,
        Self::Recruiter,
        Self::LearningPerformanceOfficer,
        Self::PayrollOfficer,
        Self::Manager,
        Self::Employee,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Administrator => "Administrator",
            Self::HrGeneralist => "HR Generalist",
            Self::Recruiter => "Recruiter",
            Self::LearningPerformanceOfficer => "Learning & Performance Officer",
            Self::PayrollOfficer => "Payroll Officer",
            Self::Manager => "Manager",
            Self::Employee => "Employee",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct StaffRoles(Vec<StaffRole>);
impl StaffRoles {
    pub fn new(roles: Vec<StaffRole>) -> Result<Self> {
        ensure!(
            !roles.is_empty() && roles.len() <= MAX_STAFF_ROLES,
            "Choose 1–6 staff roles, or Employee alone."
        );
        let unique: BTreeSet<_> = roles.iter().copied().collect();
        ensure!(
            unique.len() == roles.len(),
            "A staff role cannot be assigned twice."
        );
        ensure!(
            unique.len() == 1 || !unique.contains(&StaffRole::Employee),
            "Employee alone is the portal-only assignment; choose staff roles separately."
        );
        Ok(Self(unique.into_iter().collect()))
    }
    pub fn contains(&self, role: StaffRole) -> bool {
        self.0.contains(&role)
    }
    pub fn as_slice(&self) -> &[StaffRole] {
        &self.0
    }
    pub fn uses_desktop(&self) -> bool {
        !self.contains(StaffRole::Employee)
    }
    pub fn allows(&self, capability: Capability) -> bool {
        if self.contains(StaffRole::Administrator) {
            return true;
        }
        match capability {
            Capability::OwnRecords => true,
            Capability::Desktop => self.uses_desktop(),
            Capability::Personnel
            | Capability::AttendanceAdministration
            | Capability::LeaveAdministration
            | Capability::PrivateHrNotes
            | Capability::PersonnelDocuments
            | Capability::GeneralHrReports => self.contains(StaffRole::HrGeneralist),
            Capability::Recruiting => self.contains(StaffRole::Recruiter),
            Capability::DevelopmentAdministration => {
                self.contains(StaffRole::LearningPerformanceOfficer)
            }
            Capability::PayrollPreparation
            | Capability::GovernmentPreparation
            | Capability::PayrollReports => self.contains(StaffRole::PayrollOfficer),
            Capability::TeamReview => self.contains(StaffRole::Manager),
            Capability::AccountAdministration
            | Capability::CompanyConfiguration
            | Capability::PayrollApproval
            | Capability::PaymentRecording => false,
        }
    }
    /// Keep the legacy role no broader than the complete explicit assignment.
    pub fn compatible_base_role(&self) -> crate::access::Role {
        use crate::access::Role;
        if self.contains(StaffRole::Administrator) {
            return Role::Administrator;
        }
        if [
            StaffRole::HrGeneralist,
            StaffRole::Recruiter,
            StaffRole::LearningPerformanceOfficer,
            StaffRole::PayrollOfficer,
        ]
        .into_iter()
        .all(|role| self.contains(role))
        {
            return Role::HrGeneralist;
        }
        if self.contains(StaffRole::Manager) {
            return Role::Manager;
        }
        Role::Employee
    }
    /// Preserve the old contract's rights during an explicit, audited transition.
    /// New HR Generalist assignments do not inherit these specialist roles.
    pub fn legacy(role: crate::access::Role) -> Self {
        use crate::access::Role;
        let roles = match role {
            Role::Administrator => vec![StaffRole::Administrator],
            Role::HrGeneralist => vec![
                StaffRole::HrGeneralist,
                StaffRole::Recruiter,
                StaffRole::LearningPerformanceOfficer,
                StaffRole::PayrollOfficer,
            ],
            Role::Manager => vec![StaffRole::Manager],
            Role::Employee => vec![StaffRole::Employee],
        };
        Self::new(roles).expect("fixed compatibility role sets are valid")
    }
}
impl Default for StaffRoles {
    fn default() -> Self {
        Self(vec![StaffRole::Employee])
    }
}
impl<'de> Deserialize<'de> for StaffRoles {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        struct RolesVisitor;
        impl<'de> de::Visitor<'de> for RolesVisitor {
            type Value = StaffRoles;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("one to six distinct staff roles, or Employee alone")
            }
            fn visit_seq<A: de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> std::result::Result<StaffRoles, A::Error> {
                let mut roles = Vec::with_capacity(MAX_STAFF_ROLES);
                while let Some(role) = seq.next_element::<StaffRole>()? {
                    if roles.len() == MAX_STAFF_ROLES {
                        return Err(de::Error::custom("Too many staff roles."));
                    }
                    roles.push(role);
                }
                StaffRoles::new(roles).map_err(de::Error::custom)
            }
        }
        deserializer.deserialize_seq(RolesVisitor)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    OwnRecords,
    Desktop,
    Personnel,
    AttendanceAdministration,
    LeaveAdministration,
    PrivateHrNotes,
    PersonnelDocuments,
    GeneralHrReports,
    Recruiting,
    DevelopmentAdministration,
    PayrollPreparation,
    GovernmentPreparation,
    PayrollReports,
    TeamReview,
    AccountAdministration,
    CompanyConfiguration,
    PayrollApproval,
    PaymentRecording,
}
impl Capability {
    pub const ALL: [Self; 18] = [
        Self::OwnRecords,
        Self::Desktop,
        Self::Personnel,
        Self::AttendanceAdministration,
        Self::LeaveAdministration,
        Self::PrivateHrNotes,
        Self::PersonnelDocuments,
        Self::GeneralHrReports,
        Self::Recruiting,
        Self::DevelopmentAdministration,
        Self::PayrollPreparation,
        Self::GovernmentPreparation,
        Self::PayrollReports,
        Self::TeamReview,
        Self::AccountAdministration,
        Self::CompanyConfiguration,
        Self::PayrollApproval,
        Self::PaymentRecording,
    ];
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Context {
    pub schema_version: u16,
    pub company_id: Uuid,
    pub user_id: Uuid,
    pub employee_id: Uuid,
    pub base_role: crate::access::Role,
    pub revision: i64,
    pub runtime_ready: bool,
    pub roles: StaffRoles,
}
impl Context {
    pub fn validate(&self, access: &crate::access::Access) -> Result<()> {
        ensure!(
            self.schema_version == SCHEMA_VERSION,
            "This staff-role version is not supported; update HRIS."
        );
        ensure!(
            !self.company_id.is_nil() && !self.user_id.is_nil() && !self.employee_id.is_nil(),
            "Staff-role identity is incomplete."
        );
        ensure!(
            self.company_id == access.company_id
                && self.user_id == access.user_id
                && Some(self.employee_id) == access.employee_id
                && self.base_role == access.role,
            "Staff-role context does not match the current protected membership."
        );
        ensure!(
            self.revision > 0,
            "Refresh the staff-role revision before continuing."
        );
        ensure!(
            !self.runtime_ready || self.roles.compatible_base_role() == self.base_role,
            "The legacy role would grant unrelated access; the protected role transition is incomplete."
        );
        Ok(())
    }
    pub fn effective_roles(&self) -> StaffRoles {
        if self.runtime_ready {
            self.roles.clone()
        } else {
            StaffRoles::legacy(self.base_role)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoleChange {
    pub id: Uuid,
    pub user_id: Uuid,
    pub active: bool,
    pub expected_revision: i64,
    pub expected_base_role: crate::access::Role,
    pub expected_member_updated_at: DateTime<Utc>,
    pub roles: StaffRoles,
    pub reason: String,
}
impl RoleChange {
    pub fn validate(&self, actor: &Context) -> Result<()> {
        ensure!(
            actor.schema_version == SCHEMA_VERSION
                && actor.revision > 0
                && !actor.company_id.is_nil()
                && !actor.user_id.is_nil()
                && !actor.employee_id.is_nil(),
            "Refresh a valid staff-role context first."
        );
        ensure!(
            actor.base_role == crate::access::Role::Administrator
                && actor
                    .effective_roles()
                    .allows(Capability::AccountAdministration),
            "Only a current Administrator assigns staff roles."
        );
        ensure!(
            actor.runtime_ready,
            "Staff-role editing is not enabled until the protected workspace rollout is verified."
        );
        ensure!(
            !self.id.is_nil()
                && !self.user_id.is_nil()
                && self.expected_revision > 0
                && self.expected_revision < i64::MAX,
            "A request identity, target account and current revision are required."
        );
        ensure!(
            self.user_id != actor.user_id,
            "Another Administrator must change your roles."
        );
        ensure!(
            self.reason == self.reason.trim()
                && (1..=500).contains(&self.reason.chars().count())
                && !self.reason.chars().any(char::is_control),
            "Enter a reason of 1–500 characters without surrounding whitespace or control characters."
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::access::{Access, Role};
    fn roles(values: &[StaffRole]) -> StaffRoles {
        StaffRoles::new(values.to_vec()).unwrap()
    }
    fn context(base_role: Role) -> Context {
        Context {
            schema_version: 1,
            company_id: Uuid::from_u128(1),
            user_id: Uuid::from_u128(2),
            employee_id: Uuid::from_u128(3),
            base_role,
            revision: 1,
            runtime_ready: true,
            roles: StaffRoles::legacy(base_role),
        }
    }
    #[test]
    fn combined_development_is_one_role_without_hr_or_payroll_powers() {
        let r = roles(&[StaffRole::LearningPerformanceOfficer]);
        assert!(r.allows(Capability::DevelopmentAdministration));
        for c in [
            Capability::PayrollPreparation,
            Capability::PayrollApproval,
            Capability::Personnel,
            Capability::PrivateHrNotes,
            Capability::PersonnelDocuments,
            Capability::Recruiting,
            Capability::AccountAdministration,
        ] {
            assert!(!r.allows(c), "{c:?}");
        }
        assert_eq!(
            serde_json::to_string(&r).unwrap(),
            r#"["learning_performance_officer"]"#
        );
        assert!(serde_json::from_str::<StaffRoles>(r#"["learning_officer"]"#).is_err());
        assert!(serde_json::from_str::<StaffRoles>(r#"["performance_officer"]"#).is_err());
    }
    #[test]
    fn small_company_combines_responsibilities_without_approval_elevation() {
        let r = roles(&[StaffRole::PayrollOfficer, StaffRole::HrGeneralist]);
        assert!(r.allows(Capability::Personnel) && r.allows(Capability::PayrollPreparation));
        assert!(!r.allows(Capability::PayrollApproval) && !r.allows(Capability::PaymentRecording));
        assert!(!r.allows(Capability::DevelopmentAdministration));
        assert!(!roles(&[StaffRole::HrGeneralist]).allows(Capability::PayrollPreparation));
    }
    #[test]
    fn specialist_role_matrix_never_grants_account_or_company_administration() {
        for role in StaffRole::ALL
            .into_iter()
            .filter(|r| *r != StaffRole::Administrator)
        {
            let r = roles(&[role]);
            for c in [
                Capability::AccountAdministration,
                Capability::CompanyConfiguration,
                Capability::PayrollApproval,
                Capability::PaymentRecording,
            ] {
                assert!(!r.allows(c), "{role:?} {c:?}");
            }
        }
        let admin = roles(&[StaffRole::Administrator]);
        for c in Capability::ALL {
            assert!(admin.allows(c));
        }
    }
    #[test]
    fn recruiter_and_payroll_are_distinct_from_general_hr_and_development() {
        let r = roles(&[StaffRole::Recruiter]);
        assert!(r.allows(Capability::Recruiting));
        assert!(
            !r.allows(Capability::Personnel)
                && !r.allows(Capability::DevelopmentAdministration)
                && !r.allows(Capability::PayrollPreparation)
        );
        let p = roles(&[StaffRole::PayrollOfficer]);
        assert!(
            p.allows(Capability::GovernmentPreparation) && p.allows(Capability::PayrollReports)
        );
        assert!(!p.allows(Capability::PersonnelDocuments) && !p.allows(Capability::PrivateHrNotes));
    }
    #[test]
    fn employee_alone_is_portal_only_and_manager_does_not_gain_company_scopes() {
        let e = StaffRoles::default();
        assert!(!e.uses_desktop());
        assert!(e.allows(Capability::OwnRecords));
        for c in Capability::ALL {
            if c != Capability::OwnRecords {
                assert!(!e.allows(c));
            }
        }
        let m = roles(&[StaffRole::Manager]);
        assert!(m.allows(Capability::TeamReview));
        assert!(
            !m.allows(Capability::GeneralHrReports)
                && !m.allows(Capability::DevelopmentAdministration)
        );
    }
    #[test]
    fn malformed_duplicate_oversized_and_mixed_portal_assignments_fail_closed() {
        for value in [
            r#"[]"#,
            r#"["recruiter","recruiter"]"#,
            r#"["employee","manager"]"#,
            r#"["administrator","hr_generalist","recruiter","learning_performance_officer","payroll_officer","manager","employee"]"#,
            r#"["owner"]"#,
            r#"null"#,
            r#"[123]"#,
        ] {
            assert!(
                serde_json::from_str::<StaffRoles>(value).is_err(),
                "{value}"
            );
        }
        assert!(StaffRoles::new(vec![StaffRole::Manager; 7]).is_err());
        assert_eq!(
            roles(&[StaffRole::PayrollOfficer, StaffRole::Recruiter]).as_slice(),
            &[StaffRole::Recruiter, StaffRole::PayrollOfficer]
        );
    }
    #[test]
    fn legacy_hr_mapping_preserves_existing_rights_but_is_not_the_new_default() {
        let old = StaffRoles::legacy(Role::HrGeneralist);
        for c in [
            Capability::Personnel,
            Capability::Recruiting,
            Capability::DevelopmentAdministration,
            Capability::PayrollPreparation,
        ] {
            assert!(old.allows(c));
        }
        assert_eq!(old.as_slice().len(), 4);
        assert_eq!(StaffRole::ALL.len(), 7);
    }
    #[test]
    fn context_is_bound_to_current_company_account_employee_and_base_role() {
        let c = context(Role::Administrator);
        let mut a = Access::demo_admin();
        a.company_id = c.company_id;
        a.user_id = c.user_id;
        a.employee_id = Some(c.employee_id);
        assert!(c.validate(&a).is_ok());
        let mut wrong = c.clone();
        wrong.company_id = Uuid::from_u128(10);
        assert!(wrong.validate(&a).is_err());
        let mut wrong = c.clone();
        wrong.user_id = Uuid::from_u128(10);
        assert!(wrong.validate(&a).is_err());
        let mut wrong = c.clone();
        wrong.employee_id = Uuid::from_u128(10);
        assert!(wrong.validate(&a).is_err());
        let mut wrong = c.clone();
        wrong.base_role = Role::HrGeneralist;
        assert!(wrong.validate(&a).is_err());
        let mut wrong = c.clone();
        wrong.schema_version = 2;
        assert!(wrong.validate(&a).is_err());
        let mut wrong = c.clone();
        wrong.revision = 0;
        assert!(wrong.validate(&a).is_err());
        let mut wrong = c.clone();
        wrong.employee_id = Uuid::nil();
        assert!(wrong.validate(&a).is_err());
    }
    #[test]
    fn inactive_rollout_cannot_enable_a_draft_role_or_remove_existing_rights() {
        let mut c = context(Role::Employee);
        c.roles = roles(&[StaffRole::Administrator]);
        c.runtime_ready = false;
        assert!(!c.effective_roles().uses_desktop());
        let mut c = context(Role::HrGeneralist);
        c.roles = StaffRoles::default();
        c.runtime_ready = false;
        assert!(c.effective_roles().allows(Capability::PayrollPreparation));
    }
    #[test]
    fn role_changes_require_current_admin_ready_runtime_revision_and_reason() {
        let mut actor = context(Role::Administrator);
        let request = RoleChange {
            id: Uuid::from_u128(4),
            user_id: Uuid::from_u128(5),
            active: true,
            expected_revision: 1,
            expected_base_role: Role::Employee,
            expected_member_updated_at: Utc::now(),
            roles: roles(&[StaffRole::LearningPerformanceOfficer]),
            reason: "Assign development responsibilities".into(),
        };
        assert!(request.validate(&actor).is_ok());
        actor.runtime_ready = false;
        assert!(request.validate(&actor).is_err());
        actor = context(Role::HrGeneralist);
        assert!(request.validate(&actor).is_err());
        actor = context(Role::Administrator);
        let mut bad = request.clone();
        bad.user_id = actor.user_id;
        assert!(bad.validate(&actor).is_err());
        let mut bad = request.clone();
        bad.id = Uuid::nil();
        assert!(bad.validate(&actor).is_err());
        let mut bad = request.clone();
        bad.expected_revision = 0;
        assert!(bad.validate(&actor).is_err());
        for reason in [
            "".to_owned(),
            " leading".to_owned(),
            "line\nbreak".to_owned(),
            "x".repeat(501),
        ] {
            let mut bad = request.clone();
            bad.reason = reason;
            assert!(bad.validate(&actor).is_err());
        }
    }
    #[test]
    fn native_and_database_use_the_same_reviewed_capability_cases() {
        #[derive(Deserialize)]
        struct Case {
            roles: StaffRoles,
            capabilities: Vec<String>,
        }
        let cases: Vec<Case> = serde_json::from_str(include_str!(
            "../tests/fixtures/staff_role_capabilities.json"
        ))
        .unwrap();
        assert_eq!(cases.len(), 9);
        for case in cases {
            let expected: BTreeSet<_> = case.capabilities.iter().cloned().collect();
            assert_eq!(expected.len(), case.capabilities.len());
            let actual: BTreeSet<_> = Capability::ALL
                .into_iter()
                .filter(|c| case.roles.allows(*c))
                .map(|c| {
                    serde_json::to_value(c)
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .to_owned()
                })
                .collect();
            assert_eq!(actual, expected, "{:?}", case.roles);
        }
    }
}
