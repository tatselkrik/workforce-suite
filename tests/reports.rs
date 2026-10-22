use chrono::{Duration, Utc};
use hris_desktop::{
    access::{Access, Role},
    breaks::{BreakAction, BreakDraft},
    calculations::PolicyDraft,
    corrections::CorrectionDraft,
    domain::*,
    overtime::{Draft, OvertimeAction, Status},
    reports::{self, Filter, Kind, Report},
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

fn filter(s: &DemoStore, kind: Kind) -> Filter {
    let mut f = Filter::new("2026-03-04".parse().unwrap());
    f.kind = kind;
    f.starts_on = "2026-03-04".parse().unwrap();
    f.ends_on = f.starts_on;
    f.include_missing = false;
    f.employee_id = Some(s.data.employees[0].id);
    f
}
fn cell<'a>(r: &'a Report, row: usize, title: &str) -> &'a str {
    &r.rows[row][r.headers.iter().position(|h| *h == title).unwrap()]
}

#[test]
fn workforce_trends_match_saved_attendance_totals_and_current_overtime_authorizations() {
    let (mut s, draft) = fixture();
    s.execute(Action::Overtime(OvertimeAction::Submit(draft)))
        .unwrap();
    let request = s.data.overtime.records[0].clone();
    s.data.access.employee_id = Some(s.data.employees[1].id);
    s.data.access.user_id = uuid::Uuid::from_u128(777);
    s.execute(Action::Overtime(OvertimeAction::Resolve {
        id: request.id,
        expected_revision: request.revision,
        status: Status::Approved,
        minutes: Some(45),
        reason: "Synthetic partial approval".into(),
    }))
    .unwrap();
    let summary = reports::build(&s.data, &filter(&s, Kind::Summary), Utc::now()).unwrap();
    let trends = reports::build(&s.data, &filter(&s, Kind::Trends), Utc::now()).unwrap();
    for name in [
        "Completed records",
        "Open records",
        "Late minutes",
        "Early departure minutes",
        "Overtime candidate minutes",
        "Current approved overtime minutes",
    ] {
        assert_eq!(cell(&summary, 0, name), cell(&trends, 0, name));
    }
    assert_eq!(cell(&trends, 0, "Current approved overtime minutes"), "45");
    s.data.attendance[0].checked_out_at =
        Some(s.data.attendance[0].checked_out_at.unwrap() + Duration::minutes(1));
    assert_eq!(
        cell(
            &reports::build(&s.data, &filter(&s, Kind::Trends), Utc::now()).unwrap(),
            0,
            "Current approved overtime minutes"
        ),
        "0"
    );
}

#[test]
fn reports_reject_portal_signed_out_cross_company_and_outside_manager_scope() {
    let (mut s, _) = fixture();
    let mut f = filter(&s, Kind::Attendance);
    let target = s.data.employees[0].id;
    s.data.access = Access {
        company_id: s.data.company.id,
        user_id: uuid::Uuid::from_u128(779),
        employee_id: Some(s.data.employees[1].id),
        role: Role::Manager,
        direct_reports: vec![target],
        ..Default::default()
    };
    let mut outside = s.data.attendance[0].clone();
    outside.id = uuid::Uuid::new_v4();
    outside.employee_id = s.data.employees[2].id;
    s.data.attendance.push(outside);
    f.employee_id = None;
    let r = reports::build(&s.data, &f, Utc::now()).unwrap();
    assert_eq!(r.rows.len(), 1);
    assert_eq!(cell(&r, 0, "Full name"), s.data.employees[0].full_name);
    f.employee_id = Some(s.data.employees[2].id);
    assert!(reports::build(&s.data, &f, Utc::now()).is_err());
    f.employee_id = Some(target);
    s.data.access.role = Role::Employee;
    assert!(reports::build(&s.data, &f, Utc::now()).is_err());
    s.data.access.role = Role::Administrator;
    s.data.mode = Mode::SignedOut;
    assert!(reports::build(&s.data, &f, Utc::now()).is_err());
    s.data.mode = Mode::Demo;
    s.data.access.company_id = uuid::Uuid::new_v4();
    assert!(reports::build(&s.data, &f, Utc::now()).is_err());
}

