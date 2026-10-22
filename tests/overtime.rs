use chrono::{Duration, Utc};
use hris_desktop::{
    access::{Access, Role},
    breaks::{BreakAction, BreakDraft},
    calculations::{ApplyPolicy, PolicyDraft},
    corrections::CorrectionDraft,
    domain::*,
    overtime::*,
    shifts::*,
};
fn fixture() -> (DemoStore, Draft) {
    let mut s = DemoStore::new();
    s.data.attendance.clear();
    s.data.corrections.clear();
    s.data.leave.clear();
    s.data.breaks = Default::default();
    s.data.shifts = Default::default();
    s.data.employees[0].joined_on = None;
    let mut p = PolicyDraft::new(&s.data.calculations.policy);
    p.policy.overtime_min_minutes = 30;
    p.policy.overtime_round_minutes = 15;
    p.reason = "Synthetic overtime rules".into();
    s.execute(Action::SaveCalculationPolicy(p)).unwrap();
    s.execute(Action::SaveShiftTemplate(ShiftTemplateDraft {
        name: "Synthetic overtime day".into(),
        days: vec![1, 2, 3, 4, 5, 6, 7],
        works_on_holidays: true,
        ..Default::default()
    }))
    .unwrap();
    let date = "2026-03-04".parse().unwrap();
    let employee = s.data.employees[0].id;
    let mut a = ShiftAssignmentDraft::new(&s.data, employee, date);
    a.template_id = Some(s.data.shifts.templates[0].id);
    a.expected_template_revision = Some(0);
    a.reason = "Synthetic overtime assignment".into();
    s.execute(Action::AssignShift(a)).unwrap();
    let mut d = CorrectionDraft::new(employee, date, None, &s.data);
    d.checked_in = "08:17".into();
    d.checked_out = "18:47".into();
    d.reason = "Synthetic completed overtime attendance".into();
    s.execute(Action::CorrectAttendance(
        d.change(&s.data, Utc::now()).unwrap(),
    ))
    .unwrap();
    let parent = s.data.attendance[0].clone();
    for (name, start, end) in [
        ("Synthetic meal", "12:00", "13:00"),
        ("Synthetic outside break", "17:15", "17:30"),
    ] {
        let mut b = BreakDraft::new(&s.data, &parent, None);
        b.name = name.into();
        b.started_time = start.into();
        b.ended_time = end.into();
        b.reason = "Synthetic actual break".into();
        s.execute(Action::AttendanceBreak(BreakAction::Save(b)))
            .unwrap();
    }
    let mut draft = Draft::new(&s.data, &parent).unwrap();
    draft.minutes = 60;
    draft.reason = "Synthetic work outside the shift".into();
    (s, draft)
}
fn manager(s: &mut DemoStore, employee: uuid::Uuid) {
    s.data.access = Access {
        company_id: s.data.company.id,
        user_id: uuid::Uuid::from_u128(779),
        employee_id: Some(s.data.employees[1].id),
        role: Role::Manager,
        direct_reports: vec![employee],
        ..Default::default()
    };
}
fn decide(
    s: &mut DemoStore,
    id: uuid::Uuid,
    status: Status,
    minutes: Option<u32>,
) -> anyhow::Result<()> {
    let revision = s
        .data
        .overtime
        .records
        .iter()
        .find(|r| r.id == id)
        .unwrap()
        .revision;
    s.execute(Action::Overtime(OvertimeAction::Resolve {
        id,
        expected_revision: revision,
        status,
        minutes,
        reason: "Synthetic documented overtime decision".into(),
    }))
}
#[test]
fn approval_and_revocation_retain_minutes_and_history_without_changing_attendance() {
    let (mut s, d) = fixture();
    let facts = s.data.attendance.clone();
    let employee = d.basis.attendance.employee_id;
    assert_eq!(d.basis.minutes, 90);
    s.execute(Action::Overtime(OvertimeAction::Submit(d)))
        .unwrap();
    let id = s.data.overtime.records[0].id;
    assert_eq!(s.data.attendance, facts);
    assert!(decide(&mut s, id, Status::Approved, Some(60)).is_err());
    let requester = s.data.access.clone();
    manager(&mut s, employee);
    decide(&mut s, id, Status::Approved, Some(45)).unwrap();
    assert_eq!(
        effective_minutes(&s.data, &s.data.overtime.records[0]),
        Some(45)
    );
    assert_eq!(s.data.attendance, facts);
    decide(&mut s, id, Status::Revoked, None).unwrap();
    assert_eq!(s.data.overtime.records[0].approved_minutes, Some(45));
    assert_eq!(
        effective_minutes(&s.data, &s.data.overtime.records[0]),
        None
    );
    assert_eq!(s.data.overtime.events.len(), 3);
    assert_eq!(s.data.overtime.events[2].before_approved_minutes, Some(45));
    s.data.access = requester;
    let mut d = Draft::new(&s.data, &s.data.attendance[0]).unwrap();
    d.reason = "Synthetic revised request after revocation".into();
    s.execute(Action::Overtime(OvertimeAction::Submit(d)))
        .unwrap();
}
#[test]
fn requests_and_approvals_are_bounded_by_candidate_and_requested_minutes() {
    let (mut s, mut d) = fixture();
    d.minutes = 91;
    assert!(payload(&OvertimeAction::Submit(d.clone()), &s.data).is_err());
    d.minutes = 0;
    assert!(payload(&OvertimeAction::Submit(d.clone()), &s.data).is_err());
    d.minutes = 60;
    let mut forged = d.clone();
    forged.basis.minutes = 999;
    assert!(payload(&OvertimeAction::Submit(forged), &s.data).is_err());
    s.execute(Action::Overtime(OvertimeAction::Submit(d.clone())))
        .unwrap();
    assert!(
        s.execute(Action::Overtime(OvertimeAction::Submit(d)))
            .is_err()
    );
    let id = s.data.overtime.records[0].id;
    let employee = s.data.overtime.records[0].employee_id;
    manager(&mut s, employee);
    assert!(decide(&mut s, id, Status::Approved, Some(61)).is_err());
    assert!(decide(&mut s, id, Status::Approved, Some(0)).is_err());
    assert!(decide(&mut s, id, Status::Approved, None).is_err());
    decide(&mut s, id, Status::Approved, Some(60)).unwrap();
    assert!(decide(&mut s, id, Status::Approved, Some(60)).is_err());
}
#[test]
fn rejection_cancellation_and_scope_changes_preserve_original_facts() {
    let (mut s, d) = fixture();
    let facts = s.data.attendance.clone();
    let employee = d.basis.attendance.employee_id;
    let owner = s.data.access.clone();
    s.execute(Action::Overtime(OvertimeAction::Submit(d.clone())))
        .unwrap();
    let id = s.data.overtime.records[0].id;
    decide(&mut s, id, Status::Cancelled, None).unwrap();
    assert_eq!(s.data.overtime.events.len(), 2);
    s.execute(Action::Overtime(OvertimeAction::Submit(d)))
        .unwrap();
    let id = s.data.overtime.records[1].id;
    manager(&mut s, employee);
    s.data.access.direct_reports.clear();
    assert!(decide(&mut s, id, Status::Rejected, None).is_err());
    s.data.access.direct_reports.push(employee);
    decide(&mut s, id, Status::Rejected, None).unwrap();
    assert_eq!(s.data.attendance, facts);
    s.data.access = owner;
    assert!(decide(&mut s, id, Status::Cancelled, None).is_err());
}
#[test]
fn new_breaks_invalidate_review_even_if_calculated_minutes_are_unchanged() {
    let (mut s, d) = fixture();
    let employee = d.basis.attendance.employee_id;
    let parent = d.basis.attendance.clone();
    s.execute(Action::Overtime(OvertimeAction::Submit(d)))
        .unwrap();
    let id = s.data.overtime.records[0].id;
    let mut b = BreakDraft::new(&s.data, &parent, None);
    b.name = "Synthetic later paid break".into();
    b.paid = true;
    b.started_time = "10:00".into();
    b.ended_time = "10:15".into();
    b.reason = "Synthetic concurrent break".into();
    s.execute(Action::AttendanceBreak(BreakAction::Save(b)))
        .unwrap();
    assert_eq!(capture(&s.data, &parent).unwrap().minutes, 90);
    assert!(!matches_current(&s.data, &s.data.overtime.records[0]));
    manager(&mut s, employee);
    assert!(decide(&mut s, id, Status::Approved, Some(60)).is_err());
    assert_eq!(s.data.overtime.events.len(), 1);
    assert_eq!(s.data.overtime.records[0].status, Status::Pending);
    decide(&mut s, id, Status::Rejected, None).unwrap();
}
#[test]
fn company_rule_edits_retain_basis_but_explicit_record_application_invalidates_it() {
    let (mut s, d) = fixture();
    let employee = d.basis.attendance.employee_id;
    let parent = d.basis.attendance.clone();
    let owner = s.data.access.clone();
    s.execute(Action::Overtime(OvertimeAction::Submit(d)))
        .unwrap();
    let id = s.data.overtime.records[0].id;
    manager(&mut s, employee);
    decide(&mut s, id, Status::Approved, Some(60)).unwrap();
    s.data.access = owner;
    let mut p = PolicyDraft::new(&s.data.calculations.policy);
    p.policy.overtime_min_minutes = 0;
    p.policy.overtime_round_minutes = 1;
    p.reason = "Synthetic future rule update".into();
    s.execute(Action::SaveCalculationPolicy(p)).unwrap();
    assert_eq!(
        effective_minutes(&s.data, &s.data.overtime.records[0]),
        Some(60)
    );
    let update = ApplyPolicy {
        attendance_id: parent.id,
        expected_revision: 0,
        expected_policy_revision: s.data.calculations.policy.revision,
        reason: "Synthetic explicit rule application".into(),
    };
    s.execute(Action::ApplyCalculationPolicy(update)).unwrap();
    assert_eq!(
        effective_minutes(&s.data, &s.data.overtime.records[0]),
        None
    );
    assert_eq!(s.data.overtime.records[0].status, Status::Approved);
    assert_eq!(s.data.overtime.events.len(), 2);
}
#[test]
fn approved_leave_and_open_attendance_withhold_candidates() {
    let (mut s, d) = fixture();
    let parent = d.basis.attendance.clone();
    let mut leave = DemoStore::new().data.leave[0].clone();
    leave.employee_id = parent.employee_id;
    leave.starts_on = parent.work_date;
    leave.ends_on = parent.work_date;
    leave.status = LeaveStatus::Approved;
    leave.policy.charged_dates = vec![hris_desktop::leave::DayCharge {
        date: parent.work_date,
        units: 1,
    }];
    leave.policy.charged_units = 1;
    s.data.leave.push(leave);
    assert!(capture(&s.data, &parent).is_none());
    s.data.leave.clear();
    let mut open = parent.clone();
    open.checked_out_at = None;
    assert!(capture(&s.data, &open).is_none());
    s.data.employees[0].status = EmployeeStatus::Archived;
    assert!(payload(&OvertimeAction::Submit(d), &s.data).is_err());
}
#[test]
fn fractional_seconds_floor_per_outside_interval_before_totaling_minutes() {
    let (mut s, _) = fixture();
    s.data.breaks.records.clear();
    let mut a = s.data.attendance[0].clone();
    let snap = a.calculation_snapshot.as_mut().unwrap();
    snap.policy.include_pre_shift_overtime = true;
    snap.policy.overtime_min_minutes = 0;
    snap.policy.overtime_round_minutes = 1;
    a.checked_in_at = snap.planned_start.unwrap() - Duration::milliseconds(59_400);
    a.checked_out_at = Some(snap.planned_end.unwrap() + Duration::milliseconds(700));
    assert!(capture(&s.data, &a).is_none());
}
