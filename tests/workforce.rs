use chrono::{Duration, TimeZone, Utc};
use hris_desktop::{
    access::Role,
    domain::*,
    leave::{DayCharge, RequestPolicy},
    reports::{self, Filter, Kind},
    workforce::{Analysis, Granularity, GroupBy},
};
use uuid::Uuid;
fn date(s: &str) -> chrono::NaiveDate {
    s.parse().unwrap()
}
fn seed() -> DemoStore {
    let mut s = DemoStore::new();
    s.data.attendance.clear();
    s.data.leave.clear();
    s.data.employees.truncate(4);
    for (i, e) in s.data.employees.iter_mut().enumerate() {
        e.department = if i < 2 {
            "Synthetic HR"
        } else {
            "Synthetic Operations"
        }
        .into();
        e.status = if i == 3 {
            EmployeeStatus::Archived
        } else {
            EmployeeStatus::Active
        };
    }
    s
}
fn f(kind: Kind) -> Filter {
    let mut f = Filter::new(date("2024-03-02"));
    f.kind = kind;
    f.starts_on = date("2024-02-28");
    f.ends_on = date("2024-03-02");
    f
}
fn cell<'a>(r: &'a reports::Report, row: usize, name: &str) -> &'a str {
    &r.rows[row][r.headers.iter().position(|h| *h == name).unwrap()]
}
fn leave(s: &DemoStore, status: LeaveStatus, day: &str, units: i32) -> LeaveRequest {
    LeaveRequest {
        id: Uuid::new_v4(),
        company_id: s.data.company.id,
        employee_id: s.data.employees[0].id,
        kind: LeaveKind::Annual,
        starts_on: date(day),
        ends_on: date(day),
        reason: "Synthetic private reason excluded from analysis".into(),
        status,
        policy: RequestPolicy {
            charged_units: units,
            charged_dates: vec![DayCharge {
                date: date(day),
                units,
            }],
            ..Default::default()
        },
    }
}
#[test]
fn current_headcount_groups_active_archived_unset_and_company_scope() {
    let mut s = seed();
    s.data.employees[2].department.clear();
    let mut foreign = s.data.employees[0].clone();
    foreign.id = Uuid::new_v4();
    foreign.company_id = Uuid::new_v4();
    foreign.department = "Foreign company".into();
    s.data.employees.push(foreign);
    let r = reports::build(
        &s.data,
        &f(Kind::Headcount),
        Utc.with_ymd_and_hms(2026, 10, 22, 18, 0, 0).unwrap(),
    )
    .unwrap();
    assert_eq!(r.employees, 4);
    assert_eq!(r.rows.len(), 3);
    assert_eq!(cell(&r, 0, "Snapshot date (company)"), "2026-10-23");
    let Analysis::Headcount { groups, .. } = r.analysis.unwrap() else {
        panic!()
    };
    assert_eq!(groups.iter().map(|g| g.active).sum::<usize>(), 3);
    assert_eq!(groups.iter().map(|g| g.archived).sum::<usize>(), 1);
    assert!(groups.iter().any(|g| g.name == "Not set" && g.active == 1));
    assert!(!groups.iter().any(|g| g.name == "Foreign company"));
    let mut only = f(Kind::Headcount);
    only.include_archived = false;
    assert_eq!(
        reports::build(&s.data, &only, Utc::now())
            .unwrap()
            .employees,
        3
    );
    for grouping in [GroupBy::Location, GroupBy::EmploymentType] {
        only.headcount_by = grouping;
        let r = reports::build(&s.data, &only, Utc::now()).unwrap();
        assert_eq!(
            r.rows
                .iter()
                .map(|row| row[3].parse::<usize>().unwrap())
                .sum::<usize>(),
            3
        );
    }
}
#[test]
fn workforce_aggregates_never_expand_manager_or_company_access() {
    let mut s = seed();
    s.data.access.role = Role::Manager;
    s.data.access.employee_id = Some(s.data.employees[0].id);
    s.data.access.direct_reports = vec![s.data.employees[1].id];
    for kind in [Kind::Headcount, Kind::Trends] {
        let mut f = f(kind);
        assert_eq!(
            reports::build(&s.data, &f, Utc::now()).unwrap().employees,
            2
        );
        f.employee_id = Some(s.data.employees[2].id);
        assert!(reports::build(&s.data, &f, Utc::now()).is_err());
    }
    s.data.access.role = Role::Employee;
    assert!(reports::build(&s.data, &f(Kind::Headcount), Utc::now()).is_err());
    s.data.access.role = Role::Administrator;
    s.data.mode = Mode::SignedOut;
    assert!(reports::build(&s.data, &f(Kind::Trends), Utc::now()).is_err());
    s.data.mode = Mode::Demo;
    s.data.access.company_id = Uuid::new_v4();
    assert!(reports::build(&s.data, &f(Kind::Trends), Utc::now()).is_err());
}
#[test]
fn daily_trends_preserve_saved_work_dates_half_days_and_zero_buckets() {
    let mut s = seed();
    let parent = DemoStore::new().data.attendance[0].clone();
    let mut a = parent;
    a.company_id = s.data.company.id;
    a.employee_id = s.data.employees[0].id;
    a.work_date = date("2024-02-29");
    a.checked_in_at = Utc.with_ymd_and_hms(2024, 3, 1, 0, 0, 0).unwrap();
    a.checked_out_at = None;
    s.data.attendance.push(a);
    for (status, day, units) in [
        (LeaveStatus::Approved, "2024-02-29", 1),
        (LeaveStatus::Pending, "2024-03-02", 2),
        (LeaveStatus::Cancelled, "2024-03-01", 2),
        (LeaveStatus::Rejected, "2024-02-28", 2),
    ] {
        s.data.leave.push(leave(&s, status, day, units));
    }
    let r = reports::build(&s.data, &f(Kind::Trends), Utc::now()).unwrap();
    assert_eq!(r.rows.len(), 4);
    assert_eq!(cell(&r, 0, "Attendance employee-days"), "0");
    assert_eq!(cell(&r, 1, "Attendance employee-days"), "1");
    assert_eq!(cell(&r, 1, "Open records"), "1");
    assert_eq!(cell(&r, 1, "Approved leave days"), "0.5");
    assert_eq!(cell(&r, 2, "Attendance employee-days"), "0");
    assert_eq!(cell(&r, 2, "Approved leave days"), "0");
    assert_eq!(cell(&r, 3, "Pending leave days"), "1");
    let csv = String::from_utf8(r.csv().unwrap()).unwrap();
    assert!(!csv.contains("private reason"));
    assert!(!csv.contains("Absent"));
}
#[test]
fn weekly_and_monthly_buckets_clip_edges_and_preserve_leap_day_charges() {
    let mut s = seed();
    s.data
        .leave
        .push(leave(&s, LeaveStatus::Approved, "2024-02-29", 1));
    s.data
        .leave
        .push(leave(&s, LeaveStatus::Approved, "2024-03-01", 2));
    let mut filter = f(Kind::Trends);
    filter.trend_bucket = Granularity::Monthly;
    let r = reports::build(&s.data, &filter, Utc::now()).unwrap();
    assert_eq!(r.rows.len(), 2);
    assert_eq!(cell(&r, 0, "Period start"), "2024-02-28");
    assert_eq!(cell(&r, 0, "Period end"), "2024-02-29");
    assert_eq!(cell(&r, 0, "Approved leave days"), "0.5");
    assert_eq!(cell(&r, 1, "Period end"), "2024-03-02");
    filter.starts_on = date("2024-02-29");
    filter.ends_on = date("2024-03-05");
    filter.trend_bucket = Granularity::Weekly;
    let r = reports::build(&s.data, &filter, Utc::now()).unwrap();
    assert_eq!(r.rows.len(), 2);
    assert_eq!(cell(&r, 0, "Period start"), "2024-02-29");
    assert_eq!(cell(&r, 0, "Period end"), "2024-03-03");
    assert_eq!(cell(&r, 0, "Approved leave days"), "1.5");
    assert_eq!(cell(&r, 1, "Period start"), "2024-03-04");
    assert_eq!(cell(&r, 1, "Period end"), "2024-03-05");
}
#[test]
fn workforce_limits_duplicate_records_and_empty_data_fail_or_render_explicitly() {
    let mut s = seed();
    let mut filter = f(Kind::Trends);
    filter.ends_on = filter.starts_on + Duration::days(366);
    assert!(reports::build(&s.data, &filter, Utc::now()).is_err());
    filter = f(Kind::Trends);
    let mut a = DemoStore::new().data.attendance[0].clone();
    a.work_date = filter.starts_on;
    a.employee_id = s.data.employees[0].id;
    s.data.attendance = vec![a.clone(), a];
    assert!(reports::build(&s.data, &filter, Utc::now()).is_err());
    s.data.employees.clear();
    let r = reports::build(&s.data, &filter, Utc::now()).unwrap();
    assert_eq!(r.employees, 0);
    assert_eq!(r.rows.len(), 4);
    assert!(r.rows.iter().all(|row| row[2] == "0"));
    assert!(
        reports::build(&s.data, &f(Kind::Headcount), Utc::now())
            .unwrap()
            .rows
            .is_empty()
    );
}
#[test]
fn headcount_csv_uses_generation_snapshot_date_and_safe_group_labels() {
    let mut s = seed();
    s.data.employees[0].department = "=SYNTHETIC()".into();
    let now = Utc.with_ymd_and_hms(2026, 10, 22, 18, 0, 0).unwrap();
    let r = reports::build(&s.data, &f(Kind::Headcount), now).unwrap();
    let csv = String::from_utf8(r.csv().unwrap()).unwrap();
    assert!(csv.contains("\"'=SYNTHETIC()\""));
    assert!(csv.contains("\"2026-10-23\",\"2026-10-23\""));
    assert!(!csv.contains("2024-02-28"));
}
#[test]
fn earlier_saved_filters_decode_with_safe_new_workforce_defaults() {
    let mut json = serde_json::to_value(f(Kind::Summary)).unwrap();
    json.as_object_mut().unwrap().remove("headcount_by");
    json.as_object_mut().unwrap().remove("trend_bucket");
    let filter: Filter = serde_json::from_value(json).unwrap();
    assert_eq!(filter.headcount_by, GroupBy::Department);
    assert_eq!(filter.trend_bucket, Granularity::Daily);
}
