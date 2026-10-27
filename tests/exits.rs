use hris_desktop::{
    access::Role,
    domain::{Action, DemoStore, EmployeeStatus, Mode},
    exits::*,
};
use uuid::Uuid;
fn fixture() -> (DemoStore, CaseDraft) {
    let mut s = DemoStore::new();
    let mut e = s.data.employees[0].clone();
    e.id = Uuid::new_v4();
    e.employee_code = "EXIT-T001".into();
    e.full_name = "Synthetic departing staff".into();
    e.reports_to = None;
    e.joined_on = Some("2020-01-01".parse().unwrap());
    e.status = EmployeeStatus::Active;
    let mut a = s.data.accounts[1].clone();
    a.user_id = Uuid::new_v4();
    a.employee_id = Some(e.id);
    a.is_owner = false;
    a.role = Role::Employee;
    s.data.accounts.push(a);
    let mut d = CaseDraft::new(&s.data, Some(e.id));
    d.reason = "Synthetic exit preparation".into();
    d.private_notes = "Private case details".into();
    s.data.employees.push(e);
    (s, d)
}
fn start() -> (DemoStore, Uuid) {
    let (mut s, d) = fixture();
    let id = d.id;
    s.execute(Action::Exit(ExitAction::CaseSave(d))).unwrap();
    (s, id)
}
fn decide(s: &mut DemoStore, id: Uuid, status: &str) -> anyhow::Result<()> {
    let revision = s
        .data
        .exits
        .cases
        .iter()
        .find(|c| c.id == id)
        .unwrap()
        .revision;
    s.execute(Action::Exit(ExitAction::CaseStatus {
        id,
        revision,
        status: status.into(),
        reason: "Synthetic workflow decision".into(),
    }))
}
fn resolve(s: &mut DemoStore) {
    for t in s.data.exits.tasks.clone() {
        s.execute(Action::Exit(ExitAction::TaskStatus {
            id: t.id,
            revision: t.revision,
            status: "done".into(),
            reason: "Synthetic completion".into(),
        }))
        .unwrap();
    }
}
#[test]
fn creation_retry_preserves_original_identity_and_current_edits() {
    let (mut s, d) = fixture();
    let a = ExitAction::CaseSave(d.clone());
    s.execute(Action::Exit(a.clone())).unwrap();
    s.execute(Action::Exit(a.clone())).unwrap();
    assert_eq!(s.data.exits.cases.len(), 1);
    assert_eq!(s.data.exits.events.len(), 1);
    assert_eq!(s.data.exits.tasks.len(), 5);
    assert!(a.saved(&s.data));
    let mut edit = CaseDraft::existing(&s.data.exits.cases[0]);
    edit.private_notes = "Changed notes".into();
    edit.reason = "Correct notes".into();
    s.execute(Action::Exit(ExitAction::CaseSave(edit))).unwrap();
    s.execute(Action::Exit(a.clone())).unwrap();
    assert_eq!(s.data.exits.cases[0].private_notes, "Changed notes");
    assert!(!a.saved(&s.data));
    let mut changed = d;
    changed.reason = "Different original retry".into();
    assert!(
        s.execute(Action::Exit(ExitAction::CaseSave(changed)))
            .is_err()
    );
}
#[test]
fn required_clearance_and_admin_finalization_preserve_records_and_history() {
    let (mut s, id) = start();
    let person = s.data.exits.cases[0].employee_id;
    assert!(decide(&mut s, id, "cleared").is_err());
    resolve(&mut s);
    decide(&mut s, id, "cleared").unwrap();
    let counts = (
        s.data.employees.len(),
        s.data.accounts.len(),
        s.data.documents.len(),
    );
    s.data.access.role = Role::HrGeneralist;
    assert!(decide(&mut s, id, "finalized").is_err());
    s.data.access.role = Role::Administrator;
    assert!(readiness(&s.data, id).unwrap().ready());
    decide(&mut s, id, "finalized").unwrap();
    assert_eq!(
        s.data
            .employees
            .iter()
            .find(|e| e.id == person)
            .unwrap()
            .status,
        EmployeeStatus::Archived
    );
    assert!(
        s.data
            .accounts
            .iter()
            .filter(|a| a.employee_id == Some(person))
            .all(|a| !a.active)
    );
    assert_eq!(
        counts,
        (
            s.data.employees.len(),
            s.data.accounts.len(),
            s.data.documents.len()
        )
    );
    assert_eq!(s.data.exits.cases[0].status, "finalized");
    assert!(decide(&mut s, id, "open").is_err());
    assert!(
        s.data
            .employment_history
            .iter()
            .any(|e| e.employee_id == person)
    );
}
#[test]
fn manager_gets_own_task_actions_without_private_case_or_waiver_rights() {
    let (mut s, id) = start();
    let manager = s
        .data
        .accounts
        .iter()
        .find(|a| a.role == Role::Manager)
        .unwrap()
        .clone();
    let person = manager.employee_id.unwrap();
    s.data
        .recruiting
        .interviewers
        .push(hris_desktop::recruiting::Interviewer {
            employee_id: person,
            full_name: "Synthetic manager".into(),
            role: Role::Manager,
            roles: None,
        });
    let assigned_id = s.data.exits.tasks[0].id;
    let mut d = TaskDraft::existing(&s.data.exits.tasks[0]);
    d.assigned_employee_id = Some(person);
    d.reason = "Assign clearance".into();
    s.execute(Action::Exit(ExitAction::TaskSave(d))).unwrap();
    s.data.access.role = Role::Manager;
    s.data.access.user_id = manager.user_id;
    s.data.access.employee_id = Some(person);
    assert!(case(&s.data, id).is_err());
    assert!(readiness(&s.data, id).is_none());
    let t = s
        .data
        .exits
        .tasks
        .iter()
        .find(|t| t.id == assigned_id)
        .unwrap()
        .clone();
    s.execute(Action::Exit(ExitAction::TaskStatus {
        id: t.id,
        revision: t.revision,
        status: "done".into(),
        reason: "Asset returned".into(),
    }))
    .unwrap();
    for (status, task) in [
        (
            "waived",
            s.data
                .exits
                .tasks
                .iter()
                .find(|t| t.id == assigned_id)
                .unwrap()
                .clone(),
        ),
        (
            "done",
            s.data
                .exits
                .tasks
                .iter()
                .find(|t| t.id != assigned_id)
                .unwrap()
                .clone(),
        ),
    ] {
        assert!(
            s.execute(Action::Exit(ExitAction::TaskStatus {
                id: task.id,
                revision: task.revision,
                status: status.into(),
                reason: "Not permitted".into()
            }))
            .is_err()
        );
    }
    let value = serde_json::to_value(&s.data.exits.tasks[0]).unwrap();
    assert!(value.get("private_notes").is_none());
    assert!(value.get("category").is_none());
}
#[test]
fn last_day_owner_and_own_account_are_explicit_readiness_blockers() {
    let (mut s, d) = fixture();
    let mut future = d.clone();
    future.last_working_on = "2199-01-02".into();
    s.execute(Action::Exit(ExitAction::CaseSave(future)))
        .unwrap();
    assert!(
        readiness(&s.data, d.id)
            .unwrap()
            .blockers()
            .iter()
            .any(|b| b.contains("not arrived"))
    );
    let mut own = CaseDraft::new(&s.data, s.data.access.employee_id);
    own.reason = "Owner guard".into();
    let id = own.id;
    s.execute(Action::Exit(ExitAction::CaseSave(own))).unwrap();
    let r = readiness(&s.data, id).unwrap();
    assert!(r.own_account && r.installation_owner);
}
#[test]
fn later_named_approval_steps_and_future_policy_rules_block_exit() {
    let (mut s, id) = start();
    let person = s.data.exits.cases[0].employee_id;
    let mut r = s.data.leave[0].clone();
    r.status = hris_desktop::domain::LeaveStatus::Pending;
    r.policy.approval_steps = vec![hris_desktop::leave::ApprovalStep {
        kind: hris_desktop::leave::ApproverKind::Person,
        employee_id: Some(person),
        approved_by: None,
        approved_at: None,
    }];
    s.data.leave.push(r);
    s.data.leave_setup.policy.types[0]
        .approval_chain
        .push(hris_desktop::leave::ApprovalRule {
            kind: hris_desktop::leave::ApproverKind::Person,
            employee_id: Some(person),
        });
    let r = readiness(&s.data, id).unwrap();
    assert_eq!(r.pending_assigned_leave, 1);
    assert_eq!(r.named_leave_policies, 1);
    assert!(r.blockers().iter().any(|b| b.contains("resubmit")));
}
#[test]
fn editing_last_day_invalidates_task_revisions_and_receipts_check_all_details() {
    let (mut s, id) = start();
    let old = s.data.exits.tasks[0].clone();
    let mut d = CaseDraft::existing(&s.data.exits.cases[0]);
    d.last_working_on = "2099-01-02".into();
    d.reason = "Revised last day".into();
    let a = ExitAction::CaseSave(d);
    s.execute(Action::Exit(a.clone())).unwrap();
    assert!(a.saved(&s.data));
    assert!(
        s.execute(Action::Exit(ExitAction::TaskStatus {
            id: old.id,
            revision: old.revision,
            status: "done".into(),
            reason: "Old date".into()
        }))
        .is_err()
    );
    s.data.exits.cases[0].category = "other".into();
    assert!(!a.saved(&s.data));
    assert!(!readiness(&s.data, id).unwrap().last_day_reached);
}
#[test]
fn cancelled_exit_does_not_disable_access_and_closed_tasks_cannot_change() {
    let (mut s, id) = start();
    let person = s.data.exits.cases[0].employee_id;
    decide(&mut s, id, "cancelled").unwrap();
    assert!(
        s.data
            .accounts
            .iter()
            .find(|a| a.employee_id == Some(person))
            .unwrap()
            .active
    );
    assert_eq!(
        s.data
            .employees
            .iter()
            .find(|e| e.id == person)
            .unwrap()
            .status,
        EmployeeStatus::Active
    );
    let t = &s.data.exits.tasks[0];
    assert!(
        ExitAction::TaskStatus {
            id: t.id,
            revision: t.revision,
            status: "done".into(),
            reason: "Closed".into()
        }
        .payload(&s.data)
        .is_err()
    );
}
#[test]
fn signed_out_employee_and_cross_company_access_are_rejected() {
    for mode in [Mode::Demo, Mode::SignedOut] {
        let (mut s, d) = fixture();
        s.data.mode = mode;
        s.data.access.role = Role::Employee;
        assert!(s.execute(Action::Exit(ExitAction::CaseSave(d))).is_err());
    }
    let (mut s, d) = fixture();
    s.data.access.company_id = Uuid::new_v4();
    assert!(s.execute(Action::Exit(ExitAction::CaseSave(d))).is_err());
}
