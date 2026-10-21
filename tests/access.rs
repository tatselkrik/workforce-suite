use hris_desktop::{
    access::{Access, AccountDraft, Role},
    domain::{Action, DemoStore, LeaveDraft, LeaveStatus},
};
use uuid::Uuid;

#[test]
fn employee_and_manager_commands_cannot_mutate_team_records_or_approve_own_leave() {
    let data = DemoStore::new().data;
    let own = data.employees[1].id;
    let report = data.employees[3].id;
    let other = data.employees[2].id;
    let mut access = Access {
        user_id: Uuid::new_v4(),
        role: Role::Manager,
        employee_id: Some(own),
        direct_reports: vec![report],
        ..Default::default()
    };
    assert!(access.authorize(&Action::CheckIn(own), &data).is_ok());
    assert!(access.authorize(&Action::CheckIn(report), &data).is_err());
    assert!(
        access
            .authorize(&Action::SaveCompany(data.company.clone()), &data)
            .is_err()
    );
    let own_leave = data.leave.iter().find(|r| r.employee_id == own).unwrap();
    let report_leave = data.leave.iter().find(|r| r.employee_id == report).unwrap();
    assert!(
        access
            .authorize(
                &Action::DecideLeave(own_leave.id, LeaveStatus::Approved),
                &data
            )
            .is_err()
    );
    assert!(
        access
            .authorize(
                &Action::DecideLeave(report_leave.id, LeaveStatus::Approved),
                &data
            )
            .is_ok()
    );
    assert!(!access.can_review(other));
    access.role = Role::Employee;
    assert!(
        access
            .authorize(
                &Action::DecideLeave(report_leave.id, LeaveStatus::Approved),
                &data
            )
            .is_err()
    );
    assert!(
        access
            .authorize(
                &Action::RequestLeave(LeaveDraft {
                    employee_id: Some(report),
                    ..Default::default()
                }),
                &data
            )
            .is_err()
    );
    assert!(
        access
            .authorize(
                &Action::RequestLeave(LeaveDraft {
                    employee_id: Some(own),
                    ..Default::default()
                }),
                &data
            )
            .is_ok()
    );
}

#[test]
fn account_editor_requires_employee_links_and_rejects_self_reporting() {
    let data = DemoStore::new().data;
    let mut draft = AccountDraft::new();
    draft.email = "synthetic@example.invalid".into();
    draft.password = "synthetic initial passphrase".into();
    draft.confirmation = draft.password.clone();
    assert!(draft.validate(&data).is_err());
    draft.employee_id = Some(data.employees[2].id);
    assert!(draft.validate(&data).is_ok());
    draft.user_id = Some(Uuid::nil());
    draft.manager_user_id = Some(Uuid::nil());
    assert!(draft.validate(&data).is_err());
    draft.manager_user_id = None;
    draft.role = Role::HrGeneralist;
    draft.employee_id = None;
    assert!(draft.validate(&data).is_ok());
}

#[test]
fn hr_can_manage_records_but_not_accounts_settings_or_own_approvals() {
    let data = DemoStore::new().data;
    let own = data.employees[1].id;
    let hr = Access {
        role: Role::HrGeneralist,
        employee_id: Some(own),
        ..Default::default()
    };
    assert!(!hr.role.manages_accounts());
    assert!(hr.role.manages_hr());
    assert!(
        hr.authorize(&Action::SaveCompany(data.company.clone()), &data)
            .is_err()
    );
    assert!(!hr.can_review(own));
    assert!(hr.can_review(data.employees[3].id));
}

#[test]
fn desktop_is_for_assigned_staff_roles_and_employee_accounts_are_portal_only() {
    assert!(Role::Administrator.uses_desktop());
    assert!(Role::HrGeneralist.uses_desktop());
    assert!(Role::Manager.uses_desktop());
    assert!(!Role::Employee.uses_desktop());
}
