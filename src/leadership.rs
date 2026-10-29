//! Aggregate-only leadership oversight; no confidential or financial source records.
use crate::{access::Access, staff_roles::Capability};
use anyhow::{Result, ensure};
use chrono::{DateTime, Datelike, Duration, Months, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};
use uuid::Uuid;

pub fn eligible(a: &Access) -> bool {
    a.uses_desktop() && a.allows(Capability::AccountAdministration)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Filter {
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
}
impl Filter {
    pub fn new(today: NaiveDate) -> Self {
        Self {
            starts_on: today.with_day(1).unwrap(),
            ends_on: today,
        }
    }
    pub fn validate(&self, today: NaiveDate) -> Result<()> {
        ensure!(
            (1900..=2200).contains(&self.starts_on.year())
                && self.starts_on <= self.ends_on
                && self.ends_on <= today
                && (self.ends_on - self.starts_on).num_days() < 366,
            "Choose up to 366 dates ending no later than company today."
        );
        Ok(())
    }
    pub fn filename(&self, ext: &str) -> String {
        format!("hr-leadership-{}-{}.{}", self.starts_on, self.ends_on, ext)
    }
    pub fn intervals(&self) -> Vec<(NaiveDate, NaiveDate)> {
        let mut result = Vec::new();
        let mut start = self.starts_on;
        while start <= self.ends_on {
            let next = start
                .with_day(1)
                .unwrap()
                .checked_add_months(Months::new(1))
                .unwrap();
            result.push((start, self.ends_on.min(next - Duration::days(1))));
            start = next;
        }
        result
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupBy {
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
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Workforce {
    pub active: u32,
    pub archived: u32,
    pub unknown_join: u32,
    pub future_join: u32,
    pub joined_in_period: u32,
    pub finalized_exits_in_period: u32,
    pub active_without_department: u32,
    pub active_without_location: u32,
    pub active_without_type: u32,
    pub active_without_manager: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Group {
    pub by: GroupBy,
    pub name: String,
    pub unassigned: bool,
    pub active: u32,
    pub archived: u32,
}
impl Group {
    pub fn label(&self) -> String {
        if self.unassigned {
            "Not set".into()
        } else if self.name == "Not set" {
            "Not set (recorded label)".into()
        } else {
            self.name.clone()
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Month {
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    pub joined: u32,
    pub finalized_exits: u32,
    pub attendance_completed: u32,
    pub attendance_open: u32,
    pub approved_leave_units: u32,
    pub pending_leave_units: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Metric {
    ApplicantsPipeline,
    OffersSent,
    AcceptedUnconverted,
    OnboardingOpen,
    ExitsOpen,
    GoalsActive,
    GoalsOverdue,
    ReviewsUnpublished,
    ReviewsOverdue,
    TrainingOpen,
    TrainingOverdue,
    TrainingCompletedInPeriod,
    CertificatesExpired,
    CertificatesExpiring30,
    CareerActive,
}
impl Metric {
    pub const ALL: [Self; 15] = [
        Self::ApplicantsPipeline,
        Self::OffersSent,
        Self::AcceptedUnconverted,
        Self::OnboardingOpen,
        Self::ExitsOpen,
        Self::GoalsActive,
        Self::GoalsOverdue,
        Self::ReviewsUnpublished,
        Self::ReviewsOverdue,
        Self::TrainingOpen,
        Self::TrainingOverdue,
        Self::TrainingCompletedInPeriod,
        Self::CertificatesExpired,
        Self::CertificatesExpiring30,
        Self::CareerActive,
    ];
    pub fn section(self) -> &'static str {
        match self {
            Self::ApplicantsPipeline
            | Self::OffersSent
            | Self::AcceptedUnconverted
            | Self::OnboardingOpen
            | Self::ExitsOpen => "Hiring & transitions",
            _ => "Learning & Performance",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::ApplicantsPipeline => "Applicants in active stages",
            Self::OffersSent => "Sent offers awaiting response",
            Self::AcceptedUnconverted => "Accepted offers awaiting hire conversion",
            Self::OnboardingOpen => "Open onboarding cases",
            Self::ExitsOpen => "Open / cleared exit cases",
            Self::GoalsActive => "Active goals",
            Self::GoalsOverdue => "Active goals past due",
            Self::ReviewsUnpublished => "Draft / submitted appraisals",
            Self::ReviewsOverdue => "Unpublished appraisals past due",
            Self::TrainingOpen => "Planned / in-progress training",
            Self::TrainingOverdue => "Open training past due",
            Self::TrainingCompletedInPeriod => "Training completed in selected period",
            Self::CertificatesExpired => "Expired recorded certifications",
            Self::CertificatesExpiring30 => "Certifications expiring within 30 days",
            Self::CareerActive => "Active career plans",
        }
    }
    pub fn basis(self) -> &'static str {
        match self {
            Self::ApplicantsPipeline => {
                "Current non-archived applicants in applied, screening, interview or offer stage"
            }
            Self::AcceptedUnconverted => {
                "Current accepted offers without a recorded hire conversion"
            }
            Self::GoalsOverdue | Self::ReviewsOverdue | Self::TrainingOverdue => {
                "Current open status; recorded due date strictly before company today"
            }
            Self::TrainingCompletedInPeriod => {
                "Current completed status and HR-recorded completion date in selected period"
            }
            Self::CertificatesExpired => {
                "Current valid, non-archived, already-issued certificates with expiry before company today"
            }
            Self::CertificatesExpiring30 => {
                "Current valid, non-archived, already-issued certificates expiring from today through today + 30 days"
            }
            _ => {
                "Current saved workflow state across all retained records; not a historical state reconstruction"
            }
        }
    }
    fn source(self) -> &'static str {
        match self {
            Self::ApplicantsPipeline => "applicants",
            Self::OffersSent | Self::AcceptedUnconverted => "offers",
            Self::OnboardingOpen => "onboarding",
            Self::ExitsOpen => "exits",
            Self::GoalsActive | Self::GoalsOverdue => "goals",
            Self::ReviewsUnpublished | Self::ReviewsOverdue => "reviews",
            Self::TrainingOpen | Self::TrainingOverdue | Self::TrainingCompletedInPeriod => {
                "training"
            }
            Self::CertificatesExpired | Self::CertificatesExpiring30 => "certificates",
            Self::CareerActive => "career",
        }
    }
}
pub const SOURCES: [&str; 14] = [
    "employees",
    "attendance",
    "leave_requests",
    "leave_charge_dates",
    "applicants",
    "offers",
    "hire_conversions",
    "onboarding",
    "exits",
    "goals",
    "reviews",
    "training",
    "certificates",
    "career",
];
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub schema_version: u16,
    pub company_id: Uuid,
    pub user_id: Uuid,
    pub company_name: String,
    pub generated_at: DateTime<Utc>,
    pub today: NaiveDate,
    pub time_zone: String,
    pub filter: Filter,
    pub source_counts: BTreeMap<String, u32>,
    pub workforce: Workforce,
    pub groups: Vec<Group>,
    pub months: Vec<Month>,
    pub workload: BTreeMap<Metric, u32>,
}
fn label(s: &str, max: usize) -> bool {
    s.trim() == s && s.chars().count() <= max && !s.chars().any(char::is_control)
}
impl Report {
    pub fn validate(&self, a: &Access, filter: &Filter) -> Result<()> {
        ensure!(
            eligible(a)
                && self.schema_version == 1
                && self.company_id == a.company_id
                && self.user_id == a.user_id
                && !a.user_id.is_nil()
                && !a.company_id.is_nil(),
            "Current Administrator leadership access is required."
        );
        ensure!(
            self.filter == *filter,
            "Leadership report does not match the requested dates."
        );
        self.filter.validate(self.today)?;
        let tz: chrono_tz::Tz = self.time_zone.parse()?;
        ensure!(
            self.generated_at.with_timezone(&tz).date_naive() == self.today
                && label(&self.company_name, 100)
                && !self.company_name.is_empty(),
            "Leadership generation metadata is invalid."
        );
        ensure!(
            self.source_counts.len() == SOURCES.len()
                && SOURCES
                    .iter()
                    .all(|k| self.source_counts.get(*k).is_some_and(|v| *v <= 50_000)),
            "Leadership source limits are invalid; narrow the report."
        );
        let w = &self.workforce;
        let total = u64::from(w.active) + u64::from(w.archived);
        ensure!(
            total == u64::from(self.source_counts["employees"])
                && u64::from(w.unknown_join)
                    + u64::from(w.future_join)
                    + u64::from(w.joined_in_period)
                    <= total
                && w.finalized_exits_in_period <= self.source_counts["exits"],
            "Workforce totals do not agree."
        );
        ensure!(
            [
                w.active_without_department,
                w.active_without_location,
                w.active_without_type,
                w.active_without_manager
            ]
            .iter()
            .all(|n| *n <= w.active),
            "Workforce quality counts are invalid."
        );
        ensure!(
            self.groups.len() <= 600,
            "Too many leadership groups; report blocked."
        );
        let mut seen = BTreeSet::new();
        for g in &self.groups {
            ensure!(
                label(&g.name, 80)
                    && g.unassigned == g.name.is_empty()
                    && (u64::from(g.active) + u64::from(g.archived)) > 0
                    && seen.insert((g.by, g.unassigned, g.name.clone())),
                "Leadership group labels or totals are invalid."
            );
        }
        for by in GroupBy::ALL {
            ensure!(
                self.groups
                    .iter()
                    .filter(|g| g.by == by)
                    .map(|g| u64::from(g.active))
                    .sum::<u64>()
                    == u64::from(w.active)
                    && self
                        .groups
                        .iter()
                        .filter(|g| g.by == by)
                        .map(|g| u64::from(g.archived))
                        .sum::<u64>()
                        == u64::from(w.archived),
                "Leadership groups do not reconcile to current employee records."
            );
        }
        let intervals = self.filter.intervals();
        ensure!(
            self.months.len() == intervals.len(),
            "Leadership monthly periods are incomplete."
        );
        let mut sums = [0u64; 6];
        for (m, (start, end)) in self.months.iter().zip(intervals) {
            ensure!(
                m.starts_on == start && m.ends_on == end,
                "Leadership monthly periods are invalid."
            );
            for (i, v) in [
                m.joined,
                m.finalized_exits,
                m.attendance_completed,
                m.attendance_open,
                m.approved_leave_units,
                m.pending_leave_units,
            ]
            .into_iter()
            .enumerate()
            {
                sums[i] += u64::from(v);
            }
        }
        ensure!(
            sums[0] == u64::from(w.joined_in_period)
                && sums[1] == u64::from(w.finalized_exits_in_period)
                && sums[2] + sums[3] == u64::from(self.source_counts["attendance"])
                && sums[4] + sums[5] <= 2 * u64::from(self.source_counts["leave_charge_dates"]),
            "Leadership monthly totals do not reconcile."
        );
        ensure!(
            self.workload.len() == Metric::ALL.len()
                && Metric::ALL.iter().all(|m| self
                    .workload
                    .get(m)
                    .is_some_and(|n| *n <= self.source_counts[m.source()])),
            "Leadership workload totals are invalid."
        );
        for (a, b) in [
            (Metric::GoalsOverdue, Metric::GoalsActive),
            (Metric::ReviewsOverdue, Metric::ReviewsUnpublished),
            (Metric::TrainingOverdue, Metric::TrainingOpen),
        ] {
            ensure!(
                self.workload[&a] <= self.workload[&b],
                "Overdue counts exceed their open workflow totals."
            );
        }
        ensure!(
            u64::from(self.workload[&Metric::CertificatesExpired])
                + u64::from(self.workload[&Metric::CertificatesExpiring30])
                <= u64::from(self.source_counts["certificates"]),
            "Certification counts overlap."
        );
        Ok(())
    }
    pub fn rows(&self) -> Vec<Vec<String>> {
        let mut rows = Vec::new();
        let mut add = |section: &str,
                       metric: &str,
                       group: String,
                       start: NaiveDate,
                       end: NaiveDate,
                       value: String,
                       unit: &str,
                       basis: &str| {
            rows.push(vec![
                self.company_name.clone(),
                self.generated_at.to_rfc3339(),
                self.time_zone.clone(),
                self.today.to_string(),
                section.into(),
                metric.into(),
                group,
                start.to_string(),
                end.to_string(),
                value,
                unit.into(),
                basis.into(),
            ])
        };
        let w = &self.workforce;
        for (label, n, basis) in [
            (
                "Active employee records",
                w.active,
                "Recorded status now; future joining dates do not alter status",
            ),
            (
                "Archived employee records",
                w.archived,
                "Recorded status now; no inferred separation date",
            ),
            (
                "Unknown joining dates",
                w.unknown_join,
                "All retained employee records with no recorded joining date",
            ),
            (
                "Future joining dates",
                w.future_join,
                "All retained employee records with joining date after company today",
            ),
            (
                "Active records without department",
                w.active_without_department,
                "Current active records; blank recorded department",
            ),
            (
                "Active records without location",
                w.active_without_location,
                "Current active records; no assigned work location",
            ),
            (
                "Active records without employment type",
                w.active_without_type,
                "Current active records; no assigned employment type",
            ),
            (
                "Active records without manager",
                w.active_without_manager,
                "Current active records; no reporting manager; this may be intentional",
            ),
        ] {
            add(
                "Current workforce",
                label,
                String::new(),
                self.today,
                self.today,
                n.to_string(),
                "records",
                basis,
            );
        }
        for g in &self.groups {
            for (metric, n) in [("Active", g.active), ("Archived", g.archived)] {
                add(
                    g.by.label(),
                    metric,
                    g.label(),
                    self.today,
                    self.today,
                    n.to_string(),
                    "records",
                    "Current labels and recorded status; not historical grouping",
                );
            }
        }
        for m in &self.months {
            for (metric, n, unit, basis) in [
                (
                    "Recorded joining dates",
                    m.joined,
                    "records",
                    "Current recorded joining date in selected interval; not verified hire events",
                ),
                (
                    "Finalized exits",
                    m.finalized_exits,
                    "cases",
                    "Current finalized case; recorded last-working date in selected interval; archiving alone excluded",
                ),
                (
                    "Completed attendance",
                    m.attendance_completed,
                    "employee-days",
                    "Recorded work dates with check-out; missing rows are not absences",
                ),
                (
                    "Open attendance",
                    m.attendance_open,
                    "employee-days",
                    "Recorded work dates without check-out",
                ),
                (
                    "Approved leave",
                    m.approved_leave_units,
                    "half-day units",
                    "Immutable saved charge dates; current approved state",
                ),
                (
                    "Pending leave",
                    m.pending_leave_units,
                    "half-day units",
                    "Immutable saved charge dates; current pending state",
                ),
            ] {
                add(
                    "Selected period",
                    metric,
                    String::new(),
                    m.starts_on,
                    m.ends_on,
                    n.to_string(),
                    unit,
                    basis,
                );
            }
        }
        for m in Metric::ALL {
            let period = m == Metric::TrainingCompletedInPeriod;
            add(
                m.section(),
                m.label(),
                String::new(),
                if period {
                    self.filter.starts_on
                } else {
                    self.today
                },
                if period {
                    self.filter.ends_on
                } else {
                    self.today
                },
                self.workload[&m].to_string(),
                "records",
                m.basis(),
            );
        }
        rows
    }
    pub fn export_bytes(&self, a: &Access, path: &Path) -> Result<Vec<u8>> {
        self.validate(a, &self.filter)?;
        let headers = [
            "Company",
            "Generated at (UTC)",
            "Company time zone",
            "Current snapshot date",
            "Section",
            "Metric",
            "Group",
            "Period start",
            "Period end",
            "Value",
            "Unit",
            "Basis / limitation",
        ];
        let rows = self.rows();
        if crate::tabular::is_xlsx(path) {
            let mut kinds = vec![crate::tabular::ColumnKind::Text; headers.len()];
            kinds[3] = crate::tabular::ColumnKind::Date;
            kinds[7] = crate::tabular::ColumnKind::Date;
            kinds[8] = crate::tabular::ColumnKind::Date;
            kinds[9] = crate::tabular::ColumnKind::Integer;
            crate::tabular::xlsx(&headers, &rows, &kinds)
        } else {
            ensure!(
                path.extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("csv")),
                "Choose CSV or Excel (.xlsx)."
            );
            let mut s = String::from("\u{feff}");
            for row in std::iter::once(headers.iter().map(|s| s.to_string()).collect()).chain(rows)
            {
                s.push_str(
                    &row.iter()
                        .map(|v| crate::reports::csv_cell(v))
                        .collect::<Result<Vec<_>>>()?
                        .join(","),
                );
                s.push_str("\r\n");
            }
            Ok(s.into_bytes())
        }
    }
}
#[cfg(any(test, feature = "qa-screenshot"))]
pub fn preview(data: &crate::domain::Snapshot) -> Report {
    let today = data.company.today();
    let filter = Filter {
        starts_on: today.with_day(1).unwrap() - Duration::days(90),
        ends_on: today,
    };
    let mut sources: SMap = SOURCES.into_iter().map(|k| (k.into(), 0)).collect();
    sources.insert("employees".into(), 525);
    sources.insert("attendance".into(), 1200);
    sources.insert("exits".into(), 8);
    sources.insert("leave_charge_dates".into(), 125);
    sources.insert("leave_requests".into(), 80);
    let mut months: Vec<Month> = filter
        .intervals()
        .into_iter()
        .map(|(a, b)| Month {
            starts_on: a,
            ends_on: b,
            ..Default::default()
        })
        .collect();
    let last = months.last_mut().unwrap();
    last.joined = 12;
    last.finalized_exits = 3;
    last.attendance_completed = 1180;
    last.attendance_open = 20;
    last.approved_leave_units = 201;
    last.pending_leave_units = 49;
    let workforce = Workforce {
        active: 500,
        archived: 25,
        unknown_join: 2,
        future_join: 1,
        joined_in_period: 12,
        finalized_exits_in_period: 3,
        active_without_department: 4,
        active_without_location: 10,
        active_without_type: 8,
        active_without_manager: 6,
    };
    let groups = GroupBy::ALL
        .into_iter()
        .flat_map(|by| {
            [
                ("Operations", 400, 20),
                ("Shared services", 96, 5),
                ("", 4, 0),
            ]
            .map(move |(name, active, archived)| Group {
                by,
                name: name.into(),
                unassigned: name.is_empty(),
                active,
                archived,
            })
        })
        .collect();
    let mut workload = BTreeMap::new();
    for (k, m) in Metric::ALL.into_iter().enumerate() {
        sources.insert(m.source().into(), 100);
        workload.insert(
            m,
            if matches!(
                m,
                Metric::GoalsOverdue | Metric::ReviewsOverdue | Metric::TrainingOverdue
            ) {
                2
            } else {
                (k + 5) as u32
            },
        );
    }
    Report {
        schema_version: 1,
        company_id: data.access.company_id,
        user_id: data.access.user_id,
        company_name: data.company.name.clone(),
        generated_at: Utc::now(),
        today,
        time_zone: data.company.timezone.clone(),
        filter,
        source_counts: sources,
        workforce,
        groups,
        months,
        workload,
    }
}
#[cfg(any(test, feature = "qa-screenshot"))]
type SMap = BTreeMap<String, u32>;

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Access, Report) {
        let mut d = crate::domain::DemoStore::new().data;
        d.access.user_id = Uuid::new_v4();
        let r = preview(&d);
        r.validate(&d.access, &r.filter).unwrap();
        (d.access, r)
    }
    #[test]
    fn limits_clip_months_and_accept_leap_year_without_future_dates() {
        let today = NaiveDate::from_ymd_opt(2024, 12, 31).unwrap();
        let f = Filter {
            starts_on: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            ends_on: today,
        };
        f.validate(today).unwrap();
        assert_eq!(f.intervals().len(), 12);
        assert_eq!(f.intervals()[1].1.day(), 29);
        assert!(
            Filter {
                starts_on: f.starts_on - Duration::days(1),
                ..f
            }
            .validate(today)
            .is_err()
        );
        assert!(
            Filter {
                ends_on: today + Duration::days(1),
                ..f
            }
            .validate(today)
            .is_err()
        );
        assert!(
            Filter {
                starts_on: today,
                ends_on: today - Duration::days(1)
            }
            .validate(today)
            .is_err()
        );
        let short = Filter {
            starts_on: NaiveDate::from_ymd_opt(2024, 1, 31).unwrap(),
            ends_on: NaiveDate::from_ymd_opt(2024, 2, 1).unwrap(),
        };
        assert_eq!(
            short.intervals(),
            vec![
                (short.starts_on, short.starts_on),
                (short.ends_on, short.ends_on)
            ]
        );
    }
    #[test]
    fn only_current_administrator_company_actor_and_filter_bindings_are_accepted() {
        let (a, r) = fixture();
        for role in [
            crate::access::Role::HrGeneralist,
            crate::access::Role::Manager,
            crate::access::Role::Employee,
        ] {
            let mut other = a.clone();
            other.role = role;
            assert!(!eligible(&other));
            assert!(r.validate(&other, &r.filter).is_err());
        }
        let mut other = a.clone();
        other.company_id = Uuid::new_v4();
        assert!(r.validate(&other, &r.filter).is_err());
        other = a.clone();
        other.user_id = Uuid::new_v4();
        assert!(r.validate(&other, &r.filter).is_err());
        assert!(
            r.validate(
                &a,
                &Filter {
                    starts_on: r.filter.starts_on + Duration::days(1),
                    ..r.filter
                }
            )
            .is_err()
        );
    }
    #[test]
    fn unknown_fields_reject_private_source_injection() {
        let (_, r) = fixture();
        for key in [
            "employee_id",
            "case_notes",
            "dependents",
            "salary",
            "applicant_name",
        ] {
            let mut v = serde_json::to_value(&r).unwrap();
            v[key] = serde_json::json!("Must not be projected");
            assert!(serde_json::from_value::<Report>(v).is_err());
        }
    }
    #[test]
    fn inconsistent_totals_groups_budgets_and_periods_fail_closed() {
        let (a, r) = fixture();
        let mut cases = Vec::new();
        let mut x = r.clone();
        x.workforce.active += 1;
        cases.push(x);
        let mut x = r.clone();
        x.groups[0].active += 1;
        cases.push(x);
        let mut x = r.clone();
        x.groups.push(x.groups[0].clone());
        cases.push(x);
        let mut x = r.clone();
        x.months.remove(0);
        cases.push(x);
        let mut x = r.clone();
        x.months[0].ends_on += Duration::days(1);
        cases.push(x);
        let mut x = r.clone();
        x.months[0].approved_leave_units = 100_001;
        cases.push(x);
        let mut x = r.clone();
        x.source_counts.insert("attendance".into(), 50_001);
        cases.push(x);
        let mut x = r.clone();
        x.workload.insert(Metric::GoalsOverdue, 99);
        cases.push(x);
        let mut x = r.clone();
        x.workload.remove(&Metric::CareerActive);
        cases.push(x);
        let mut x = r.clone();
        x.today -= Duration::days(1);
        cases.push(x);
        let mut x = r.clone();
        x.groups[0].name = "\nUnexpected".into();
        cases.push(x);
        for bad in cases {
            assert!(bad.validate(&a, &r.filter).is_err());
        }
    }
    #[test]
    fn assigned_not_set_labels_are_distinct_and_source_ids_are_absent_from_exports() {
        let (a, mut r) = fixture();
        r.groups[0].name = "Not set".into();
        assert_eq!(r.groups[0].label(), "Not set (recorded label)");
        let csv = String::from_utf8(r.export_bytes(&a, Path::new("report.csv")).unwrap()).unwrap();
        for private in [
            a.company_id.to_string(),
            a.user_id.to_string(),
            "salary".into(),
            "case_notes".into(),
            "employee_id".into(),
        ] {
            assert!(!csv.contains(&private));
        }
        assert!(csv.contains("half-day units"));
        assert!(csv.contains("Current recorded status") || csv.contains("Recorded status now"));
    }
    #[test]
    fn csv_formula_groups_are_neutralized_and_excel_is_literal() {
        let (a, mut r) = fixture();
        r.groups[0].name = "=1+1".into();
        let csv = String::from_utf8(r.export_bytes(&a, Path::new("report.csv")).unwrap()).unwrap();
        assert!(csv.contains("'=1+1"));
        let bytes = r.export_bytes(&a, Path::new("report.xlsx")).unwrap();
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        use std::io::Read;
        let mut xml = String::new();
        zip.by_name("xl/worksheets/sheet1.xml")
            .unwrap()
            .read_to_string(&mut xml)
            .unwrap();
        assert!(!xml.contains("<f>"));
        assert!(r.export_bytes(&a, Path::new("report.txt")).is_err());
    }
    #[test]
    #[ignore = "requires fresh local SQL vectors"]
    fn postgres_projection_decodes_without_private_fields() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("work/leadership-db-vectors");
        let mut n = 0;
        for e in std::fs::read_dir(root).unwrap() {
            let p = e.unwrap().path();
            if p.extension().is_some_and(|e| e == "json") {
                let r: Report = serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
                let mut a = crate::domain::DemoStore::new().data.access;
                a.user_id = r.user_id;
                a.company_id = r.company_id;
                r.validate(&a, &r.filter).unwrap();
                r.export_bytes(&a, Path::new("r.csv")).unwrap();
                r.export_bytes(&a, Path::new("r.xlsx")).unwrap();
                n += 1;
            }
        }
        assert!(n >= 3);
    }
}
