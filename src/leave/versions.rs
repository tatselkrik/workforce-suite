//! Prospective policy selection. Stored request charges/plans are never mutated.
//! Protected storage/UI adoption is separate from these deterministic selectors.
use super::{Accrual, DayCharge, DayPart, LeavePolicy, LeaveType};
use anyhow::{Result, ensure};
use chrono::{Datelike, Days, NaiveDate};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scheduled {
    pub id: Uuid,
    pub effective_on: NaiveDate,
    pub policy: LeavePolicy,
    pub withdrawn: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Timeline {
    /// Starting compatibility snapshot; never evidence of reconstructed history.
    pub baseline: LeavePolicy,
    pub versions: Vec<Scheduled>,
}
fn entitlement_changed(a: &LeaveType, b: &LeaveType) -> bool {
    a.use_employee_allowance != b.use_employee_allowance
        || a.annual_units != b.annual_units
        || a.accrual != b.accrual
        || a.carryover_units != b.carryover_units
        || a.carryover_expiry_month != b.carryover_expiry_month
}
impl Timeline {
    pub fn policy_on(&self, date: NaiveDate) -> &LeavePolicy {
        self.versions
            .iter()
            .filter(|v| !v.withdrawn && v.effective_on <= date)
            .max_by_key(|v| v.effective_on)
            .map_or(&self.baseline, |v| &v.policy)
    }

    pub fn validate(&self) -> Result<()> {
        self.baseline.validate(None)?;
        ensure!(
            !self.baseline.company_id.is_nil() && self.baseline.revision >= 0,
            "Invalid starting policy identity."
        );
        ensure!(
            self.versions.len() <= 1000,
            "Too many retained policy versions."
        );
        let mut ids = std::collections::HashSet::new();
        let mut active_dates = std::collections::HashSet::new();
        let mut previous = &self.baseline;
        let mut last_revision = self.baseline.revision;
        let mut last = None;
        for v in &self.versions {
            ensure!(
                !v.id.is_nil()
                    && ids.insert(v.id)
                    && v.policy.company_id == self.baseline.company_id
                    && v.policy.tracking_starts_year == self.baseline.tracking_starts_year
                    && (1900..=2200).contains(&v.effective_on.year())
                    && v.policy.revision > last_revision,
                "Invalid policy version identity or ordering."
            );
            last_revision = v.policy.revision;
            if v.withdrawn {
                v.policy.validate(None)?;
                continue;
            }
            ensure!(
                last.is_none_or(|d| v.effective_on > d) && active_dates.insert(v.effective_on),
                "Active policy dates must advance in order."
            );
            v.policy.validate(Some(previous))?;
            previous = &v.policy;
            last = Some(v.effective_on);
        }
        Ok(())
    }

    pub fn validate_schedule(&self, proposed: &Scheduled, today: NaiveDate) -> Result<()> {
        self.validate()?;
        ensure!(
            !proposed.withdrawn
                && proposed.effective_on >= today
                && proposed.effective_on.year() <= today.year() + 5,
            "Choose today or a future policy date within five years."
        );
        let previous = self.versions.iter().rev().find(|v| !v.withdrawn);
        if let Some(last) = previous {
            ensure!(
                proposed.effective_on > last.effective_on,
                "Append after the latest active version, or withdraw its future tail first."
            );
        }
        let old = previous.map_or(&self.baseline, |v| &v.policy);
        for t in &proposed.policy.types {
            let change = match old.leave_type(&t.code) {
                Some(before) => entitlement_changed(before, t),
                None => t.accrual != Accrual::Unlimited,
            };
            if change {
                ensure!(
                    proposed.effective_on.year() > today.year()
                        && proposed.effective_on.month() == 1
                        && proposed.effective_on.day() == 1,
                    "Grant, accrual and carry changes start on a future January1. Use audited credits for a reviewed current-year adjustment."
                );
            }
        }
        let mut next = self.clone();
        next.versions.push(proposed.clone());
        next.validate()
    }

    pub fn can_withdraw(
        &self,
        id: Uuid,
        today: NaiveDate,
        active_request_revisions: &[i64],
    ) -> bool {
        self.versions
            .iter()
            .rev()
            .find(|v| !v.withdrawn)
            .is_some_and(|v| {
                v.id == id
                    && v.effective_on > today
                    && !active_request_revisions.contains(&v.policy.revision)
            })
    }

    /// Calendar/type activation follows each charged date. The request-start
    /// version separately supplies its saved label/approval plan.
    pub fn charges(
        &self,
        code: &str,
        start: NaiveDate,
        end: NaiveDate,
        part: DayPart,
    ) -> Result<Vec<DayCharge>> {
        self.validate()?;
        ensure!(
            start <= end && start.year() == end.year() && (part == DayPart::Full || start == end),
            "Use one calendar year and one date for a half-day."
        );
        let mut charges = Vec::new();
        for n in 0..=end.signed_duration_since(start).num_days() as u64 {
            let date = start.checked_add_days(Days::new(n)).unwrap();
            let policy = self.policy_on(date);
            let t = policy.leave_type(code).ok_or_else(|| {
                anyhow::anyhow!("Leave type is unavailable on part of this date range.")
            })?;
            ensure!(
                t.active,
                "Split the request around dates where this leave type is inactive."
            );
            if policy.working(date) {
                charges.push(DayCharge {
                    date,
                    units: if part == DayPart::Full { 2 } else { 1 },
                });
            }
        }
        ensure!(
            !charges.is_empty(),
            "Choose at least one scheduled non-holiday workday."
        );
        Ok(charges)
    }
}
