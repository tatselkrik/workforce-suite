use chrono::NaiveDate;
use hris_desktop::{attendance::daily_roster, domain::*};
use uuid::Uuid;

fn date(text: &str) -> NaiveDate {
    parse_date(text).unwrap()
}
fn employee(company: Uuid, code: &str, joined: &str, status: EmployeeStatus) -> Employee {
    EmployeeDraft {
        employee_code: code.into(),
        full_name: format!("Synthetic {code}"),
        joined_on: joined.into(),
        status,
        ..Default::default()
    }
    .employee(company)
    .unwrap()
}

#[test]
fn historical_roster_separates_missing_and_approved_leave_and_retains_archived_records() {
    let mut data = DemoStore::new().data;
    data.employees.clear();
    data.attendance.clear();
    data.leave.clear();
    for (code, joined, status) in [
        ("PRESENT", "2024-01-01", EmployeeStatus::Active),
        ("MISSING", "", EmployeeStatus::Active),
        ("FUTURE", "2024-03-01", EmployeeStatus::Active),
        ("ARCHIVED", "2024-01-01", EmployeeStatus::Archived),
        ("ARCHIVED-NONE", "2024-01-01", EmployeeStatus::Archived),
        ("LEAVE", "2024-01-01", EmployeeStatus::Active),
    ] {
        data.employees
            .push(employee(data.company.id, code, joined, status));
    }
    let day = date("2024-02-29");
    for employee in [&data.employees[0], &data.employees[3]] {
        data.attendance.push(Attendance {
            planned_shift: None,
            revision: 0,
            id: Uuid::new_v4(),
            company_id: data.company.id,
            employee_id: employee.id,
            work_date: day,
            checked_in_at: day.and_hms_opt(1, 0, 0).unwrap().and_utc(),
            checked_out_at: None,
        });
    }
    data.leave.push(LeaveRequest {
        policy: Default::default(),
        id: Uuid::new_v4(),
        company_id: data.company.id,
        employee_id: data.employees[5].id,
        kind: LeaveKind::Annual,
        starts_on: date("2024-02-28"),
        ends_on: date("2024-03-04"),
        reason: String::new(),
        status: LeaveStatus::Approved,
    });
    let rows = daily_roster(&data, day);
    assert_eq!(rows.len(), 4);
    assert_eq!(rows.iter().filter(|r| r.attendance.is_some()).count(), 2);
    assert_eq!(
        rows.iter()
            .filter(|r| r.attendance.is_none() && r.approved_leave.is_some())
            .count(),
        1
    );
    assert_eq!(
        rows.iter()
            .filter(|r| r.attendance.is_none() && r.approved_leave.is_none())
            .count(),
        1
    );
    assert!(!rows.iter().any(
        |r| r.employee.employee_code == "FUTURE" || r.employee.employee_code == "ARCHIVED-NONE"
    ));
    assert!(
        rows.iter()
            .any(|r| r.employee.employee_code == "ARCHIVED" && r.attendance.is_some())
    );
    assert!(
        daily_roster(&data, date("2024-03-02"))
            .iter()
            .all(|r| r.approved_leave.is_none())
    );
    data.leave[0].status = LeaveStatus::Pending;
    assert!(
        daily_roster(&data, day)
            .iter()
            .all(|r| r.approved_leave.is_none())
    );
}

#[test]
fn roster_uses_only_the_available_role_scoped_people_and_the_selected_day() {
    let mut data = DemoStore::new().data;
    let own = data.access.employee_id.unwrap();
    data.employees.retain(|e| e.id == own);
    for employee in &mut data.employees {
        employee.joined_on = None;
    }
    let rows = daily_roster(&data, data.company.today());
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].employee.id, own);
    let previous = data.company.today().pred_opt().unwrap();
    assert!(daily_roster(&data, previous).iter().all(|r| {
        r.attendance
            .as_ref()
            .is_none_or(|a| a.work_date == previous)
    }));
}
