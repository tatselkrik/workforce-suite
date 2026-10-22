use crate::{
    calculations,
    domain::{EmployeeStatus, LeaveStatus, Snapshot},
    employment::EmploymentKind,
    leave,
    reports::{self, Filter, Kind, Report},
};
use anyhow::{Result, ensure};
use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupBy {
    #[default]
    Department,
    Location,
    EmploymentType,
}
impl GroupBy {
    pub const ALL: [Self; 3] = [Self::Department, Self::Location, Self::EmploymentType];
    pub fn label(self) -> &'static str {
        match self {
            Self::Department => "Department",
            Self::Location => "Work location",
            Self::EmploymentType => "Employment type",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Granularity {
    #[default]
    Daily,
    Weekly,
    Monthly,
}
impl Granularity {
    pub const ALL: [Self; 3] = [Self::Daily, Self::Weekly, Self::Monthly];
    pub fn label(self) -> &'static str {
        match self {
            Self::Daily => "Daily",
            Self::Weekly => "Weekly (Monday)",
            Self::Monthly => "Monthly",
        }
    }
    fn anchor(self, date: NaiveDate) -> NaiveDate {
        match self {
            Self::Daily => date,
            Self::Weekly => date
                .checked_sub_signed(Duration::days(i64::from(
                    date.weekday().num_days_from_monday(),
                )))
                .unwrap_or(date),
            Self::Monthly => date.with_day(1).unwrap_or(date),
        }
    }
}
#[derive(Clone, Debug)]
pub struct HeadcountGroup {
    pub name: String,
    pub active: usize,
    pub archived: usize,
}
#[derive(Clone, Debug, Default)]
pub struct Trend {
    pub start: NaiveDate,
    pub end: NaiveDate,
    pub records: i64,
    pub completed: i64,
    pub open: i64,
    pub late_records: i64,
    pub late_minutes: i64,
    pub under_minutes: i64,
    pub overtime_candidate: i64,
    pub overtime_approved: i64,
    pub review: i64,
    pub approved_units: i32,
    pub pending_units: i32,
}
#[derive(Clone, Debug)]
pub enum Analysis {
    Headcount {
        as_of: NaiveDate,
        groups: Vec<HeadcountGroup>,
    },
    Trends(Vec<Trend>),
}
pub const HEADCOUNT_HEADERS: [&str; 6] = [
    "Snapshot date (company)",
    "Group by",
    "Group (current)",
    "Active employees",
    "Archived employees",
    "Employee records",
];
pub const TREND_HEADERS: [&str; 14] = [
    "Period start",
    "Period end",
    "Attendance employee-days",
    "Completed records",
    "Open records",
    "Late records",
    "Late minutes",
    "Early departure minutes",
    "Overtime candidate minutes",
    "Current approved overtime minutes",
    "Records needing calculation review",
    "Approved leave days",
    "Pending leave days",
    "Bucket",
];

// Called only after reports::build has validated the signed-in company and
// employee filter. All aggregates use the same current permitted employee set.
pub(crate) fn build(data: &Snapshot, filter: &Filter, now: DateTime<Utc>) -> Result<Report> {
    let people = reports::employees(data, filter);
    ensure!(
        people.len() <= reports::MAX_ROWS,
        "Choose at most 50,000 employee records for workforce analysis."
    );
    let mut report = Report {
        filter: filter.clone(),
        company: data.company.name.clone(),
        timezone: data.company.timezone.clone(),
        generated_at: now,
        headers: Vec::new(),
        rows: Vec::new(),
        employees: people.len(),
        analysis: None,
    };
    if filter.kind == Kind::Headcount {
        let as_of = now.with_timezone(&data.company.tz()).date_naive();
        let mut grouped: BTreeMap<String, (usize, usize)> = BTreeMap::new();
        for e in people {
            let label = match filter.headcount_by {
                GroupBy::Department => {
                    if e.department.trim().is_empty() {
                        "Not set".into()
                    } else {
                        e.department.clone()
                    }
                }
                GroupBy::Location => EmploymentKind::WorkLocation.display(data, e.work_location_id),
                GroupBy::EmploymentType => {
                    EmploymentKind::EmploymentType.display(data, e.employment_type_id)
                }
            };
            let counts = grouped.entry(label).or_default();
            match e.status {
                EmployeeStatus::Active => counts.0 += 1,
                EmployeeStatus::Archived => counts.1 += 1,
            }
        }
        let groups: Vec<_> = grouped
            .into_iter()
            .map(|(name, (active, archived))| HeadcountGroup {
                name,
                active,
                archived,
            })
            .collect();
        report.headers = HEADCOUNT_HEADERS.to_vec();
        report.rows = groups
            .iter()
            .map(|g| {
                vec![
                    as_of.to_string(),
                    filter.headcount_by.label().into(),
                    g.name.clone(),
                    g.active.to_string(),
                    g.archived.to_string(),
                    (g.active + g.archived).to_string(),
                ]
            })
            .collect();
        report.analysis = Some(Analysis::Headcount { as_of, groups });
        return Ok(report);
    }
    ensure!(filter.kind == Kind::Trends, "Unknown workforce report.");
    let days = (filter.ends_on - filter.starts_on).num_days() + 1;
    ensure!(
        people.len().saturating_mul(days as usize) <= reports::MAX_ROWS,
        "Workforce trends exceed 50,000 employee-days. Shorten the period or choose an employee/department."
    );
    let selected: BTreeSet<_> = people.iter().map(|e| e.id).collect();
    let mut buckets: BTreeMap<NaiveDate, Trend> = BTreeMap::new();
    for offset in 0..days {
        let day = filter.starts_on + Duration::days(offset);
        let bucket = buckets
            .entry(filter.trend_bucket.anchor(day))
            .or_insert_with(|| Trend {
                start: day,
                end: day,
                ..Default::default()
            });
        bucket.end = day;
    }
    let mut leaves_by: BTreeMap<Uuid, Vec<_>> = BTreeMap::new();
    for request in data
        .leave
        .iter()
        .filter(|r| r.company_id == data.company.id && selected.contains(&r.employee_id))
    {
        if request.status == LeaveStatus::Approved {
            leaves_by
                .entry(request.employee_id)
                .or_default()
                .push(request.clone());
        }
        if !matches!(request.status, LeaveStatus::Approved | LeaveStatus::Pending) {
            continue;
        }
        for charge in request
            .charges()
            .into_iter()
            .filter(|c| c.date >= filter.starts_on && c.date <= filter.ends_on)
        {
            ensure!(
                (1..=2).contains(&charge.units),
                "A saved leave charge is invalid. Review it before reporting."
            );
            let bucket = buckets
                .get_mut(&filter.trend_bucket.anchor(charge.date))
                .unwrap();
            let units = if request.status == LeaveStatus::Approved {
                &mut bucket.approved_units
            } else {
                &mut bucket.pending_units
            };
            *units = units
                .checked_add(charge.units)
                .ok_or_else(|| anyhow::anyhow!("Leave totals are out of range."))?;
        }
    }
    let mut breaks_by: BTreeMap<Uuid, Vec<_>> = BTreeMap::new();
    for b in data
        .breaks
        .records
        .iter()
        .filter(|b| b.company_id == data.company.id && selected.contains(&b.employee_id))
    {
        breaks_by
            .entry(b.attendance_id)
            .or_default()
            .push(b.clone());
    }
    let mut overtime_by: BTreeMap<Uuid, Vec<_>> = BTreeMap::new();
    for r in data
        .overtime
        .records
        .iter()
        .filter(|r| r.company_id == data.company.id && selected.contains(&r.employee_id))
    {
        overtime_by.entry(r.attendance_id).or_default().push(r);
    }
    let mut calc_data = Snapshot::signed_out();
    calc_data.company = data.company.clone();
    let mut seen = BTreeSet::new();
    for a in data.attendance.iter().filter(|a| {
        a.company_id == data.company.id
            && selected.contains(&a.employee_id)
            && a.work_date >= filter.starts_on
            && a.work_date <= filter.ends_on
    }) {
        ensure!(
            seen.insert((a.employee_id, a.work_date)),
            "Duplicate employee work dates prevent reliable trends."
        );
        let bucket = buckets
            .get_mut(&filter.trend_bucket.anchor(a.work_date))
            .unwrap();
        bucket.records += 1;
        if a.checked_out_at.is_some() {
            bucket.completed += 1;
        } else {
            bucket.open += 1;
        }
        calc_data.attendance = vec![a.clone()];
        calc_data.leave = leaves_by.get(&a.employee_id).cloned().unwrap_or_default();
        calc_data.breaks.records = breaks_by.get(&a.id).cloned().unwrap_or_default();
        let calc = calculations::calculate(&calc_data, a);
        if calc.status != calculations::CalculationStatus::Ready {
            bucket.review += 1;
        }
        let late = calc.late_seconds.unwrap_or(0) / 60;
        if late > 0 {
            bucket.late_records += 1;
        }
        bucket.late_minutes += late;
        bucket.under_minutes += calc.undertime_seconds.unwrap_or(0) / 60;
        bucket.overtime_candidate += calc.overtime_minutes.unwrap_or(0);
        bucket.overtime_approved += overtime_by
            .get(&a.id)
            .into_iter()
            .flatten()
            .filter_map(|r| crate::overtime::effective_minutes(&calc_data, r))
            .map(i64::from)
            .sum::<i64>();
    }
    let trends: Vec<_> = buckets.into_values().collect();
    report.headers = TREND_HEADERS.to_vec();
    report.rows = trends
        .iter()
        .map(|t| {
            vec![
                t.start.to_string(),
                t.end.to_string(),
                t.records.to_string(),
                t.completed.to_string(),
                t.open.to_string(),
                t.late_records.to_string(),
                t.late_minutes.to_string(),
                t.under_minutes.to_string(),
                t.overtime_candidate.to_string(),
                t.overtime_approved.to_string(),
                t.review.to_string(),
                leave::days(t.approved_units),
                leave::days(t.pending_units),
                filter.trend_bucket.label().into(),
            ]
        })
        .collect();
    report.analysis = Some(Analysis::Trends(trends));
    Ok(report)
}
