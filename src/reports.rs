use crate::{
    access::{Access, Role},
    calculations,
    domain::{Employee, EmployeeStatus, LeaveStatus, Mode, Snapshot},
    employment::EmploymentKind,
    leave,
};
use anyhow::{Context, Result, ensure};
use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc};
use std::{
    collections::BTreeMap,
    io::Write,
    path::{Path, PathBuf},
};
use uuid::Uuid;

pub const MAX_ROWS: usize = 50_000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kind {
    #[default]
    Attendance,
    Leave,
    Summary,
}
impl Kind {
    pub const ALL: [Self; 3] = [Self::Attendance, Self::Leave, Self::Summary];
    pub fn label(self) -> &'static str {
        match self {
            Self::Attendance => "Attendance detail",
            Self::Leave => "Leave detail",
            Self::Summary => "Employee summary",
        }
    }
    pub fn slug(self) -> &'static str {
        match self {
            Self::Attendance => "attendance",
            Self::Leave => "leave",
            Self::Summary => "employee-summary",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Filter {
    pub kind: Kind,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    pub employee_id: Option<Uuid>,
    pub department: String,
    pub include_archived: bool,
    pub include_missing: bool,
    pub leave_status: Option<LeaveStatus>,
}
impl Filter {
    pub fn new(today: NaiveDate) -> Self {
        Self {
            kind: Kind::Attendance,
            starts_on: today.with_day(1).unwrap_or(today),
            ends_on: today,
            employee_id: None,
            department: String::new(),
            include_archived: true,
            include_missing: true,
            leave_status: None,
        }
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.ends_on >= self.starts_on,
            "End date must be on or after start date."
        );
        ensure!(
            (self.ends_on - self.starts_on).num_days() < 366,
            "Choose at most 366 days per report."
        );
        Ok(())
    }
    pub fn filename(&self) -> String {
        format!(
            "hris-{}-{}-{}.csv",
            self.kind.slug(),
            self.starts_on,
            self.ends_on
        )
    }
}

#[derive(Clone, Debug)]
pub struct Report {
    pub filter: Filter,
    pub company: String,
    pub timezone: String,
    pub generated_at: DateTime<Utc>,
    pub headers: Vec<&'static str>,
    pub rows: Vec<Vec<String>>,
    pub employees: usize,
}

