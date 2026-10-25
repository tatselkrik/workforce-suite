//! Exact takeover arithmetic for reviewed leave opening balances.
//!
//! This calculation alone posts nothing. Protected preview/posting, policy-date
//! selection and scoped native context must be in place before it is integrated.
use super::Balance;
use anyhow::{Result, ensure};
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

const MAX_BASE_UNITS: i32 = 4_000_000; // Bounded existing credit ledger plus grant.
const MAX_YEAR_UNITS: i32 = 732;

/// Only active pending/approved filed requests, charged under their saved dates.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Booking {
    pub request_id: Uuid,
    pub units: i32,
    pub early_units: i32,
    pub approved: bool,
}

/// Immutable minimum calculation basis. HR reasons/source references are separate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Opening {
    pub id: Uuid,
    pub employee_id: Uuid,
    pub type_code: String,
    pub effective_on: NaiveDate,
    pub carry_expires_on: NaiveDate,
    pub base_earned_at_opening: i32,
    pub target_available_units: i32,
    pub remaining_carry_units: i32,
    pub previously_consumed_carry_units: i32,
    pub baseline: Vec<Booking>,
}

pub struct Capture<'a> {
    pub id: Uuid,
    pub employee_id: Uuid,
    pub type_code: &'a str,
    pub effective_on: NaiveDate,
    pub carry_expires_on: NaiveDate,
    pub joined_on: Option<NaiveDate>,
    pub base_earned_units: i32,
    pub legacy_gross_carry_units: i32,
    pub carry_cap_units: i32,
    /// The target already accounts for every displayed HRIS reservation.
    pub target_available_units: i32,
    /// Unspent expiring carry inside that target, not a second grant.
    pub remaining_carry_units: i32,
    pub source_outstanding_requests_reviewed: bool,
    pub bookings: &'a [Booking],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Projection {
    pub balance: Balance,
    /// Only this component may feed next year's capped carry. Never carry carry.
    pub unused_base_units: i32,
    pub unspent_carry_units: i32,
    pub consumed_carry_units: i32,
}

fn code(s: &str) -> bool {
    (1..=30).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}
fn bookings(rows: &[Booking]) -> Result<(i32, i32, i32)> {
    ensure!(rows.len() <= 732, "Too many active leave bookings.");
    let mut ids = HashSet::new();
    let (mut total, mut early, mut approved) = (0i64, 0i64, 0i64);
    for r in rows {
        ensure!(
            !r.request_id.is_nil()
                && ids.insert(r.request_id)
                && (1..=MAX_YEAR_UNITS).contains(&r.units)
                && (0..=r.units).contains(&r.early_units),
            "Invalid or duplicate saved leave booking."
        );
        total += i64::from(r.units);
        early += i64::from(r.early_units);
        if r.approved {
            approved += i64::from(r.units);
        }
    }
    ensure!(
        total <= i64::from(MAX_YEAR_UNITS),
        "Year bookings exceed one full calendar year."
    );
    Ok((total as i32, early as i32, approved as i32))
}

impl Opening {
    pub fn capture(input: Capture<'_>) -> Result<Self> {
        ensure!(
            input.source_outstanding_requests_reviewed,
            "Review outstanding source-system requests before posting an opening."
        );
        let (_, early, _) = bookings(input.bookings)?;
        ensure!(
            (0..=MAX_BASE_UNITS).contains(&input.base_earned_units)
                && (0..=MAX_YEAR_UNITS).contains(&input.target_available_units)
                && (0..=input.target_available_units).contains(&input.remaining_carry_units)
                && (0..=MAX_YEAR_UNITS).contains(&input.carry_cap_units)
                && (0..=input.carry_cap_units).contains(&input.legacy_gross_carry_units),
            "Opening requires a supported grant and carry basis."
        );
        let consumed = input.legacy_gross_carry_units.min(early);
        ensure!(
            consumed + input.remaining_carry_units <= input.carry_cap_units,
            "Remaining imported carry plus already consumed carry exceeds this policy's cap."
        );
        if consumed > 0 || input.remaining_carry_units > 0 {
            ensure!(
                input
                    .joined_on
                    .is_some_and(|j| j.year() < input.effective_on.year()),
                "Carry takeover needs a known prior-year service date."
            );
        }
        if let Some(j) = input.joined_on {
            ensure!(
                j <= input.effective_on,
                "An opening cannot precede employment."
            );
        }
        let opening = Self {
            id: input.id,
            employee_id: input.employee_id,
            type_code: input.type_code.into(),
            effective_on: input.effective_on,
            carry_expires_on: input.carry_expires_on,
            base_earned_at_opening: input.base_earned_units,
            target_available_units: input.target_available_units,
            remaining_carry_units: input.remaining_carry_units,
            previously_consumed_carry_units: consumed,
            baseline: input.bookings.to_vec(),
        };
        opening.validate()?;
        Ok(opening)
    }

