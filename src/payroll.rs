//! Philippine private-sector monthly payroll. All amounts are integer centavos.
use crate::{
    access::Role,
    domain::{Mode, Snapshot},
};
use anyhow::{Context, Result, ensure};
use chrono::{DateTime, Datelike, Duration, NaiveDate, Timelike, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

pub const RULE_VERSION: &str = "PH-private-2026-10-23-v2";
pub const SOURCES: &[(&str, &str)] = &[
    (
        "DOLE pay rules",
        "https://nwpc.dole.gov.ph/wp-content/uploads/2024/11/Workers-Statutory-Monetary-Benefits-Handbook-2024-Edition.pdf",
    ),
    (
        "SSS 2024-006",
        "https://www.sss.gov.ph/wp-content/uploads/2024/12/CI-2024-006-Publication.pdf",
    ),
    (
        "PhilHealth 2025-0002",
        "https://www.philhealth.gov.ph/advisories/2025/PA2025-0002.pdf",
    ),
    (
        "Pag-IBIG / DBM 2024-2",
        "https://www.dbm.gov.ph/wp-content/uploads/Issuances/2024/Circular-Letter/CIRCULAR-LETTER-NO-2024-2-DATED-FEBRUARY-01-2024.pdf",
    ),
    (
        "BIR withholding",
        "https://bir-cdn.bir.gov.ph/local/pdf/Annex%20E%20RR%2011-2018.pdf",
    ),
    (
        "BIR de minimis",
        "https://bir-cdn.bir.gov.ph/BIR/pdf/RR%20No.%2029-2025%20digest%20FINAL.pdf",
    ),
];
pub fn money(cents: i64) -> String {
    format!(
        "PHP {}{}.{:02}",
        if cents < 0 { "-" } else { "" },
        cents.unsigned_abs() / 100,
        cents.unsigned_abs() % 100
    )
}
pub fn month_end(d: NaiveDate) -> NaiveDate {
    d.with_day(1)
        .unwrap()
        .checked_add_months(chrono::Months::new(1))
        .unwrap()
        .pred_opt()
        .unwrap()
}
pub fn month_start(d: NaiveDate) -> NaiveDate {
    d.with_day(1).unwrap()
}
pub fn ratio(v: i64, n: i64, d: i64) -> i64 {
    ((i128::from(v) * i128::from(n) + i128::from(d) / 2) / i128::from(d)) as i64
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Component {
    pub label: String,
    pub kind: String,
    pub amount_cents: i64,
    pub reference: String,
}
pub const COMPONENT_KINDS: &[&str] = &[
    "taxable_allowance",
    "rice",
    "laundry",
    "reimbursement",
    "deduction",
    "advance",
    "employer_benefit",
];
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Pay {
    pub basis: String,
    pub rate_cents: i64,
    pub annual_divisor: u32,
    pub monthly_reference_cents: i64,
    pub premium_eligible: bool,
    pub minimum_wage_earner: bool,
    pub sss: bool,
    pub philhealth: bool,
    pub pagibig: bool,
    pub coverage_note: String,
    pub components: Vec<Component>,
    pub tin: String,
    pub sss_number: String,
    pub philhealth_number: String,
    pub pagibig_number: String,
}
impl Default for Pay {
    fn default() -> Self {
        Self {
            basis: "monthly".into(),
            rate_cents: 0,
            annual_divisor: 365,
            monthly_reference_cents: 0,
            premium_eligible: true,
            minimum_wage_earner: false,
            sss: true,
            philhealth: true,
            pagibig: true,
            coverage_note: String::new(),
            components: vec![],
            tin: String::new(),
            sss_number: String::new(),
            philhealth_number: String::new(),
            pagibig_number: String::new(),
        }
    }
}
impl Pay {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            ["monthly", "daily"].contains(&self.basis.as_str())
                && (1..=100_000_000_00).contains(&self.rate_cents),
            "Enter a positive monthly or daily salary (maximum PHP100 million)."
        );
        ensure!(
            [365, 366].contains(&self.annual_divisor)
                && (0..=100_000_000_00).contains(&self.monthly_reference_cents),
            "Use a 365/366-day divisor for true monthly-paid salary and check the fixed monthly reference salary."
        );
        ensure!(
            self.basis != "daily" || self.monthly_reference_cents > 0,
            "Daily pay needs a fixed monthly reference salary for PhilHealth."
        );
        text(&self.coverage_note, 1000, false)?;
        ensure!(
            (self.premium_eligible
                && !self.minimum_wage_earner
                && self.sss
                && self.philhealth
                && self.pagibig)
                || !self.coverage_note.trim().is_empty(),
            "Document the legal basis for coverage exceptions or minimum-wage-earner status."
        );
        for s in [
            &self.tin,
            &self.sss_number,
            &self.philhealth_number,
            &self.pagibig_number,
        ] {
            text(s, 40, false)?;
        }
        components(&self.components)?;
        Ok(())
    }
    pub fn daily(&self) -> (i64, i64) {
        if self.basis == "monthly" {
            (self.rate_cents * 12, i64::from(self.annual_divisor))
        } else {
            (self.rate_cents, 1)
        }
    }
}
fn text(s: &str, max: usize, required: bool) -> Result<()> {
    ensure!(
        (!required || !s.trim().is_empty())
            && s.chars().count() <= max
            && !s.chars().any(|c| c.is_control() && c != '\n' && c != '\t'),
        "Check required text and length (maximum {max})."
    );
    Ok(())
}
fn components(cs: &[Component]) -> Result<()> {
    ensure!(cs.len() <= 40, "At most 40 components per list.");
    for c in cs {
        text(&c.label, 100, true)?;
        text(&c.reference, 500, false)?;
        ensure!(
            COMPONENT_KINDS.contains(&c.kind.as_str())
                && (0..=100_000_000_00).contains(&c.amount_cents),
            "Check component kind and positive amount."
        );
        ensure!(
            !["reimbursement", "deduction", "advance"].contains(&c.kind.as_str())
                || !c.reference.trim().is_empty(),
            "Reimbursements, voluntary deductions and advances require evidence/consent or a payment reference."
        );
    }
    Ok(())
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Day {
    pub date: NaiveDate,
    pub kind: String,
    pub rest: bool,
    pub regular_minutes: u32,
    pub overtime_minutes: u32,
    pub night_regular: u32,
    pub night_overtime: u32,
    pub paid_leave_minutes: u32,
    pub unworked_holiday_minutes: u32,
    pub unpaid_minutes: u32,
}
pub const DAY_KINDS: &[&str] = &[
    "ordinary",
    "special_working",
    "special",
    "double_special",
    "regular",
    "double_regular",
];
pub fn day_factor(kind: &str, rest: bool) -> Result<i64> {
    Ok(match (kind, rest) {
        ("ordinary" | "special_working", false) => 100,
        ("ordinary" | "special_working", true) => 130,
        ("special", false) => 130,
        ("special", true) => 150,
        ("double_special", false) => 150,
        ("double_special", true) => 195,
        ("regular", false) => 200,
        ("regular", true) => 260,
        ("double_regular", false) => 300,
        ("double_regular", true) => 390,
        _ => anyhow::bail!("Choose a supported payroll day classification."),
    })
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Worksheet {
    pub days: Vec<Day>,
    pub adjustments: Vec<Component>,
    pub basic_override_cents: Option<i64>,
    pub sss_basis_override: Option<i64>,
    pub philhealth_basis_override: Option<i64>,
    pub pagibig_basis_override: Option<i64>,
    pub exception_note: String,
    pub review_note: String,
    pub reviewed: bool,
}
impl Worksheet {
    pub fn validate(&self, p: &Pay, month: NaiveDate) -> Result<()> {
        ensure!(
            self.days.len() <= 31,
            "At most one reviewed row per calendar date."
        );
        let mut dates = std::collections::HashSet::new();
        for d in &self.days {
            day_factor(&d.kind, d.rest)?;
            ensure!(
                d.date >= month && d.date <= month_end(month) && dates.insert(d.date),
                "Use unique dates inside the payroll month."
            );
            ensure!(
                d.regular_minutes + d.paid_leave_minutes <= 480
                    && d.overtime_minutes <= 960
                    && d.night_regular <= d.regular_minutes
                    && d.night_overtime <= d.overtime_minutes
                    && d.unpaid_minutes <= 480
                    && d.unworked_holiday_minutes <= 480
                    && d.regular_minutes + d.paid_leave_minutes + d.unpaid_minutes <= 480,
                "Check the day's minutes, eight-hour regular limit and night subsets."
            );
            ensure!(
                d.unworked_holiday_minutes == 0
                    || (d.regular_minutes == 0
                        && d.overtime_minutes == 0
                        && d.paid_leave_minutes == 0
                        && d.unpaid_minutes == 0
                        && ["regular", "double_regular"].contains(&d.kind.as_str())),
                "Unworked holiday pay requires an unworked regular holiday and reviewed eligibility."
            );
        }
        components(&self.adjustments)?;
        text(&self.exception_note, 1000, false)?;
        text(&self.review_note, 2000, self.reviewed)?;
        for v in [
            self.basic_override_cents,
            self.sss_basis_override,
            self.philhealth_basis_override,
            self.pagibig_basis_override,
        ]
        .into_iter()
        .flatten()
        {
            ensure!(
                (0..=100_000_000_00).contains(&v) && !self.exception_note.trim().is_empty(),
                "Overrides need a bounded amount and a documented payroll review."
            );
        }
        ensure!(
            self.basic_override_cents
                .is_none_or(|v| p.basis == "monthly" && v <= p.rate_cents),
            "Basic override is a monthly proration no greater than the salary; use an adjustment for additional pay."
        );
        Ok(())
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Totals {
    pub basic: i64,
    pub paid_leave: i64,
    pub unworked_holiday: i64,
    pub premium: i64,
    pub overtime: i64,
    pub night: i64,
    pub allowances: i64,
    pub exempt_allowances: i64,
    pub reimbursement: i64,
    pub benefits: i64,
    pub voluntary_deductions: i64,
    pub advances: i64,
    pub gross: i64,
    pub taxable: i64,
    pub sss_basis: i64,
    pub sss_msc: i64,
    pub sss_employee: i64,
    pub sss_employer: i64,
    pub sss_regular_employee: i64,
    pub sss_regular_employer: i64,
    pub sss_mpf_employee: i64,
    pub sss_mpf_employer: i64,
    pub ec: i64,
    pub philhealth_employee: i64,
    pub philhealth_employer: i64,
    pub pagibig_employee: i64,
    pub pagibig_employer: i64,
    pub withholding: i64,
    pub net: i64,
    pub thirteenth_accrual: i64,
}
pub fn monthly_tax(t: i64) -> i64 {
    if t <= 2_083_300 {
        0
    } else if t < 3_333_300 {
        ratio(t - 2_083_300, 15, 100)
    } else if t < 6_666_700 {
        187_500 + ratio(t - 3_333_300, 20, 100)
    } else if t < 16_666_700 {
        854_180 + ratio(t - 6_666_700, 25, 100)
    } else if t < 66_666_700 {
        3_354_180 + ratio(t - 16_666_700, 30, 100)
    } else {
        18_354_180 + ratio(t - 66_666_700, 35, 100)
    }
}
pub fn calculate(p: &Pay, w: &Worksheet, month: NaiveDate) -> Result<Totals> {
    p.validate()?;
    w.validate(p, month)?;
    ensure!(
        month.day() == 1 && month >= NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(),
        "This rule pack supports monthly registers from February 2026."
    );
    let (dn, dd) = p.daily();
    let hr = |mins: i64, factor: i64| ratio(dn, mins * factor, dd * 480 * 100);
    let mut t = Totals::default();
    let monthly = p.basis == "monthly";
    if monthly {
        t.basic = w.basic_override_cents.unwrap_or(p.rate_cents);
    }
    for d in &w.days {
        let f = if p.premium_eligible {
            day_factor(&d.kind, d.rest)?
        } else {
            100
        };
        let ot = if f == 100 { 125 } else { 130 };
        if monthly {
            t.basic -= hr(i64::from(d.unpaid_minutes), 100);
            // One unworked day's pay is already in true monthly salary. The
            // second coincident regular holiday adds another eligible day.
            if p.premium_eligible && d.kind == "double_regular" {
                t.unworked_holiday += hr(i64::from(d.unworked_holiday_minutes), 100);
            }
        } else {
            t.basic += hr(i64::from(d.regular_minutes), 100);
            t.paid_leave += hr(i64::from(d.paid_leave_minutes), 100);
            if p.premium_eligible {
                t.unworked_holiday += hr(
                    i64::from(d.unworked_holiday_minutes),
                    if d.kind == "double_regular" { 200 } else { 100 },
                );
            }
        }
        t.premium += hr(i64::from(d.regular_minutes), f - 100);
        if p.premium_eligible {
            t.overtime += ratio(
                dn,
                i64::from(d.overtime_minutes) * f * ot,
                dd * 480 * 10_000,
            );
            t.night += ratio(dn, i64::from(d.night_regular) * f, dd * 480 * 1000)
                + ratio(dn, i64::from(d.night_overtime) * f * ot, dd * 480 * 100_000);
        }
    }
    ensure!(
        t.basic >= 0,
        "Unpaid time cannot exceed the reviewed basic salary."
    );
    let (mut rice, mut laundry) = (0, 0);
    for c in p.components.iter().chain(&w.adjustments) {
        match c.kind.as_str() {
            "taxable_allowance" => t.allowances += c.amount_cents,
            "rice" => {
                rice += c.amount_cents;
                t.allowances += c.amount_cents;
            }
            "laundry" => {
                laundry += c.amount_cents;
                t.allowances += c.amount_cents;
            }
            "reimbursement" => t.reimbursement += c.amount_cents,
            "deduction" => t.voluntary_deductions += c.amount_cents,
            "advance" => t.advances += c.amount_cents,
            "employer_benefit" => t.benefits += c.amount_cents,
            _ => unreachable!(),
        }
    }
    t.exempt_allowances = rice.min(250_000) + laundry.min(40_000);
    t.gross = t.basic
        + t.paid_leave
        + t.unworked_holiday
        + t.premium
        + t.overtime
        + t.night
        + t.allowances;
    t.sss_basis = w.sss_basis_override.unwrap_or(t.gross);
    if p.sss && t.sss_basis > 0 {
        t.sss_msc = ((t.sss_basis + 25_000) / 50_000 * 50_000).clamp(500_000, 3_500_000);
        t.sss_employee = ratio(t.sss_msc, 5, 100);
        t.sss_employer = ratio(t.sss_msc, 10, 100);
        t.sss_regular_employee = ratio(t.sss_msc.min(2_000_000), 5, 100);
        t.sss_regular_employer = ratio(t.sss_msc.min(2_000_000), 10, 100);
        t.sss_mpf_employee = t.sss_employee - t.sss_regular_employee;
        t.sss_mpf_employer = t.sss_employer - t.sss_regular_employer;
        t.ec = if t.sss_msc < 1_500_000 { 1000 } else { 3000 };
    }
    let ph = w.philhealth_basis_override.unwrap_or(if monthly {
        p.rate_cents
    } else {
        p.monthly_reference_cents
    });
    if p.philhealth && ph > 0 {
        let total = ratio(ph.clamp(1_000_000, 10_000_000), 5, 100);
        t.philhealth_employee = ratio(total, 1, 2);
        t.philhealth_employer = total - t.philhealth_employee;
    }
    let pi = w
        .pagibig_basis_override
        .unwrap_or(t.basic + t.paid_leave + t.allowances)
        .min(1_000_000);
    if p.pagibig && pi > 0 {
        t.pagibig_employee = ratio(pi, if pi <= 150_000 { 1 } else { 2 }, 100);
        t.pagibig_employer = ratio(pi, 2, 100);
    }
    let statutory = t.sss_employee + t.philhealth_employee + t.pagibig_employee;
    t.taxable = (if p.minimum_wage_earner {
        t.allowances - t.exempt_allowances
    } else {
        t.gross - t.exempt_allowances
    } - statutory)
        .max(0);
    t.withholding = monthly_tax(t.taxable);
    t.net =
        t.gross + t.reimbursement - statutory - t.withholding - t.voluntary_deductions - t.advances;
    t.thirteenth_accrual = ratio(t.basic, 1, 12);
    Ok(t)
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Profile {
    pub id: Uuid,
    pub company_id: Uuid,
    pub employee_id: Uuid,
    pub effective_on: NaiveDate,
    pub pay: Pay,
    pub revision: i64,
    pub created_by: Uuid,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Holiday {
    pub id: Uuid,
    pub company_id: Uuid,
    pub date: NaiveDate,
    pub name: String,
    pub kind: String,
    pub source: String,
    pub revision: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Run {
    pub id: Uuid,
    pub company_id: Uuid,
    pub month: NaiveDate,
    pub title: String,
    pub status: String,
    pub revision: i64,
    pub created_by: Uuid,
    pub submitted_by: Option<Uuid>,
    pub approved_by: Option<Uuid>,
    pub paid_on: Option<NaiveDate>,
    pub payment_reference: String,
    pub rule_version: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Item {
    pub id: Uuid,
    pub company_id: Uuid,
    pub run_id: Uuid,
    pub employee_id: Uuid,
    pub employee_name: String,
    pub employee_code: String,
    pub department: String,
    pub profile: Profile,
    #[serde(default)]
    pub source_basis: Value,
    pub worksheet: Worksheet,
    pub totals: Totals,
    pub revision: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Slip {
    pub id: Uuid,
    pub company_id: Uuid,
    pub run_id: Uuid,
    pub employee_id: Uuid,
    pub month: NaiveDate,
    pub company_name: String,
    pub item: Item,
    pub approved_by: Uuid,
    pub approved_at: DateTime<Utc>,
    pub rule_version: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub id: Uuid,
    pub company_id: Uuid,
    pub record_id: Uuid,
    pub kind: String,
    pub reason: String,
    pub actor_name: String,
    pub actor_role: Role,
    pub before_record: Option<Value>,
    pub after_record: Value,
    pub recorded_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Default)]
pub struct Setup {
    pub profiles: Vec<Profile>,
    pub holidays: Vec<Holiday>,
    pub runs: Vec<Run>,
    pub items: Vec<Item>,
    pub slips: Vec<Slip>,
    pub events: Vec<Event>,
}
impl Setup {
    pub fn profile(&self, employee: Uuid, month: NaiveDate) -> Option<&Profile> {
        self.profiles
            .iter()
            .filter(|p| p.employee_id == employee && p.effective_on <= month)
            .max_by_key(|p| p.effective_on)
    }
}
#[derive(Clone, Debug)]
pub struct PayrollAction {
    pub operation: String,
    pub record: Value,
}
impl PayrollAction {
    pub fn new(operation: &str, record: Value) -> Self {
        Self {
            operation: operation.into(),
            record,
        }
    }
    pub fn payload(&self, data: &Snapshot) -> Result<Value> {
        ensure!(
            data.mode != Mode::SignedOut
                && data.access.role.manages_hr()
                && data.access.company_id == data.company.id,
            "Only HR and Administrators manage payroll."
        );
        ensure!(
            [
                "profile_save",
                "holiday_save",
                "run_create",
                "item_add",
                "item_save",
                "item_refresh",
                "item_remove",
                "run_submit",
                "run_return",
                "run_cancel",
                "run_approve",
                "run_paid"
            ]
            .contains(&self.operation.as_str()),
            "Unknown payroll action."
        );
        ensure!(
            self.record.is_object() && self.record.to_string().len() <= 131072,
            "Use bounded payroll details."
        );
        let id = self.record["id"]
            .as_str()
            .context("Record identity required.")?
            .parse::<Uuid>()?;
        ensure!(!id.is_nil(), "Record identity required.");
        text(self.record["reason"].as_str().unwrap_or(""), 500, true)?;
        match self.operation.as_str() {
            "profile_save" => {
                let p: Pay = serde_json::from_value(self.record["pay"].clone())?;
                p.validate()?;
                let eid: Uuid = serde_json::from_value(self.record["employee_id"].clone())
                    .context("Choose an employee.")?;
                ensure!(
                    data.employees.iter().any(|e| e.id == eid),
                    "Choose an employee in this company."
                );
                let date: NaiveDate = serde_json::from_value(self.record["effective_on"].clone())
                    .context("Enter the effective month as YYYY-MM-01.")?;
                ensure!(
                    date.day() == 1 && date >= NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(),
                    "Use a month first from February 2026."
                );
            }
            "run_create" => {
                let month: NaiveDate = serde_json::from_value(self.record["month"].clone())
                    .context("Enter the payroll month as YYYY-MM-01.")?;
                ensure!(
                    month.day() == 1 && month >= NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(),
                    "Use a month first from February 2026."
                );
                text(self.record["title"].as_str().unwrap_or(""), 120, true)?;
            }
            "holiday_save" => {
                let _: NaiveDate = serde_json::from_value(self.record["date"].clone())
                    .context("Enter a holiday date as YYYY-MM-DD.")?;
                text(self.record["name"].as_str().unwrap_or(""), 120, true)?;
                text(self.record["source"].as_str().unwrap_or(""), 500, true)?;
                day_factor(self.record["kind"].as_str().unwrap_or(""), false)?;
            }
            "run_submit" => {
                ensure!(
                    self.record["scope_reviewed"] == true,
                    "Review and confirm the included and omitted employees."
                );
            }
            "item_save" => {
                let i = data
                    .payroll
                    .items
                    .iter()
                    .find(|i| i.id == id)
                    .context("Refresh the employee worksheet.")?;
                let run = data
                    .payroll
                    .runs
                    .iter()
                    .find(|r| r.id == i.run_id)
                    .context("Run missing.")?;
                let w: Worksheet = serde_json::from_value(self.record["worksheet"].clone())?;
                w.validate(&i.profile.pay, run.month)?;
            }
            "run_approve" | "run_paid" => {
                ensure!(
                    data.access.role == Role::Administrator,
                    "Only Administrators approve payroll or record payment."
                );
            }
            _ => {}
        }
        Ok(json!({"p_operation":self.operation,"p_record":self.record}))
    }
    pub fn saved(&self, data: &Snapshot) -> bool {
        let id = self.record["id"]
            .as_str()
            .and_then(|s| s.parse::<Uuid>().ok());
        data.payroll.events.iter().any(|e| {
            Some(e.record_id) == id
                && e.kind == self.operation
                && e.reason == self.record["reason"].as_str().unwrap_or("").trim()
        })
    }
}
/// Proposals only: actual work is split at calendar midnight/night boundaries,
/// deducting unpaid breaks and treating time beyond eight hours as OT. HR reviews
/// holiday/rest classifications, eligibility and statutory-vs-contract differences.
pub fn attendance_proposal(
    data: &Snapshot,
    employee: Uuid,
    month: NaiveDate,
) -> (Vec<Day>, Vec<String>) {
    let mut days: std::collections::BTreeMap<NaiveDate, Day> = Default::default();
    let mut warnings = vec![];
    let tz = data
        .company
        .timezone
        .parse::<chrono_tz::Tz>()
        .unwrap_or(chrono_tz::Asia::Manila);
    for a in data.attendance.iter().filter(|a| a.employee_id == employee) {
        let Some(end) = a.checked_out_at else {
            if a.work_date >= month && a.work_date <= month_end(month) {
                warnings.push(format!("{} has an unfinished check-out.", a.work_date));
            }
            continue;
        };
        if end < a.checked_in_at || end - a.checked_in_at > Duration::hours(48) {
            continue;
        }
        let mut n = 0u32;
        let mut t = a.checked_in_at;
        while t + Duration::minutes(1) <= end {
            let next = t + Duration::minutes(1);
            let local = t.with_timezone(&tz);
            let date = local.date_naive();
            let unpaid = data
                .breaks
                .for_attendance(a.id)
                .any(|b| !b.paid && b.ended_at.is_some_and(|e| b.started_at < next && e > t));
            if !unpaid {
                let overtime = n >= 480;
                n += 1;
                if date >= month && date <= month_end(month) {
                    let kind = data
                        .payroll
                        .holidays
                        .iter()
                        .find(|h| h.date == date)
                        .map(|h| h.kind.clone())
                        .unwrap_or_else(|| "ordinary".into());
                    let d = days.entry(date).or_insert_with(|| Day {
                        date,
                        kind,
                        rest: a.planned_shift.as_ref().is_some_and(|p| {
                            !p.days.contains(&date.weekday().number_from_monday())
                        }),
                        ..Default::default()
                    });
                    let night = local.hour() >= 22 || local.hour() < 6;
                    if overtime {
                        d.overtime_minutes += 1;
                        if night {
                            d.night_overtime += 1;
                        }
                    } else {
                        d.regular_minutes += 1;
                        if night {
                            d.night_regular += 1;
                        }
                    }
                }
            }
            t = next;
        }
    }
    for d in days.values() {
        if d.regular_minutes > 480 {
            warnings.push(format!(
                "{} has multiple shifts; review the statutory workday allocation.",
                d.date
            ));
        }
    }
    warnings.push("Review rest days, holiday classifications, paid leave and unworked-holiday eligibility. Missing days have no automatic deduction. Proposals use whole fully recorded minutes; review records with fractional break boundaries.".into());
    (days.into_values().collect(), warnings)
}
pub fn totals_rows(t: &Totals) -> Vec<(&'static str, i64)> {
    vec![
        ("Basic pay", t.basic),
        ("Paid leave (daily pay)", t.paid_leave),
        ("Unworked regular holidays", t.unworked_holiday),
        ("Holiday / rest-day premium", t.premium),
        ("Overtime", t.overtime),
        ("Night differential", t.night),
        ("Allowances", t.allowances),
        ("Included exempt allowances", t.exempt_allowances),
        ("Gross compensation", t.gross),
        ("Reimbursements", t.reimbursement),
        ("SSS employee", t.sss_employee),
        ("PhilHealth employee", t.philhealth_employee),
        ("Pag-IBIG employee", t.pagibig_employee),
        ("Taxable compensation", t.taxable),
        ("Withholding tax", t.withholding),
        ("Other deductions", t.voluntary_deductions),
        ("Salary advances", t.advances),
        ("Net due", t.net),
        ("SSS employer", t.sss_employer),
        ("EC employer", t.ec),
        ("PhilHealth employer", t.philhealth_employer),
        ("Pag-IBIG employer", t.pagibig_employer),
        ("Employer benefit memo", t.benefits),
        ("13th-month accrual memo", t.thirteenth_accrual),
    ]
}

fn demo_basis(data: &Snapshot, eid: Uuid, mon: NaiveDate) -> Result<Value> {
    let e = data
        .employees
        .iter()
        .find(|e| e.id == eid)
        .context("Employee missing.")?;
    let p = data
        .payroll
        .profile(eid, mon)
        .context("Save an effective salary profile first.")?;
    Ok(
        json!({"employee":e,"profile":p,"attendance":data.attendance.iter().filter(|a|a.employee_id==eid&&a.work_date>=mon&&a.work_date<=month_end(mon)).collect::<Vec<_>>(),"breaks":data.breaks.records,"overtime":data.overtime.records,"leave":data.leave,"holidays":data.payroll.holidays,"timezone":data.company.timezone}),
    )
}
pub fn apply_demo(data: &mut Snapshot, a: &PayrollAction) -> Result<()> {
    a.payload(data)?;
    let mut copy = data.clone();
    demo_change(&mut copy, a)?;
    *data = copy;
    Ok(())
}
fn demo_change(data: &mut Snapshot, a: &PayrollAction) -> Result<()> {
    let v = &a.record;
    let id: Uuid = serde_json::from_value(v["id"].clone())?;
    let expected = v["expected_revision"].as_i64();
    let mut old = None;
    let result;
    let actor = data.access.user_id;
    let cid = data.company.id;
    let uid = |key: &str| -> Result<Uuid> { Ok(serde_json::from_value(v[key].clone())?) };
    match a.operation.as_str() {
        "profile_save" => {
            let eid = uid("employee_id")?;
            ensure!(
                data.employees.iter().any(|e| e.id == eid),
                "Choose an employee."
            );
            let date: NaiveDate = serde_json::from_value(v["effective_on"].clone())?;
            ensure!(
                date.day() == 1 && date >= NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(),
                "Use a month first from February 2026."
            );
            let pay: Pay = serde_json::from_value(v["pay"].clone())?;
            if let Some(i) = data.payroll.profiles.iter().position(|p| p.id == id) {
                let p = &mut data.payroll.profiles[i];
                ensure!(
                    Some(p.revision) == expected && p.employee_id == eid && p.effective_on == date,
                    "Salary version changed. Refresh."
                );
                old = Some(serde_json::to_value(&*p)?);
                p.pay = pay;
                p.revision += 1;
                result = serde_json::to_value(p)?;
            } else {
                ensure!(
                    expected.is_none()
                        && !data
                            .payroll
                            .profiles
                            .iter()
                            .any(|p| p.employee_id == eid && p.effective_on == date),
                    "Salary version exists."
                );
                let p = Profile {
                    id,
                    company_id: cid,
                    employee_id: eid,
                    effective_on: date,
                    pay,
                    revision: 0,
                    created_by: actor,
                };
                result = serde_json::to_value(&p)?;
                data.payroll.profiles.push(p);
            }
        }
        "holiday_save" => {
            let date = serde_json::from_value(v["date"].clone())?;
            let mut h = Holiday {
                id,
                company_id: cid,
                date,
                name: v["name"].as_str().unwrap_or("").into(),
                kind: v["kind"].as_str().unwrap_or("").into(),
                source: v["source"].as_str().unwrap_or("").into(),
                revision: 0,
            };
            text(&h.name, 120, true)?;
            text(&h.source, 500, true)?;
            day_factor(&h.kind, false)?;
            ensure!(
                !data
                    .payroll
                    .holidays
                    .iter()
                    .any(|x| x.id != id && x.date == date),
                "That date already exists."
            );
            if let Some(i) = data.payroll.holidays.iter().position(|h| h.id == id) {
                let prior = &data.payroll.holidays[i];
                ensure!(
                    expected == Some(prior.revision),
                    "Holiday changed. Refresh."
                );
                old = Some(serde_json::to_value(prior)?);
                h.revision = prior.revision + 1;
                data.payroll.holidays[i] = h.clone();
            } else {
                ensure!(expected.is_none(), "Holiday missing.");
                data.payroll.holidays.push(h.clone());
            }
            result = serde_json::to_value(h)?;
        }
        "run_create" => {
            let month: NaiveDate = serde_json::from_value(v["month"].clone())?;
            ensure!(
                month.day() == 1
                    && month >= NaiveDate::from_ymd_opt(2026, 2, 1).unwrap()
                    && !data
                        .payroll
                        .runs
                        .iter()
                        .any(|r| r.month == month && r.status != "cancelled"),
                "Use an unregistered month first from February 2026."
            );
            let title = v["title"].as_str().unwrap_or("").to_owned();
            text(&title, 120, true)?;
            let r = Run {
                id,
                company_id: cid,
                month,
                title,
                status: "draft".into(),
                revision: 0,
                created_by: actor,
                submitted_by: None,
                approved_by: None,
                paid_on: None,
                payment_reference: String::new(),
                rule_version: RULE_VERSION.into(),
            };
            result = serde_json::to_value(&r)?;
            data.payroll.runs.push(r);
        }
        "item_add" | "item_save" | "item_refresh" | "item_remove" => {
            let existing = data.payroll.items.iter().find(|i| i.id == id).cloned();
            let run_id = if let Some(i) = &existing {
                ensure!(expected == Some(i.revision), "Worksheet changed. Refresh.");
                i.run_id
            } else {
                ensure!(a.operation == "item_add", "Worksheet missing.");
                uid("run_id")?
            };
            let ri = data
                .payroll
                .runs
                .iter()
                .position(|r| r.id == run_id)
                .context("Run missing.")?;
            let r = &data.payroll.runs[ri];
            ensure!(r.status == "draft", "Only draft worksheets can change.");
            let mon = r.month;
            if a.operation == "item_remove" {
                let i = existing.context("Worksheet missing.")?;
                old = Some(serde_json::to_value(&i)?);
                result = serde_json::to_value(i)?;
                data.payroll.items.retain(|i| i.id != id);
            } else {
                let eid = if let Some(i) = &existing {
                    i.employee_id
                } else {
                    uid("employee_id")?
                };
                ensure!(
                    !data
                        .payroll
                        .items
                        .iter()
                        .any(|i| i.id != id && i.run_id == run_id && i.employee_id == eid),
                    "Employee already included."
                );
                let fresh = demo_basis(data, eid, mon)?;
                let e = data.employees.iter().find(|e| e.id == eid).unwrap();
                let p = data.payroll.profile(eid, mon).unwrap().clone();
                let mut w = existing
                    .as_ref()
                    .map(|i| i.worksheet.clone())
                    .unwrap_or_default();
                if a.operation == "item_save" {
                    ensure!(
                        existing.as_ref().is_some_and(|i| i.source_basis == fresh),
                        "Sources changed. Refresh source and review again."
                    );
                    w = serde_json::from_value(v["worksheet"].clone())?;
                } else {
                    w.reviewed = false;
                    w.review_note.clear();
                }
                let t = calculate(&p.pay, &w, mon)?;
                let item = Item {
                    id,
                    company_id: cid,
                    run_id,
                    employee_id: eid,
                    employee_name: e.full_name.clone(),
                    employee_code: e.employee_code.clone(),
                    department: e.department.clone(),
                    profile: p,
                    source_basis: fresh,
                    worksheet: w,
                    totals: t,
                    revision: existing.as_ref().map(|i| i.revision + 1).unwrap_or(0),
                };
                result = serde_json::to_value(&item)?;
                if let Some(i) = existing {
                    old = Some(serde_json::to_value(i)?);
                    *data.payroll.items.iter_mut().find(|i| i.id == id).unwrap() = item;
                } else {
                    data.payroll.items.push(item);
                }
            }
            data.payroll.runs[ri].revision += 1;
        }
        _ => {
            let ri = data
                .payroll
                .runs
                .iter()
                .position(|r| r.id == id)
                .context("Run missing.")?;
            let mut r = data.payroll.runs[ri].clone();
            ensure!(Some(r.revision) == expected, "Run changed. Refresh.");
            old = Some(serde_json::to_value(&r)?);
            match a.operation.as_str() {
                "run_submit" | "run_approve" => {
                    ensure!(
                        month_end(r.month) <= data.company.today() && r.month.year() == 2026,
                        "Use a completed 2026 month."
                    );
                    let items: Vec<_> = data
                        .payroll
                        .items
                        .iter()
                        .filter(|i| i.run_id == id)
                        .cloned()
                        .collect();
                    ensure!(!items.is_empty(), "Add employees first.");
                    for i in &items {
                        ensure!(
                            i.worksheet.reviewed
                                && !i.worksheet.review_note.is_empty()
                                && i.totals.net >= 0
                                && i.source_basis == demo_basis(data, i.employee_id, r.month)?,
                            "Review all worksheets and refresh changed sources."
                        );
                    }
                    if a.operation == "run_submit" {
                        ensure!(
                            r.status == "draft" && v["scope_reviewed"] == true,
                            "Confirm the included employees."
                        );
                        r.status = "submitted".into();
                        r.submitted_by = Some(actor);
                    } else {
                        ensure!(
                            r.status == "submitted"
                                && r.submitted_by != Some(actor)
                                && data.access.role == Role::Administrator,
                            "Another Administrator approves submitted payroll."
                        );
                        r.status = "approved".into();
                        r.approved_by = Some(actor);
                        for mut item in items {
                            item.source_basis = Value::Null;
                            item.profile.pay.coverage_note.clear();
                            for c in &mut item.profile.pay.components {
                                c.reference.clear();
                            }
                            item.worksheet.review_note.clear();
                            item.worksheet.exception_note.clear();
                            for c in &mut item.worksheet.adjustments {
                                c.reference.clear();
                            }
                            data.payroll.slips.push(Slip {
                                id: item.id,
                                company_id: cid,
                                run_id: id,
                                employee_id: item.employee_id,
                                month: r.month,
                                company_name: data.company.name.clone(),
                                item,
                                approved_by: actor,
                                approved_at: Utc::now(),
                                rule_version: RULE_VERSION.into(),
                            });
                        }
                    }
                }
                "run_return" => {
                    ensure!(
                        r.status == "submitted",
                        "Only submitted payroll can return to draft."
                    );
                    r.status = "draft".into();
                    r.submitted_by = None;
                }
                "run_cancel" => {
                    ensure!(
                        ["draft", "submitted"].contains(&r.status.as_str()),
                        "Approved payroll is immutable."
                    );
                    r.status = "cancelled".into();
                    r.submitted_by = None;
                }
                "run_paid" => {
                    let paid: NaiveDate = serde_json::from_value(v["paid_on"].clone())?;
                    ensure!(
                        r.status == "approved" && paid >= r.month && paid <= data.company.today(),
                        "Record an actual date for approved payroll."
                    );
                    r.payment_reference = v["payment_reference"].as_str().unwrap_or("").into();
                    text(&r.payment_reference, 500, true)?;
                    r.paid_on = Some(paid);
                    r.status = "paid".into();
                }
                _ => anyhow::bail!("Unsupported payroll operation."),
            };
            r.revision += 1;
            result = serde_json::to_value(&r)?;
            data.payroll.runs[ri] = r;
        }
    }
    data.payroll.events.push(Event {
        id: Uuid::new_v4(),
        company_id: cid,
        record_id: id,
        kind: a.operation.clone(),
        reason: v["reason"].as_str().unwrap_or("").into(),
        actor_name: data
            .own_employee()
            .map(|e| e.full_name.clone())
            .unwrap_or_else(|| "HR staff".into()),
        actor_role: data.access.role,
        before_record: old,
        after_record: result,
        recorded_at: Utc::now(),
    });
    Ok(())
}

pub fn export_csv(data: &Snapshot, kind: &str, id: Option<Uuid>, year: i32) -> Result<Vec<u8>> {
    ensure!(
        data.mode != Mode::SignedOut && data.access.company_id == data.company.id,
        "Sign in to export company payroll."
    );
    ensure!(
        ["register", "statutory", "annual", "payslip"].contains(&kind),
        "Choose a payroll report."
    );
    let hr = data.access.role.manages_hr();
    ensure!(
        kind == "payslip" || hr,
        "Only HR and Administrators export company payroll."
    );
    let mut rows: Vec<(String, String, Item)> = vec![];
    if kind == "payslip" {
        let s = data
            .payroll
            .slips
            .iter()
            .find(|s| Some(s.id) == id)
            .context("Approved payslip not found.")?;
        ensure!(
            hr || data.access.employee_id == Some(s.employee_id),
            "Only your own payslip is available."
        );
        rows.push((s.month.to_string(), "approved".into(), s.item.clone()));
    } else if kind == "annual" {
        let mut grouped: std::collections::BTreeMap<Uuid, (Vec<NaiveDate>, Item)> =
            Default::default();
        let mut slips: Vec<_> = data
            .payroll
            .slips
            .iter()
            .filter(|s| s.month.year() == year)
            .collect();
        slips.sort_by_key(|s| s.month);
        for s in slips {
            let entry = grouped.entry(s.employee_id).or_insert_with(|| {
                let mut i = s.item.clone();
                i.totals = Totals::default();
                (vec![], i)
            });
            entry.0.push(s.month);
            let mut acc = serde_json::to_value(&entry.1.totals)?;
            let value = serde_json::to_value(&s.item.totals)?;
            for (k, v) in value.as_object().unwrap() {
                acc[k] = json!(acc[k].as_i64().unwrap_or(0) + v.as_i64().unwrap());
            }
            entry.1.totals = serde_json::from_value(acc)?;
            entry.1.employee_name = s.item.employee_name.clone();
            entry.1.employee_code = s.item.employee_code.clone();
        }
        for (_, (months, item)) in grouped {
            rows.push((
                format!(
                    "{year}: {}",
                    months
                        .iter()
                        .map(|d| d.format("%m").to_string())
                        .collect::<Vec<_>>()
                        .join("/")
                ),
                "approved recorded months only; no annual tax adjustment".into(),
                item,
            ));
        }
    } else {
        let r = data
            .payroll
            .runs
            .iter()
            .find(|r| Some(r.id) == id)
            .context("Payroll run not found.")?;
        rows = data
            .payroll
            .items
            .iter()
            .filter(|i| i.run_id == r.id)
            .cloned()
            .map(|i| (r.month.to_string(), r.status.clone(), i))
            .collect();
    }
    let sample = serde_json::to_value(Totals::default())?;
    let columns: Vec<_> = sample
        .as_object()
        .unwrap()
        .keys()
        .filter(|k| kind != "annual" || !["sss_msc", "sss_basis"].contains(&k.as_str()))
        .cloned()
        .collect();
    let mut headers = vec![
        "company",
        "report",
        "status",
        "period / recorded months",
        "employee code",
        "employee name",
        "department",
        "TIN",
        "SSS number",
        "PhilHealth number",
        "Pag-IBIG number",
        "rule version",
        "generated UTC",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    headers.extend(columns.iter().cloned());
    let line = |cells: Vec<String>| -> Result<String> {
        Ok(cells
            .iter()
            .map(|s| crate::reports::csv_cell(s))
            .collect::<Result<Vec<_>>>()?
            .join(",")
            + "\r\n")
    };
    let mut csv = String::from("\u{feff}");
    csv.push_str(&line(headers)?);
    rows.sort_by_key(|r| r.2.employee_code.clone());
    for (period, status, i) in rows {
        let p = &i.profile.pay;
        let mut cells = vec![
            data.company.name.clone(),
            kind.into(),
            status,
            period,
            i.employee_code,
            i.employee_name,
            i.department,
            p.tin.clone(),
            p.sss_number.clone(),
            p.philhealth_number.clone(),
            p.pagibig_number.clone(),
            RULE_VERSION.into(),
            Utc::now().to_rfc3339(),
        ];
        let t = serde_json::to_value(&i.totals)?;
        for k in &columns {
            let v = t[k].as_i64().unwrap();
            cells.push(format!(
                "{}{}.{:02}",
                if v < 0 { "-" } else { "" },
                v.unsigned_abs() / 100,
                v.unsigned_abs() % 100
            ));
        }
        csv.push_str(&line(cells)?);
    }
    Ok(csv.into_bytes())
}
