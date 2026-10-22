use chrono::{NaiveDate, TimeZone, Utc};
use hris_desktop::{access::Role, attendance::daily_roster, domain::*, shifts::*};

fn date(s: &str) -> NaiveDate {
    s.parse().unwrap()
}
fn setup() -> (DemoStore, uuid::Uuid, uuid::Uuid) {
    let mut s = DemoStore::new();
    s.data.attendance.clear();
    s.data.leave.clear();
    let employee = s.data.employees[0].id;
    let d = ShiftTemplateDraft {
        name: "Night".into(),
        start: "22:00".into(),
        end: "06:00".into(),
        days: vec![1, 2, 3, 4, 5, 6, 7],
        ..Default::default()
    };
    s.execute(Action::SaveShiftTemplate(d)).unwrap();
    let shift = s.data.shifts.templates[0].id;
    (s, employee, shift)
}
fn assign(
    s: &mut DemoStore,
    e: uuid::Uuid,
    t: Option<uuid::Uuid>,
    on: &str,
) -> ShiftAssignmentDraft {
    let mut d = ShiftAssignmentDraft::new(&s.data, e, date(on));
    d.template_id = t;
    d.expected_template_revision = t.and_then(|id| {
        s.data
            .shifts
            .templates
            .iter()
            .find(|t| t.id == id)
            .map(|t| t.revision)
    });
    d.reason = "Synthetic assignment".into();
    s.execute(Action::AssignShift(d.clone())).unwrap();
    d
}
#[test]
fn validates_clock_days_names_and_hr_permissions() {
    let (s, _, _) = setup();
    let mut d = ShiftTemplateDraft {
        name: "Day".into(),
        ..Default::default()
    };
    for time in ["24:00", "08:60", "8:00", "08:00:00", "０８:００", ""] {
        d.start = time.into();
        assert!(d.template(&s.data).is_err(), "{time}");
    }
    d.start = "08:00".into();
    d.end = "08:00".into();
    assert!(d.template(&s.data).is_err());
    d.end = "17:00".into();
    for days in [vec![], vec![1, 1], vec![0], vec![8]] {
        d.days = days;
        assert!(d.template(&s.data).is_err());
    }
    d.days = vec![5, 1];
    assert_eq!(d.template(&s.data).unwrap().days, vec![1, 5]);
    let mut data = s.data.clone();
    for role in [Role::Manager, Role::Employee] {
        data.access.role = role;
        assert!(d.template(&data).is_err());
        assert!(
            data.access
                .authorize(&Action::SaveShiftTemplate(d.clone()), &data)
                .is_err()
        );
    }
    data.access.role = Role::HrGeneralist;
    assert!(d.template(&data).is_ok());
}
#[test]
fn assignment_snapshots_and_template_versions_survive_rename_deactivation_and_revisions() {
    let (mut s, e, t) = setup();
    let old = assign(&mut s, e, Some(t), "2027-01-04");
    let plan = s.data.shifts.plan(e, date("2027-01-04")).unwrap().clone();
    let mut edit = ShiftTemplateDraft::from(&s.data.shifts.templates[0]);
    edit.name = "Revised night".into();
    edit.start = "23:00".into();
    edit.active = false;
    s.execute(Action::SaveShiftTemplate(edit.clone())).unwrap();
    assert!(s.execute(Action::SaveShiftTemplate(edit)).is_err());
    assert_eq!(s.data.shifts.plan(e, date("2027-01-04")), Some(&plan));
    assert_eq!(s.data.shifts.versions.len(), 2);
    assert!(s.execute(Action::AssignShift(old)).is_err());
    let mut next = ShiftAssignmentDraft::new(&s.data, e, date("2027-02-01"));
    next.template_id = Some(t);
    next.reason = "Change".into();
    assert!(next.assignment(&s.data).is_err());
}
#[test]
fn dated_timeline_replaces_same_date_keeps_audit_and_returns_to_calendar() {
    let (mut s, e, t) = setup();
    assign(&mut s, e, Some(t), "2027-01-04");
    assign(&mut s, e, None, "2027-02-01");
    assert!(s.data.shifts.plan(e, date("2027-01-03")).is_none());
    assert!(s.data.shifts.plan(e, date("2027-01-31")).is_some());
    assert!(s.data.shifts.plan(e, date("2027-02-01")).is_none());
    assign(&mut s, e, Some(t), "2027-02-01");
    assert_eq!(s.data.shifts.assignments.len(), 3);
    assert_eq!(
        s.data
            .shifts
            .assignment(e, date("2027-02-01"))
            .unwrap()
            .sequence,
        3
    );
    assert_eq!(s.data.shifts.assignments[1].plan, None);
}
#[test]
fn rejects_overnight_transition_overlap_and_stale_template_or_timezone() {
    let (mut s, e, t) = setup();
    assign(&mut s, e, Some(t), "2027-01-04");
    s.execute(Action::SaveShiftTemplate(ShiftTemplateDraft {
        name: "Early".into(),
        start: "04:00".into(),
        end: "12:00".into(),
        days: vec![1, 2, 3, 4, 5, 6, 7],
        ..Default::default()
    }))
    .unwrap();
    let early = s
        .data
        .shifts
        .templates
        .iter()
        .find(|t| t.name == "Early")
        .unwrap()
        .id;
    let mut d = ShiftAssignmentDraft::new(&s.data, e, date("2027-01-05"));
    d.template_id = Some(early);
    d.expected_template_revision = Some(0);
    d.reason = "Overlap".into();
    assert!(d.assignment(&s.data).is_err());
    d.template_id = Some(t);
    d.expected_template_revision = Some(99);
    assert!(d.assignment(&s.data).is_err());
    d.expected_template_revision = Some(0);
    d.expected_timezone = "UTC".into();
    assert!(d.assignment(&s.data).is_err());
}
#[test]
fn night_workdate_uses_previous_start_date_and_holidays_respect_saved_rule() {
    let (mut s, e, t) = setup();
    assign(&mut s, e, Some(t), "2027-01-04");
    let at = Utc.with_ymd_and_hms(2027, 1, 4, 17, 0, 0).unwrap(); // Manila Jan 5, 01:00
    assert_eq!(work_date_at(&s.data, e, at), date("2027-01-04"));
    let off = Utc.with_ymd_and_hms(2027, 1, 4, 22, 0, 0).unwrap();
    assert_eq!(work_date_at(&s.data, e, off), date("2027-01-05"));
    s.data
        .leave_setup
        .policy
        .holidays
        .push(hris_desktop::leave::Holiday {
            date: date("2027-01-04"),
            name: "Synthetic holiday".into(),
        });
    assert_eq!(work_date_at(&s.data, e, at), date("2027-01-05"));
    let p = s.data.shifts.plan(e, date("2027-01-04")).unwrap();
    assert!(p.contains_clock(date("2027-01-04"), at));
}
#[test]
fn roster_uses_individual_days_and_does_not_rewrite_leave_or_existing_punches() {
    let (mut s, e, t) = setup();
    let original_leave = s.data.leave.clone();
    assign(&mut s, e, Some(t), "2027-01-04");
    let sunday = daily_roster(&s.data, date("2027-01-10"));
    assert!(
        sunday
            .iter()
            .find(|r| r.employee.id == e)
            .unwrap()
            .scheduled_workday
    );
    let r = sunday.iter().find(|r| r.employee.id != e).unwrap();
    assert!(!r.scheduled_workday);
    assert_eq!(
        serde_json::to_value(original_leave).unwrap(),
        serde_json::to_value(&s.data.leave).unwrap()
    );
    let at = Utc.with_ymd_and_hms(2027, 1, 4, 15, 0, 0).unwrap();
    let plan = s.data.shifts.plan(e, date("2027-01-04")).cloned();
    s.data.attendance.push(Attendance {
        calculation_snapshot: None,
        planned_shift: plan.clone(),
        id: uuid::Uuid::new_v4(),
        company_id: s.data.company.id,
        employee_id: e,
        work_date: date("2027-01-04"),
        checked_in_at: at,
        checked_out_at: Some(at + chrono::Duration::hours(8)),
        revision: 0,
    });
    assign(&mut s, e, None, "2027-01-04");
    assert_eq!(
        daily_roster(&s.data, date("2027-01-04"))
            .iter()
            .find(|r| r.employee.id == e)
            .unwrap()
            .planned_shift,
        plan
    );
}
#[test]
fn correction_can_retain_after_midnight_check_in_and_rejects_out_of_shift_date() {
    let (mut s, e, t) = setup();
    assign(&mut s, e, Some(t), "2026-10-20");
    let now = Utc.with_ymd_and_hms(2026, 10, 22, 0, 0, 0).unwrap();
    let mut draft =
        hris_desktop::corrections::CorrectionDraft::new(e, date("2026-10-20"), None, &s.data);
    draft.checked_in_date = "2026-10-21".into();
    draft.checked_in = "01:00".into();
    draft.checked_out_date = "2026-10-21".into();
    draft.checked_out = "06:00".into();
    draft.reason = "Missed night check-in".into();
    assert!(draft.change(&s.data, now).is_ok());
    draft.checked_in = "09:00".into();
    assert!(draft.change(&s.data, now).is_err());
}
#[test]
fn forbids_assignment_before_hire_archived_employee_and_blank_reason() {
    let (mut s, e, t) = setup();
    s.data
        .employees
        .iter_mut()
        .find(|p| p.id == e)
        .unwrap()
        .joined_on = Some(date("2027-01-10"));
    let mut d = ShiftAssignmentDraft::new(&s.data, e, date("2027-01-09"));
    d.template_id = Some(t);
    d.expected_template_revision = Some(0);
    d.reason = "Setup".into();
    assert!(d.assignment(&s.data).is_err());
    d.effective_on = "2027-01-10".into();
    d.reason.clear();
    assert!(d.assignment(&s.data).is_err());
    d.reason = "Setup".into();
    assert!(d.assignment(&s.data).is_ok());
    s.data
        .employees
        .iter_mut()
        .find(|p| p.id == e)
        .unwrap()
        .status = EmployeeStatus::Archived;
    assert!(d.assignment(&s.data).is_err());
}
