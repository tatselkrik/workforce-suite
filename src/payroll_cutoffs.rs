//! Semi-monthly payroll. Issued first-cutoff facts are inputs to monthly
//! contribution/cap reconciliation; they are never recomputed or overwritten.
use crate::payroll::{self, Pay, Totals, Worksheet, month_end, ratio};
use crate::{
    access::Role,
    domain::{Mode, Snapshot},
};
use anyhow::{Context, Result, ensure};
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

pub const RULE_VERSION: &str = "PH-semi-2026-10-23-v1";
mod demo;
pub use demo::apply as apply_demo;
mod exports;
pub use exports::export_csv;
pub fn bounds(month: NaiveDate, part: u8) -> Result<(NaiveDate, NaiveDate)> {
    ensure!(
        month.day() == 1 && (2026..=2200).contains(&month.year()) && (1..=2).contains(&part),
        "Choose a supported month first and cutoff 1 or 2."
    );
    Ok(if part == 1 {
        (month, month.with_day(15).unwrap())
    } else {
        (month.with_day(16).unwrap(), month_end(month))
    })
}
/// Half-up centavo rounding follows the existing engine and Annex E's
/// semi-monthly table effective 2023 onwards, rather than halving monthly tax.
pub fn semi_monthly_tax(t: i64) -> i64 {
    if t <= 1_041_700 {
        0
    } else if t < 1_666_700 {
        ratio(t - 1_041_700, 15, 100)
    } else if t < 3_333_300 {
        93_750 + ratio(t - 1_666_700, 20, 100)
    } else if t < 8_333_300 {
        427_070 + ratio(t - 3_333_300, 25, 100)
    } else if t < 33_333_300 {
        1_677_070 + ratio(t - 8_333_300, 30, 100)
    } else {
        9_177_070 + ratio(t - 33_333_300, 35, 100)
    }
}
pub fn allocated(amount: i64, part: u8) -> i64 {
    if part == 1 {
        amount / 2
    } else {
        amount - amount / 2
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Calculation {
    pub month: NaiveDate,
    pub part: u8,
    pub totals: Totals,
    pub rice_cents: i64,
    pub laundry_cents: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Run {
    #[serde(flatten)]
    pub base: payroll::Run,
    pub part: u8,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    pub pay_date: NaiveDate,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Item {
    #[serde(flatten)]
    pub base: payroll::Item,
    pub calculation: Calculation,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Slip {
    #[serde(flatten)]
    pub base: payroll::Slip,
    pub part: u8,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    pub pay_date: NaiveDate,
    pub calculation: Calculation,
}
#[derive(Clone, Debug, Default)]
pub struct Setup {
    pub runs: Vec<Run>,
    pub items: Vec<Item>,
    pub slips: Vec<Slip>,
}
pub fn previous(i: &payroll::Item) -> Result<Option<Calculation>> {
    let v = &i.source_basis["first_cutoff"]["calculation"];
    Ok(if v.is_null() {
        None
    } else {
        Some(serde_json::from_value(v.clone())?)
    })
}
fn text(v: &Value, key: &str, max: usize) -> Result<()> {
    let s = v[key].as_str().unwrap_or("");
    ensure!(
        !s.trim().is_empty()
            && s.chars().count() <= max
            && !s.chars().any(|c| c.is_control() && c != '\n' && c != '\t'),
        "Enter {key} within {max} characters."
    );
    Ok(())
}
pub fn payload(a: &payroll::PayrollAction, data: &Snapshot) -> Result<Value> {
    ensure!(
        data.mode != Mode::SignedOut
            && data.access.allows(crate::staff_roles::Capability::PayrollPreparation)
            && data.access.company_id == data.company.id,
        "Only HR and Administrators manage cutoff payroll."
    );
    let op = a
        .operation
        .strip_prefix("cutoff_")
        .context("Unknown cutoff operation.")?;
    let fields: &[&str] = match op {
        "run_create" => &["month", "part", "pay_date", "title"],
        "run_update" => &["pay_date", "title"],
        "item_add" => &["run_id", "employee_id"],
        "item_save" => &["worksheet"],
        "run_submit" => &["scope_reviewed"],
        "run_paid" => &["paid_on", "payment_reference"],
        "item_refresh" | "item_remove" | "run_return" | "run_cancel" | "run_approve" => &[],
        _ => anyhow::bail!("Unknown cutoff operation."),
    };
    let obj = a.record.as_object().context("Enter cutoff details.")?;
    ensure!(
        a.record.to_string().len() <= 131072
            && obj.keys().all(
                |k| ["id", "expected_revision", "reason"].contains(&k.as_str())
                    || fields.contains(&k.as_str())
            ),
        "Unsupported or excessive cutoff fields."
    );
    let id: Uuid = serde_json::from_value(a.record["id"].clone())?;
    ensure!(!id.is_nil(), "Record identity required.");
    text(&a.record, "reason", 500)?;
    if !a.record["expected_revision"].is_null() {
        ensure!(
            a.record["expected_revision"]
                .as_i64()
                .is_some_and(|n| n >= 0),
            "Refresh the cutoff revision."
        );
    }
    if op == "run_create" || op == "run_update" {
        let (month, part) = if op == "run_create" {
            (
                serde_json::from_value(a.record["month"].clone())?,
                serde_json::from_value(a.record["part"].clone())?,
            )
        } else {
            let r = data
                .payroll
                .cutoffs
                .runs
                .iter()
                .find(|r| r.base.id == id)
                .context("Refresh this cutoff.")?;
            (r.base.month, r.part)
        };
        ensure!(
            month >= NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(),
            "Use February2026 or later."
        );
        let (_, end) = bounds(month, part)?;
        let date: NaiveDate = serde_json::from_value(a.record["pay_date"].clone())?;
        ensure!(
            date >= end && date <= end + chrono::Duration::days(16),
            "Pay date must be on/after cutoff end, within16days."
        );
        text(&a.record, "title", 120)?;
    }
    if op == "item_save" {
        let i = data
            .payroll
            .cutoffs
            .items
            .iter()
            .find(|i| i.base.id == id)
            .context("Refresh this worksheet.")?;
        let r = data
            .payroll
            .cutoffs
            .runs
            .iter()
            .find(|r| r.base.id == i.base.run_id)
            .context("Refresh the cutoff.")?;
        let w: Worksheet = serde_json::from_value(a.record["worksheet"].clone())?;
        calculate(
            &i.base.profile.pay,
            &w,
            r.base.month,
            r.part,
            previous(&i.base)?.as_ref(),
        )?;
    }
    if op == "run_submit" {
        ensure!(
            a.record["scope_reviewed"] == true,
            "Review included/omitted employees before submitting."
        );
    }
    if op == "run_approve" || op == "run_paid" {
        ensure!(
            data.access.role == Role::Administrator,
            "Only Administrators approve or record cutoff payment."
        );
    }
    if op == "run_paid" {
        let _: NaiveDate = serde_json::from_value(a.record["paid_on"].clone())?;
        text(&a.record, "payment_reference", 500)?;
    }
    Ok(json!({"p_operation":op,"p_record":a.record}))
}
pub fn calculate(
    p: &Pay,
    w: &Worksheet,
    month: NaiveDate,
    part: u8,
    previous: Option<&Calculation>,
) -> Result<Calculation> {
    p.validate()?;
    w.validate(p, month)?;
    let (start, end) = bounds(month, part)?;
    ensure!(
        w.days.iter().all(|d| d.date >= start && d.date <= end),
        "Every worksheet date must be inside this cutoff."
    );
    ensure!(
        part != 1 || previous.is_none(),
        "First cutoff cannot take previous-cutoff amounts."
    );
    ensure!(
        previous.is_none_or(|v| part == 2 && v.month == month && v.part == 1),
        "Reconcile only this month's issued first cutoff."
    );
    ensure!(
        part != 1
            || [
                w.sss_basis_override,
                w.philhealth_basis_override,
                w.pagibig_basis_override
            ]
            .iter()
            .all(Option::is_none),
        "Monthly contribution basis overrides belong to the second cutoff."
    );
    ensure!(
        w.basic_override_cents
            .is_none_or(|v| v <= allocated(p.rate_cents, part)),
        "Cutoff basic proration cannot exceed the allocated half salary."
    );
    let mut period_pay = p.clone();
    for c in &mut period_pay.components {
        c.amount_cents = allocated(c.amount_cents, part);
    }
    // Keep the full monthly/daily hourly reference for premiums. Only basic
    // salary and recurring components are allocated between the two cutoffs.
    period_pay.sss = false;
    period_pay.philhealth = false;
    period_pay.pagibig = false;
    period_pay.coverage_note =
        "Cutoff allocation; monthly contributions reconciled on cutoff 2".into();
    let mut period = w.clone();
    if p.basis == "monthly" {
        period.basic_override_cents = Some(
            w.basic_override_cents
                .unwrap_or(allocated(p.rate_cents, part)),
        );
    }
    period.sss_basis_override = None;
    period.philhealth_basis_override = None;
    period.pagibig_basis_override = None;
    period.exception_note =
        "Internal cutoff allocation; original HR review retained separately".into();
    let mut t = payroll::calculate(&period_pay, &period, month)?;
    let rice = period_pay
        .components
        .iter()
        .chain(&w.adjustments)
        .filter(|c| c.kind == "rice")
        .map(|c| c.amount_cents)
        .sum::<i64>();
    let laundry = period_pay
        .components
        .iter()
        .chain(&w.adjustments)
        .filter(|c| c.kind == "laundry")
        .map(|c| c.amount_cents)
        .sum::<i64>();
    let prior = previous.map(|v| v.totals.clone()).unwrap_or_default();
    let prior_rice = previous.map_or(0, |v| v.rice_cents);
    let prior_laundry = previous.map_or(0, |v| v.laundry_cents);
    t.exempt_allowances = (rice + prior_rice).min(250_000) - prior_rice.min(250_000)
        + (laundry + prior_laundry).min(40_000)
        - prior_laundry.min(40_000);
    if part == 2 {
        let mut monthly_pay = p.clone();
        monthly_pay.components.clear();
        let monthly = Worksheet {
            sss_basis_override:Some(w.sss_basis_override.unwrap_or(prior.gross+t.gross)),
            philhealth_basis_override:Some(w.philhealth_basis_override.unwrap_or(if p.basis=="monthly" {p.rate_cents} else {p.monthly_reference_cents})),
            pagibig_basis_override:Some(w.pagibig_basis_override.unwrap_or(prior.basic+prior.paid_leave+prior.allowances+t.basic+t.paid_leave+t.allowances)),
            exception_note:"Monthly contribution reconciliation from issued first cutoff and current reviewed cutoff".into(),
            ..Default::default()
        };
        let s = payroll::calculate(&monthly_pay, &monthly, month)?;
        t.sss_basis = s.sss_basis;
        t.sss_msc = s.sss_msc;
        t.sss_employee = s.sss_employee;
        t.sss_employer = s.sss_employer;
        t.sss_regular_employee = s.sss_regular_employee;
        t.sss_regular_employer = s.sss_regular_employer;
        t.sss_mpf_employee = s.sss_mpf_employee;
        t.sss_mpf_employer = s.sss_mpf_employer;
        t.ec = s.ec;
        t.philhealth_employee = s.philhealth_employee;
        t.philhealth_employer = s.philhealth_employer;
        t.pagibig_employee = s.pagibig_employee;
        t.pagibig_employer = s.pagibig_employer;
        t.thirteenth_accrual = ratio(prior.basic + t.basic, 1, 12) - prior.thirteenth_accrual;
    } else {
        t.sss_basis = 0;
    }
    let statutory = t.sss_employee + t.philhealth_employee + t.pagibig_employee;
    t.taxable = (if p.minimum_wage_earner {
        t.allowances - t.exempt_allowances
    } else {
        t.gross - t.exempt_allowances
    } - statutory)
        .max(0);
    t.withholding = semi_monthly_tax(t.taxable);
    t.net =
        t.gross + t.reimbursement - statutory - t.withholding - t.voluntary_deductions - t.advances;
    Ok(Calculation {
        month,
        part,
        totals: t,
        rice_cents: rice,
        laundry_cents: laundry,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::payroll::{Component, Day};
    fn month() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 9, 1).unwrap()
    }
    fn pay() -> Pay {
        Pay {
            rate_cents: 3_000_001,
            ..Default::default()
        }
    }
    #[test]
    fn odd_cent_salary_and_recurring_amounts_allocate_exactly_once() {
        let mut p = pay();
        p.components = vec![Component {
            label: "Allowance".into(),
            kind: "taxable_allowance".into(),
            amount_cents: 501,
            reference: String::new(),
        }];
        let a = calculate(&p, &Worksheet::default(), month(), 1, None).unwrap();
        let b = calculate(&p, &Worksheet::default(), month(), 2, Some(&a)).unwrap();
        assert_eq!(a.totals.basic + b.totals.basic, p.rate_cents);
        assert_eq!(a.totals.allowances + b.totals.allowances, 501);
        assert_eq!(a.totals.sss_employee, 0);
        assert_eq!(b.totals.sss_employee, 150000);
        assert_eq!(
            a.totals.philhealth_employee + b.totals.philhealth_employee,
            75000
        );
        assert_eq!(
            a.totals.thirteenth_accrual + b.totals.thirteenth_accrual,
            ratio(p.rate_cents, 1, 12)
        );
    }
    #[test]
    fn second_cutoff_uses_issued_first_gross_and_a_single_monthly_exemption_cap() {
        let mut p = pay();
        p.components = vec![Component {
            label: "Rice".into(),
            kind: "rice".into(),
            amount_cents: 600000,
            reference: String::new(),
        }];
        let a = calculate(&p, &Worksheet::default(), month(), 1, None).unwrap();
        let b = calculate(&p, &Worksheet::default(), month(), 2, Some(&a)).unwrap();
        assert_eq!(a.totals.exempt_allowances, 250000);
        assert_eq!(b.totals.exempt_allowances, 0);
        assert_eq!(b.totals.sss_basis, a.totals.gross + b.totals.gross);
        assert_eq!(b.totals.sss_employee, 175000);
        assert_eq!(b.totals.pagibig_employee, 20000);
    }
    #[test]
    fn semi_monthly_table_has_its_own_thresholds_and_boundary_bases() {
        for (t, want) in [
            (1041700, 0),
            (1041800, 15),
            (1666700, 93750),
            (3333300, 427070),
            (8333300, 1677070),
            (33333300, 9177070),
        ] {
            assert_eq!(semi_monthly_tax(t), want);
        }
        assert_ne!(semi_monthly_tax(1500000), payroll::monthly_tax(1500000));
    }
    #[test]
    fn premiums_keep_full_salary_hourly_reference_and_period_dates_are_enforced() {
        let p = pay();
        let day = Day {
            date: month(),
            kind: "ordinary".into(),
            overtime_minutes: 60,
            ..Default::default()
        };
        let w = Worksheet {
            days: vec![day],
            ..Default::default()
        };
        let a = calculate(&p, &w, month(), 1, None).unwrap();
        assert_eq!(
            a.totals.overtime,
            payroll::calculate(&p, &w, month()).unwrap().overtime
        );
        assert!(calculate(&p, &w, month(), 2, None).is_err());
    }
    #[test]
    fn a_daily_new_joiner_can_have_only_second_cutoff_without_projecting_absences() {
        let p = Pay {
            basis: "daily".into(),
            rate_cents: 80000,
            monthly_reference_cents: 2080000,
            ..Default::default()
        };
        let w = Worksheet {
            days: vec![Day {
                date: month().with_day(16).unwrap(),
                kind: "ordinary".into(),
                regular_minutes: 480,
                ..Default::default()
            }],
            ..Default::default()
        };
        let a = calculate(&p, &w, month(), 2, None).unwrap();
        assert_eq!(a.totals.basic, 80000);
        assert_eq!(a.totals.sss_basis, 80000);
    }
}
