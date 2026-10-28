//! Government reconciliation arithmetic, separate from filings and payroll cash.
//! Adapters must supply a fresh protected snapshot; this module is not an API.
use anyhow::{Result, ensure};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;
pub mod annual;
pub mod annual_pay;
pub mod bir_declarations;
pub mod bir_file;
pub mod bir_identity;
mod export;
pub mod preparation;
#[cfg(any(test, feature = "qa-screenshot"))]
mod preview;
pub mod registration;
pub mod sss_collection;
pub mod sss_r3;
mod workflow;
#[cfg(any(test, feature = "qa-screenshot"))]
pub(crate) use preview::fixture as preview;
pub use workflow::{
    Action, Allocation, Basis, EmployeeLabel, Event, Filter, Receipt, ReceiptAllocation,
    Reconciliation, Review, ReviewWire,
};
pub use workflow::{decimal, parse_cents, validate_plan};
impl Agency {
    pub fn label(self) -> &'static str {
        match self {
            Self::Bir => "BIR",
            Self::Sss => "SSS",
            Self::Philhealth => "PhilHealth",
            Self::Pagibig => "Pag-IBIG",
        }
    }
}
impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Monthly => "Monthly salary",
            Self::Cutoff => "Cutoff salary",
            Self::Thirteenth => "13th-month payout",
            Self::Amendment => "Linked amendment",
            Self::FinalAdjustment => "Final tax adjustment",
            Self::FinalContribution => "Final contribution adjustment",
            Self::AnnualTax => "Annual tax settlement",
        }
    }
}
impl State {
    pub fn label(self) -> &'static str {
        match self {
            Self::Approved => "Approved, not received",
            Self::Received => "Received",
            Self::ReceiptMissing => "Individual receipt missing",
            Self::DatedAfterAsOf => "Receipt dated after report date",
            Self::Withdrawn => "Withdrawn",
            Self::ReviewOnly => "Review only",
        }
    }
}

