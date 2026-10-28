//! Recorded regional basic-rate comparisons. No salary or tax mutation.
use crate::{
    access::Role,
    domain::{Mode, Snapshot},
    payroll::{Pay, PayrollAction},
};
use anyhow::{Context, Result, ensure};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::OnceLock;
use uuid::Uuid;
pub const VERSION: &str = "PH-regional-wages-2026-10-24-v1";
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Rate {
    pub id: String,
    pub series_key: String,
    pub region: String,
    pub label: String,
    pub area: String,
    pub order: String,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    pub daily_basic_cents: i64,
    pub source_url: String,
    pub order_url: String,
    pub note: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Catalog {
    pub version: String,
    pub verified_on: NaiveDate,
    pub supported_until: NaiveDate,
    pub scope: String,
    pub regional_notes: Vec<String>,
    pub rates: Vec<Rate>,
}
pub fn catalog() -> &'static Catalog {
    static DATA: OnceLock<Catalog> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("data/ph-regional-wages-2026-10-24.json"))
            .expect("Verified bundled wage catalog")
    })
}
pub fn series() -> Vec<&'static Rate> {
    let mut keys = std::collections::BTreeSet::new();
    catalog()
        .rates
        .iter()
        .filter(|r| keys.insert(r.series_key.as_str()))
        .collect()
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Classification {
    pub id: Uuid,
    pub company_id: Uuid,
    pub employee_id: Uuid,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    pub coverage: String,
    pub series_key: String,
    pub workplace: String,
    pub evidence: String,
    pub note: String,
    pub employee_basis: Value,
    pub status: String,
    pub revision: i64,
    pub created_by: Uuid,
    pub prepared_by: Uuid,
    pub approved_by: Option<Uuid>,
    pub approved_at: Option<DateTime<Utc>>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
    pub id: Uuid,
    pub company_id: Uuid,
    pub enforced: bool,
    pub revision: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Check {
    pub id: Uuid,
    pub company_id: Uuid,
    pub run_id: Uuid,
    pub kind: String,
    pub stage: String,
    pub run_revision: i64,
    pub enforced: bool,
    pub assessment: Value,
    pub recorded_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Default)]
