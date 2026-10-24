//! Annual Philippine compensation tax reconciliation, using actual paid facts.
use anyhow::{Result, ensure};
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
mod workflow;
pub use workflow::{Record, Setup, Slip, apply_demo, payload, source};
pub(crate) use workflow::demo_basis;
mod exports;
pub use exports::{export_csv, totals_rows};
pub const RULE_VERSION: &str = "PH-annual-tax-2026-10-24-v1";
pub const LIMIT: i64 = 100_000_000_000;
pub const BENEFIT_CAP: i64 = 9_000_000;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Income {
    pub id: String,
    pub starts_on: Option<NaiveDate>,
    pub ends_on: Option<NaiveDate>,
    /// Taxable compensation already net of ordinary mandatory contributions.
    pub taxable_cents: i64,
    pub withholding_cents: i64,
    pub benefit_gross_cents: i64,
    pub benefit_exempt_cents: i64,
    /// Only additional mandatory contributions not already removed from taxable.
    pub contribution_cents: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub starts_on: NaiveDate,
    pub service_end: NaiveDate,
    pub as_of: NaiveDate,
    pub income: Vec<Income>,
    pub prior_adjustment_cents: i64,
    pub pending_payments: u32,
    pub pending_adjustments: u32,
    pub external_thirteenth_floor: i64,
    pub other_benefits_floor: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Opening {
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    pub taxable_cents: i64,
    pub withholding_cents: i64,
    pub benefit_gross_cents: i64,
    pub benefit_exempt_cents: i64,
    pub reference: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PreviousEmployer {
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    pub employer: String,
    pub taxable_cents: i64,
    /// Certified withholding after that employer's refund/annual adjustment.
    pub withholding_cents: i64,
    pub benefit_gross_cents: i64,
    pub benefit_exempt_cents: i64,
    pub reference: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Worksheet {
    pub openings: Vec<Opening>,
    pub previous_employers: Vec<PreviousEmployer>,
    /// Benefits currently included in regular taxable figures; never additional pay.
    pub reclassified_benefit_cents: i64,
    pub classification_note: String,
    pub taxable_adjustment_cents: i64,
    pub adjustment_note: String,
    pub last_pay_available_cents: i64,
    pub settlement_note: String,
    pub final_compensation_reviewed: bool,
    pub previous_employers_reviewed: bool,
    pub benefits_reviewed: bool,
    pub review_note: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Calculation {
    pub current_taxable_cents: i64,
    pub prior_taxable_cents: i64,
    pub benefit_gross_cents: i64,
    pub benefit_exempt_cents: i64,
    pub benefit_taxable_cents: i64,
    pub benefit_basis_adjustment_cents: i64,
    pub taxable_adjustment_cents: i64,
    pub taxable_cents: i64,
    pub annual_tax_cents: i64,
    pub current_withheld_cents: i64,
    pub prior_withheld_cents: i64,
    pub prior_adjustment_cents: i64,
    /// Positive = additional withholding; negative = refund advice.
    pub adjustment_cents: i64,
    pub additional_withholding_cents: i64,
    pub refund_cents: i64,
    pub available_deduction_cents: i64,
    pub employer_advance_cents: i64,
    pub unreconciled_benefits_cents: i64,
    pub coverage_gaps: Vec<crate::payroll_thirteenth::Gap>,
}
pub(super) fn text(s: &str, max: usize, required: bool) -> Result<()> {
    ensure!(
        s.chars().count() <= max
            && (!required || !s.trim().is_empty())
            && !s.chars().any(|c| c.is_control() && c != '\n' && c != '\t'),
        "Provide bounded readable annual tax review/evidence text."
    );
    Ok(())
}
fn money(n: i64) -> Result<()> {
    ensure!(
        (0..=LIMIT).contains(&n),
        "Use bounded nonnegative integer centavos."
    );
    Ok(())
}
/// BIR RR11-2018 annual schedule effective2023 onwards; never monthly thresholds.
pub fn annual_tax(taxable: i64) -> Result<i64> {
    money(taxable)?;
    let (floor, base, rate) = if taxable <= 25_000_000 {
        (0, 0, 0)
    } else if taxable <= 40_000_000 {
        (25_000_000, 0, 15)
    } else if taxable <= 80_000_000 {
        (40_000_000, 2_250_000, 20)
    } else if taxable <= 200_000_000 {
        (80_000_000, 10_250_000, 25)
    } else if taxable <= 800_000_000 {
        (200_000_000, 40_250_000, 30)
    } else {
        (800_000_000, 220_250_000, 35)
    };
    Ok(base + crate::payroll::ratio(taxable - floor, rate, 100))
}
fn amounts(taxable: i64, withheld: i64, gross: i64, exempt: i64) -> Result<()> {
    for n in [taxable, withheld, gross, exempt] {
        money(n)?;
    }
    ensure!(
        exempt <= gross && exempt <= BENEFIT_CAP && taxable >= gross - exempt,
        "Taxable and exempt benefit figures must reconcile with the certified income."
    );
    Ok(())
}
pub fn calculate(s: &Source, w: &Worksheet) -> Result<Calculation> {
    ensure!(
        (2026..=2200).contains(&s.as_of.year())
            && s.starts_on.year() == s.as_of.year()
            && s.service_end.year() == s.as_of.year()
            && s.starts_on <= s.service_end
            && s.service_end <= s.as_of
            && s.income.len() <= 120
            && w.openings.len() <= 32
            && w.previous_employers.len() <= 8,
        "Use a bounded, current-year employment/payment period."
    );
    for note in [
        &w.classification_note,
        &w.adjustment_note,
        &w.settlement_note,
        &w.review_note,
    ] {
        text(note, 2000, false)?;
    }
    for n in [
        w.reclassified_benefit_cents,
        w.last_pay_available_cents,
        s.external_thirteenth_floor,
        s.other_benefits_floor,
    ] {
        money(n)?;
    }
    ensure!(
        (-LIMIT..=LIMIT).contains(&w.taxable_adjustment_cents)
            && (-LIMIT..=LIMIT).contains(&s.prior_adjustment_cents),
        "Annual tax adjustments exceed the supported bounds."
    );
    ensure!(
        w.reclassified_benefit_cents == 0 || !w.classification_note.trim().is_empty(),
        "Evidence is required to reclassify benefits already included in taxable compensation."
    );
    ensure!(
        w.taxable_adjustment_cents == 0 || !w.adjustment_note.trim().is_empty(),
        "Document any correction to annual taxable compensation."
    );
    let mut c = Calculation::default();
    let mut dates = HashSet::new();
    let mut ids = HashSet::new();
    let mut current_benefits = 0i64;
    let mut taxed_benefits = 0i64;
    let mut contributions = 0i64;
    let mut cover = |lo: NaiveDate, hi: NaiveDate| -> Result<()> {
        ensure!(
            lo >= s.starts_on && hi <= s.service_end && lo <= hi,
            "Opening/issued coverage must fit current employment."
        );
        let mut d = lo;
        loop {
            ensure!(
                dates.insert(d),
                "Opening and paid payroll intervals cannot overlap."
            );
            if d == hi {
                break;
            }
            d = d.succ_opt().unwrap();
        }
        Ok(())
    };
    for p in &s.income {
        text(&p.id, 80, true)?;
        ensure!(ids.insert(p.id.clone()), "Duplicate paid source identity.");
        amounts(
            p.taxable_cents,
            p.withholding_cents,
            p.benefit_gross_cents,
            p.benefit_exempt_cents,
        )?;
        money(p.contribution_cents)?;
        match (p.starts_on, p.ends_on) {
            (Some(lo), Some(hi)) => cover(lo, hi)?,
            (None, None) => (),
            _ => anyhow::bail!("Paid coverage needs both dates, or neither."),
        }
        c.current_taxable_cents += p.taxable_cents;
        c.current_withheld_cents += p.withholding_cents;
        current_benefits += p.benefit_gross_cents;
        taxed_benefits += p.benefit_gross_cents - p.benefit_exempt_cents;
        contributions += p.contribution_cents;
    }
    let mut opening_benefits = 0i64;
    for p in &w.openings {
        text(&p.reference, 500, true)?;
        amounts(
            p.taxable_cents,
            p.withholding_cents,
            p.benefit_gross_cents,
            p.benefit_exempt_cents,
        )?;
        cover(p.starts_on, p.ends_on)?;
        c.current_taxable_cents += p.taxable_cents;
        c.current_withheld_cents += p.withholding_cents;
        opening_benefits += p.benefit_gross_cents;
        taxed_benefits += p.benefit_gross_cents - p.benefit_exempt_cents;
    }
    drop(cover);
    let mut prior_dates = HashSet::new();
    let mut prior_benefits = 0i64;
    for p in &w.previous_employers {
        text(&p.employer, 120, true)?;
        text(&p.reference, 500, true)?;
        amounts(
            p.taxable_cents,
            p.withholding_cents,
            p.benefit_gross_cents,
            p.benefit_exempt_cents,
        )?;
        ensure!(
            p.starts_on.year() == s.as_of.year()
                && p.starts_on <= p.ends_on
                && p.ends_on < s.starts_on,
            "Prior-employer certificates must cover successive employment before the current hire, in this tax year."
        );
        let mut d = p.starts_on;
        loop {
            ensure!(
                prior_dates.insert(d),
                "Overlapping/concurrent employers need separate tax review."
            );
            if d == p.ends_on {
                break;
            }
            d = d.succ_opt().unwrap();
        }
        c.prior_taxable_cents += p.taxable_cents;
        c.prior_withheld_cents += p.withholding_cents;
        prior_benefits += p.benefit_gross_cents;
        taxed_benefits += p.benefit_gross_cents - p.benefit_exempt_cents;
    }
    let current_taxed_benefits = s
        .income
        .iter()
        .map(|p| p.benefit_gross_cents - p.benefit_exempt_cents)
        .sum::<i64>()
        + w.openings
            .iter()
            .map(|p| p.benefit_gross_cents - p.benefit_exempt_cents)
            .sum::<i64>();
    ensure!(
        w.reclassified_benefit_cents <= c.current_taxable_cents - current_taxed_benefits,
        "Reclassification cannot exceed current compensation still included as regular taxable income."
    );
    current_benefits += opening_benefits + w.reclassified_benefit_cents;
    c.benefit_gross_cents = current_benefits + prior_benefits;
    c.benefit_exempt_cents = c.benefit_gross_cents.min(BENEFIT_CAP);
    c.benefit_taxable_cents = c.benefit_gross_cents - c.benefit_exempt_cents;
    c.benefit_basis_adjustment_cents =
        c.benefit_taxable_cents - taxed_benefits - w.reclassified_benefit_cents;
    c.taxable_adjustment_cents = w.taxable_adjustment_cents;
    let before_contributions = c.current_taxable_cents
        + c.prior_taxable_cents
        + c.benefit_basis_adjustment_cents
        + w.taxable_adjustment_cents;
    ensure!(
        before_contributions >= 0,
        "Corrections exceed annual taxable compensation."
    );
    let taxable = (before_contributions - contributions).max(0);
    ensure!(
        taxable >= 0,
        "Annual contributions/corrections exceed taxable compensation; review classification."
    );
    c.taxable_cents = taxable;
    c.annual_tax_cents = annual_tax(taxable)?;
    c.prior_adjustment_cents = s.prior_adjustment_cents;
    c.adjustment_cents = c.annual_tax_cents
        - c.current_withheld_cents
        - c.prior_withheld_cents
        - s.prior_adjustment_cents;
    c.additional_withholding_cents = c.adjustment_cents.max(0);
    c.refund_cents = (-c.adjustment_cents).max(0);
    c.available_deduction_cents = c
        .additional_withholding_cents
        .min(w.last_pay_available_cents);
    c.employer_advance_cents = c.additional_withholding_cents - c.available_deduction_cents;
    c.unreconciled_benefits_cents = (s.external_thirteenth_floor - opening_benefits).max(0).max(
        (s.external_thirteenth_floor + s.other_benefits_floor
            - opening_benefits
            - prior_benefits
            - w.reclassified_benefit_cents)
            .max(0),
    );
    for n in [
        c.current_taxable_cents,
        c.prior_taxable_cents,
        c.benefit_gross_cents,
        c.current_withheld_cents,
        c.prior_withheld_cents,
        c.additional_withholding_cents,
        c.refund_cents,
        contributions,
    ] {
        money(n)?;
    }
    let mut d = s.starts_on;
    loop {
        if !dates.contains(&d) {
            if let Some(g) = c
                .coverage_gaps
                .last_mut()
                .filter(|g| g.ends_on.succ_opt() == Some(d))
            {
                g.ends_on = d;
            } else {
                c.coverage_gaps.push(crate::payroll_thirteenth::Gap {
                    starts_on: d,
                    ends_on: d,
                });
            }
        }
        if d == s.service_end {
            break;
        }
        d = d.succ_opt().unwrap();
    }
    Ok(c)
}
pub fn ready(s: &Source, w: &Worksheet) -> Result<Calculation> {
    let c = calculate(s, w)?;
    ensure!(
        c.coverage_gaps.is_empty(),
        "Evidence is required for all missing paid employment intervals, including zero-income coverage."
    );
    ensure!(
        s.pending_payments == 0 && s.pending_adjustments == 0,
        "Record outstanding issued payments/earlier tax settlements before closing annual tax."
    );
    ensure!(
        c.unreconciled_benefits_cents == 0,
        "Reconcile benefit facts already used by approved13th-month pay."
    );
    ensure!(
        w.final_compensation_reviewed
            && w.previous_employers_reviewed
            && w.benefits_reviewed
            && !w.review_note.trim().is_empty()
            && !w.settlement_note.trim().is_empty(),
        "Review final paid compensation, prior-employer certificates, benefits and settlement arrangements, including zero/no previous employer."
    );
    Ok(c)
}
