//! Prospective Philippine final-pay calculations. No money is transferred here.
use crate::{payroll, payroll_tax as tax};
use anyhow::{Result, ensure};
use chrono::{Datelike, Months, NaiveDate};
use serde::{Deserialize, Serialize};
pub const RULE_VERSION: &str = "PH-final-pay-2026-10-24-v2";
const LIMIT: i64 = tax::LIMIT;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Pending {
    pub id: String,
    pub income: tax::Income,
    pub net_cents: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub employment_start: NaiveDate,
    pub service_end: NaiveDate,
    pub cash_date: NaiveDate,
    pub annual: tax::Source,
    pub outstanding: Vec<Pending>,
    pub previous_vacation_days_hundredths: u32,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Vacation {
    pub days_hundredths: u32,
    pub daily_cents: i64,
    pub previous_days_hundredths: u32,
    pub reference: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Separation {
    pub cause: String,
    pub monthly_basic_cents: i64,
    pub regular_allowances_cents: i64,
    pub total_award_cents: i64,
    pub previously_paid_cents: i64,
    pub tax_exempt: bool,
    pub reference: String,
}
impl Default for Separation {
    fn default() -> Self {
        Self {
            cause: "none".into(),
            monthly_basic_cents: 0,
            regular_allowances_cents: 0,
            total_award_cents: 0,
            previously_paid_cents: 0,
            tax_exempt: false,
            reference: String::new(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Retirement {
    pub method: String,
    pub daily_basic_cents: i64,
    pub plan_award_cents: i64,
    pub previously_paid_cents: i64,
    pub birth_date: Option<NaiveDate>,
    pub ordinary_statutory_coverage: bool,
    pub employee_plan_covered: bool,
    pub tax_treatment: String,
    pub qualified_plan_reviewed: bool,
    pub first_qualified_availment: bool,
    pub reference: String,
}
impl Default for Retirement {
    fn default() -> Self {
        Self {
            method: "none".into(),
            daily_basic_cents: 0,
            plan_award_cents: 0,
            previously_paid_cents: 0,
            birth_date: None,
            ordinary_statutory_coverage: false,
            employee_plan_covered: false,
            tax_treatment: "taxable".into(),
            qualified_plan_reviewed: false,
            first_qualified_availment: false,
            reference: String::new(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Line {
    pub label: String,
    /// taxable, shared_benefit, deposit_return, lawful_deduction.
    pub kind: String,
    pub amount_cents: i64,
    pub reference: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Worksheet {
    pub tax: tax::Worksheet,
    pub vacation: Vacation,
    pub separation: Separation,
    pub retirement: Retirement,
    pub lines: Vec<Line>,
    pub sss_adjustment_cents: i64,
    pub philhealth_adjustment_cents: i64,
    pub pagibig_adjustment_cents: i64,
    pub contribution_note: String,
    pub release_within_days: u32,
    pub entitlements_reviewed: bool,
    pub source_scope_reviewed: bool,
    pub contributions_reviewed: bool,
    pub review_note: String,
    /// Private evidence that delayed pay was not actually/constructively received earlier.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub cash_year_reviewed: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub cash_year_note: String,
}
impl Default for Worksheet {
    fn default() -> Self {
        Self {
            tax: tax::Worksheet::default(),
            vacation: Vacation::default(),
            separation: Separation::default(),
            retirement: Retirement::default(),
            lines: vec![],
            sss_adjustment_cents: 0,
            philhealth_adjustment_cents: 0,
            pagibig_adjustment_cents: 0,
            contribution_note: String::new(),
            release_within_days: 30,
            entitlements_reviewed: false,
            source_scope_reviewed: false,
            contributions_reviewed: false,
            review_note: String::new(),
            cash_year_reviewed: false,
            cash_year_note: String::new(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct Calculation {
    pub credited_service_years: u32,
    pub vacation_gross_cents: i64,
    pub vacation_exempt_cents: i64,
    pub vacation_shared_benefit_cents: i64,
    pub separation_floor_cents: i64,
    pub separation_cents: i64,
    pub separation_overpaid_cents: i64,
    pub retirement_floor_cents: i64,
    pub retirement_cents: i64,
    pub retirement_overpaid_cents: i64,
    pub taxable_extras_cents: i64,
    pub shared_extras_cents: i64,
    pub exempt_extras_cents: i64,
    pub deposit_returns_cents: i64,
    pub additional_contributions_cents: i64,
    pub lawful_deductions_cents: i64,
    pub source_net_cents: i64,
    pub extras_gross_cents: i64,
    pub cash_before_tax_cents: i64,
    pub tax: tax::Calculation,
    pub net_cents: i64,
    pub due_on: Option<NaiveDate>,
    pub planned_late: bool,
}
fn money(n: i64) -> Result<()> {
    ensure!(
        (0..=LIMIT).contains(&n),
        "Use bounded nonnegative integer centavos."
    );
    Ok(())
}
fn text(s: &str, max: usize, required: bool) -> Result<()> {
    ensure!(
        s.chars().count() <= max
            && (!required || !s.trim().is_empty())
            && !s.chars().any(|c| c.is_control() && c != '\n' && c != '\t'),
        "Provide bounded readable final-pay evidence and review text."
    );
    Ok(())
}
/// Service includes the last working day. Calendar anniversaries, not days/365.
pub fn service_years(start: NaiveDate, end: NaiveDate) -> Result<(u32, u32)> {
    ensure!(
        start.year() >= 1900 && start <= end && end.year() <= 2200,
        "Review the actual employment start and last working day."
    );
    let exclusive = end.succ_opt().unwrap();
    let mut full = (end.year() - start.year()) as u32;
    while start.checked_add_months(Months::new(full * 12)).unwrap() > exclusive {
        full -= 1;
    }
    if start
        .checked_add_months(Months::new((full + 1) * 12))
        .unwrap()
        <= exclusive
    {
        full += 1;
    }
    let rounded = full
        + u32::from(
            start
                .checked_add_months(Months::new(full * 12 + 6))
                .unwrap()
                <= exclusive,
        );
    Ok((full, rounded))
}
fn age(birth: NaiveDate, end: NaiveDate) -> Result<u32> {
    ensure!(
        birth.year() >= 1900 && birth <= end,
        "Review the retirement date of birth."
    );
    Ok(end.years_since(birth).unwrap())
}
pub fn calculate(s: &Source, w: &Worksheet) -> Result<Calculation> {
    ensure!(
        s.cash_date >= NaiveDate::from_ymd_opt(2026, 1, 6).unwrap()
            && s.cash_date.year() == 2026
            && s.service_end >= NaiveDate::from_ymd_opt(2025, 1, 1).unwrap()
            && s.service_end <= s.cash_date
            && s.employment_start <= s.service_end,
        "This reviewed final-pay pack supports 2025 or 2026 separation with cash from 6 January through December 2026. Other historical separation or later payment years need a reviewed rule pack."
    );
    ensure!(
        s.annual.as_of == s.cash_date
            && s.annual.service_end == s.service_end
            && s.annual.starts_on
                == s.employment_start
                    .max(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap())
            && s.outstanding.len() <= 120
            && s.annual.pending_payments as usize == s.outstanding.len(),
        "Refresh final salary, received income and outstanding source scope."
    );
    ensure!(
        w.lines.len() <= 32 && (1..=30).contains(&w.release_within_days),
        "Use at most 32 final amounts and a release deadline within 30 days of separation."
    );
    for note in [&w.contribution_note, &w.review_note] {
        text(note, 2000, false)?;
    }
    text(&w.cash_year_note, 2000, false)?;
    let (full, credited) = service_years(s.employment_start, s.service_end)?;
    let mut c = Calculation {
        credited_service_years: credited,
        due_on: s
            .service_end
            .checked_add_signed(chrono::Duration::days(i64::from(w.release_within_days))),
        ..Default::default()
    };
    c.planned_late = Some(s.cash_date) > c.due_on;
    let v = &w.vacation;
    ensure!(
        v.days_hundredths <= 36600
            && v.previous_days_hundredths <= 36600
            && v.previous_days_hundredths >= s.previous_vacation_days_hundredths,
        "Review total vacation days already monetized in this cash year; retain earlier recorded facts."
    );
    money(v.daily_cents)?;
    text(
        &v.reference,
        1000,
        v.days_hundredths > 0 || v.previous_days_hundredths > 0,
    )?;
    ensure!(
        v.days_hundredths == 0 || v.daily_cents > 0,
        "A reviewed daily rate is required to convert vacation leave."
    );
    c.vacation_gross_cents = payroll::ratio(v.daily_cents, i64::from(v.days_hundredths), 100);
    c.vacation_exempt_cents = payroll::ratio(
        v.daily_cents,
        i64::from(
            v.days_hundredths
                .min(1200u32.saturating_sub(v.previous_days_hundredths)),
        ),
        100,
    );
    c.vacation_shared_benefit_cents = c.vacation_gross_cents - c.vacation_exempt_cents;
    c.shared_extras_cents = c.vacation_shared_benefit_cents;
    c.exempt_extras_cents = c.vacation_exempt_cents;
    let p = &w.separation;
    let factor = match p.cause.as_str() {
        "redundancy" | "labor_saving_devices" => 2,
        "retrenchment" | "closure_without_serious_losses" | "disease" => 1,
        "none"
        | "resignation"
        | "just_cause"
        | "closure_serious_losses"
        | "death_or_disability"
        | "company_policy" => 0,
        _ => anyhow::bail!(
            "Use a reviewed separation cause; no cause is inferred from an account role."
        ),
    };
    for n in [
        p.monthly_basic_cents,
        p.regular_allowances_cents,
        p.total_award_cents,
        p.previously_paid_cents,
    ] {
        money(n)?;
    }
    let base = p.monthly_basic_cents + p.regular_allowances_cents;
    money(base)?;
    ensure!(
        factor == 0 || base > 0,
        "Review the latest basic plus regular allowances for statutory separation pay."
    );
    c.separation_floor_cents = if factor == 0 {
        0
    } else {
        base.max(payroll::ratio(base, i64::from(credited * factor), 2))
    };
    ensure!(
        p.total_award_cents >= c.separation_floor_cents,
        "The separation award must meet its cause-specific statutory floor, including regular allowances."
    );
    ensure!(
        p.cause != "none"
            || (p.total_award_cents == 0 && p.previously_paid_cents == 0 && !p.tax_exempt),
        "Classify a separation award before entering amounts or an exemption."
    );
    ensure!(
        !p.tax_exempt
            || [
                "redundancy",
                "labor_saving_devices",
                "retrenchment",
                "closure_without_serious_losses",
                "closure_serious_losses",
                "disease",
                "death_or_disability"
            ]
            .contains(&p.cause.as_str()),
        "Resignation, just cause and generic policy awards do not automatically qualify for separation tax exemption."
    );
    text(&p.reference, 1000, p.cause != "none")?;
    c.separation_cents = (p.total_award_cents - p.previously_paid_cents).max(0);
    c.separation_overpaid_cents = (p.previously_paid_cents - p.total_award_cents).max(0);
    if p.tax_exempt {
        c.exempt_extras_cents += c.separation_cents;
    } else {
        c.taxable_extras_cents += c.separation_cents;
    }
    let r = &w.retirement;
    for n in [
        r.daily_basic_cents,
        r.plan_award_cents,
        r.previously_paid_cents,
    ] {
        money(n)?;
    }
    ensure!(
        ["none", "statutory", "plan"].contains(&r.method.as_str())
            && ["taxable", "statutory_no_plan", "qualified_plan"]
                .contains(&r.tax_treatment.as_str()),
        "Use a reviewed private-sector retirement method and tax treatment."
    );
    let active = r.method != "none";
    let years_old = r.birth_date.map(|b| age(b, s.service_end)).transpose()?;
    let ordinary = r.ordinary_statutory_coverage && full >= 5 && years_old.is_some_and(|a| a >= 60);
    if active {
        text(&r.reference, 1000, true)?;
        ensure!(
            r.birth_date.is_some(),
            "Review retirement age from evidenced birth details."
        );
        ensure!(
            r.method != "statutory" || ordinary,
            "Ordinary statutory retirement requires covered private employment, actual five-year service and retirement age 60 or later. Six-month monetary rounding does not grant eligibility."
        );
        if ordinary {
            ensure!(
                r.daily_basic_cents > 0,
                "Review the latest retirement daily basic excluding COLA/allowances unless a more favorable agreement applies."
            );
            c.retirement_floor_cents =
                payroll::ratio(r.daily_basic_cents, i64::from(credited) * 45, 2);
        }
    } else {
        text(&r.reference, 1000, false)?;
        ensure!(
            r.daily_basic_cents == 0
                && r.plan_award_cents == 0
                && r.previously_paid_cents == 0
                && r.tax_treatment == "taxable",
            "Classify a retirement award before entering amounts or exemptions."
        );
    }
    if r.tax_treatment == "statutory_no_plan" {
        ensure!(
            active && ordinary && !r.employee_plan_covered,
            "RA7641 tax treatment requires eligible statutory retirement and an employee not covered by an employer retirement plan. Being ineligible under a covered plan is not a fallback exemption."
        );
    }
    if r.tax_treatment == "qualified_plan" {
        ensure!(
            active
                && r.employee_plan_covered
                && r.qualified_plan_reviewed
                && r.first_qualified_availment
                && full >= 10
                && years_old.is_some_and(|a| a >= 50),
            "A qualified-plan exemption requires evidenced plan coverage/qualification, age 50, actual ten-year service and first qualifying availment."
        );
    }
    let award = if active {
        r.plan_award_cents.max(c.retirement_floor_cents)
    } else {
        0
    };
    c.retirement_cents = (award - r.previously_paid_cents).max(0);
    c.retirement_overpaid_cents = (r.previously_paid_cents - award).max(0);
    if r.tax_treatment == "taxable" {
        c.taxable_extras_cents += c.retirement_cents;
    } else {
        c.exempt_extras_cents += c.retirement_cents;
    }
    for l in &w.lines {
        text(&l.label, 120, true)?;
        text(&l.reference, 1000, true)?;
        money(l.amount_cents)?;
        match l.kind.as_str() {
            "taxable" => c.taxable_extras_cents += l.amount_cents,
            "shared_benefit" => c.shared_extras_cents += l.amount_cents,
            "deposit_return" => c.deposit_returns_cents += l.amount_cents,
            "lawful_deduction" => c.lawful_deductions_cents += l.amount_cents,
            _ => anyhow::bail!(
                "Classify additional amounts as taxable, shared benefit, deposit return or evidenced lawful deduction."
            ),
        }
    }
    for n in [
        w.sss_adjustment_cents,
        w.philhealth_adjustment_cents,
        w.pagibig_adjustment_cents,
    ] {
        money(n)?;
        c.additional_contributions_cents += n;
    }
    c.extras_gross_cents = c.taxable_extras_cents
        + c.shared_extras_cents
        + c.exempt_extras_cents
        + c.deposit_returns_cents;
    let mut annual = s.annual.clone();
    let mut ids = annual
        .income
        .iter()
        .map(|i| i.id.clone())
        .collect::<std::collections::HashSet<_>>();
    for pending in &s.outstanding {
        text(&pending.id, 80, true)?;
        money(pending.net_cents)?;
        ensure!(
            pending.id == pending.income.id
                && ids.insert(pending.id.clone())
                && pending.id != "final:additional",
            "Outstanding and received pay must have unique matching source identities."
        );
        annual.income.push(pending.income.clone());
        c.source_net_cents += pending.net_cents;
    }
    ensure!(
        ids.insert("final:additional".into()),
        "A final-pay source cannot include its own additional amounts."
    );
    annual.income.push(tax::Income {
        id: "final:additional".into(),
        starts_on: None,
        ends_on: None,
        taxable_cents: c.taxable_extras_cents + c.shared_extras_cents,
        withholding_cents: 0,
        benefit_gross_cents: c.shared_extras_cents,
        benefit_exempt_cents: 0,
        contribution_cents: c.additional_contributions_cents,
    });
    // All outstanding issued sources in this cash-date scope participate in the settlement.
    annual.pending_payments = 0;
    c.cash_before_tax_cents = c.source_net_cents + c.extras_gross_cents
        - c.additional_contributions_cents
        - c.lawful_deductions_cents;
    money(c.cash_before_tax_cents)?;
    let mut tw = w.tax.clone();
    tw.last_pay_available_cents = c.cash_before_tax_cents;
    c.tax = tax::calculate(&annual, &tw)?;
    c.net_cents = c.cash_before_tax_cents - c.tax.available_deduction_cents + c.tax.refund_cents;
    for n in [
        c.vacation_gross_cents,
        c.separation_floor_cents,
        c.retirement_floor_cents,
        c.taxable_extras_cents,
        c.shared_extras_cents,
        c.exempt_extras_cents,
        c.deposit_returns_cents,
        c.additional_contributions_cents,
        c.lawful_deductions_cents,
        c.source_net_cents,
        c.extras_gross_cents,
        c.net_cents,
    ] {
        money(n)?;
    }
    Ok(c)
}
pub fn ready(s: &Source, w: &Worksheet) -> Result<Calculation> {
    let c = calculate(s, w)?;
    ensure!(s.service_end.year() == s.cash_date.year()
        || (w.cash_year_reviewed && !w.cash_year_note.trim().is_empty()),
        "Review the payment year's actual/constructive receipt evidence, prior-year withholding and other employment before issuing delayed final pay. Do not move earlier received income into this year.");
    ensure!(
        s.annual.pending_adjustments == 0
            && c.tax.coverage_gaps.is_empty()
            && c.tax.unreconciled_benefits_cents == 0,
        "Resolve earlier tax advice and review all annual income coverage/shared benefits before final pay."
    );
    ensure!(
        c.separation_overpaid_cents == 0 && c.retirement_overpaid_cents == 0,
        "Earlier separation/retirement payments exceed the reviewed award. Resolve the discrepancy without automatic wage recovery."
    );
    ensure!(
        w.entitlements_reviewed
            && w.source_scope_reviewed
            && w.contributions_reviewed
            && !w.review_note.trim().is_empty()
            && !w.contribution_note.trim().is_empty(),
        "Review owed salary/13th-month scope, leave, separation/retirement, deposits, lawful deductions and additional contributions, including zero."
    );
    ensure!(
        w.tax.final_compensation_reviewed
            && w.tax.previous_employers_reviewed
            && w.tax.benefits_reviewed
            && !w.tax.review_note.trim().is_empty()
            && !w.tax.settlement_note.trim().is_empty(),
        "Review prospective annual income, net prior-employer certificates, shared benefits and tax/refund/employer handling."
    );
    Ok(c)
}

mod workflow;
pub use workflow::{
    Record, Setup, Slip, Status, apply_demo, demo_basis, payload, reset_reviews, source,
};
pub(crate) use workflow::{augment_annual_demo_basis, augment_benefit_demo_basis, guard_payment};

mod exports;
pub use exports::{export_csv, status, totals_rows};
