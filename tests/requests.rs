use chrono::{Duration, NaiveDate, Utc};
use hris_desktop::{
    access::{Access, Role},
    corrections::CorrectionDraft,
    domain::*,
    requests::*,
};
fn fixture() -> (DemoStore, CorrectionDraft) {
    let mut s = DemoStore::new();
    s.data.attendance.clear();
    s.data.corrections.clear();
    s.data.leave.clear();
    s.data.breaks = Default::default();
    s.data.employees[0].joined_on = None;
    let id = s.data.employees[0].id;
    let day = s.data.company.today() - Duration::days(3);
    let mut d = CorrectionDraft::new(id, day, None, &s.data);
    d.checked_in = "08:15".into();
    d.checked_out = "17:00".into();
    d.reason = "Synthetic missed check-in".into();
    (s, d)
}
fn resolve(s: &mut DemoStore, id: uuid::Uuid, status: Status) -> anyhow::Result<()> {
    s.execute(Action::AttendanceRequest(RequestAction::Resolve {
        id,
        expected_revision: 0,
        status,
        reason: "Synthetic documented decision".into(),
    }))
}
fn manager(s: &mut DemoStore, target: uuid::Uuid) {
    s.data.access = Access {
        user_id: uuid::Uuid::from_u128(777),
        employee_id: Some(s.data.employees[1].id),
        role: Role::Manager,
        direct_reports: vec![target],
        company_id: s.data.company.id,
        ..Default::default()
    };
}
#[test]
fn proposals_do_not_change_attendance_and_own_cancel_retains_history() {
    let (mut s, d) = fixture();
    s.execute(Action::AttendanceRequest(RequestAction::Submit(d.clone())))
        .unwrap();
    let r = s.data.requests.records[0].clone();
    assert!(s.data.attendance.is_empty());
    assert_eq!(r.status, Status::Pending);
    assert!(
        s.execute(Action::AttendanceRequest(RequestAction::Submit(d.clone())))
            .is_err()
    );
    assert!(resolve(&mut s, r.id, Status::Approved).is_err());
    assert!(resolve(&mut s, r.id, Status::Rejected).is_err());
    resolve(&mut s, r.id, Status::Cancelled).unwrap();
    assert_eq!(s.data.requests.records[0].status, Status::Cancelled);
    assert_eq!(s.data.requests.events.len(), 2);
    assert!(s.data.attendance.is_empty());
    assert!(resolve(&mut s, r.id, Status::Cancelled).is_err());
    s.execute(Action::AttendanceRequest(RequestAction::Submit(d)))
        .unwrap();
    assert_eq!(s.data.requests.records.len(), 2);
}
#[test]
fn manager_approval_creates_missing_attendance_and_one_linked_audit() {
    let (mut s, d) = fixture();
    let target = d.employee_id;
    s.execute(Action::AttendanceRequest(RequestAction::Submit(d)))
        .unwrap();
    let r = s.data.requests.records[0].clone();
    manager(&mut s, target);
    resolve(&mut s, r.id, Status::Approved).unwrap();
    let a = &s.data.attendance[0];
    assert_eq!(a.checked_in_at, r.proposed_in);
    assert_eq!(a.checked_out_at, r.proposed_out);
    assert_eq!(a.revision, 0);
    assert!(a.calculation_snapshot.is_some());
    assert_eq!(s.data.corrections[0].actor_role, Role::Manager);
    assert_eq!(
        s.data.requests.records[0].correction_id,
        Some(s.data.corrections[0].id)
    );
    assert_eq!(s.data.requests.events[1].after_record.as_ref(), Some(a));
    assert!(resolve(&mut s, r.id, Status::Approved).is_err());
}
#[test]
fn approval_edits_times_while_retaining_recorded_rules_and_breaks() {
    let (mut s, d) = fixture();
    let c = d.change(&s.data, Utc::now()).unwrap();
    s.execute(Action::CorrectAttendance(c)).unwrap();
    let before = s.data.attendance[0].clone();
    let mut edit = CorrectionDraft::new(
        before.employee_id,
        before.work_date,
        Some(before.clone()),
        &s.data,
    );
    edit.checked_in = "08:30".into();
    edit.reason = "Synthetic corrected arrival".into();
    s.execute(Action::AttendanceRequest(RequestAction::Submit(edit)))
        .unwrap();
    let id = s.data.requests.records[0].id;
    let breaks = s.data.breaks.records.clone();
    manager(&mut s, before.employee_id);
    resolve(&mut s, id, Status::Approved).unwrap();
    let after = &s.data.attendance[0];
    assert_ne!(after.checked_in_at, before.checked_in_at);
    assert_eq!(after.revision, 1);
    assert_eq!(after.calculation_snapshot, before.calculation_snapshot);
    assert_eq!(after.planned_shift, before.planned_shift);
    assert_eq!(s.data.breaks.records, breaks);
    assert_eq!(
        s.data.requests.events[1].before_record.as_ref(),
        Some(&before)
    );
}
#[test]
fn changed_attendance_and_manager_scope_block_approval_without_partial_writes() {
    let (mut s, d) = fixture();
    s.execute(Action::AttendanceRequest(RequestAction::Submit(d.clone())))
        .unwrap();
    let id = s.data.requests.records[0].id;
    let original = s.data.access.clone();
    s.execute(Action::CorrectAttendance(
        d.change(&s.data, Utc::now()).unwrap(),
    ))
    .unwrap();
    manager(&mut s, d.employee_id);
    assert!(resolve(&mut s, id, Status::Approved).is_err());
    assert_eq!(s.data.requests.records[0].status, Status::Pending);
    assert_eq!(s.data.requests.events.len(), 1);
    assert_eq!(s.data.corrections.len(), 1);
    s.data.access.direct_reports.clear();
    assert!(resolve(&mut s, id, Status::Rejected).is_err());
    s.data.access = original;
    resolve(&mut s, id, Status::Cancelled).unwrap();
}
#[test]
fn only_own_active_employee_can_submit_valid_non_overlapping_times() {
    let (mut s, mut d) = fixture();
    let now = Utc::now();
    d.employee_id = s.data.employees[1].id;
    assert!(payload(&RequestAction::Submit(d.clone()), &s.data, now).is_err());
    d.employee_id = s.data.employees[0].id;
    d.reason.clear();
    assert!(payload(&RequestAction::Submit(d.clone()), &s.data, now).is_err());
    d.reason = "Synthetic request".into();
    d.checked_out = "07:00".into();
    assert!(payload(&RequestAction::Submit(d.clone()), &s.data, now).is_err());
    d.checked_out = "17:00".into();
    s.data.employees[0].status = EmployeeStatus::Archived;
    assert!(payload(&RequestAction::Submit(d.clone()), &s.data, now).is_err());
    s.data.employees[0].status = EmployeeStatus::Active;
    s.data.employees[0].joined_on = Some(s.data.company.today());
    assert!(payload(&RequestAction::Submit(d), &s.data, now).is_err());
}
#[test]
fn rejection_preserves_original_proposal_and_times_and_rejects_blank_decisions() {
    let (mut s, d) = fixture();
    s.execute(Action::AttendanceRequest(RequestAction::Submit(d.clone())))
        .unwrap();
    let r = s.data.requests.records[0].clone();
    manager(&mut s, d.employee_id);
    let a = RequestAction::Resolve {
        id: r.id,
        expected_revision: 0,
        status: Status::Rejected,
        reason: String::new(),
    };
    assert!(s.execute(Action::AttendanceRequest(a)).is_err());
    resolve(&mut s, r.id, Status::Rejected).unwrap();
    assert_eq!(s.data.requests.records[0].proposed_in, r.proposed_in);
    assert_eq!(s.data.requests.records[0].reason, r.reason);
    assert!(s.data.attendance.is_empty());
    assert_eq!(s.data.requests.events[1].status, Status::Rejected);
    assert!(resolve(&mut s, r.id, Status::Approved).is_err());
}
#[test]
fn timezone_changes_and_ambiguous_local_times_require_a_fresh_request() {
    let (mut s, mut d) = fixture();
    s.execute(Action::AttendanceRequest(RequestAction::Submit(d.clone())))
        .unwrap();
    let r = s.data.requests.records[0].clone();
    s.data.company.timezone = "Asia/Tokyo".into();
    assert!(stale(&s.data, &r));
    manager(&mut s, d.employee_id);
    assert!(resolve(&mut s, r.id, Status::Approved).is_err());
    s.data.company.timezone = "America/New_York".into();
    d.work_date = NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
    d.checked_in_date = d.work_date.to_string();
    d.checked_in = "01:30".into();
    assert!(
        d.proposal(&s.data, Utc::now() + Duration::days(60))
            .is_err()
    );
}