#[test]
fn completed_report_separates_actual_minutes_candidates_partial_and_historical_approval() {
    let (mut s, d) = fixture();
    let f = filter(&s, Kind::Attendance);
    s.execute(Action::Overtime(OvertimeAction::Submit(d)))
        .unwrap();
    let id = s.data.overtime.records[0].id;
    let target = s.data.employees[0].id;
    s.data.access.employee_id = Some(s.data.employees[1].id);
    s.data.access.user_id = uuid::Uuid::from_u128(777);
    s.execute(Action::Overtime(OvertimeAction::Resolve {
        id,
        expected_revision: 0,
        status: Status::Approved,
        minutes: Some(45),
        reason: "Synthetic partial review".into(),
    }))
    .unwrap();
    let r = reports::build(&s.data, &f, Utc::now()).unwrap();
    for (title, value) in [
        ("Gross minutes", "630"),
        ("Completed unpaid break minutes", "75"),
        ("Minutes excluding unpaid breaks", "555"),
        ("Late minutes", "17"),
        ("Early departure minutes", "0"),
        ("Overtime candidate minutes", "90"),
        ("Current approved overtime minutes", "45"),
        ("Rule version (saved)", "1"),
    ] {
        assert_eq!(cell(&r, 0, title), value, "{title}");
    }
    assert!(cell(&r, 0, "Check-in (local)").ends_with("+08:00"));
    let a = s.data.attendance[0].clone();
    let mut c = CorrectionDraft::new(target, a.work_date, Some(a), &s.data);
    c.checked_out = "18:52".into();
    c.reason = "Synthetic corrected checkout".into();
    s.execute(Action::CorrectAttendance(
        c.change(&s.data, Utc::now()).unwrap(),
    ))
    .unwrap();
    let r = reports::build(&s.data, &f, Utc::now()).unwrap();
    assert_eq!(cell(&r, 0, "Current approved overtime minutes"), "0");
    assert_eq!(s.data.overtime.records[0].approved_minutes, Some(45));
}

#[test]
fn missing_days_do_not_infer_preemployment_archived_absence_or_future_lateness() {
    let (mut s, _) = fixture();
    let mut f = filter(&s, Kind::Attendance);
    f.starts_on = "2026-03-03".parse().unwrap();
    f.ends_on = "2026-03-05".parse().unwrap();
    f.include_missing = true;
    s.data.employees[0].joined_on = Some("2026-03-04".parse().unwrap());
    let r = reports::build(&s.data, &f, Utc::now()).unwrap();
    assert_eq!(r.rows.len(), 2);
    assert_eq!(cell(&r, 1, "Record status"), "Scheduled / no check-in");
    assert_eq!(cell(&r, 1, "Late minutes"), "");
    s.data.employees[0].status = EmployeeStatus::Archived;
    let r = reports::build(&s.data, &f, Utc::now()).unwrap();
    assert_eq!(r.rows.len(), 1);
    assert_eq!(cell(&r, 0, "Record status"), "Completed");
    f.include_archived = false;
    assert!(
        reports::build(&s.data, &f, Utc::now())
            .unwrap()
            .rows
            .is_empty()
    );
    s.data.employees[0].status = EmployeeStatus::Active;
    f.starts_on = s.data.company.today() + Duration::days(2);
    f.ends_on = f.starts_on;
    let r = reports::build(&s.data, &f, Utc::now()).unwrap();
    assert_eq!(cell(&r, 0, "Record status"), "Not yet due / no check-in");
}

#[test]
fn leave_reports_clip_saved_half_day_charges_and_keep_cancelled_history_separate() {
    use hris_desktop::leave::{DayCharge, DayPart, RequestPolicy};
    let (mut s, _) = fixture();
    let employee = s.data.employees[0].id;
    let day = "2026-03-05".parse().unwrap();
    let request = LeaveRequest {
        id: uuid::Uuid::new_v4(),
        company_id: s.data.company.id,
        employee_id: employee,
        kind: LeaveKind::Custom,
        starts_on: "2026-03-04".parse().unwrap(),
        ends_on: day,
        reason: "PRIVATE REASON MUST NOT EXPORT".into(),
        status: LeaveStatus::Approved,
        policy: RequestPolicy {
            type_code: "filed_type".into(),
            type_name: "Saved study leave".into(),
            day_part: DayPart::Am,
            charged_units: 3,
            charged_dates: vec![
                DayCharge {
                    date: "2026-03-04".parse().unwrap(),
                    units: 2,
                },
                DayCharge {
                    date: day,
                    units: 1,
                },
            ],
            policy_revision: 9,
            ..Default::default()
        },
    };
    s.data.leave = vec![request.clone()];
    let mut cancelled = request;
    cancelled.id = uuid::Uuid::new_v4();
    cancelled.status = LeaveStatus::Cancelled;
    s.data.leave.push(cancelled);
    let mut f = filter(&s, Kind::Leave);
    f.starts_on = day;
    f.ends_on = day;
    let r = reports::build(&s.data, &f, Utc::now()).unwrap();
    assert_eq!(r.rows.len(), 2);
    assert!(r.rows.iter().all(|row| {
        row[r
            .headers
            .iter()
            .position(|h| *h == "Filed charge on date (days)")
            .unwrap()]
            == "0.5"
    }));
    assert_eq!(cell(&r, 0, "Leave type (filed)"), "Saved study leave");
    assert_eq!(cell(&r, 0, "Filed total (days)"), "1.5");
    assert_eq!(cell(&r, 0, "Filed policy version"), "9");
    let csv = String::from_utf8(r.csv().unwrap()).unwrap();
    assert!(!csv.contains("PRIVATE REASON"));
    assert!(!csv.contains("synthetic.employee@example.invalid"));
    f.kind = Kind::Summary;
    let r = reports::build(&s.data, &f, Utc::now()).unwrap();
    assert_eq!(cell(&r, 0, "Approved leave days"), "0.5");
    assert_eq!(cell(&r, 0, "Pending leave days"), "0");
    f.kind = Kind::Attendance;
    f.include_missing = true;
    let r = reports::build(&s.data, &f, Utc::now()).unwrap();
    assert_eq!(cell(&r, 0, "Record status"), "Half-day leave / no check-in");
    f.kind = Kind::Leave;
    f.leave_status = Some(LeaveStatus::Cancelled);
    assert_eq!(
        reports::build(&s.data, &f, Utc::now()).unwrap().rows.len(),
        1
    );
}

