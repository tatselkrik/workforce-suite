use chrono::{Datelike, NaiveDate};
use hris_desktop::{backend::ConnectionConfig, domain::*};
use uuid::Uuid;

fn date(value: &str) -> NaiveDate {
    parse_date(value).unwrap()
}
fn draft(store: &DemoStore) -> EmployeeDraft {
    EmployeeDraft {
        employee_code: "NEW-001".into(),
        full_name: "Demo Employee".into(),
        department: "Operations".into(),
        job_title: "Associate".into(),
        joined_on: store.data.company.today().to_string(),
        annual_leave_days: 15,
        ..Default::default()
    }
}
fn leave(employee_id: Uuid, start: &str, end: &str) -> LeaveDraft {
    LeaveDraft {
        employee_id: Some(employee_id),
        kind: LeaveKind::Annual,
        starts_on: start.into(),
        ends_on: end.into(),
        reason: String::new(),
    }
}

#[test]
fn employee_validation_normalizes_fields_and_rejects_duplicate_codes() {
    let mut store = DemoStore::new();
    let mut form = draft(&store);
    form.employee_code = " new-001 ".into();
    form.full_name = " Demo Employee ".into();
    store.execute(Action::SaveEmployee(form.clone())).unwrap();
    assert_eq!(
        store.data.employees.last().unwrap().employee_code,
        "NEW-001"
    );
    assert_eq!(
        store.data.employees.last().unwrap().full_name,
        "Demo Employee"
    );
    let before = store.data.employees.len();
    assert!(store.execute(Action::SaveEmployee(form)).is_err());
    assert_eq!(before, store.data.employees.len());
}

#[test]
fn employee_update_preserves_identity_and_history() {
    let mut store = DemoStore::new();
    let employee = store.data.employees[0].clone();
    let mut form = EmployeeDraft::from(&employee);
    form.full_name = "Updated Sample".into();
    store.execute(Action::SaveEmployee(form)).unwrap();
    assert_eq!(store.data.employees[0].id, employee.id);
    assert_eq!(
        store
            .data
            .employee_name(store.data.attendance[0].employee_id),
        "Updated Sample"
    );
}

#[test]
fn required_fields_and_invalid_dates_fail_without_mutating() {
    let mut store = DemoStore::new();
    let before = store.data.employees.clone();
    let mut form = draft(&store);
    form.full_name = " ".into();
    assert!(store.execute(Action::SaveEmployee(form)).is_err());
    let mut form = draft(&store);
    form.joined_on = "2026-02-30".into();
    assert!(store.execute(Action::SaveEmployee(form)).is_err());
    assert_eq!(before, store.data.employees);
    assert!(parse_date("2026-2-01").is_err());
    assert!(parse_date("1800-01-01").is_err());
}

#[test]
fn checkin_is_unique_and_checkout_is_final() {
    let mut store = DemoStore::new();
    let employee = store.data.employees[6].id;
    store.execute(Action::CheckIn(employee)).unwrap();
    assert!(store.execute(Action::CheckIn(employee)).is_err());
    let id = store.data.attendance.last().unwrap().id;
    store.execute(Action::CheckOut(id)).unwrap();
    assert!(
        store
            .data
            .attendance
            .last()
            .unwrap()
            .checked_out_at
            .is_some()
    );
    assert!(store.execute(Action::CheckOut(id)).is_err());
}

#[test]
fn a_previous_open_shift_must_be_closed_first() {
    let mut store = DemoStore::new();
    let employee = store.data.attendance[0].employee_id;
    store.data.attendance[0].work_date -= chrono::Duration::days(1);
    assert!(
        store
            .execute(Action::CheckIn(employee))
            .unwrap_err()
            .to_string()
            .contains("previous open")
    );
}

#[test]
fn archived_employees_cannot_receive_new_records() {
    let mut store = DemoStore::new();
    let employee = store.data.employees[7].id;
    assert!(store.execute(Action::CheckIn(employee)).is_err());
    assert!(
        store
            .execute(Action::RequestLeave(leave(
                employee,
                "2027-01-04",
                "2027-01-05"
            )))
            .is_err()
    );
}

#[test]
fn workdays_exclude_weekends_and_include_both_endpoints() {
    assert_eq!(weekdays(date("2026-10-19"), date("2026-10-25")), 5);
    assert_eq!(weekdays(date("2026-10-23"), date("2026-10-26")), 2);
    assert_eq!(weekdays(date("2026-10-24"), date("2026-10-25")), 0);
    assert_eq!(weekdays(date("2026-10-19"), date("2026-10-19")), 1);
    assert_eq!(weekdays(date("2026-10-20"), date("2026-10-19")), 0);
}

#[test]
fn invalid_leave_ranges_and_cross_year_requests_are_rejected() {
    let company = Company::default();
    let employee = Uuid::from_u128(100);
    assert!(
        leave(employee, "2027-01-05", "2027-01-04")
            .request(company.id)
            .is_err()
    );
    assert!(
        leave(employee, "2026-12-31", "2027-01-04")
            .request(company.id)
            .is_err()
    );
    assert!(
        leave(employee, "2026-10-24", "2026-10-25")
            .request(company.id)
            .is_err()
    );
}

