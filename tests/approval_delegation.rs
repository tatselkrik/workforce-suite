use chrono::{Duration, Utc};
use hris_desktop::{
    access::{Access, Role},
    approvals::*,
    domain::*,
    leave::{self, ApprovalStep, ApproverKind},
};
use uuid::Uuid;
fn id(n: u128) -> Uuid {
    Uuid::from_u128(48000 + n)
}
fn admin() -> Snapshot {
    let mut d = DemoStore::new().data;
    d.access = Access {
        company_id: d.company.id,
        user_id: id(1),
        employee_id: Some(d.employees[0].id),
        role: Role::Administrator,
        ..Default::default()
    };
    let mut c = Context::empty(&d);
    c.staff = (0..3)
        .map(|n| Staff {
            user_id: id(10 + n as u128),
            employee_id: d.employees[n].id,
            name: d.employees[n].full_name.clone(),
            role: Role::Manager,
            staff_roles: None,
        })
        .collect();
    d.approvals = Some(c);
    d
}
fn proposal(d: &Snapshot) -> Change {
    let c = d.approvals.as_ref().unwrap();
    Change::Grant {
        id: id(30),
        expected_revision: c.revision,
        scope: Scope::Leave,
        principal_id: c.staff[0].user_id,
        delegate_id: c.staff[1].user_id,
        starts_on: c.as_of,
        ends_on: c.as_of + Duration::days(7),
        reason: "Fictional documented temporary substitute".into(),
        reference: "Fictional reference".into(),
    }
}
fn delegated() -> Snapshot {
    let mut d = admin();
    let p = proposal(&d);
    apply_demo(&mut d, &p).unwrap();
    let g = d.approvals.as_ref().unwrap().grants[0].clone();
    d.access = Access {
        company_id: d.company.id,
        user_id: g.delegate_id,
        role: Role::Manager,
        employee_id: Some(g.delegate_employee_id),
        ..Default::default()
    };
    let mut row = d.leave[0].clone();
    row.id = id(50);
    row.employee_id = d.employees[2].id;
    row.status = LeaveStatus::Pending;
    row.policy.revision = 0;
    row.policy.approval_steps = vec![ApprovalStep {
        kind: ApproverKind::Manager,
        employee_id: Some(g.principal_employee_id),
        ..Default::default()
    }];
    d.leave = vec![row.clone()];
    let c = d.approvals.as_mut().unwrap();
    c.user_id = d.access.user_id;
    c.employee_id = d.access.employee_id;
    c.role = d.access.role;
    c.audit.clear();
    c.staff.clear();
    c.permits = vec![Permit {
        scope: Scope::Leave,
        request_id: row.id,
        request_revision: 0,
        delegation_id: g.id,
        source_current: true,
    }];
    d
}
#[test]
fn role_grade_never_delegates_hr_or_admin_to_manager() {
    for p in Role::ALL {
        for d in Role::ALL {
            assert_eq!(
                substitute_role(p, d),
                match p {
                    Role::Administrator => d == Role::Administrator,
                    Role::HrGeneralist => d.manages_hr(),
                    Role::Manager => d.uses_desktop(),
                    Role::Employee => false,
                }
            );
        }
    }
}
#[test]
fn fictional_demo_staff_have_valid_non_nil_identities() {
    let d = DemoStore::new().data;
    Context::empty(&d).validate(&d).unwrap();
}
#[test]
fn dated_grant_and_revocation_retain_original_and_exact_retry() {
    let mut d = admin();
    let p = proposal(&d);
    apply_demo(&mut d, &p).unwrap();
    apply_demo(&mut d, &p).unwrap();
    assert_eq!(d.approvals.as_ref().unwrap().revision, 1);
    assert_eq!(d.approvals.as_ref().unwrap().grants.len(), 1);
    let c = d.approvals.as_ref().unwrap();
    c.validate(&d).unwrap();
    assert!(p.matches_history(c));
    let reverse = Change::Revoke {
        id: id(31),
        expected_revision: 1,
        delegation_id: p.id(),
        reason: "Fictional early return".into(),
    };
    apply_demo(&mut d, &reverse).unwrap();
    apply_demo(&mut d, &reverse).unwrap();
    let c = d.approvals.as_ref().unwrap();
    assert_eq!(c.grants.len(), 1);
    assert_eq!(c.revocations.len(), 1);
    assert!(!c.grants[0].current);
    assert_eq!(c.revision, 2);
    c.validate(&d).unwrap();
}
#[test]
fn requests_have_bound_identity_dates_revision_and_scope() {
    let d = admin();
    let Change::Grant {
        id,
        expected_revision,
        scope,
        principal_id,
        delegate_id,
        starts_on,
        ends_on,
        reason,
        reference,
    } = proposal(&d)
    else {
        unreachable!()
    };
    let base = Change::Grant {
        id,
        expected_revision,
        scope,
        principal_id,
        delegate_id,
        starts_on,
        ends_on,
        reason: reason.clone(),
        reference: reference.clone(),
    };
    assert!(base.payload(&d).is_ok());
    for (start, end) in [
        (starts_on - Duration::days(1), ends_on),
        (starts_on, starts_on - Duration::days(1)),
        (starts_on, starts_on + Duration::days(367)),
    ] {
        assert!(
            Change::Grant {
                id,
                expected_revision,
                scope,
                principal_id,
                delegate_id,
                starts_on: start,
                ends_on: end,
                reason: reason.clone(),
                reference: reference.clone()
            }
            .payload(&d)
            .is_err()
        );
    }
    let mut v = serde_json::to_value(&base).unwrap();
    v["expected_revision"] = serde_json::json!(4);
    assert!(
        serde_json::from_value::<Change>(v)
            .unwrap()
            .payload(&d)
            .is_err()
    );
    let mut v = serde_json::to_value(&base).unwrap();
    v["extra"] = true.into();
    assert!(serde_json::from_value::<Change>(v).is_err());
    let mut v = serde_json::to_value(&base).unwrap();
    v["scope"] = "payroll".into();
    assert!(serde_json::from_value::<Change>(v).is_err());
}
#[test]
fn only_administrators_may_save_and_overlapping_dates_are_blocked() {
    let mut d = admin();
    let p = proposal(&d);
    for role in [Role::Employee, Role::Manager, Role::HrGeneralist] {
        d.access.role = role;
        assert!(p.payload(&d).is_err());
    }
    d.access.role = Role::Administrator;
    apply_demo(&mut d, &p).unwrap();
    let mut duplicate = serde_json::to_value(proposal(&d)).unwrap();
    duplicate["id"] = serde_json::json!(id(39));
    assert!(
        serde_json::from_value::<Change>(duplicate)
            .unwrap()
            .payload(&d)
            .is_err()
    );
}
#[test]
fn substitute_can_review_exact_foreign_request_without_personnel_scope() {
    let mut d = delegated();
    d.approvals.as_ref().unwrap().validate(&d).unwrap();
    assert!(!leave::can_review_access(&d.access, &d.leave[0]));
    assert!(leave::can_review(&d, &d.leave[0]));
    assert!(!d.access.can_read_contacts(d.leave[0].employee_id));
    assert!(!d.access.can_review(d.leave[0].employee_id));
    let action = Action::ReviewLeave(
        d.leave[0].id,
        0,
        true,
        "Fictional delegated approval".into(),
    );
    assert!(d.access.authorize(&action, &d).is_ok());
    let mut foreign = d.access.clone();
    foreign.user_id = id(999);
    assert!(foreign.authorize(&action, &d).is_err());
    d.leave[0].policy.revision = 1;
    assert!(!leave::can_review(&d, &d.leave[0]));
    assert!(d.approvals.as_ref().unwrap().validate(&d).is_err());
}
#[test]
fn own_expired_revoked_stale_or_mismatched_context_cannot_review() {
    for case in 0..6 {
        let mut d = delegated();
        match case {
            0 => d.leave[0].employee_id = d.access.employee_id.unwrap(),
            1 => d.approvals.as_mut().unwrap().as_of -= Duration::days(1),
            2 => d.approvals.as_mut().unwrap().grants[0].current = false,
            3 => {
                d.approvals.as_mut().unwrap().grants[0].ends_on =
                    d.company.today() - Duration::days(1)
            }
            4 => d.approvals.as_mut().unwrap().user_id = id(888),
            _ => d.leave[0].status = LeaveStatus::Approved,
        }
        assert!(!leave::can_review(&d, &d.leave[0]), "case{case}");
    }
}
#[test]
fn private_audit_and_foreign_grants_are_rejected_for_substitutes() {
    let d = delegated();
    let mut c = d.approvals.clone().unwrap();
    c.audit.push(Audit {
        id: c.grants[0].id,
        reason: "Private HR reason".into(),
        reference: "Private reference".into(),
        actor_name: "HR".into(),
        created_at: Utc::now(),
    });
    assert!(c.validate(&d).is_err());
    let mut c = d.approvals.clone().unwrap();
    c.grants[0].delegate_id = id(111);
    assert!(c.validate(&d).is_err());
    let mut v = serde_json::to_value(d.approvals.as_ref().unwrap()).unwrap();
    v["secret_evidence"] = "not allowed".into();
    assert!(serde_json::from_value::<Context>(v).is_err());
}
#[test]
fn a_changed_identity_is_not_treated_as_an_idempotent_save() {
    let mut d = admin();
    let p = proposal(&d);
    apply_demo(&mut d, &p).unwrap();
    let mut changed = serde_json::to_value(&p).unwrap();
    changed["reason"] = "Different saved content".into();
    let changed: Change = serde_json::from_value(changed).unwrap();
    assert!(!changed.matches_history(d.approvals.as_ref().unwrap()));
    assert!(changed.payload(&d).is_err());
}