pub fn same_scope(a: &Access, b: &Access) -> bool {
    let mut left = a.direct_reports.clone();
    let mut right = b.direct_reports.clone();
    left.sort();
    left.dedup();
    right.sort();
    right.dedup();
    a.company_id == b.company_id
        && a.user_id == b.user_id
        && a.employee_id == b.employee_id
        && a.role == b.role
        && left == right
}
pub fn permitted(data: &Snapshot, employee: Uuid) -> bool {
    data.access.role.manages_hr()
        || (data.access.role == Role::Manager
            && (data.access.employee_id == Some(employee)
                || data.access.direct_reports.contains(&employee)))
}
pub fn employees<'a>(data: &'a Snapshot, f: &Filter) -> Vec<&'a Employee> {
    let mut people: Vec<_> = data
        .employees
        .iter()
        .filter(|e| {
            e.company_id == data.company.id
                && permitted(data, e.id)
                && (f.include_archived || e.status == EmployeeStatus::Active)
                && f.employee_id.is_none_or(|id| id == e.id)
                && (f.department.is_empty() || e.department == f.department)
        })
        .collect();
    people.sort_by(|a, b| {
        a.full_name
            .to_lowercase()
            .cmp(&b.full_name.to_lowercase())
            .then(a.employee_code.cmp(&b.employee_code))
            .then(a.id.cmp(&b.id))
    });
    people
}
const COMMON: [&str; 7] = [
    "Employee code",
    "Full name",
    "Department (current)",
    "Job title (current)",
    "Employment type (current)",
    "Work location (current)",
    "Employee status (current)",
];
fn person(data: &Snapshot, e: &Employee) -> Vec<String> {
    vec![
        e.employee_code.clone(),
        e.full_name.clone(),
        e.department.clone(),
        e.job_title.clone(),
        EmploymentKind::EmploymentType.display(data, e.employment_type_id),
        EmploymentKind::WorkLocation.display(data, e.work_location_id),
        e.status.label().into(),
    ]
}
fn optional_minutes(seconds: Option<i64>) -> String {
    seconds
        .map(|s| (s.max(0) / 60).to_string())
        .unwrap_or_default()
}
fn optional_number(value: Option<i64>) -> String {
    value.map(|v| v.to_string()).unwrap_or_default()
}
#[derive(Default)]
struct Totals {
    records: usize,
    completed: usize,
    open: usize,
    missing: usize,
    scheduled_missing: usize,
    gross: i64,
    net: i64,
    unavailable: usize,
    review: usize,
    late: i64,
    under: i64,
    candidate: i64,
    approved: i64,
    corrections: usize,
    approved_leave: i32,
    pending_leave: i32,
}
fn dates(f: &Filter) -> impl Iterator<Item = NaiveDate> {
    let start = f.starts_on;
    (0..=(f.ends_on - f.starts_on).num_days()).map(move |i| start + Duration::days(i))
}
fn add_row(report: &mut Report, row: Vec<String>) -> Result<()> {
    ensure!(
        report.rows.len() < MAX_ROWS,
        "Report exceeds 50,000 rows. Shorten the date range or choose an employee/department."
    );
    report.rows.push(row);
    Ok(())
}
pub fn build(data: &Snapshot, f: &Filter, now: DateTime<Utc>) -> Result<Report> {
    f.validate()?;
    ensure!(
        data.mode != Mode::SignedOut
            && data.access.role.uses_desktop()
            && !data.company.id.is_nil()
            && data.access.company_id == data.company.id,
        "Sign in with an assigned staff role to run reports."
    );
    if let Some(id) = f.employee_id {
        ensure!(
            data.employees
                .iter()
                .any(|e| e.id == id && e.company_id == data.company.id && permitted(data, id)),
            "This employee is outside your current report access. Choose another employee."
        );
    }
    let people = employees(data, f);
    let selected: std::collections::BTreeSet<_> = people.iter().map(|e| e.id).collect();
    let mut requests_by: BTreeMap<Uuid, Vec<&crate::domain::LeaveRequest>> = BTreeMap::new();
    let mut approved_days: BTreeMap<(Uuid, NaiveDate), i32> = BTreeMap::new();
    let mut pending_days: BTreeMap<(Uuid, NaiveDate), i32> = BTreeMap::new();
    for r in &data.leave {
        if r.company_id != data.company.id
            || !selected.contains(&r.employee_id)
            || r.starts_on > f.ends_on
            || r.ends_on < f.starts_on
        {
            continue;
        }
        requests_by.entry(r.employee_id).or_default().push(r);
        let charges = match r.status {
            LeaveStatus::Approved => Some(&mut approved_days),
            LeaveStatus::Pending => Some(&mut pending_days),
            _ => None,
        };
        if let Some(charges) = charges {
            for c in r
                .charges()
                .into_iter()
                .filter(|c| c.date >= f.starts_on && c.date <= f.ends_on)
            {
                *charges.entry((r.employee_id, c.date)).or_default() += c.units;
            }
        }
    }
    let mut corrections_by: BTreeMap<(Uuid, NaiveDate), usize> = BTreeMap::new();
    for r in data.requests.records.iter().filter(|r| {
        r.company_id == data.company.id
            && r.status == crate::requests::Status::Pending
            && selected.contains(&r.employee_id)
    }) {
        *corrections_by
            .entry((r.employee_id, r.work_date))
            .or_default() += 1;
    }
    let mut breaks_by: BTreeMap<Uuid, Vec<_>> = BTreeMap::new();
    for b in data.breaks.records.iter().filter(|b| {
        b.company_id == data.company.id && selected.contains(&b.employee_id) && !b.voided
    }) {
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
    let mut assignments_by: BTreeMap<Uuid, Vec<_>> = BTreeMap::new();
    for a in data.shifts.assignments.iter().filter(|a| {
        a.company_id == data.company.id
            && selected.contains(&a.employee_id)
            && a.effective_on <= f.ends_on
    }) {
        assignments_by.entry(a.employee_id).or_default().push(a);
    }
    for assignments in assignments_by.values_mut() {
        assignments.sort_by_key(|a| (a.effective_on, a.sequence));
    }
    // Reuse the existing calculator with only the current employee's leave
    // and record's breaks; large exports must not rescan every company row.
    let mut calculation_data = Snapshot::signed_out();
    calculation_data.company = data.company.clone();
    let mut report = Report {
        filter: f.clone(),
        company: data.company.name.clone(),
        timezone: data.company.timezone.clone(),
        generated_at: now,
        headers: COMMON.to_vec(),
        rows: Vec::new(),
        employees: people.len(),
    };
    match f.kind {
        Kind::Attendance | Kind::Summary => {
            let detail = f.kind == Kind::Attendance;
            ensure!(
                people
                    .len()
                    .saturating_mul(((f.ends_on - f.starts_on).num_days() + 1) as usize)
                    <= MAX_ROWS,
                "Report exceeds 50,000 employee-days. Shorten the date range or choose an employee/department."
            );
            if detail {
                report.headers.extend([
                    "Work date",
                    "Record status",
                    "Check-in (local)",
                    "Checkout (local)",
                    "Record time zone",
                    "Shift (saved/dated)",
                    "Rule version (saved)",
                    "Gross minutes",
                    "Completed unpaid break minutes",
                    "Minutes excluding unpaid breaks",
                    "Calculation status",
                    "Late minutes",
                    "Early departure minutes",
                    "Overtime candidate minutes",
                    "Current approved overtime minutes",
                    "Approved leave days",
                    "Pending correction requests",
                ]);
            } else {
                report.headers.extend([
                    "Attendance records",
                    "Completed records",
                    "Open records",
                    "Days without check-in",
                    "Scheduled past days without check-in",
                    "Gross minutes",
                    "Minutes excluding unpaid breaks",
                    "Records without completed net totals",
                    "Records needing calculation review",
                    "Late minutes",
                    "Early departure minutes",
                    "Overtime candidate minutes",
                    "Current approved overtime minutes",
                    "Approved leave days",
                    "Pending leave days",
                    "Pending correction requests",
                ]);
            }
            let mut work = BTreeMap::new();
            for a in &data.attendance {
                if a.company_id == data.company.id
                    && selected.contains(&a.employee_id)
                    && a.work_date >= f.starts_on
                    && a.work_date <= f.ends_on
                {
                    ensure!(
                        work.insert((a.employee_id, a.work_date), a).is_none(),
                        "Duplicate employee work dates prevent a reliable report."
                    );
                }
            }
            for e in people {
                calculation_data.leave = requests_by
                    .get(&e.id)
                    .into_iter()
                    .flatten()
                    .filter(|r| r.status == LeaveStatus::Approved)
                    .map(|r| (*r).clone())
                    .collect();
                let mut total = Totals::default();
                for day in dates(f) {
                    let a = work.get(&(e.id, day)).copied();
                    // Archived people have no inferred termination date. Preserve
                    // their actual records/leave but do not invent missing days.
                    let include_missing = f.include_missing
                        && e.status == EmployeeStatus::Active
                        && e.joined_on.is_none_or(|joined| day >= joined);
                    let approved_leave = *approved_days.get(&(e.id, day)).unwrap_or(&0);
                    total.approved_leave += approved_leave;
                    total.pending_leave += *pending_days.get(&(e.id, day)).unwrap_or(&0);
                    let corrections = *corrections_by.get(&(e.id, day)).unwrap_or(&0);
                    total.corrections += corrections;
                    if a.is_none() && !include_missing {
                        continue;
                    }
                    let plan = a.and_then(|a| a.planned_shift.as_ref()).or_else(|| {
                        assignments_by
                            .get(&e.id)
                            .into_iter()
                            .flatten()
                            .rev()
                            .find(|a| a.effective_on <= day)
                            .and_then(|a| a.plan.as_ref())
                    });
                    let scheduled = a
                        .and_then(|a| a.calculation_snapshot.as_ref())
                        .map(|s| s.scheduled_workday)
                        .unwrap_or_else(|| plan.is_some_and(|p| p.working(data, day)));
                    let mut row = person(data, e);
                    row.push(day.to_string());
                    if let Some(a) = a {
                        total.records += 1;
                        if a.checked_out_at.is_some() {
                            total.completed += 1
                        } else {
                            total.open += 1
                        }
                        calculation_data.attendance = vec![a.clone()];
                        calculation_data.breaks.records =
                            breaks_by.get(&a.id).cloned().unwrap_or_default();
                        let calc = calculations::calculate(&calculation_data, a);
                        if calc.status != calculations::CalculationStatus::Ready {
                            total.review += 1
                        }
                        let breaks = calculation_data.breaks.totals(a.id);
                        let gross = a
                            .checked_out_at
                            .map(|out| (out - a.checked_in_at).num_seconds().max(0));
                        total.gross += gross.unwrap_or(0) / 60;
                        total.net += calc.net_seconds.unwrap_or(0) / 60;
                        if calc.net_seconds.is_none() {
                            total.unavailable += 1
                        }
                        total.late += calc.late_seconds.unwrap_or(0) / 60;
                        total.under += calc.undertime_seconds.unwrap_or(0) / 60;
                        total.candidate += calc.overtime_minutes.unwrap_or(0);
                        let approved: i64 = overtime_by
                            .get(&a.id)
                            .into_iter()
                            .flatten()
                            .filter_map(|r| {
                                crate::overtime::effective_minutes(&calculation_data, r)
                            })
                            .map(i64::from)
                            .sum();
                        total.approved += approved;
                        let zone = a
                            .calculation_snapshot
                            .as_ref()
                            .map(|s| s.timezone.as_str())
                            .or_else(|| a.planned_shift.as_ref().map(|p| p.timezone.as_str()))
                            .unwrap_or(&data.company.timezone);
                        let tz=zone.parse::<chrono_tz::Tz>().context("A saved attendance time zone is invalid. Review this record before reporting.")?;
                        row.extend([
                            if a.checked_out_at.is_some() {
                                "Completed"
                            } else {
                                "Open"
                            }
                            .into(),
                            a.checked_in_at.with_timezone(&tz).to_rfc3339(),
                            a.checked_out_at
                                .map(|t| t.with_timezone(&tz).to_rfc3339())
                                .unwrap_or_default(),
                            zone.into(),
                            a.planned_shift
                                .as_ref()
                                .map(|p| format!("{} · {}", p.name, p.hours()))
                                .unwrap_or_else(|| "No saved shift".into()),
                            a.calculation_snapshot
                                .as_ref()
                                .map(|s| s.policy.revision.to_string())
                                .unwrap_or_default(),
                            optional_minutes(gross),
                            (breaks.unpaid_seconds / 60).to_string(),
                            optional_minutes(calc.net_seconds),
                            calc.status.label().into(),
                            optional_minutes(calc.late_seconds),
                            optional_minutes(calc.undertime_seconds),
                            optional_number(calc.overtime_minutes),
                            approved.to_string(),
                            leave::days(approved_leave),
                            corrections.to_string(),
                        ]);
                    } else {
                        let status = if day >= data.company.today() {
                            "Not yet due / no check-in"
                        } else if approved_leave >= 2 {
                            "Approved leave / no check-in"
                        } else if approved_leave > 0 {
                            "Half-day leave / no check-in"
                        } else if scheduled {
                            "Scheduled / no check-in"
                        } else if plan.is_some() {
                            "Off day / no check-in"
                        } else {
                            "Schedule unassigned / no check-in"
                        };
                        total.missing += 1;
                        if day < data.company.today() && scheduled && approved_leave == 0 {
                            total.scheduled_missing += 1
                        }
                        row.extend([
                            status.into(),
                            String::new(),
                            String::new(),
                            plan.map(|p| p.timezone.clone())
                                .unwrap_or_else(|| data.company.timezone.clone()),
                            plan.map(|p| format!("{} · {}", p.name, p.hours()))
                                .unwrap_or_else(|| "Unassigned".into()),
                            String::new(),
                            String::new(),
                            String::new(),
                            String::new(),
                            "No recorded attendance".into(),
                            String::new(),
                            String::new(),
                            String::new(),
                            "0".into(),
                            leave::days(approved_leave),
                            corrections.to_string(),
                        ]);
                    }
                    if detail {
                        add_row(&mut report, row)?;
                    }
                }
                if !detail {
                    let mut row = person(data, e);
                    row.extend([
                        total.records.to_string(),
                        total.completed.to_string(),
                        total.open.to_string(),
                        total.missing.to_string(),
                        total.scheduled_missing.to_string(),
                        total.gross.to_string(),
                        total.net.to_string(),
                        total.unavailable.to_string(),
                        total.review.to_string(),
                        total.late.to_string(),
                        total.under.to_string(),
                        total.candidate.to_string(),
                        total.approved.to_string(),
                        leave::days(total.approved_leave),
                        leave::days(total.pending_leave),
                        total.corrections.to_string(),
                    ]);
                    add_row(&mut report, row)?;
                }
            }
        }
        Kind::Leave => {
            report.headers.extend([
                "Request ID",
                "Leave type (filed)",
                "Status",
                "Request start",
                "Request end",
                "Charge date",
                "Filed charge on date (days)",
                "Filed total (days)",
                "Day part",
                "Filed policy version",
                "Approved steps",
                "Total approval steps",
            ]);
            for e in people {
                let mut requests: Vec<_> = requests_by
                    .get(&e.id)
                    .into_iter()
                    .flatten()
                    .copied()
                    .filter(|r| f.leave_status.is_none_or(|s| s == r.status))
                    .collect();
                requests.sort_by_key(|r| (r.starts_on, r.id));
                for r in requests {
                    for charge in r
                        .charges()
                        .into_iter()
                        .filter(|c| c.date >= f.starts_on && c.date <= f.ends_on)
                    {
                        let mut row = person(data, e);
                        row.extend([
                            r.id.to_string(),
                            r.type_name().into(),
                            r.status.label().into(),
                            r.starts_on.to_string(),
                            r.ends_on.to_string(),
                            charge.date.to_string(),
                            leave::days(charge.units),
                            leave::days(r.units()),
                            r.policy.day_part.label().into(),
                            r.policy.policy_revision.to_string(),
                            r.policy
                                .approval_steps
                                .iter()
                                .filter(|s| s.approved_by.is_some())
                                .count()
                                .to_string(),
                            r.policy.approval_steps.len().to_string(),
                        ]);
                        add_row(&mut report, row)?;
                    }
                }
            }
        }
    }
    ensure!(
        report.rows.iter().all(|r| r.len() == report.headers.len()),
        "The report columns do not match. Export was blocked."
    );
    Ok(report)
}

fn csv_cell(value: &str) -> Result<String> {
    ensure!(
        !value.contains('\0'),
        "A report field contains an invalid null character."
    );
    let formula = value
        .trim_start_matches(|c: char| c.is_whitespace() || c == '\u{feff}')
        .starts_with(['=', '+', '-', '@'])
        || value.starts_with(['\t', '\r', '\n']);
    Ok(format!(
        "\"{}{}\"",
        if formula { "'" } else { "" },
        value.replace('"', "\"\"")
    ))
}
impl Report {
    pub fn csv(&self) -> Result<Vec<u8>> {
        let meta = [
            "Company",
            "Report",
            "From",
            "To",
            "Generated at (UTC)",
            "Company time zone",
        ];
        let mut text = String::from("\u{feff}");
        let mut headers = meta.to_vec();
        headers.extend(self.headers.iter().copied());
        text.push_str(
            &headers
                .into_iter()
                .map(csv_cell)
                .collect::<Result<Vec<_>>>()?
                .join(","),
        );
        text.push_str("\r\n");
        let common = [
            self.company.clone(),
            self.filter.kind.label().into(),
            self.filter.starts_on.to_string(),
            self.filter.ends_on.to_string(),
            self.generated_at.to_rfc3339(),
            self.timezone.clone(),
        ];
        let common_csv = common
            .iter()
            .map(|s| csv_cell(s))
            .collect::<Result<Vec<_>>>()?
            .join(",");
        for row in &self.rows {
            ensure!(
                row.len() == self.headers.len(),
                "The report columns do not match."
            );
            text.push_str(&common_csv);
            text.push(',');
            text.push_str(
                &row.iter()
                    .map(|s| csv_cell(s))
                    .collect::<Result<Vec<_>>>()?
                    .join(","),
            );
            text.push_str("\r\n");
        }
        Ok(text.into_bytes())
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        ensure!(
            path.extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("csv")),
            "Use a .csv filename for this report."
        );
        let bytes = self.csv()?;
        let mut file=std::fs::OpenOptions::new().write(true).create_new(true).open(path).context("Could not create the report. Choose a new filename in an existing writable folder; existing files are preserved.")?;
        if let Err(error) = file.write_all(&bytes).and_then(|_| file.sync_all()) {
            drop(file);
            let _ = std::fs::remove_file(path);
            return Err(error).context("The report could not be saved completely.");
        }
        Ok(())
    }
}

