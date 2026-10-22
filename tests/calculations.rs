use chrono::{TimeZone, Utc};
use hris_desktop::{
    access::Role,
    breaks::{BreakAction, BreakDraft},
    calculations::*,
    corrections::CorrectionDraft,
    domain::*,
    shifts::*,
};
fn fixture(start: &str, end: &str) -> (DemoStore, Attendance) {
    let mut store = DemoStore::new();
    store.data.attendance.clear();
    store.data.corrections.clear();
    store.data.leave.clear();
    store.data.employees[0].joined_on = Some("2020-01-01".parse().unwrap());
    let mut policy = PolicyDraft::new(&store.data.calculations.policy);
    policy.policy.late_grace_minutes = 5;
    policy.policy.undertime_grace_minutes = 10;
    policy.policy.overtime_min_minutes = 30;
    policy.policy.overtime_round_minutes = 15;
    policy.reason = "Synthetic calculation rules".into();
    store
        .execute(Action::SaveCalculationPolicy(policy))
        .unwrap();
    store
        .execute(Action::SaveShiftTemplate(ShiftTemplateDraft {
            name: "Synthetic day calculation".into(),
            days: vec![1, 2, 3, 4, 5, 6, 7],
            works_on_holidays: true,
            ..Default::default()
        }))
        .unwrap();
    let person = store.data.employees[0].id;
    let date = "2026-01-06".parse().unwrap();
    let mut a = ShiftAssignmentDraft::new(&store.data, person, date);
    a.template_id = Some(store.data.shifts.templates[0].id);
    a.expected_template_revision = Some(0);
    a.reason = "Synthetic calculation schedule".into();
    store.execute(Action::AssignShift(a)).unwrap();
    let mut c = CorrectionDraft::new(person, date, None, &store.data);
    c.checked_in = start.into();
    c.checked_out = end.into();
    c.reason = "Synthetic calculation attendance".into();
    store
        .execute(Action::CorrectAttendance(
            c.change(&store.data, Utc::now()).unwrap(),
        ))
        .unwrap();
    let row = store.data.attendance[0].clone();
    (store, row)
}
fn record(s: &mut DemoStore, a: &Attendance, start: &str, end: &str, paid: bool) {
    let mut b = BreakDraft::new(&s.data, a, None);
    b.started_time = start.into();
    b.ended_time = end.into();
    b.paid = paid;
    b.reason = "Synthetic actual break".into();
    s.execute(Action::AttendanceBreak(BreakAction::Save(b)))
        .unwrap();
}
fn apply(s: &mut DemoStore, a: &Attendance) {
    s.execute(Action::ApplyCalculationPolicy(ApplyPolicy {
        attendance_id: a.id,
        expected_revision: a.revision,
        expected_policy_revision: s.data.calculations.policy.revision,
        reason: "Synthetic explicit policy application".into(),
    }))
    .unwrap();
}
#[test]
fn whole_minutes_subtract_grace_and_separate_early_departure() {
    let (s, a) = fixture("08:17", "16:47");
    let c = calculate(&s.data, &a);
    assert_eq!(c.status, CalculationStatus::Ready);
    assert_eq!(
        (c.late_seconds, c.undertime_seconds, c.overtime_minutes),
        (Some(12 * 60), Some(3 * 60), Some(0))
    );
    let (s, a) = fixture("08:05:59", "16:49:01");
    let c = calculate(&s.data, &a);
    assert_eq!((c.late_seconds, c.undertime_seconds), (Some(0), Some(0)));
}
#[test]
fn minimum_and_rounding_use_only_actual_outside_unpaid_time() {
    let (mut s, a) = fixture("08:00", "18:07");
    record(&mut s, &a, "17:15", "17:30", false);
    record(&mut s, &a, "17:30", "17:45", true);
    let c = calculate(&s.data, &a);
    assert_eq!(
        (c.raw_overtime_seconds, c.overtime_minutes, c.net_seconds),
        (Some(52 * 60), Some(45), Some(592 * 60))
    );
    let (s, a) = fixture("08:00", "17:29:59");
    assert_eq!(calculate(&s.data, &a).overtime_minutes, Some(0));
    let (s, a) = fixture("08:00", "17:30");
    assert_eq!(calculate(&s.data, &a).overtime_minutes, Some(30));
}
#[test]
fn optional_before_shift_counts_actual_intersections_without_double_deduction() {
    let (mut s, a) = fixture("07:30", "18:00");
    record(&mut s, &a, "07:40", "07:50", false);
    record(&mut s, &a, "17:30", "17:40", false);
    assert_eq!(calculate(&s.data, &a).overtime_minutes, Some(45));
    let mut p = PolicyDraft::new(&s.data.calculations.policy);
    p.policy.include_pre_shift_overtime = true;
    p.reason = "Synthetic pre-shift candidate policy".into();
    s.execute(Action::SaveCalculationPolicy(p)).unwrap();
    assert_eq!(calculate(&s.data, &a).overtime_minutes, Some(45));
    apply(&mut s, &a);
    let c = calculate(&s.data, &s.data.attendance[0]);
    assert_eq!(
        (c.raw_overtime_seconds, c.overtime_minutes),
        (Some(70 * 60), Some(60))
    );
}
#[test]
fn records_entirely_outside_shift_do_not_invent_unworked_overtime() {
    let (s, a) = fixture("18:00", "19:00");
    let c = calculate(&s.data, &a);
    assert_eq!(
        (c.late_seconds, c.undertime_seconds, c.overtime_minutes),
        (Some(535 * 60), Some(0), Some(60))
    );
    let (s, a) = fixture("06:00", "07:00");
    let c = calculate(&s.data, &a);
    assert_eq!(
        (c.late_seconds, c.undertime_seconds, c.overtime_minutes),
        (Some(0), Some(530 * 60), Some(0))
    );
}
#[test]
fn saved_rules_and_calendar_survive_changes_and_explicit_application_is_audited() {
    let (mut s, a) = fixture("08:17", "18:00");
    let original = a.calculation_snapshot.clone();
    let mut p = PolicyDraft::new(&s.data.calculations.policy);
    p.policy.late_grace_minutes = 20;
    p.reason = "Synthetic changed grace".into();
    s.execute(Action::SaveCalculationPolicy(p.clone())).unwrap();
    assert!(s.execute(Action::SaveCalculationPolicy(p)).is_err());
    assert_eq!(s.data.attendance[0].calculation_snapshot, original);
    assert_eq!(calculate(&s.data, &a).late_seconds, Some(12 * 60));
    apply(&mut s, &a);
    let revised = s.data.attendance[0].clone();
    assert_eq!(revised.revision, 1);
    assert_eq!(calculate(&s.data, &revised).late_seconds, Some(0));
    assert_eq!(s.data.calculations.attendance_events.len(), 1);
    assert!(
        s.data.calculations.attendance_events[0]
            .before_snapshot
            .is_some()
    );
    assert!(
        s.execute(Action::ApplyCalculationPolicy(ApplyPolicy {
            attendance_id: a.id,
            expected_revision: 0,
            expected_policy_revision: 2,
            reason: "Stale".into()
        }))
        .is_err()
    );
    let mut c = CorrectionDraft::new(a.employee_id, a.work_date, Some(revised.clone()), &s.data);
    c.checked_out = "18:15".into();
    c.reason = "Synthetic later checkout".into();
    s.execute(Action::CorrectAttendance(
        c.change(&s.data, Utc::now()).unwrap(),
    ))
    .unwrap();
    assert_eq!(
        s.data.attendance[0].calculation_snapshot,
        revised.calculation_snapshot
    );
}
#[test]
fn no_shift_no_policy_off_day_and_approved_leave_do_not_infer_penalties() {
    let (s, mut a) = fixture("09:00", "16:00");
    a.calculation_snapshot = None;
    assert_eq!(
        calculate(&s.data, &a).status,
        CalculationStatus::MissingPolicy
    );
    a.planned_shift = None;
    a.calculation_snapshot = Some(CalculationSnapshot::capture(&s.data, &a));
    assert_eq!(
        calculate(&s.data, &a).status,
        CalculationStatus::MissingShift
    );
    let (mut s, mut a) = fixture("09:00", "16:00");
    a.calculation_snapshot.as_mut().unwrap().scheduled_workday = false;
    let c = calculate(&s.data, &a);
    assert_eq!(
        (c.status, c.late_seconds, c.overtime_minutes),
        (CalculationStatus::Unscheduled, None, None)
    );
    a.calculation_snapshot.as_mut().unwrap().scheduled_workday = true;
    let mut leave = LeaveDraft {
        employee_id: Some(a.employee_id),
        starts_on: a.work_date.to_string(),
        ends_on: a.work_date.to_string(),
        reason: "Synthetic approved leave".into(),
        ..Default::default()
    };
    leave.kind = LeaveKind::Annual;
    s.execute(Action::RequestLeave(leave)).unwrap();
    s.data.leave[0].status = LeaveStatus::Approved;
    let c = calculate(&s.data, &a);
    assert_eq!(
        (
            c.status,
            c.late_seconds,
            c.undertime_seconds,
            c.overtime_minutes
        ),
        (CalculationStatus::LeaveReview, None, None, None)
    );
    s.data.leave[0].status = LeaveStatus::Cancelled;
    assert_eq!(calculate(&s.data, &a).status, CalculationStatus::Ready);
}
#[test]
fn open_attendance_has_arrival_only_and_never_a_finished_total() {
    let (s, mut a) = fixture("08:17", "18:00");
    a.checked_out_at = None;
    let c = calculate(&s.data, &a);
    assert_eq!(
        (
            c.status,
            c.late_seconds,
            c.undertime_seconds,
            c.overtime_minutes,
            c.net_seconds
        ),
        (CalculationStatus::InProgress, Some(720), None, None, None)
    );
}
#[test]
fn overnight_and_daylight_saving_use_saved_utc_boundaries() {
    let (mut s, mut a) = fixture("08:00", "17:00");
    let p = a.planned_shift.as_mut().unwrap();
    p.start_minute = 1320;
    p.end_minute = 360;
    a.checked_in_at = Utc.with_ymd_and_hms(2026, 1, 6, 14, 16, 0).unwrap();
    a.checked_out_at = Some(Utc.with_ymd_and_hms(2026, 1, 6, 23, 12, 0).unwrap());
    a.calculation_snapshot = Some(CalculationSnapshot::capture(&s.data, &a));
    let c = calculate(&s.data, &a);
    assert_eq!(
        (c.late_seconds, c.overtime_minutes),
        (Some(11 * 60), Some(60))
    );
    a.work_date = "2026-03-08".parse().unwrap();
    let p = a.planned_shift.as_mut().unwrap();
    p.timezone = "America/New_York".into();
    p.start_minute = 90;
    p.end_minute = 210;
    a.checked_in_at = Utc.with_ymd_and_hms(2026, 3, 8, 6, 35, 0).unwrap();
    a.checked_out_at = Some(Utc.with_ymd_and_hms(2026, 3, 8, 7, 45, 0).unwrap());
    a.calculation_snapshot = Some(CalculationSnapshot::capture(&s.data, &a));
    let snap = a.calculation_snapshot.as_ref().unwrap();
    assert_eq!(
        (snap.planned_end.unwrap() - snap.planned_start.unwrap()).num_minutes(),
        60
    );
    assert_eq!(calculate(&s.data, &a).late_seconds, Some(0));
    a.planned_shift.as_mut().unwrap().start_minute = 150;
    a.calculation_snapshot = Some(CalculationSnapshot::capture(&s.data, &a));
    assert_eq!(
        calculate(&s.data, &a).status,
        CalculationStatus::BoundaryReview
    );
    s.data.company.timezone = "UTC".into();
    assert_eq!(
        a.calculation_snapshot.as_ref().unwrap().timezone,
        "America/New_York"
    );
}
#[test]
fn roles_validation_and_legacy_snapshots_are_protected() {
    let (mut s, a) = fixture("08:17", "18:00");
    let mut p = PolicyDraft::new(&s.data.calculations.policy);
    p.policy.late_grace_minutes = 121;
    p.reason = "Synthetic invalid".into();
    assert!(p.payload(&s.data).is_err());
    p.policy.late_grace_minutes = 0;
    p.policy.overtime_round_minutes = 7;
    assert!(p.payload(&s.data).is_err());
    p.policy.overtime_round_minutes = 15;
    s.data.access.role = Role::HrGeneralist;
    assert!(p.payload(&s.data).is_err());
    s.data.access.role = Role::Manager;
    assert!(
        ApplyPolicy {
            attendance_id: a.id,
            expected_revision: 0,
            expected_policy_revision: 1,
            reason: "Forbidden".into()
        }
        .payload(&s.data)
        .is_err()
    );
    let mut json = serde_json::to_value(&a).unwrap();
    json.as_object_mut().unwrap().remove("calculation_snapshot");
    let old: Attendance = serde_json::from_value(json).unwrap();
    assert!(old.calculation_snapshot.is_none());
}
