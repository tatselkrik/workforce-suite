use crate::domain::{Attendance, Employee, EmployeeStatus, LeaveKind, LeaveStatus, Snapshot};
use chrono::{Datelike, NaiveDate};

#[derive(Clone)]
pub struct DailyRow {
    pub employee: Employee,
    pub attendance: Option<Attendance>,
    pub approved_leave: Option<LeaveKind>,
}

/// A missing record is not proof of absence: shifts/holidays are not configured yet.
/// Historical records survive archiving; future hires do not appear as missing.
pub fn daily_roster(data: &Snapshot, date: NaiveDate) -> Vec<DailyRow> {
    let mut rows: Vec<_> = data
        .employees
        .iter()
        .filter_map(|employee| {
            let attendance = data
                .attendance
                .iter()
                .find(|a| a.employee_id == employee.id && a.work_date == date)
                .cloned();
            let approved_leave = data
                .leave
                .iter()
                .find(|l| {
                    l.employee_id == employee.id
                        && l.status == LeaveStatus::Approved
                        && l.starts_on <= date
                        && l.ends_on >= date
                        && date.weekday().num_days_from_monday() < 5
                })
                .map(|l| l.kind);
            let employed = employee.status == EmployeeStatus::Active
                && employee.joined_on.is_none_or(|joined| joined <= date);
            (attendance.is_some() || approved_leave.is_some() || employed).then(|| DailyRow {
                employee: employee.clone(),
                attendance,
                approved_leave,
            })
        })
        .collect();
    rows.sort_by(|a, b| a.employee.full_name.cmp(&b.employee.full_name));
    rows
}