#[test]
fn incomplete_records_have_blank_final_values_and_summary_review_counts() {
    let (mut s, _) = fixture();
    s.data.attendance[0].checked_out_at = None;
    s.data.breaks.records.clear();
    let f = filter(&s, Kind::Attendance);
    let r = reports::build(&s.data, &f, Utc::now()).unwrap();
    for title in [
        "Gross minutes",
        "Minutes excluding unpaid breaks",
        "Early departure minutes",
        "Overtime candidate minutes",
    ] {
        assert_eq!(cell(&r, 0, title), "");
    }
    let mut f = f;
    f.kind = Kind::Summary;
    let r = reports::build(&s.data, &f, Utc::now()).unwrap();
    assert_eq!(cell(&r, 0, "Open records"), "1");
    assert_eq!(cell(&r, 0, "Records without completed net totals"), "1");
    assert_eq!(cell(&r, 0, "Records needing calculation review"), "1");
    s.data.attendance[0].calculation_snapshot = None;
    let r = reports::build(&s.data, &f, Utc::now()).unwrap();
    assert_eq!(cell(&r, 0, "Records needing calculation review"), "1");
}

#[test]
fn report_ranges_duplicates_and_employee_day_limits_fail_explicitly() {
    let (mut s, _) = fixture();
    let mut f = filter(&s, Kind::Summary);
    f.starts_on = f.ends_on + Duration::days(1);
    assert!(reports::build(&s.data, &f, Utc::now()).is_err());
    f.ends_on = f.starts_on + Duration::days(366);
    assert!(reports::build(&s.data, &f, Utc::now()).is_err());
    f.ends_on = f.starts_on + Duration::days(365);
    f.employee_id = None;
    let person = s.data.employees[0].clone();
    s.data.employees = (0..138)
        .map(|i| {
            let mut e = person.clone();
            e.id = uuid::Uuid::from_u128(10000 + i);
            e
        })
        .collect();
    assert!(
        reports::build(&s.data, &f, Utc::now())
            .unwrap_err()
            .to_string()
            .contains("50,000 employee-days")
    );
    s.data.employees = vec![person];
    f = filter(&s, Kind::Attendance);
    s.data.attendance.push(s.data.attendance[0].clone());
    assert!(
        reports::build(&s.data, &f, Utc::now())
            .unwrap_err()
            .to_string()
            .contains("Duplicate")
    );
}

#[test]
fn csv_handles_unicode_quotes_newlines_and_spreadsheet_formula_prefixes() {
    let (mut s, _) = fixture();
    let f = filter(&s, Kind::Attendance);
    s.data.company.name = "=COMPANY()".into();
    s.data.employees[0].full_name = "  =SUM(1,2) \"José\"\r\nline".into();
    s.data.employees[0].department = "\t@formula".into();
    let r = reports::build(&s.data, &f, Utc::now()).unwrap();
    let csv = String::from_utf8(r.csv().unwrap()).unwrap();
    assert!(csv.starts_with('\u{feff}'));
    assert!(csv.contains("\"'=COMPANY()\""));
    assert!(csv.contains("\"'  =SUM(1,2) \"\"José\"\"\r\nline\""));
    assert!(csv.contains("\"'\t@formula\""));
    assert!(csv.ends_with("\r\n"));
}

#[test]
fn csv_save_preserves_existing_files_and_refuses_wrong_extensions_and_null_cells() {
    let (s, _) = fixture();
    let f = filter(&s, Kind::Summary);
    let mut r = reports::build(&s.data, &f, Utc::now()).unwrap();
    let folder = std::env::temp_dir().join(format!("hris-report-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&folder).unwrap();
    let path = folder.join("report.csv");
    r.save(&path).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(bytes, r.csv().unwrap());
    r.rows.clear();
    assert!(r.save(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert!(r.save(&folder.join("report.exe")).is_err());
    r.company = "bad\0field".into();
    assert!(r.save(&folder.join("bad.csv")).is_err());
    assert!(!folder.join("bad.csv").exists());
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(folder).unwrap();
}

#[test]
fn export_scope_comparison_ignores_order_but_detects_revoked_team_and_role() {
    let (s, _) = fixture();
    let mut a = s.data.access.clone();
    a.direct_reports = vec![uuid::Uuid::from_u128(1), uuid::Uuid::from_u128(2)];
    let mut b = a.clone();
    b.direct_reports.reverse();
    assert!(reports::same_scope(&a, &b));
    b.direct_reports.pop();
    assert!(!reports::same_scope(&a, &b));
    b = a.clone();
    b.role = Role::Employee;
    assert!(!reports::same_scope(&a, &b));
}
