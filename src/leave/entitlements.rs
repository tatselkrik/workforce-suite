//! A captured current-year calculation basis, never invented historical grants.
use super::{Accrual, LeaveType};
use crate::domain::Employee;
use anyhow::{Result, ensure};
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrozenBasis {
    pub known_from: NaiveDate,
    pub rate_units: i32,
    pub accrual: Accrual,
    pub joined_on: Option<NaiveDate>,
    /// Legacy calculation result as of capture, not a claim about its grant date.
    pub earned_rate_at_capture: i32,
}
fn full_months(year: i32, on: NaiveDate, joined: NaiveDate) -> i32 {
    (1..=12)
        .filter(|m| {
            let first = NaiveDate::from_ymd_opt(year, *m, 1).unwrap();
            let end = if *m == 12 {
                NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap()
            } else {
                NaiveDate::from_ymd_opt(year, *m + 1, 1).unwrap()
            }
            .pred_opt()
            .unwrap();
            first >= joined && end <= on
        })
        .count() as i32
}
impl FrozenBasis {
    pub fn capture(employee: &Employee, kind: &LeaveType, on: NaiveDate) -> Result<Self> {
        ensure!(
            (1900..=2200).contains(&on.year()) && kind.accrual != Accrual::Unlimited,
            "Capture a supported finite current-year basis."
        );
        ensure!(
            employee.joined_on.is_none_or(|j| j <= on),
            "Capture cannot precede employment."
        );
        let rate = if kind.use_employee_allowance {
            i32::try_from(
                employee
                    .annual_leave_days
                    .checked_mul(2)
                    .ok_or_else(|| anyhow::anyhow!("Invalid annual allowance."))?,
            )?
        } else {
            kind.annual_units
        };
        ensure!((0..=732).contains(&rate), "Invalid annual rate.");
        let earned = match kind.accrual {
            Accrual::Upfront => rate,
            Accrual::Monthly => {
                let j = employee.joined_on.ok_or_else(|| {
                    anyhow::anyhow!("Set a known employment date before freezing monthly accrual.")
                })?;
                rate * full_months(on.year(), on, j) / 12
            }
            Accrual::Unlimited => unreachable!(),
        };
        Ok(Self {
            known_from: on,
            rate_units: rate,
            accrual: kind.accrual,
            joined_on: employee.joined_on,
            earned_rate_at_capture: earned,
        })
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            (1900..=2200).contains(&self.known_from.year())
                && (0..=732).contains(&self.rate_units)
                && self.joined_on.is_none_or(|j| j <= self.known_from),
            "Invalid frozen entitlement inputs."
        );
        let expected = match self.accrual {
            Accrual::Upfront => self.rate_units,
            Accrual::Monthly => {
                let j = self.joined_on.ok_or_else(|| {
                    anyhow::anyhow!("Frozen monthly accrual is missing employment basis.")
                })?;
                self.rate_units * full_months(self.known_from.year(), self.known_from, j) / 12
            }
            Accrual::Unlimited => {
                return Err(anyhow::anyhow!(
                    "Unlimited leave has no finite entitlement anchor."
                ));
            }
        };
        ensure!(
            self.earned_rate_at_capture == expected,
            "Frozen entitlement does not match its recorded inputs."
        );
        Ok(())
    }
    pub fn earned_rate(&self, as_of: NaiveDate) -> Result<i32> {
        self.validate()?;
        ensure!(
            as_of >= self.known_from && as_of.year() == self.known_from.year(),
            "Do not use a capture as invented history or a future-year grant."
        );
        Ok(match self.accrual {
            Accrual::Upfront => self.earned_rate_at_capture,
            Accrual::Monthly => {
                let j = self.joined_on.unwrap();
                let before =
                    self.rate_units * full_months(self.known_from.year(), self.known_from, j) / 12;
                let now = self.rate_units * full_months(as_of.year(), as_of, j) / 12;
                self.earned_rate_at_capture + now - before
            }
            Accrual::Unlimited => unreachable!(),
        })
    }
    pub fn base_with_credits(&self, as_of: NaiveDate, dated_credit_total: i32) -> Result<i32> {
        self.earned_rate(as_of)?
            .checked_add(dated_credit_total)
            .ok_or_else(|| anyhow::anyhow!("Entitlement total exceeds supported arithmetic."))
    }
}
