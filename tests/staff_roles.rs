use hris_desktop::{
    access::{Access, Role},
    staff_roles::{Capability, Context, StaffRole, StaffRoles},
};
use serde_json::json;
use uuid::Uuid;
fn access(roles: Vec<StaffRole>) -> Access {
    let roles = StaffRoles::new(roles).unwrap();
    let base_role = roles.compatible_base_role();
    let mut a = Access {
        company_id: Uuid::from_u128(1),
        user_id: Uuid::from_u128(2),
        employee_id: Some(Uuid::from_u128(3)),
        role: base_role,
        ..Default::default()
    };
    a.bind_staff_context(Context {
        schema_version: 1,
        company_id: a.company_id,
        user_id: a.user_id,
        employee_id: a.employee_id.unwrap(),
        base_role,
        revision: 1,
        runtime_ready: true,
        roles,
    })
    .unwrap();
    a
}
#[test]
fn development_admits_staff_without_other_employee_contacts_or_clocks() {
    let a = access(vec![StaffRole::LearningPerformanceOfficer]);
    assert!(a.uses_desktop());
    assert!(a.allows(Capability::DevelopmentAdministration));
    assert!(a.can_read_contacts(a.employee_id.unwrap()));
    assert!(a.can_record(a.employee_id.unwrap()));
    assert!(!a.can_read_contacts(Uuid::from_u128(10)));
    assert!(!a.can_record(Uuid::from_u128(10)));
    assert!(!a.can_review(Uuid::from_u128(10)));
    assert!(!a.allows(Capability::PayrollPreparation));
}
#[test]
fn multiple_roles_union_responsibilities_without_self_review_or_payroll_approval() {
    let mut a = access(vec![
        StaffRole::HrGeneralist,
        StaffRole::PayrollOfficer,
        StaffRole::Manager,
    ]);
    let report = Uuid::from_u128(10);
    a.direct_reports.push(report);
    assert_eq!(a.role, Role::Manager);
    assert!(a.can_read_contacts(report));
    assert!(a.can_record(report));
    assert!(a.can_review(report));
    assert!(!a.can_review(a.employee_id.unwrap()));
    assert!(a.allows(Capability::PayrollPreparation));
    assert!(!a.allows(Capability::PayrollApproval));
}
#[test]
fn stale_context_fails_closed_including_own_record_actions() {
    let mut a = access(vec![StaffRole::Administrator]);
    a.staff_context.as_mut().unwrap().user_id = Uuid::from_u128(99);
    assert!(!a.uses_desktop());
    assert!(!a.allows(Capability::Personnel));
    assert!(!a.can_review(Uuid::from_u128(10)));
    assert!(!a.can_read_contacts(a.employee_id.unwrap()));
    assert!(!a.can_record(a.employee_id.unwrap()));
}
#[test]
fn overbroad_legacy_projection_is_refused_before_binding() {
    let mut a = access(vec![StaffRole::HrGeneralist]);
    let mut c = a.staff_context.clone().unwrap();
    a.role = Role::HrGeneralist;
    c.base_role = Role::HrGeneralist;
    assert!(a.bind_staff_context(c).is_err());
    let mut a = access(vec![StaffRole::PayrollOfficer]);
    let mut c = a.staff_context.clone().unwrap();
    c.roles = StaffRoles::new(vec![StaffRole::Administrator]).unwrap();
    assert!(a.bind_staff_context(c).is_err());
}
#[test]
fn legacy_access_json_cannot_supply_privileged_staff_context() {
    let actor = access(vec![StaffRole::Administrator]);
    let a:Access=serde_json::from_value(json!({"company_id":actor.company_id,"user_id":actor.user_id,
  "employee_id":actor.employee_id,"role":"employee","direct_reports":[],"staff_context":actor.staff_context})).unwrap();
    assert!(a.staff_context.is_none());
    assert!(!a.uses_desktop());
    assert!(!a.allows(Capability::PayrollApproval));
}
#[test]
fn unbound_legacy_accounts_keep_existing_access_until_explicit_migration() {
    let a = Access {
        role: Role::HrGeneralist,
        ..Default::default()
    };
    assert!(a.uses_desktop());
    for c in [
        Capability::Personnel,
        Capability::Recruiting,
        Capability::DevelopmentAdministration,
        Capability::PayrollPreparation,
    ] {
        assert!(a.allows(c));
    }
    assert!(!a.allows(Capability::PayrollApproval));
}
#[test]
fn legacy_role_projection_does_not_expose_unassigned_specialties() {
    assert_eq!(
        StaffRoles::new(vec![StaffRole::Recruiter])
            .unwrap()
            .compatible_base_role(),
        Role::Employee
    );
    assert_eq!(
        StaffRoles::new(vec![
            StaffRole::LearningPerformanceOfficer,
            StaffRole::PayrollOfficer
        ])
        .unwrap()
        .compatible_base_role(),
        Role::Employee
    );
    assert_eq!(
        StaffRoles::legacy(Role::HrGeneralist).compatible_base_role(),
        Role::HrGeneralist
    );
    assert_eq!(
        StaffRoles::new(vec![StaffRole::Manager, StaffRole::PayrollOfficer])
            .unwrap()
            .compatible_base_role(),
        Role::Manager
    );
}
