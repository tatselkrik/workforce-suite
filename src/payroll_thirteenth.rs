//! Current-employer 13th-month entitlement and shared annual benefit exclusion.
//! Source facts are issued earnings, not salary projections or accrual memos.
use anyhow::{Result, ensure};
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

pub const RULE_VERSION: &str = "PH-13th-2026-10-23-v1";
pub const BENEFIT_CAP: i64 = 9_000_000;
const LIMIT: i64 = 100_000_000_000;
mod workflow;
pub use workflow::{Item, Run, Setup, Slip, apply_demo, payload, published, source};
mod exports;
pub use exports::{export_csv, totals_rows};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Period {
    pub id: Uuid,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    pub basic_cents: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Prior {
    pub id: Uuid,
    pub gross_cents: i64,
    pub exempt_cents: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub starts_on: NaiveDate,
    pub as_of: NaiveDate,
    pub periods: Vec<Period>,
    pub prior: Vec<Prior>,
    pub external_thirteenth_floor: i64,
    pub other_benefits_floor: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Opening {
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    pub basic_cents: i64,
    pub reference: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Worksheet {
    pub openings: Vec<Opening>,
    pub classification: String,
    pub eligibility_note: String,
    pub served_one_month: bool,
    pub basic_adjustment_cents: i64,
    pub adjustment_note: String,
    pub policy_topup_cents: i64,
    pub policy_note: String,
    /// Same current employer's earlier off-system 13th-month payments only.
    pub external_thirteenth_cents: i64,
    /// Other benefits sharing the cap, including prior-employer benefit facts.
    pub other_benefits_cents: i64,
    pub opening_note: String,
    pub requested_gross_cents: Option<i64>,
    /// Reviewed supplementary/cumulative withholding, not a standalone tax table.
    pub withholding_cents: i64,
    pub tax_reviewed: bool,
    pub tax_note: String,
    /// Explicit reviewed month-level contribution adjustments, never a new full charge.
    pub sss_adjustment_cents: i64,
    pub philhealth_adjustment_cents: i64,
    pub pagibig_adjustment_cents: i64,
    pub contributions_reviewed: bool,
    pub contribution_note: String,
    pub reviewed: bool,
    pub review_note: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Gap {
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Calculation {
    pub basic_cents: i64,
    pub entitlement_cents: i64,
    pub prior_company_cents: i64,
    pub remaining_cents: i64,
    pub overpayment_cents: i64,
    pub gross_cents: i64,
    pub exempt_cents: i64,
    pub taxable_cents: i64,
    pub withholding_cents: i64,
    pub sss_adjustment_cents: i64,
    pub philhealth_adjustment_cents: i64,
    pub pagibig_adjustment_cents: i64,
    pub net_cents: i64,
    pub coverage_gaps: Vec<Gap>,
}
fn text(s: &str, max: usize, required: bool) -> Result<()> {
    ensure!(
        s.chars().count() <= max
            && (!required || !s.trim().is_empty())
            && !s.chars().any(|c| c.is_control() && c != '\n' && c != '\t'),
        "Provide a bounded readable review or evidence reference."
    );
    Ok(())
}
fn money(v: i64) -> Result<()> {
    ensure!(
        (0..=LIMIT).contains(&v),
        "Use a bounded nonnegative centavo amount."
    );
    Ok(())
}
pub fn calculate(s: &Source, w: &Worksheet) -> Result<Calculation> {
    ensure!(
        (2026..=2200).contains(&s.as_of.year())
            && s.starts_on.year() == s.as_of.year()
            && s.starts_on <= s.as_of
            && s.periods.len() <= 24
            && s.prior.len() <= 24
            && w.openings.len() <= 32,
        "Use a bounded current-year earnings coverage period."
    );
    text(&w.classification, 30, false)?;
    ensure!(
        ["", "rank_and_file", "company_policy"].contains(&w.classification.as_str()),
        "Review actual employment coverage independently of the account role."
    );
    for t in [
        &w.eligibility_note,
        &w.adjustment_note,
        &w.policy_note,
        &w.opening_note,
        &w.tax_note,
        &w.contribution_note,
        &w.review_note,
    ] {
        text(t, 2000, false)?;
    }
    for v in [
        w.policy_topup_cents,
        w.external_thirteenth_cents,
        w.other_benefits_cents,
        w.withholding_cents,
        w.sss_adjustment_cents,
        w.philhealth_adjustment_cents,
        w.pagibig_adjustment_cents,
        s.external_thirteenth_floor,
        s.other_benefits_floor,
    ] {
        money(v)?;
    }
    ensure!(
        (-LIMIT..=LIMIT).contains(&w.basic_adjustment_cents),
        "Basic adjustment is too large."
    );
    ensure!(
        w.basic_adjustment_cents == 0 || !w.adjustment_note.trim().is_empty(),
        "Document why earned basic requires adjustment."
    );
    ensure!(
        w.policy_topup_cents == 0 || !w.policy_note.trim().is_empty(),
        "Document the more favorable company benefit."
    );
    ensure!(
        w.external_thirteenth_cents >= s.external_thirteenth_floor
            && w.other_benefits_cents >= s.other_benefits_floor,
        "Earlier approved external payment and benefit facts cannot be reduced here."
    );
    ensure!(
        (w.external_thirteenth_cents == 0 && w.other_benefits_cents == 0)
            || !w.opening_note.trim().is_empty(),
        "Document prior payments and shared benefit-cap usage."
    );
    let mut dates = HashSet::new();
    let mut ids = HashSet::new();
    let mut basic = w.basic_adjustment_cents;
    let mut cover = |lo: NaiveDate, hi: NaiveDate, amount: i64| -> Result<()> {
        money(amount)?;
        ensure!(
            lo >= s.starts_on && hi <= s.as_of && lo <= hi,
            "Earnings/opening dates must fit the covered current-employer period."
        );
        let mut day = lo;
        loop {
            ensure!(
                dates.insert(day),
                "Opening and issued payroll periods cannot overlap."
            );
            if day == hi {
                break;
            }
            day = day.succ_opt().unwrap();
        }
        basic = basic
            .checked_add(amount)
            .ok_or_else(|| anyhow::anyhow!("Basic amount overflow."))?;
        Ok(())
    };
    for p in &s.periods {
        ensure!(
            ids.insert(p.id),
            "An issued payroll source appeared more than once."
        );
        cover(p.starts_on, p.ends_on, p.basic_cents)?;
    }
    for o in &w.openings {
        text(&o.reference, 500, true)?;
        cover(o.starts_on, o.ends_on, o.basic_cents)?;
    }
    money(basic)?;
    let mut gaps: Vec<Gap> = Vec::new();
    let mut day = s.starts_on;
    loop {
        if !dates.contains(&day) {
            if let Some(g) = gaps
                .last_mut()
                .filter(|g| g.ends_on.succ_opt() == Some(day))
            {
                g.ends_on = day;
            } else {
                gaps.push(Gap {
                    starts_on: day,
                    ends_on: day,
                });
            }
        }
        if day == s.as_of {
            break;
        }
        day = day.succ_opt().unwrap();
    }
    let mut prior = w.external_thirteenth_cents;
    let mut previous_exempt = 0i64;
    ids.clear();
    for p in &s.prior {
        money(p.gross_cents)?;
        money(p.exempt_cents)?;
        ensure!(
            ids.insert(p.id) && p.exempt_cents <= p.gross_cents,
            "Prior issued benefit facts do not reconcile."
        );
        prior = prior
            .checked_add(p.gross_cents)
            .ok_or_else(|| anyhow::anyhow!("Prior benefit overflow."))?;
        previous_exempt += p.exempt_cents;
    }
    money(prior)?;
    ensure!(
        previous_exempt <= BENEFIT_CAP,
        "Prior issued exemptions exceed the annual benefit cap."
    );
    let entitlement = crate::payroll::ratio(basic, 1, 12) + w.policy_topup_cents;
    money(entitlement)?;
    let remaining = (entitlement - prior).max(0);
    let gross = w.requested_gross_cents.unwrap_or(remaining);
    money(gross)?;
    ensure!(
        gross <= remaining,
        "Requested payout exceeds the unpaid current-employer entitlement."
    );
    let used = (prior + w.other_benefits_cents).min(BENEFIT_CAP).max(
        (previous_exempt + w.external_thirteenth_cents + w.other_benefits_cents).min(BENEFIT_CAP),
    );
    let exempt = gross.min(BENEFIT_CAP - used);
    let net = gross
        - w.withholding_cents
        - w.sss_adjustment_cents
        - w.philhealth_adjustment_cents
        - w.pagibig_adjustment_cents;
    ensure!(
        net >= 0,
        "Reviewed withholding and contribution adjustments exceed this payout."
    );
    Ok(Calculation {
        basic_cents: basic,
        entitlement_cents: entitlement,
        prior_company_cents: prior,
        remaining_cents: remaining,
        overpayment_cents: (prior - entitlement).max(0),
        gross_cents: gross,
        exempt_cents: exempt,
        taxable_cents: gross - exempt,
        withholding_cents: w.withholding_cents,
        sss_adjustment_cents: w.sss_adjustment_cents,
        philhealth_adjustment_cents: w.philhealth_adjustment_cents,
        pagibig_adjustment_cents: w.pagibig_adjustment_cents,
        net_cents: net,
        coverage_gaps: gaps,
    })
}
pub fn ready(s: &Source, w: &Worksheet) -> Result<Calculation> {
    let c = calculate(s, w)?;
    ensure!(
        c.coverage_gaps.is_empty(),
        "Review opening basic for every missing payroll period, including zero-paid intervals."
    );
    ensure!(
        c.overpayment_cents == 0 && c.gross_cents > 0,
        "Review earlier payments; there is no positive unpaid entitlement to issue."
    );
    ensure!(
        !w.classification.is_empty()
            && !w.eligibility_note.trim().is_empty()
            && (w.classification == "company_policy" || w.served_one_month),
        "Review actual coverage and the one-month service requirement; account roles do not determine eligibility."
    );
    ensure!(
        w.reviewed && !w.review_note.trim().is_empty(),
        "Review this employee's annual benefit worksheet."
    );
    ensure!(
        w.tax_reviewed && !w.tax_note.trim().is_empty(),
        "Record reviewed supplementary/cumulative withholding and its evidence, including a reviewed zero."
    );
    ensure!(
        w.contributions_reviewed && !w.contribution_note.trim().is_empty(),
        "Review month-level contribution reconciliation; do not deduct a second full monthly charge."
    );
    Ok(c)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn d(month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, month, day).unwrap()
    }
    fn source() -> Source {
        Source {
            starts_on: d(1, 1),
            as_of: d(9, 30),
            periods: vec![Period {
                id: Uuid::new_v4(),
                starts_on: d(2, 1),
                ends_on: d(9, 30),
                basic_cents: 24_000_000,
            }],
            prior: vec![],
            external_thirteenth_floor: 0,
            other_benefits_floor: 0,
        }
    }
    fn reviewed() -> Worksheet {
        Worksheet {
            openings: vec![Opening {
                starts_on: d(1, 1),
                ends_on: d(1, 31),
                basic_cents: 3_000_000,
                reference: "Reviewed January ledger".into(),
            }],
            classification: "rank_and_file".into(),
            eligibility_note: "Actual covered employment reviewed".into(),
            served_one_month: true,
            reviewed: true,
            review_note: "Issued earnings and opening reconciled".into(),
            tax_reviewed: true,
            tax_note: "Shared cap and supplementary withholding reviewed".into(),
            contributions_reviewed: true,
            contribution_note: "Existing monthly contribution records reconciled".into(),
            ..Default::default()
        }
    }
    #[test]
    fn actual_earned_basic_and_opening_produce_cumulative_entitlement() {
        let c = ready(&source(), &reviewed()).unwrap();
        assert_eq!(
            (c.basic_cents, c.entitlement_cents, c.net_cents),
            (27_000_000, 2_250_000, 2_250_000)
        );
    }
    #[test]
    fn missing_january_is_visible_and_never_silently_zero_for_issuance() {
        let mut w = reviewed();
        w.openings.clear();
        let c = calculate(&source(), &w).unwrap();
        assert_eq!(
            c.coverage_gaps,
            vec![Gap {
                starts_on: d(1, 1),
                ends_on: d(1, 31)
            }]
        );
        assert!(ready(&source(), &w).is_err());
        w.openings = vec![Opening {
            starts_on: d(1, 1),
            ends_on: d(1, 31),
            basic_cents: 0,
            reference: "Reviewed unpaid interval evidence".into(),
        }];
        assert!(ready(&source(), &w).is_ok());
    }
    #[test]
    fn overlapping_openings_and_duplicate_sources_are_rejected() {
        let mut w = reviewed();
        w.openings[0].ends_on = d(2, 1);
        assert!(calculate(&source(), &w).is_err());
        let mut s = source();
        s.periods.push(s.periods[0].clone());
        assert!(calculate(&s, &reviewed()).is_err());
    }
    #[test]
    fn prior_approved_payout_is_not_paid_twice() {
        let mut s = source();
        s.prior.push(Prior {
            id: Uuid::new_v4(),
            gross_cents: 1_000_000,
            exempt_cents: 1_000_000,
        });
        let c = ready(&s, &reviewed()).unwrap();
        assert_eq!(
            (c.prior_company_cents, c.gross_cents),
            (1_000_000, 1_250_000)
        );
        let mut w = reviewed();
        w.requested_gross_cents = Some(1_250_001);
        assert!(calculate(&s, &w).is_err());
    }
    #[test]
    fn annual_exclusion_is_shared_with_other_benefits_and_earlier_payments() {
        let mut s = source();
        s.prior.push(Prior {
            id: Uuid::new_v4(),
            gross_cents: 1_000_000,
            exempt_cents: 1_000_000,
        });
        let mut w = reviewed();
        w.other_benefits_cents = 7_500_000;
        w.opening_note = "Reviewed other benefits".into();
        let c = ready(&s, &w).unwrap();
        assert_eq!(
            (c.gross_cents, c.exempt_cents, c.taxable_cents),
            (1_250_000, 500_000, 750_000)
        );
    }
    #[test]
    fn eligibility_is_explicit_and_independent_of_access_role() {
        let mut w = reviewed();
        w.served_one_month = false;
        assert!(ready(&source(), &w).is_err());
        w.classification = "company_policy".into();
        assert!(ready(&source(), &w).is_ok());
        w.classification = "administrator".into();
        assert!(calculate(&source(), &w).is_err());
    }
    #[test]
    fn withholding_and_contributions_need_explicit_review_even_when_zero() {
        let mut w = reviewed();
        w.tax_reviewed = false;
        assert!(ready(&source(), &w).is_err());
        w.tax_reviewed = true;
        w.contribution_note.clear();
        assert!(ready(&source(), &w).is_err());
    }
    #[test]
    fn imported_prior_employer_benefits_do_not_increase_current_company_basic() {
        let mut w = reviewed();
        w.other_benefits_cents = 9_000_000;
        w.opening_note = "Prior-employer benefit facts, not current-employer basic".into();
        let c = ready(&source(), &w).unwrap();
        assert_eq!(
            (c.basic_cents, c.gross_cents, c.exempt_cents),
            (27_000_000, 2_250_000, 0)
        );
    }
    #[test]
    fn external_same_company_payment_and_benefit_facts_cannot_regress() {
        let mut s = source();
        s.external_thirteenth_floor = 100;
        assert!(calculate(&s, &reviewed()).is_err());
        let mut w = reviewed();
        w.external_thirteenth_cents = 100;
        w.opening_note = "Previously approved external payment".into();
        assert!(calculate(&s, &w).is_ok());
        s.other_benefits_floor = 50;
        assert!(calculate(&s, &w).is_err());
    }
    #[test]
    fn overpayment_is_visible_and_never_recovered_from_wages_automatically() {
        let mut s = source();
        s.prior.push(Prior {
            id: Uuid::new_v4(),
            gross_cents: 3_000_000,
            exempt_cents: 3_000_000,
        });
        let c = calculate(&s, &reviewed()).unwrap();
        assert_eq!((c.gross_cents, c.overpayment_cents), (0, 750_000));
        assert!(ready(&s, &reviewed()).is_err());
    }
    #[test]
    fn partial_payouts_and_reviewed_deductions_reconcile_net() {
        let mut w = reviewed();
        w.requested_gross_cents = Some(100_000);
        w.withholding_cents = 10_000;
        w.sss_adjustment_cents = 1_000;
        assert_eq!(ready(&source(), &w).unwrap().net_cents, 89_000);
        w.withholding_cents = 100_000;
        assert!(calculate(&source(), &w).is_err());
    }
    #[test]
    fn centavo_rounding_occurs_once_and_positive_policy_topup_is_documented() {
        let mut w = reviewed();
        w.openings[0].basic_cents += 6;
        let c = ready(&source(), &w).unwrap();
        assert_eq!(c.entitlement_cents, 2_250_001);
        w.policy_topup_cents = 100;
        assert!(calculate(&source(), &w).is_err());
        w.policy_note = "More favorable company policy".into();
        assert_eq!(ready(&source(), &w).unwrap().entitlement_cents, 2_250_101);
    }
}