#[cfg(windows)]
pub fn choose_export(filename: &str) -> Result<Option<PathBuf>> {
    use std::{ffi::OsString, os::windows::ffi::OsStringExt};
    use windows_sys::Win32::UI::{Controls::Dialogs::*, Input::KeyboardAndMouse::GetActiveWindow};
    let mut path = vec![0u16; 32768];
    for (slot, c) in path.iter_mut().zip(filename.encode_utf16()) {
        *slot = c
    }
    let filter: Vec<u16> = "CSV spreadsheet (*.csv)\0*.csv\0\0"
        .encode_utf16()
        .collect();
    let title: Vec<u16> = "Export HRIS report to a new file\0"
        .encode_utf16()
        .collect();
    let ext: Vec<u16> = "csv\0".encode_utf16().collect();
    // SAFETY: the owned buffers remain valid throughout the synchronous dialog.
    let mut dialog: OPENFILENAMEW = unsafe { std::mem::zeroed() };
    dialog.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    dialog.hwndOwner = unsafe { GetActiveWindow() };
    dialog.lpstrFilter = filter.as_ptr();
    dialog.lpstrTitle = title.as_ptr();
    dialog.lpstrDefExt = ext.as_ptr();
    dialog.lpstrFile = path.as_mut_ptr();
    dialog.nMaxFile = path.len() as u32;
    dialog.Flags = OFN_EXPLORER | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR | OFN_DONTADDTORECENT;
    if unsafe { GetSaveFileNameW(&mut dialog) } != 0 {
        let length = path.iter().position(|c| *c == 0).unwrap_or(path.len());
        Ok(Some(PathBuf::from(OsString::from_wide(&path[..length]))))
    } else {
        ensure!(
            unsafe { CommDlgExtendedError() } == 0,
            "Could not open the report file chooser."
        );
        Ok(None)
    }
}
#[cfg(not(windows))]
pub fn choose_export(_: &str) -> Result<Option<PathBuf>> {
    anyhow::bail!("Report file selection currently supports Windows only.")
}