#[test]
fn overlapping_pending_and_approved_leave_is_blocked_but_rejected_dates_are_reusable() {
    let mut store = DemoStore::new();
    store.data.leave.clear();
    let employee = store.data.employees[2].id;
    let form = leave(employee, "2027-01-04", "2027-01-06");
    store.execute(Action::RequestLeave(form.clone())).unwrap();
    assert!(
        store
            .execute(Action::RequestLeave(leave(
                employee,
                "2027-01-06",
                "2027-01-07"
            )))
            .is_err()
    );
    let id = store.data.leave[0].id;
    store
        .execute(Action::DecideLeave(id, LeaveStatus::Rejected))
        .unwrap();
    store.execute(Action::RequestLeave(form)).unwrap();
    assert_eq!(store.data.leave.len(), 2);
}

#[test]
fn only_approved_annual_leave_consumes_the_balance() {
    let mut store = DemoStore::new();
    store.data.leave.clear();
    let employee = store.data.employees[2].id;
    store
        .execute(Action::RequestLeave(leave(
            employee,
            "2027-01-04",
            "2027-01-05",
        )))
        .unwrap();
    assert_eq!(store.data.annual_used(employee, 2027), 0);
    let id = store.data.leave[0].id;
    store
        .execute(Action::DecideLeave(id, LeaveStatus::Approved))
        .unwrap();
    assert_eq!(store.data.annual_used(employee, 2027), 2);
    assert_eq!(store.data.annual_used(employee, 2026), 0);
    assert!(
        store
            .execute(Action::DecideLeave(id, LeaveStatus::Rejected))
            .is_err()
    );
}

#[test]
fn approval_cannot_exceed_the_annual_allowance() {
    let mut store = DemoStore::new();
    store.data.leave.clear();
    store.data.employees[0].annual_leave_days = 1;
    let employee = store.data.employees[0].id;
    store
        .execute(Action::RequestLeave(leave(
            employee,
            "2027-01-04",
            "2027-01-05",
        )))
        .unwrap();
    let id = store.data.leave[0].id;
    assert!(
        store
            .execute(Action::DecideLeave(id, LeaveStatus::Approved))
            .is_err()
    );
    assert_eq!(store.data.leave[0].status, LeaveStatus::Pending);
}

#[test]
fn company_settings_validate_timezone_and_do_not_rewrite_existing_allowances() {
    let mut store = DemoStore::new();
    let mut company = store.data.company.clone();
    company.timezone = "Invalid/Zone".into();
    assert!(store.execute(Action::SaveCompany(company)).is_err());
    let mut company = store.data.company.clone();
    company.annual_leave_days = 20;
    store.execute(Action::SaveCompany(company)).unwrap();
    assert_eq!(store.data.employees[0].annual_leave_days, 15);
}

#[test]
fn approved_current_year_leave_prevents_lowering_the_allowance() {
    let mut store = DemoStore::new();
    store.data.leave.clear();
    let employee = store.data.employees[2].id;
    let year = store.data.company.today().year();
    let monday = (1..=7)
        .map(|d| NaiveDate::from_ymd_opt(year, 1, d).unwrap())
        .find(|d| d.weekday().number_from_monday() == 1)
        .unwrap();
    store
        .execute(Action::RequestLeave(leave(
            employee,
            &monday.to_string(),
            &monday.to_string(),
        )))
        .unwrap();
    let id = store.data.leave[0].id;
    store
        .execute(Action::DecideLeave(id, LeaveStatus::Approved))
        .unwrap();
    let mut form = EmployeeDraft::from(&store.data.employees[2]);
    form.annual_leave_days = 0;
    assert!(store.execute(Action::SaveEmployee(form)).is_err());
}

#[test]
fn connection_accepts_only_publishable_keys_and_secure_base_urls() {
    let good = ConnectionConfig {
        url: "https://example.supabase.co".into(),
        publishable_key: "sb_publishable_example_only".into(),
    };
    assert!(good.validate().is_ok());
    for url in [
        "http://example.supabase.co",
        "https://example.supabase.co/rest/v1",
        "https://user:password@example.com",
        "https://example.com?token=value",
    ] {
        assert!(
            ConnectionConfig {
                url: url.into(),
                ..good.clone()
            }
            .validate()
            .is_err()
        );
    }
    for key in ["sb_secret_example", "eyJlegacy-service-role-token", ""] {
        assert!(
            ConnectionConfig {
                publishable_key: key.into(),
                ..good.clone()
            }
            .validate()
            .is_err()
        );
    }
    assert!(
        ConnectionConfig {
            url: "http://127.0.0.1:54321".into(),
            ..good
        }
        .validate()
        .is_ok()
    );
}