    pub fn validate(&self) -> Result<()> {
        let (_, early, _) = bookings(&self.baseline)?;
        ensure!(
            !self.id.is_nil()
                && !self.employee_id.is_nil()
                && code(&self.type_code)
                && (1900..=2200).contains(&self.effective_on.year())
                && self.carry_expires_on.year() == self.effective_on.year()
                && self
                    .carry_expires_on
                    .succ_opt()
                    .is_some_and(|d| d.day() == 1)
                && (0..=MAX_BASE_UNITS).contains(&self.base_earned_at_opening)
                && (0..=MAX_YEAR_UNITS).contains(&self.target_available_units)
                && (0..=self.target_available_units).contains(&self.remaining_carry_units)
                && (0..=early).contains(&self.previously_consumed_carry_units)
                && self.previously_consumed_carry_units + self.remaining_carry_units
                    <= MAX_YEAR_UNITS,
            "Invalid immutable opening-balance basis."
        );
        ensure!(
            self.effective_on <= self.carry_expires_on || self.remaining_carry_units == 0,
            "Expired carry cannot be imported as available leave."
        );
        Ok(())
    }

    /// `base_earned_now` must come from the protected dated entitlement/credit
    /// calculation, not a new projection of historical employee allowances.
    pub fn project(
        &self,
        as_of: NaiveDate,
        base_earned_now: i32,
        current: &[Booking],
    ) -> Result<Projection> {
        self.validate()?;
        ensure!(
            as_of >= self.effective_on
                && as_of.year() == self.effective_on.year()
                && (-MAX_BASE_UNITS..=MAX_BASE_UNITS).contains(&base_earned_now),
            "Use the opening only within its year and on/after its effective date."
        );
        let (initial_total, _, _) = bookings(&self.baseline)?;
        let (total, early, used) = bookings(current)?;
        let baseline: HashMap<_, _> = self.baseline.iter().map(|r| (r.request_id, r)).collect();
        let mut baseline_early = 0;
        for r in current {
            if let Some(original) = baseline.get(&r.request_id) {
                ensure!(
                    r.units == original.units && r.early_units == original.early_units,
                    "A baseline request's saved charges changed; refresh and review its history."
                );
                baseline_early += r.early_units;
            }
        }
        let h = self.previously_consumed_carry_units;
        let gross_carry = h + self.remaining_carry_units;
        // Baseline bookings retain their reviewed funding. Newly imported carry
        // cannot retroactively fund old base-charged bookings and become base.
        let baseline_carry = h.min(baseline_early);
        let consumed_carry =
            baseline_carry + (early - baseline_early).min(gross_carry - baseline_carry);
        let virtual_base = i64::from(self.target_available_units - self.remaining_carry_units)
            + i64::from(base_earned_now)
            - i64::from(self.base_earned_at_opening)
            + i64::from(initial_total - h);
        let unused_base = virtual_base - i64::from(total) + i64::from(consumed_carry);
        let unspent_carry = if as_of <= self.carry_expires_on {
            gross_carry - consumed_carry
        } else {
            0
        };
        let available = unused_base + i64::from(unspent_carry);
        Ok(Projection {
            balance: Balance {
                earned_units: i32::try_from(virtual_base)?,
                carryover_units: if as_of <= self.carry_expires_on {
                    gross_carry
                } else {
                    0
                },
                used_units: used,
                reserved_units: total - used,
                available_units: i32::try_from(available)?,
            },
            unused_base_units: i32::try_from(unused_base)?,
            unspent_carry_units: unspent_carry,
            consumed_carry_units: consumed_carry,
        })
    }
}
