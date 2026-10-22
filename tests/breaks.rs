use chrono::Utc;
use hris_desktop::{access::Role, breaks::*, corrections::CorrectionDraft, domain::*, shifts::*};
use uuid::Uuid;

fn fixture() -> (DemoStore, Attendance, BreakRule, BreakRule) {
    let mut s = DemoStore::new();
    s.data.attendance.clear();
    s.data.corrections.clear();
    s.data.employees[0].joined_on = Some("2020-01-01".parse().unwrap());
    let person = s.data.employees[0].id;
    let meal = BreakRule {
        id: Uuid::new_v4(),
        name: "Meal".into(),
        minutes: 60,
        paid: false,
        required: true,
    };
    let rest = BreakRule {
        id: Uuid::new_v4(),
        name: "Rest".into(),
        minutes: 15,
        paid: true,
        required: false,
    };
    s.execute(Action::SaveShiftTemplate(ShiftTemplateDraft {
        name: "Break test shift".into(),
        days: vec![1, 2, 3, 4, 5, 6, 7],
        break_rules: vec![meal.clone(), rest.clone()],
        ..Default::default()
    }))
    .unwrap();
    let mut assignment = ShiftAssignmentDraft::new(&s.data, person, "2026-01-05".parse().unwrap());
    assignment.template_id = Some(s.data.shifts.templates[0].id);
    assignment.expected_template_revision = Some(0);
    assignment.reason = "Synthetic rule setup".into();
    s.execute(Action::AssignShift(assignment)).unwrap();
    let mut c = CorrectionDraft::new(person, "2026-01-05".parse().unwrap(), None, &s.data);
    c.checked_in = "08:00".into();
    c.checked_out = "17:00".into();
    c.reason = "Synthetic attendance".into();
    let change = c.change(&s.data, Utc::now()).unwrap();
    s.execute(Action::CorrectAttendance(change)).unwrap();
    let row = s.data.attendance[0].clone();
    (s, row, meal, rest)
}
fn record(
    s: &mut DemoStore,
    a: &Attendance,
    rule: Option<Uuid>,
    start: &str,
    end: &str,
) -> AttendanceBreak {
    let mut d = BreakDraft::new(&s.data, a, None);
    d.rule_id = rule;
    d.started_time = start.into();
    d.ended_time = end.into();
    d.reason = "Synthetic break record".into();
    s.execute(Action::AttendanceBreak(BreakAction::Save(d)))
        .unwrap();
    s.data.breaks.records.last().unwrap().clone()
}
#[test]
fn validates_break_rule_names_identity_minutes_and_combined_duration() {
    let r = BreakRule {
        name: "Meal".into(),
        minutes: 60,
        ..Default::default()
    };
    assert!(validate_rules(&[], 540).is_ok());
    assert!(validate_rules(&[r.clone()], 540).is_ok());
    for mins in [0, 721, 540] {
        let mut bad = r.clone();
        bad.minutes = mins;
        assert!(validate_rules(&[bad], 540).is_err());
    }
    assert!(validate_rules(&[r.clone(), r.clone()], 540).is_err());
    let mut second = r.clone();
    second.id = Uuid::new_v4();
    second.name = "MEAL".into();
    assert!(validate_rules(&[r.clone(), second], 540).is_err());
    let mut bad = r.clone();
    bad.name = "bad\nname".into();
    assert!(validate_rules(&[bad], 540).is_err());
    let many = (0..9)
        .map(|i| BreakRule {
            name: format!("Break {i}"),
            minutes: 1,
            ..Default::default()
        })
        .collect::<Vec<_>>();
    assert!(validate_rules(&many, 540).is_err());
}
#[test]
fn required_breaks_never_automatically_deduct_and_only_actual_unpaid_time_is_excluded() {
    let (mut s, a, meal, rest) = fixture();
    let t = s.data.breaks.totals(a.id);
    assert_eq!(t.time_excluding_unpaid(&a), Some(9 * 3600));
    assert_eq!(rule_status(&s.data, a.id, &meal), "Required · not recorded");
    record(&mut s, &a, Some(meal.id), "12:00", "13:00");
    record(&mut s, &a, Some(rest.id), "15:00", "15:15");
    let t = s.data.breaks.totals(a.id);
    assert_eq!(
        (
            t.paid_seconds,
            t.unpaid_seconds,
            t.time_excluding_unpaid(&a)
        ),
        (900, 3600, Some(8 * 3600))
    );
    assert_eq!(rule_status(&s.data, a.id, &meal), "Recorded");
}
#[test]
fn saved_break_classification_ignores_a_forged_draft_and_survives_template_changes() {
    let (mut s, a, meal, _) = fixture();
    let mut edit = ShiftTemplateDraft::from(&s.data.shifts.templates[0]);
    edit.break_rules[0].paid = true;
    edit.break_rules[0].name = "Revised paid meal".into();
    edit.break_rules[0].minutes = 90;
    s.execute(Action::SaveShiftTemplate(edit)).unwrap();
    let mut d = BreakDraft::new(&s.data, &a, None);
    d.rule_id = Some(meal.id);
    d.name = "Forged classification".into();
    d.paid = true;
    d.started_time = "12:00".into();
    d.ended_time = "12:45".into();
    d.reason = "Synthetic saved rule".into();
    s.execute(Action::AttendanceBreak(BreakAction::Save(d)))
        .unwrap();
    let b = &s.data.breaks.records[0];
    assert_eq!(
        (&b.name, b.paid, b.expected_minutes),
        (&meal.name, false, 60)
    );
    assert_eq!(
        rule_status(&s.data, a.id, &meal),
        "Recorded · shorter than rule"
    );
}
#[test]
fn hr_corrections_and_voids_retain_before_after_history_and_guard_stale_writes() {
    let (mut s, a, meal, _) = fixture();
    let b = record(&mut s, &a, Some(meal.id), "12:00", "13:00");
    let mut d = BreakDraft::new(&s.data, &a, Some(b.clone()));
    d.ended_time = "12:45".into();
    d.reason = "Correct synthetic end time".into();
    s.execute(Action::AttendanceBreak(BreakAction::Save(d.clone())))
        .unwrap();
    assert!(
        s.execute(Action::AttendanceBreak(BreakAction::Save(d)))
            .is_err()
    );
    assert_eq!(s.data.breaks.events.len(), 2);
    assert_eq!(s.data.breaks.events[1].before_record.as_ref().unwrap(), &b);
    s.execute(Action::AttendanceBreak(BreakAction::Void {
        id: b.id,
        revision: 1,
        parent_revision: a.revision,
        reason: "Synthetic duplicate removed".into(),
    }))
    .unwrap();
    assert!(s.data.breaks.records[0].voided);
    assert_eq!(s.data.breaks.events.len(), 3);
    assert_eq!(s.data.breaks.totals(a.id).unpaid_seconds, 0);
    assert!(
        s.execute(Action::AttendanceBreak(BreakAction::Void {
            id: b.id,
            revision: 2,
            parent_revision: a.revision,
            reason: "Repeat".into()
        }))
        .is_err()
    );
    record(&mut s, &a, Some(meal.id), "12:00", "13:00");
    assert_eq!(s.data.breaks.records.len(), 2);
}
#[test]
fn rejects_overlap_duplicate_rule_outside_attendance_future_and_missing_reason() {
    let (mut s, a, meal, rest) = fixture();
    record(&mut s, &a, Some(meal.id), "12:00", "13:00");
    for (rule, start, end) in [
        (Some(rest.id), "12:30", "13:15"),
        (Some(meal.id), "14:00", "15:00"),
        (None, "07:00", "07:30"),
        (None, "16:30", "17:30"),
        (None, "14:00", ""),
    ] {
        let mut d = BreakDraft::new(&s.data, &a, None);
        d.rule_id = rule;
        d.started_time = start.into();
        d.ended_time = end.into();
        d.reason = "Synthetic invalid period".into();
        assert!(BreakAction::Save(d).prepare(&s.data, Utc::now()).is_err());
    }
    let mut d = BreakDraft::new(&s.data, &a, None);
    d.started_time = "14:00".into();
    d.ended_time = "14:15".into();
    assert!(BreakAction::Save(d).prepare(&s.data, Utc::now()).is_err());
    assert_eq!(s.data.breaks.records.len(), 1);
}
#[test]
fn routine_break_is_own_only_server_timed_and_must_end_before_checkout() {
    let (mut s, a, meal, _) = fixture();
    s.data.attendance.clear();
    let person = a.employee_id;
    let mut assigned = ShiftAssignmentDraft::new(&s.data, person, s.data.company.today());
    assigned.template_id = Some(s.data.shifts.templates[0].id);
    assigned.expected_template_revision = Some(0);
    assigned.reason = "Synthetic current schedule".into();
    s.execute(Action::AssignShift(assigned)).unwrap();
    s.execute(Action::CheckIn(person)).unwrap();
    let a = s.data.attendance[0].clone();
    let action = BreakAction::Start {
        attendance_id: a.id,
        parent_revision: a.revision,
        rule_id: meal.id,
    };
    let now = Utc::now();
    s.execute(Action::AttendanceBreak(action.clone())).unwrap();
    let b = s.data.breaks.records[0].clone();
    assert!(b.started_at >= now);
    assert!(s.execute(Action::CheckOut(a.id)).is_err());
    assert!(s.execute(Action::AttendanceBreak(action)).is_err());
    s.execute(Action::AttendanceBreak(BreakAction::End {
        id: b.id,
        revision: 0,
        parent_revision: a.revision,
    }))
    .unwrap();
    s.execute(Action::CheckOut(a.id)).unwrap();
    assert!(s.data.attendance[0].checked_out_at.is_some());
    assert_eq!(s.data.breaks.events.len(), 2);
}
#[test]
fn managers_cannot_change_team_breaks_and_correction_cannot_strand_break_times() {
    let (mut s, a, meal, _) = fixture();
    record(&mut s, &a, Some(meal.id), "12:00", "13:00");
    let mut c = CorrectionDraft::new(a.employee_id, a.work_date, Some(a.clone()), &s.data);
    c.checked_out = "12:30".into();
    c.reason = "Invalid shortened attendance".into();
    assert!(c.change(&s.data, Utc::now()).is_err());
    s.data.access.role = Role::Manager;
    s.data.access.employee_id = Some(s.data.employees[1].id);
    let mut d = BreakDraft::new(&s.data, &a, None);
    d.started_time = "15:00".into();
    d.ended_time = "15:15".into();
    d.reason = "Forbidden team edit".into();
    assert!(
        s.execute(Action::AttendanceBreak(BreakAction::Save(d)))
            .is_err()
    );
    assert!(
        s.execute(Action::AttendanceBreak(BreakAction::Start {
            attendance_id: a.id,
            parent_revision: a.revision,
            rule_id: meal.id
        }))
        .is_err()
    );
}
#[test]
fn legacy_shift_and_attendance_snapshots_decode_with_no_invented_break_rules() {
    let (s, a, _, _) = fixture();
    let mut t = serde_json::to_value(&s.data.shifts.templates[0]).unwrap();
    t.as_object_mut().unwrap().remove("break_rules");
    let old: ShiftTemplate = serde_json::from_value(t).unwrap();
    assert!(old.break_rules.is_empty());
    let mut plan = serde_json::to_value(a.planned_shift.unwrap()).unwrap();
    plan.as_object_mut().unwrap().remove("break_rules");
    let old: ShiftPlan = serde_json::from_value(plan).unwrap();
    assert!(old.break_rules.is_empty());
}
