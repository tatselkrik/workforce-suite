use crate::domain::{Attendance, Employee, EmployeeStatus, LeaveKind, LeaveStatus, Snapshot};
use chrono::NaiveDate;
use std::collections::{HashMap, HashSet};

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
    let company = data.company.id;
    let employees: Vec<_> = data
        .employees
        .iter()
        .filter(|employee| {
            employee.company_id == company && data.access.can_read_operational_employee(employee)
        })
        .collect();
    if employees.is_empty() {
        return Vec::new();
    }
    let employee_ids: HashSet<_> = employees.iter().map(|employee| employee.id).collect();
    let mut punches = HashMap::new();
    for record in data.attendance.iter().filter(|record| {
        record.company_id == company
            && record.work_date == date
            && employee_ids.contains(&record.employee_id)
    }) {
        punches.entry(record.employee_id).or_insert(record);
    }
    let mut approved = HashMap::new();
    for request in data.leave.iter().filter(|request| {
        request.company_id == company
            && request.status == LeaveStatus::Approved
            && employee_ids.contains(&request.employee_id)
    }) {
        for charge in request.charges().into_iter().filter(|charge| charge.date == date) {
            let (_, units) = approved.entry(request.employee_id).or_insert((request.kind, 0));
            *units += charge.units;
        }
    }

    let company_workday = crate::leave::dated::policy_on(data, date).working(date);
    let mut rows: Vec<_> = employees
        .into_iter()
        .filter_map(|employee| {
            let attendance = punches.get(&employee.id).copied();
            let leave = approved.get(&employee.id);
            let employed = employee.status == EmployeeStatus::Active
                && employee.joined_on.is_none_or(|joined| joined <= date);
            (attendance.is_some() || leave.is_some() || employed).then(|| {
                let plan = data.shifts.plan(employee.id, date);
                DailyRow {
                    employee: employee.clone(),
                    approved_leave: leave.map(|(kind, _)| *kind),
                    approved_leave_units: leave.map_or(0, |(_, units)| *units),
                    planned_shift: match attendance {
                        Some(record) => record.planned_shift.clone(),
                        None => plan.cloned(),
                    },
                    scheduled_workday: plan.map_or(company_workday, |plan| plan.working(data, date)),
                    attendance: attendance.cloned(),
                }
            })
        })
        .collect();
    rows.sort_by(|a, b| a.employee.full_name.cmp(&b.employee.full_name));
    rows
}
