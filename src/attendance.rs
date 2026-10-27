use crate::domain::{Attendance, Employee, EmployeeStatus, LeaveKind, LeaveStatus, Snapshot};
use chrono::NaiveDate;

#[derive(Clone)]
pub struct DailyRow {
    pub employee: Employee,
    pub attendance: Option<Attendance>,
    pub approved_leave: Option<LeaveKind>,
    pub approved_leave_units: i32,
    pub scheduled_workday: bool,
    pub planned_shift: Option<crate::shifts::ShiftPlan>,
}

/// A missing record is not proof of absence. Saved shifts and leave retain their snapshots.
/// Historical records survive archiving; future hires do not appear as missing.
pub fn daily_roster(data: &Snapshot, date: NaiveDate) -> Vec<DailyRow> {
    let mut rows: Vec<_> = data
        .employees
        .iter()
        .filter(|employee|data.access.can_read_operational_employee(employee))
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
                        && l.charges().iter().any(|d| d.date == date)
                })
                .map(|l| l.kind);
            let employed = employee.status == EmployeeStatus::Active
                && employee.joined_on.is_none_or(|joined| joined <= date);
            (attendance.is_some() || approved_leave.is_some() || employed).then(|| DailyRow {
                employee: employee.clone(),
                approved_leave_units: data
                    .leave
                    .iter()
                    .filter(|l| l.employee_id == employee.id && l.status == LeaveStatus::Approved)
                    .flat_map(|l| l.charges())
                    .filter(|d| d.date == date)
                    .map(|d| d.units)
                    .sum(),
                planned_shift: attendance
                    .as_ref()
                    .and_then(|a| a.planned_shift.clone())
                    .or_else(|| {
                        attendance
                            .is_none()
                            .then(|| data.shifts.plan(employee.id, date).cloned())
                            .flatten()
                    }),
                scheduled_workday: data
                    .shifts
                    .plan(employee.id, date)
                    .map(|p| p.working(data, date))
                    .unwrap_or_else(|| crate::leave::dated::policy_on(data,date).working(date)),
                attendance,
                approved_leave,
            })
        })
        .collect();
    rows.sort_by(|a, b| a.employee.full_name.cmp(&b.employee.full_name));
    rows
}