pub struct Setup {
    pub classifications: Vec<Classification>,
    pub settings: Vec<Settings>,
    pub checks: Vec<Check>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BasicPay {
    pub basis: String,
    pub rate_cents: i64,
    pub annual_divisor: u32,
}
impl From<&Pay> for BasicPay {
    fn from(p: &Pay) -> Self {
        Self {
            basis: p.basis.clone(),
            rate_cents: p.rate_cents,
            annual_divisor: p.annual_divisor,
        }
    }
}
impl BasicPay {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            ["daily", "monthly"].contains(&self.basis.as_str())
                && (1..=10_000_000_000).contains(&self.rate_cents)
                && [365, 366].contains(&self.annual_divisor),
            "Check the recorded basic salary and true-monthly divisor."
        );
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub company_id: Uuid,
    pub employee_id: Uuid,
    pub pay: BasicPay,
    pub employee_basis: Value,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    pub classifications: Vec<Classification>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Day {
    pub date: NaiveDate,
    pub status: String,
    pub classification_id: Option<Uuid>,
    pub classification_revision: Option<i64>,
    pub rate_id: Option<String>,
    pub floor_daily_cents: Option<i64>,
    pub required_salary_cents: Option<i64>,
    pub gap_salary_cents: Option<i64>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Assessment {
    pub version: String,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    pub blocking_days: u32,
    pub exemption_days: u32,
    pub max_required_salary_cents: i64,
    pub max_gap_salary_cents: i64,
    pub days: Vec<Day>,
}
pub fn employee_basis(data: &Snapshot, eid: Uuid) -> Result<Value> {
    let e = data
        .employees
        .iter()
        .find(|e| e.id == eid && e.company_id == data.company.id)
        .context("Employee unavailable.")?;
    Ok(json!({"work_location_id":e.work_location_id,"employment_type_id":e.employment_type_id}))
}
pub fn calculate(s: &Source) -> Result<Assessment> {
    s.pay.validate()?;
    ensure!(
        !s.company_id.is_nil()
            && !s.employee_id.is_nil()
            && s.starts_on <= s.ends_on
            && s.starts_on.year() == 2026
            && s.ends_on.year() == 2026,
        "Use a bounded2026 wage-review period."
    );
    ensure!(
        s.classifications.len() <= 128
            && s.classifications
                .iter()
                .all(|c| c.company_id == s.company_id && c.employee_id == s.employee_id),
        "Wage classification outside this person/company."
    );
    let mut a = Assessment {
        version: VERSION.into(),
        starts_on: s.starts_on,
        ends_on: s.ends_on,
        blocking_days: 0,
        exemption_days: 0,
        max_required_salary_cents: 0,
        max_gap_salary_cents: 0,
        days: vec![],
    };
    let mut on = s.starts_on;
    loop {
        let mut d = Day {
            date: on,
            status: "missing_classification".into(),
            classification_id: None,
            classification_revision: None,
            rate_id: None,
            floor_daily_cents: None,
            required_salary_cents: None,
            gap_salary_cents: None,
        };
        let found: Vec<_> = s
            .classifications
            .iter()
            .filter(|c| c.status == "approved" && c.starts_on <= on && c.ends_on >= on)
            .collect();
        if found.len() > 1 {
            d.status = "overlapping_classifications".into();
        } else if let Some(c) = found.first() {
            d.classification_id = Some(c.id);
            d.classification_revision = Some(c.revision);
            if c.approved_by.is_none()
                || c.approved_at.is_none()
                || c.approved_by == Some(c.created_by)
                || c.approved_by == Some(c.prepared_by)
            {
                d.status = "unreviewed_classification".into();
            } else if c.employee_basis != s.employee_basis {
                d.status = "workplace_or_employment_changed".into();
            } else if c.coverage == "exempt" {
                if c.evidence.trim().is_empty() || c.note.trim().is_empty() {
                    d.status = "unreviewed_exemption".into();
                } else {
                    d.status = "documented_exemption".into();
                    a.exemption_days += 1;
                }
            } else if c.coverage == "covered" {
                let rates: Vec<_> = catalog()
                    .rates
                    .iter()
                    .filter(|r| {
                        r.series_key == c.series_key && r.starts_on <= on && r.ends_on >= on
                    })
                    .collect();
                d.status = "rate_not_verified".into();
                if let [rate] = rates.as_slice() {
                    let (n, den) = if s.pay.basis == "monthly" {
                        (
                            i128::from(rate.daily_basic_cents) * i128::from(s.pay.annual_divisor),
                            12_i128,
                        )
                    } else {
                        (i128::from(rate.daily_basic_cents), 1_i128)
                    };
                    let required = i64::try_from((n + den - 1) / den)?;
                    let gap = (required - s.pay.rate_cents).max(0);
                    d.status = if i128::from(s.pay.rate_cents) * den >= n {
                        "meets_recorded_floor"
                    } else {
                        "below_recorded_floor"
                    }
                    .into();
                    d.rate_id = Some(rate.id.clone());
                    d.floor_daily_cents = Some(rate.daily_basic_cents);
                    d.required_salary_cents = Some(required);
                    d.gap_salary_cents = Some(gap);
                    a.max_required_salary_cents = a.max_required_salary_cents.max(required);
                    a.max_gap_salary_cents = a.max_gap_salary_cents.max(gap);
                }
            } else {
                d.status = "nonstandard_coverage_needs_review".into();
            }
        }
        if !["meets_recorded_floor", "documented_exemption"].contains(&d.status.as_str()) {
            a.blocking_days += 1;
        }
        a.days.push(d);
        if on == s.ends_on {
            break;
        }
        on = on.succ_opt().context("Wage date overflow")?;
    }
    Ok(a)
}
pub fn source(
    data: &Snapshot,
    eid: Uuid,
    pay: &Pay,
    lo: NaiveDate,
    hi: NaiveDate,
) -> Result<Source> {
    let e = data
        .employees
        .iter()
        .find(|e| e.id == eid && e.company_id == data.company.id)
        .context("Employee unavailable.")?;
    let lo = lo.max(e.joined_on.unwrap_or(lo));
    Ok(Source {
        company_id: data.company.id,
        employee_id: eid,
        pay: BasicPay::from(pay),
        employee_basis: employee_basis(data, eid)?,
        starts_on: lo,
        ends_on: hi,
        classifications: data
            .payroll
            .wages
            .classifications
            .iter()
            .filter(|c| {
                c.company_id == data.company.id
                    && c.employee_id == eid
                    && c.status == "approved"
                    && c.starts_on <= hi
                    && c.ends_on >= lo
            })
            .cloned()
            .collect(),
    })
}
pub fn current(data: &Snapshot, eid: Uuid, lo: NaiveDate, hi: NaiveDate) -> Result<Assessment> {
    let p = data
        .payroll
        .profile(eid, lo)
        .context("No effective salary profile.")?;
    calculate(&source(data, eid, &p.pay, lo, hi)?)
}
pub fn enabled(data: &Snapshot) -> bool {
    data.payroll
        .wages
        .settings
        .iter()
        .any(|s| s.id == data.company.id && s.company_id == data.company.id && s.enforced)
}
pub fn payload(a: &PayrollAction, data: &Snapshot) -> Result<Value> {
    ensure!(
        data.mode != Mode::SignedOut
            && data.access.company_id == data.company.id
            && data
                .access
                .allows(crate::staff_roles::Capability::PayrollPreparation),
        "Only HR/Admin manage wage reviews."
    );
    ensure!(
        a.record.is_object() && a.record.to_string().len() <= 16384,
        "Use bounded wage details."
    );
    let id: Uuid =
        serde_json::from_value(a.record["id"].clone()).context("Wage record identity required.")?;
    ensure!(!id.is_nil(), "Wage identity required.");
    crate::payroll::text(a.record["reason"].as_str().unwrap_or(""), 500, true)?;
    ensure!(
        a.record["catalog_version"] == VERSION,
        "Refresh the wage catalog before changing a review."
    );
    let op = a
        .operation
        .strip_prefix("wage_")
        .context("Wage operation required.")?;
    ensure!(
        ["create", "save", "approve", "retire", "cancel", "settings"].contains(&op),
        "Unsupported wage operation."
    );
    if ["approve", "retire", "settings"].contains(&op) {
        ensure!(
            data.access.role == Role::Administrator,
            "Only Administrators approve/retire wage reviews or configure enforcement."
        );
    }
    if ["create", "save"].contains(&op) {
        let lo: NaiveDate = serde_json::from_value(a.record["starts_on"].clone())
            .context("Enter a valid effective start.")?;
        let hi: NaiveDate = serde_json::from_value(a.record["ends_on"].clone())
            .context("Enter a valid effective end.")?;
        ensure!(
            lo <= hi && lo.year() == 2026 && hi.year() == 2026,
            "Use a2026 effective interval."
        );
        for key in ["workplace", "evidence", "note"] {
            crate::payroll::text(
                a.record[key].as_str().unwrap_or(""),
                if key == "workplace" { 240 } else { 1000 },
                true,
            )?;
        }
        let coverage = a.record["coverage"].as_str().unwrap_or("");
        let series = a.record["series_key"].as_str().unwrap_or("");
        ensure!(
            ["covered", "exempt"].contains(&coverage),
            "Choose an evidenced coverage."
        );
        ensure!(
            if coverage == "covered" {
                catalog().rates.iter().any(|r| r.series_key == series)
            } else {
                series.is_empty()
            },
            "Choose a verified category, or leave it empty for an evidenced exemption."
        );
        if op == "create" {
            let eid: Uuid = serde_json::from_value(a.record["employee_id"].clone())
                .context("Choose an employee.")?;
            employee_basis(data, eid)?;
        }
    }
    if ["save", "approve", "retire", "cancel"].contains(&op) {
        let c = data
            .payroll
            .wages
            .classifications
            .iter()
            .find(|c| c.id == id && c.company_id == data.company.id)
            .context("Wage review unavailable.")?;
        ensure!(
            a.record["expected_revision"].as_i64() == Some(c.revision),
            "Wage review changed. Refresh."
        );
        if op == "approve" {
            ensure!(
                c.status == "draft"
                    && ![c.created_by, c.prepared_by].contains(&data.access.user_id),
                "A different Administrator must approve this wage review."
            );
        }
    }
    if op == "settings" {
        ensure!(
            id == data.company.id && a.record["enforced"].is_boolean(),
            "Choose the company enforcement state."
        );
    }
    Ok(json!({"p_operation":op,"p_record":a.record}))
}
mod workflow;
pub use workflow::{apply_demo, record_run_check};