const LIMIT: i64 = 1_000_000_000_000_000;
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Agency {
    Bir,
    Sss,
    Philhealth,
    Pagibig,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Monthly,
    Cutoff,
    Thirteenth,
    Amendment,
    FinalAdjustment,
    FinalContribution,
    AnnualTax,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Approved,
    Received,
    ReceiptMissing,
    DatedAfterAsOf,
    Withdrawn,
    ReviewOnly,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Parts {
    pub employee_cents: Option<i64>,
    pub employer_cents: Option<i64>,
    pub ec_cents: Option<i64>,
}
impl Parts {
    pub const ZERO: Self = Self {
        employee_cents: Some(0),
        employer_cents: Some(0),
        ec_cents: Some(0),
    };
    pub fn total(self) -> Option<i64> {
        self.employee_cents?
            .checked_add(self.employer_cents?)?
            .checked_add(self.ec_cents?)
    }
    fn values(self) -> [Option<i64>; 3] {
        [self.employee_cents, self.employer_cents, self.ec_cents]
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Fact {
    pub company_id: Uuid,
    pub employee_id: Uuid,
    pub id: Uuid,
    pub agency: Agency,
    pub kind: Kind,
    pub state: State,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    /// Explicit original coverage, never guessed from approval or payment dates.
    pub contribution_month: Option<NaiveDate>,
    /// A per-person actual receipt, including the source's final-offset receipt.
    /// Never another employee's batch-paid date. A callback/advice is not receipt.
    pub received_on: Option<NaiveDate>,
    pub parts: Parts,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Remittance {
    pub id: Uuid,
    pub company_id: Uuid,
    pub agency: Agency,
    pub month: NaiveDate,
    pub paid_on: NaiveDate,
    pub amount_cents: i64,
    pub voided: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Wire {
    pub version: String,
    pub company_id: Uuid,
    pub company_name: String,
    pub generated_at: DateTime<Utc>,
    pub as_of: NaiveDate,
    pub month: NaiveDate,
    pub agency: Agency,
    pub facts: Vec<Fact>,
}
impl Wire {
    pub fn validate(&self, company: Uuid, agency: Agency, month: NaiveDate) -> Result<()> {
        ensure!(
            self.version == "government-payroll-sources-v1"
                && self.company_id == company
                && !company.is_nil()
                && self.agency == agency
                && self.month == month
                && month_ok(month)
                && month <= self.as_of
                && self.facts.len() <= 20_000
                && !self.company_name.trim().is_empty()
                && self.company_name.chars().count() <= 160
                && !self.company_name.chars().any(char::is_control),
            "Government source response does not match the requested protected scope."
        );
        let mut seen = HashSet::new();
        for f in &self.facts {
            validate_fact(f, company, self.as_of)?;
            ensure!(
                f.agency == agency && seen.insert((f.kind, f.id, f.agency)),
                "Government response contains a foreign agency or duplicate immutable source."
            );
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Summary {
    /// Known values remain visible even when the complete liability is unknown.
    pub known_parts: Parts,
    pub complete: bool,
    pub source_count: usize,
    pub unallocated_count: usize,
    pub missing_receipt_count: usize,
    pub dated_after_as_of_count: usize,
    pub unreceived_count: usize,
    pub negative_balance_count: usize,
    pub remitted_cents: i64,
    /// Positive outstanding / negative excess; neither implies a credit/refund.
    pub difference_cents: Option<i64>,
}
fn month_ok(d: NaiveDate) -> bool {
    d.day() == 1 && (2025..=2200).contains(&d.year())
}
fn add(a: i64, b: i64) -> Result<i64> {
    let n = a
        .checked_add(b)
        .filter(|v| v.unsigned_abs() <= LIMIT as u64);
    ensure!(
        n.is_some(),
        "Government totals exceed the supported centavo limit."
    );
    Ok(n.unwrap())
}
fn validate_fact(f: &Fact, cid: Uuid, asof: NaiveDate) -> Result<()> {
    validate_fact_amounts(f, cid, asof, false)
}
// Annual recorded-source review must retain an unavailable legacy BIR amount
// as unknown. The older monthly reconciliation contract remains strict.
fn validate_fact_amounts(
    f: &Fact,
    cid: Uuid,
    asof: NaiveDate,
    allow_unknown_bir_withholding: bool,
) -> Result<()> {
    ensure!(
        f.company_id == cid && !f.id.is_nil() && !f.employee_id.is_nil(),
        "Government source is outside this company or lacks its immutable identity."
    );
    ensure!(
        f.starts_on <= f.ends_on
            && f.starts_on.year() >= 1900
            && f.ends_on.year() <= 2200
            && f.contribution_month.is_none_or(month_ok)
            && f.received_on
                .is_none_or(|d| (1900..=2200).contains(&d.year())),
        "Check government coverage and actual receipt dates."
    );
    for v in f.parts.values().into_iter().flatten() {
        ensure!(
            v.unsigned_abs() <= LIMIT as u64,
            "Government amount exceeds the supported centavo limit."
        );
    }
    if f.agency != Agency::Sss {
        ensure!(
            f.parts.ec_cents == Some(0),
            "EC belongs only to SSS; do not assign or hide an EC amount."
        );
    }
    if f.agency == Agency::Bir {
        ensure!(
            f.parts.employer_cents == Some(0),
            "BIR compensation withholding has no employer contribution share."
        );
    }
    if matches!(f.kind, Kind::Monthly | Kind::Cutoff) {
        let regular_parts_ok = if allow_unknown_bir_withholding && f.agency == Agency::Bir {
            f.parts.employee_cents.is_none_or(|n| n >= 0)
        } else {
            f.parts.values().iter().all(|v| v.is_some_and(|n| n >= 0))
        };
        ensure!(
            regular_parts_ok
                && f.starts_on.with_day(1) == f.ends_on.with_day(1)
                && (f.agency == Agency::Bir || f.contribution_month == f.starts_on.with_day(1)),
            "Regular contributions require known nonnegative shares and their actual earned month."
        );
    }
    ensure!(
        matches!(f.state, State::Received | State::DatedAfterAsOf) == f.received_on.is_some()
            && (f.state != State::Received || f.received_on.is_some_and(|d| d <= asof))
            && (f.state != State::DatedAfterAsOf || f.received_on.is_some_and(|d| d > asof)),
        "Only an actual individual receipt establishes a received date."
    );
    if f.state == State::ReviewOnly {
        ensure!(
            f.parts == Parts::ZERO,
            "An overpayment review is not a contribution charge, recovery or tax reduction."
        );
    }
    if matches!(f.kind, Kind::FinalAdjustment | Kind::AnnualTax) {
        ensure!(
            f.agency == Agency::Bir,
            "Final/tax aggregate advice cannot invent agency contribution shares."
        );
    }
    Ok(())
}
/// Reconcile one agency/month as of an explicit date. Negative contribution
/// corrections are retained, never converted into automatic government credits.
pub fn reconcile(
    cid: Uuid,
    agency: Agency,
    month: NaiveDate,
    asof: NaiveDate,
    facts: &[Fact],
    remittances: &[Remittance],
) -> Result<Summary> {
    ensure!(
        !cid.is_nil()
            && month_ok(month)
            && month <= asof.with_day(1).unwrap()
            && facts.len() <= 20_000
            && remittances.len() <= 5_000,
        "Use a bounded government month and source set."
    );
    let mut unique = HashSet::new();
    let mut regular = vec![];
    let mut s = Summary {
        known_parts: Parts::ZERO,
        complete: true,
        source_count: 0,
        unallocated_count: 0,
        missing_receipt_count: 0,
        dated_after_as_of_count: 0,
        unreceived_count: 0,
        negative_balance_count: 0,
        remitted_cents: 0,
        difference_cents: None,
    };
    let mut people: HashMap<Uuid, [i64; 3]> = HashMap::new();
    for f in facts {
        validate_fact(f, cid, asof)?;
        ensure!(
            unique.insert((f.kind, f.id, f.agency)),
            "Duplicate government source facts would count the same charge twice."
        );
        if f.agency != agency || matches!(f.state, State::Withdrawn | State::ReviewOnly) {
            continue;
        }
        if f.state == State::DatedAfterAsOf {
            s.dated_after_as_of_count += 1;
            s.complete = false;
        }
        if matches!(f.kind, Kind::Monthly | Kind::Cutoff) {
            for (eid, lo, hi) in &regular {
                ensure!(
                    *eid != f.employee_id || f.ends_on < *lo || f.starts_on > *hi,
                    "Overlapping monthly/cutoff contribution sources require review."
                );
            }
            regular.push((f.employee_id, f.starts_on, f.ends_on));
        }
        let selected = if agency == Agency::Bir {
            if f.state == State::ReceiptMissing {
                s.missing_receipt_count += 1;
                s.complete = false;
                false
            } else if f.state == State::DatedAfterAsOf {
                false
            } else if let Some(d) = f.received_on {
                d.with_day(1) == Some(month)
            } else {
                if f.ends_on.with_day(1) == Some(month) {
                    s.unreceived_count += 1;
                }
                false
            }
        } else if let Some(d) = f.contribution_month {
            d == month
        } else {
            // This cannot establish which month's liability changed. Preserve a
            // blocker until an evidenced allocation exists, including older years.
            if f.parts != Parts::ZERO {
                s.unallocated_count += 1;
                s.complete = false;
            }
            false
        };
        if !selected {
            continue;
        }
        s.source_count += 1;
        let person = people.entry(f.employee_id).or_default();
        for (i, v) in f.parts.values().into_iter().enumerate() {
            person[i] = add(person[i], v.unwrap_or(0))?;
        }
        let known = s.known_parts.values();
        let mut sums = [0; 3];
        for (i, v) in f.parts.values().into_iter().enumerate() {
            if v.is_none() {
                s.complete = false;
            }
            sums[i] = add(known[i].unwrap(), v.unwrap_or(0))?;
        }
        s.known_parts = Parts {
            employee_cents: Some(sums[0]),
            employer_cents: Some(sums[1]),
            ec_cents: Some(sums[2]),
        };
    }
    let mut receipts = HashSet::new();
    for r in remittances {
        ensure!(
            r.company_id == cid
                && !r.id.is_nil()
                && month_ok(r.month)
                && r.paid_on <= asof
                && (0..=LIMIT).contains(&r.amount_cents)
                && receipts.insert(r.id),
            "Check government receipt identity, date, amount and duplicates."
        );
        if !r.voided && r.agency == agency && r.month == month {
            s.remitted_cents = add(s.remitted_cents, r.amount_cents)?;
        }
    }
    if agency != Agency::Bir {
        s.negative_balance_count = people.values().filter(|p| p.iter().any(|n| *n < 0)).count();
        if s.negative_balance_count > 0 {
            s.complete = false;
        }
    }
    if s.complete {
        let expected = add(
            add(
                s.known_parts.employee_cents.unwrap(),
                s.known_parts.employer_cents.unwrap(),
            )?,
            s.known_parts.ec_cents.unwrap(),
        )?;
        s.difference_cents = Some(add(expected, -s.remitted_cents)?);
    }
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }
    fn cid() -> Uuid {
        Uuid::from_u128(1)
    }
    fn fact(id: u128, agency: Agency) -> Fact {
        Fact {
            company_id: cid(),
            employee_id: Uuid::from_u128(2),
            id: Uuid::from_u128(id),
            agency,
            kind: Kind::Cutoff,
            state: State::Received,
            starts_on: d(2026, 9, 1),
            ends_on: d(2026, 9, 15),
            contribution_month: Some(d(2026, 9, 1)),
            received_on: Some(d(2026, 10, 2)),
            parts: Parts {
                employee_cents: Some(100),
                employer_cents: Some(if agency == Agency::Bir { 0 } else { 200 }),
                ec_cents: Some(if agency == Agency::Sss { 30 } else { 0 }),
            },
        }
    }
    fn run(a: Agency, m: u32, f: &[Fact], r: &[Remittance]) -> Result<Summary> {
        reconcile(cid(), a, d(2026, m, 1), d(2026, 10, 25), f, r)
    }
    #[test]
    fn contribution_month_and_actual_withholding_month_stay_distinct() {
        assert_eq!(
            run(Agency::Sss, 9, &[fact(3, Agency::Sss)], &[])
                .unwrap()
                .difference_cents,
            Some(330)
        );
        assert_eq!(
            run(Agency::Sss, 10, &[fact(3, Agency::Sss)], &[])
                .unwrap()
                .difference_cents,
            Some(0)
        );
        assert_eq!(
            run(Agency::Bir, 9, &[fact(3, Agency::Bir)], &[])
                .unwrap()
                .difference_cents,
            Some(0)
        );
        assert_eq!(
            run(Agency::Bir, 10, &[fact(3, Agency::Bir)], &[])
                .unwrap()
                .difference_cents,
            Some(100)
        );
    }
    #[test]
    fn two_cutoffs_add_exactly_once_and_keep_employer_ec_distinct() {
        let a = fact(3, Agency::Sss);
        let mut b = a.clone();
        b.id = Uuid::from_u128(4);
        b.starts_on = d(2026, 9, 16);
        b.ends_on = d(2026, 9, 30);
        let s = run(Agency::Sss, 9, &[a.clone(), b], &[]).unwrap();
        assert_eq!(s.known_parts.employee_cents, Some(200));
        assert_eq!(s.known_parts.employer_cents, Some(400));
        assert_eq!(s.known_parts.ec_cents, Some(60));
        assert_eq!(s.difference_cents, Some(660));
        assert!(run(Agency::Sss, 9, &[a.clone(), a], &[]).is_err());
    }
    #[test]
    fn legacy_paid_batch_does_not_invent_a_persons_received_date() {
        let mut a = fact(3, Agency::Bir);
        a.received_on = None;
        a.state = State::ReceiptMissing;
        let s = run(Agency::Bir, 10, &[a], &[]).unwrap();
        assert_eq!(s.missing_receipt_count, 1);
        assert_eq!(s.difference_cents, None);
    }
    #[test]
    fn explicitly_postdated_receipt_remains_visible_without_current_withholding() {
        let mut a = fact(3, Agency::Bir);
        a.kind = Kind::Thirteenth;
        a.state = State::DatedAfterAsOf;
        a.received_on = Some(d(2027, 1, 2));
        let s = run(Agency::Bir, 10, &[a], &[]).unwrap();
        assert_eq!(s.dated_after_as_of_count, 1);
        assert_eq!(s.known_parts.employee_cents, Some(0));
        assert_eq!(s.difference_cents, None);
    }
    #[test]
    fn approval_is_not_withholding_but_earned_contributions_remain_due() {
        for agency in [Agency::Bir, Agency::Sss] {
            let mut a = fact(3, agency);
            a.state = State::Approved;
            a.received_on = None;
            let s = run(agency, 9, &[a], &[]).unwrap();
            assert_eq!(
                s.difference_cents,
                Some(if agency == Agency::Bir { 0 } else { 330 })
            );
            if agency == Agency::Bir {
                assert_eq!(s.unreceived_count, 1);
            }
        }
    }
    #[test]
    fn unknown_coverage_and_employer_adjustments_are_not_silent_zeros() {
        let mut a = fact(3, Agency::Sss);
        a.kind = Kind::Amendment;
        a.contribution_month = None;
        let s = run(Agency::Sss, 9, &[a.clone()], &[]).unwrap();
        assert_eq!(s.unallocated_count, 1);
        assert!(!s.complete);
        assert_eq!(s.difference_cents, None);
        a.contribution_month = Some(d(2026, 9, 1));
        a.parts.employer_cents = None;
        let s = run(Agency::Sss, 9, &[a], &[]).unwrap();
        assert_eq!(s.known_parts.employee_cents, Some(100));
        assert_eq!(s.difference_cents, None);
    }
    #[test]
    fn voided_receipts_do_not_reduce_liability_and_excess_is_not_auto_credit() {
        let r = Remittance {
            id: Uuid::from_u128(8),
            company_id: cid(),
            agency: Agency::Sss,
            month: d(2026, 9, 1),
            paid_on: d(2026, 10, 1),
            amount_cents: 500,
            voided: false,
        };
        assert_eq!(
            run(Agency::Sss, 9, &[fact(3, Agency::Sss)], &[r.clone()])
                .unwrap()
                .difference_cents,
            Some(-170)
        );
        let mut v = r.clone();
        v.voided = true;
        assert_eq!(
            run(Agency::Sss, 9, &[fact(3, Agency::Sss)], &[v])
                .unwrap()
                .difference_cents,
            Some(330)
        );
        assert!(run(Agency::Sss, 9, &[], &[r.clone(), r]).is_err());
    }
    #[test]
    fn final_adjustment_is_only_its_withholding_delta_and_can_cross_years() {
        let mut original = fact(3, Agency::Bir);
        original.starts_on = d(2025, 12, 1);
        original.ends_on = d(2025, 12, 31);
        original.contribution_month = None;
        let mut final_delta = original.clone();
        final_delta.kind = Kind::FinalAdjustment;
        final_delta.id = Uuid::from_u128(4);
        final_delta.parts.employee_cents = Some(-20);
        assert_eq!(
            run(Agency::Bir, 10, &[original, final_delta], &[])
                .unwrap()
                .difference_cents,
            Some(80)
        );
    }
    #[test]
    fn withdrawn_and_review_only_facts_create_no_charge_or_recovery() {
        let mut a = fact(3, Agency::Sss);
        a.kind = Kind::Amendment;
        a.state = State::Withdrawn;
        a.received_on = None;
        assert_eq!(
            run(Agency::Sss, 9, &[a.clone()], &[])
                .unwrap()
                .difference_cents,
            Some(0)
        );
        a.state = State::ReviewOnly;
        assert!(run(Agency::Sss, 9, &[a.clone()], &[]).is_err());
        a.parts = Parts::ZERO;
        assert_eq!(
            run(Agency::Sss, 9, &[a], &[]).unwrap().difference_cents,
            Some(0)
        );
    }
    #[test]
    fn foreign_overlapping_future_and_excessive_facts_fail_closed() {
        let mut a = fact(3, Agency::Sss);
        a.company_id = Uuid::from_u128(10);
        assert!(run(Agency::Sss, 9, &[a.clone()], &[]).is_err());
        a.company_id = cid();
        let mut b = a.clone();
        b.id = Uuid::from_u128(4);
        b.kind = Kind::Monthly;
        assert!(run(Agency::Sss, 9, &[a.clone(), b], &[]).is_err());
        a.received_on = Some(d(2026, 11, 1));
        assert!(run(Agency::Sss, 9, &[a.clone()], &[]).is_err());
        a.received_on = Some(d(2026, 10, 1));
        a.parts.employee_cents = Some(i64::MIN);
        assert!(run(Agency::Sss, 9, &[a], &[]).is_err());
    }
    #[test]
    #[ignore = "Requires the explicit local PostgreSQL source-vector check."]
    fn local_postgres_vectors_match_the_strict_rust_contract() {
        let path = std::env::var("HRIS_GOVERNMENT_VECTORS")
            .expect("Set the explicit local PostgreSQL vector path.");
        let vectors: Vec<Wire> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert!(vectors.len() >= 14);
        for wire in vectors {
            wire.validate(wire.company_id, wire.agency, wire.month)
                .unwrap();
        }
    }
}