#[test]
fn full_role_bindings_reject_same_base_role_changes_but_retain_inactive_history() {
    use hris_desktop::staff_roles::{Context as StaffContext, StaffRole as S, StaffRoles};
    let mut data = delegated();
    let roles = StaffRoles::new(vec![S::Recruiter]).unwrap();
    data.access.role = Role::Employee;
    data.access.staff_contract_version = 1;
    data.access.bind_staff_context(StaffContext {
        schema_version: 1, company_id:data.access.company_id,user_id:data.access.user_id,
        employee_id:data.access.employee_id.unwrap(),base_role:Role::Employee,revision:2,
        runtime_ready:true,roles:roles.clone(),
    }).unwrap();
    let c = data.approvals.as_mut().unwrap();
    c.version = "approval-delegation-v2".into(); c.role=Role::Employee;
    c.grants[0].principal_role = Role::Employee;
    c.grants[0].delegate_role = Role::Employee;
    c.grants[0].principal_staff_roles = Some(StaffRoles::new(vec![S::PayrollOfficer]).unwrap());
    c.grants[0].delegate_staff_roles = Some(roles);
    c.validate_access(&data.access,data.company.today()).unwrap();
    data.access.staff_context.as_mut().unwrap().roles = StaffRoles::new(vec![S::LearningPerformanceOfficer]).unwrap();
    assert!(c.validate_access(&data.access,data.company.today()).is_err());
    c.grants[0].current = false; c.permits.clear();
    c.validate_access(&data.access,data.company.today()).unwrap();
    c.grants[0].delegate_staff_roles = None;
    assert!(c.validate_access(&data.access,data.company.today()).is_err());
}
#[test]
fn specialist_delegation_preserves_hr_admin_and_operational_scope_grades() {
    use hris_desktop::staff_roles::{StaffRole as S,StaffRoles};
    let roles = |items:Vec<S>| StaffRoles::new(items).unwrap();
    let hr=roles(vec![S::HrGeneralist]); let payroll=roles(vec![S::PayrollOfficer]);
    let recruiter=roles(vec![S::Recruiter]);let manager=roles(vec![S::Manager]);
    let admin=roles(vec![S::Administrator]);let employee=roles(vec![S::Employee]);
    assert!(substitute_roles(Scope::Leave,&payroll,&recruiter));
    assert!(!substitute_roles(Scope::Attendance,&payroll,&recruiter));
    assert!(!substitute_roles(Scope::Overtime,&manager,&payroll));
    assert!(substitute_roles(Scope::Overtime,&manager,&hr));
    assert!(!substitute_roles(Scope::Leave,&hr,&manager));
    assert!(!substitute_roles(Scope::Leave,&admin,&hr));
    assert!(substitute_roles(Scope::Leave,&admin,&admin));
    assert!(!substitute_roles(Scope::Leave,&recruiter,&employee));
}
