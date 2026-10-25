//! Scoped dated leave contracts. Protected hosted previews are rechecked at post.
use super::{
    Accrual, Balance, LeavePolicy, LeaveType,
    entitlements::FrozenBasis,
    openings::{Booking, Capture, Opening},
    versions::{Scheduled, Timeline},
};
use crate::{
    access::{Access, Role},
    domain::{Employee, Snapshot},
};
use anyhow::{Result, ensure};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Anchor {
    pub employee_id: Uuid,
    pub type_code: String,
    pub year: i32,
    pub basis: FrozenBasis,
    pub type_snapshot: LeaveType,
    pub incoming_carry_units: i32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Withdrawal {
    pub id: Uuid,
    pub schedule_id: Uuid,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Audit {
    pub id: Uuid,
    pub kind: String,
    pub reason: String,
    pub reference: String,
    pub created_by_name: String,
    pub posted_at: DateTime<Utc>,
    pub company_revision: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Context {
    pub version: String,
    pub company_id: Uuid,
    pub user_id: Uuid,
    pub employee_id: Option<Uuid>,
    pub role: Role,
    pub as_of: NaiveDate,
    pub revision: i64,
    pub enabled: bool,
    pub timeline: Timeline,
    pub anchors: Vec<Anchor>,
    pub openings: Vec<Opening>,
    pub withdrawals: Vec<Withdrawal>,
    pub audit: Vec<Audit>,
}
fn clean(s: &str, min: usize, max: usize) -> bool {
    (min..=max).contains(&s.chars().count()) && !s.chars().any(char::is_control)
}
fn in_scope(access: &Access, id: Uuid) -> bool {
    access.role.manages_hr()
        || access.employee_id == Some(id)
        || access.direct_reports.contains(&id)
}
impl Context {
    pub fn empty(data: &Snapshot) -> Self {
        Self {
            version: "leave-dated-v1".into(),
            company_id: data.company.id,
            user_id: data.access.user_id,
            employee_id: data.access.employee_id,
            role: data.access.role,
            as_of: data.company.today(),
            revision: 0,
            enabled: false,
            timeline: Timeline {
                baseline: data.leave_setup.policy.clone(),
                versions: vec![],
            },
            anchors: vec![],
            openings: vec![],
            withdrawals: vec![],
            audit: vec![],
        }
    }
    pub fn validate(&self, access: &Access, today: NaiveDate) -> Result<()> {
        ensure!(
            self.version == "leave-dated-v1"
                && self.company_id == access.company_id
                && self.user_id == access.user_id
                && self.employee_id == access.employee_id
                && self.role == access.role
                && self.as_of == today
                && access.role.uses_desktop(),
            "Dated leave does not match current staff access/date."
        );
        ensure!(
            (0..=5000).contains(&self.revision)
                && self.anchors.len() <= 5000
                && self.openings.len() <= 5000
                && self.audit.len() <= 5000,
            "Dated leave history exceeds the supported review limit."
        );
        self.timeline.validate()?;
        ensure!(
            self.timeline.baseline.company_id == self.company_id,
            "Policy company does not match access."
        );
        if !self.enabled {
            ensure!(
                self.revision == 0
                    && self.timeline.versions.is_empty()
                    && self.anchors.is_empty()
                    && self.openings.is_empty()
                    && self.audit.is_empty()
                    && self.withdrawals.is_empty(),
                "Disabled dated context contains posted history."
            );
        }
        let mut anchor_keys = HashSet::new();
        for a in &self.anchors {
            a.basis.validate()?;
            ensure!(
                !a.employee_id.is_nil()
                    && in_scope(access, a.employee_id)
                    && a.year == a.basis.known_from.year()
                    && a.type_code == a.type_snapshot.code
                    && a.type_snapshot.accrual == a.basis.accrual
                    && a.type_snapshot.accrual != Accrual::Unlimited
                    && (0..=732).contains(&a.type_snapshot.annual_units)
                    && (0..=732).contains(&a.type_snapshot.carryover_units)
                    && (1..=12).contains(&a.type_snapshot.carryover_expiry_month)
                    && (0..=a.type_snapshot.carryover_units).contains(&a.incoming_carry_units)
                    && anchor_keys.insert((a.employee_id, &a.type_code, a.year)),
                "Invalid or out-of-scope frozen entitlement."
            );
            ensure!(
                a.type_snapshot.use_employee_allowance
                    || a.type_snapshot.annual_units == a.basis.rate_units,
                "Frozen fixed-rate entitlement does not match its saved type."
            );
        }
        let mut ids = HashSet::new();
        let mut opening_keys = HashSet::new();
        for o in &self.openings {
            o.validate()?;
            ensure!(
                ids.insert(o.id)
                    && in_scope(access, o.employee_id)
                    && opening_keys.insert((o.employee_id, &o.type_code, o.effective_on.year()))
                    && self.anchors.iter().any(|a| a.employee_id == o.employee_id
                        && a.type_code == o.type_code
                        && a.year == o.effective_on.year()
                        && a.basis.known_from <= o.effective_on),
                "Opening is duplicate, out of scope or missing its captured entitlement."
            );
        }
        for v in &self.timeline.versions {
            ensure!(ids.insert(v.id), "Duplicate dated leave identity.");
        }
        for w in &self.withdrawals {
            ensure!(
                !w.id.is_nil()
                    && ids.insert(w.id)
                    && self
                        .timeline
                        .versions
                        .iter()
                        .any(|v| v.id == w.schedule_id && v.withdrawn),
                "Invalid withdrawal receipt."
            );
        }
        if access.role.manages_hr() {
            ensure!(
                self.audit.len() == ids.len()
                    && self.withdrawals.len()
                        == self
                            .timeline
                            .versions
                            .iter()
                            .filter(|v| v.withdrawn)
                            .count(),
                "Dated leave private history is incomplete."
            );
        } else {
            ensure!(
                self.audit.is_empty() && self.withdrawals.is_empty(),
                "Private leave history is outside current access."
            );
        }
        let mut audit_ids = HashSet::new();
        let mut revisions = HashSet::new();
        for a in &self.audit {
            ensure!(
                ids.contains(&a.id)
                    && audit_ids.insert(a.id)
                    && revisions.insert(a.company_revision)
                    && a.company_revision > 0
                    && a.company_revision <= self.revision
                    && matches!(a.kind.as_str(), "opening" | "policy" | "withdrawal")
                    && clean(a.reason.trim(), 5, 500)
                    && clean(&a.reference, 0, 160)
                    && clean(&a.created_by_name, 1, 200),
                "Invalid immutable leave audit."
            );
        }
        ensure!(
            serde_json::to_vec(self)?.len() <= 4_000_000,
            "Dated leave snapshot exceeds its supported size."
        );
        Ok(())
    }
    pub fn next_policy_revision(&self) -> i64 {
        self.timeline
            .versions
            .iter()
            .map(|v| v.policy.revision)
            .max()
            .unwrap_or(self.timeline.baseline.revision)
            .saturating_add(1)
    }
}
pub fn policy_on(data: &Snapshot, on: NaiveDate) -> &LeavePolicy {
    data.leave_setup
        .dated
        .as_ref()
        .map_or(&data.leave_setup.policy, |c| c.timeline.policy_on(on))
}
pub fn charges(
    data: &Snapshot,
    code: &str,
    start: NaiveDate,
    end: NaiveDate,
    part: super::DayPart,
) -> Result<Vec<super::DayCharge>> {
    if let Some(c) = &data.leave_setup.dated {
        c.timeline.charges(code, start, end, part)
    } else {
        data.leave_setup.policy.charges(start, end, part)
    }
}
pub fn bookings(
    data: &Snapshot,
    employee: Uuid,
    code: &str,
    year: i32,
    expiry: NaiveDate,
    exclude: Option<Uuid>,
) -> Vec<Booking> {
    let mut rows = data
        .leave
        .iter()
        .filter(|r| {
            r.employee_id == employee
                && r.type_code() == code
                && r.starts_on.year() == year
                && r.live()
                && Some(r.id) != exclude
        })
        .map(|r| Booking {
            request_id: r.id,
            units: r.units(),
            early_units: r
                .charges()
                .iter()
                .filter(|d| d.date <= expiry)
                .map(|d| d.units)
                .sum(),
            approved: r.status == crate::domain::LeaveStatus::Approved,
        })
        .collect::<Vec<_>>();
    rows.sort_by_key(|r| r.request_id);
    rows
}
pub fn balance(
    data: &Snapshot,
    e: &Employee,
    code: &str,
    year: i32,
    as_of: NaiveDate,
    exclude: Option<Uuid>,
) -> Result<Balance> {
    balance_detail(data, e, code, year, as_of, exclude).map(|(b, _)| b)
}
fn balance_detail(
    data: &Snapshot,
    e: &Employee,
    code: &str,
    year: i32,
    as_of: NaiveDate,
    exclude: Option<Uuid>,
) -> Result<(Balance, i32)> {
    let c = data
        .leave_setup
        .dated
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Refresh dated leave setup."))?;
    ensure!(
        (1900..=2200).contains(&year) && as_of.year() == year,
        "Use a supported date in the balance year."
    );
    let mut first = e
        .joined_on
        .map_or(year, |j| j.year())
        .max(c.timeline.baseline.tracking_starts_year)
        .min(year);
    if let Some(known) = c
        .anchors
        .iter()
        .filter(|a| a.employee_id == e.id && a.type_code == code)
        .map(|a| a.year)
        .min()
    {
        first = first.min(known);
    }
    let mut carry = 0;
    for y in first..=year {
        let on = if y == year {
            as_of
        } else {
            NaiveDate::from_ymd_opt(y, 12, 31).unwrap()
        };
        let a = c
            .anchors
            .iter()
            .find(|a| a.employee_id == e.id && a.type_code == code && a.year == y);
        let t = a.map(|a| &a.type_snapshot).or_else(|| {
            c.timeline
                .policy_on(NaiveDate::from_ymd_opt(y, 1, 1).unwrap())
                .leave_type(code)
        });
        let Some(t) = t.filter(|t| t.accrual != Accrual::Unlimited) else {
            ensure!(
                y != year,
                "This date has no finite entitlement for that type."
            );
            carry = 0;
            continue;
        };
        carry = a.map_or(carry.min(t.carryover_units), |a| a.incoming_carry_units);
        let mut gross = carry;
        let boundary = super::expiry(y, t.carryover_expiry_month);
        let rows = bookings(data, e.id, code, y, boundary, exclude);
        let total: i32 = rows.iter().map(|r| r.units).sum();
        let early: i32 = rows.iter().map(|r| r.early_units).sum();
        let used = rows.iter().filter(|r| r.approved).map(|r| r.units).sum();
        let credits = data
            .leave_setup
            .credits
            .as_ref()
            .map_or(0, |c| c.units(e.id, code, y, on));
        let earned = if let Some(a) = a {
            a.basis.base_with_credits(on, credits)?
        } else {
            super::earned(e, t, y, on) + credits
        };
        let (b, unused) = if let Some(o) = c.openings.iter().find(|o| {
            o.employee_id == e.id
                && o.type_code == code
                && o.effective_on.year() == y
                && o.effective_on <= on
        }) {
            let p = o.project(on, earned, &rows)?;
            gross = o.previously_consumed_carry_units + o.remaining_carry_units;
            (p.balance, p.unused_base_units)
        } else {
            let unused = earned - total + carry.min(early);
            (
                Balance {
                    earned_units: earned,
                    carryover_units: if on <= boundary { carry } else { 0 },
                    used_units: used,
                    reserved_units: total - used,
                    available_units: unused
                        + if on <= boundary {
                            carry - carry.min(early)
                        } else {
                            0
                        },
                },
                unused,
            )
        };
        if y == year {
            return Ok((b, gross));
        }
        carry = unused.max(0);
    }
    anyhow::bail!("No supported balance basis.")
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpeningRecord {
    pub id: Uuid,
    pub employee_id: Uuid,
    pub type_code: String,
    pub effective_on: NaiveDate,
    pub target_available_units: i32,
    pub remaining_carry_units: i32,
    pub source_requests_reviewed: bool,
    pub expected_revision: i64,
    pub reason: String,
    pub reference: String,
}
impl OpeningRecord {
    pub fn validate(&self, data: &Snapshot) -> Result<()> {
        ensure!(
            data.access.role.manages_hr(),
            "HR access is required to preview opening balances."
        );
        let c = data
            .leave_setup
            .dated
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Refresh leave setup first."))?;
        c.validate(&data.access, data.company.today())?;
        ensure!(
            self.expected_revision == c.revision || c.openings.iter().any(|o| o.id == self.id),
            "Leave setup changed. Refresh the preview."
        );
        ensure!(
            !self.id.is_nil()
                && data.active().any(|e| e.id == self.employee_id)
                && self.effective_on == c.as_of
                && self.source_requests_reviewed
                && (0..=732).contains(&self.target_available_units)
                && (0..=self.target_available_units).contains(&self.remaining_carry_units)
                && clean(self.reason.trim(), 5, 500)
                && clean(&self.reference, 0, 160),
            "Use an active employee, today's takeover, whole/half days and a source-review reason."
        );
        ensure!(
            c.timeline
                .policy_on(NaiveDate::from_ymd_opt(self.effective_on.year(), 1, 1).unwrap())
                .leave_type(&self.type_code)
                .is_some_and(|t| t.accrual != Accrual::Unlimited),
            "Choose a finite leave type for this year."
        );
        Ok(())
    }
    pub fn matches(&self, o: &Opening) -> bool {
        self.id == o.id
            && self.employee_id == o.employee_id
            && self.type_code == o.type_code
            && self.effective_on == o.effective_on
            && self.target_available_units == o.target_available_units
            && self.remaining_carry_units == o.remaining_carry_units
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preview {
    pub version: String,
    pub company_id: Uuid,
    pub user_id: Uuid,
    pub as_of: NaiveDate,
    pub record: OpeningRecord,
    pub fingerprint: String,
    pub opening: Opening,
    pub basis: FrozenBasis,
    pub type_snapshot: LeaveType,
    pub before: Balance,
    pub after: Balance,
}
impl Preview {
    pub fn validate(&self, data: &Snapshot, record: &OpeningRecord) -> Result<()> {
        record.validate(data)?;
        self.opening.validate()?;
        self.basis.validate()?;
        ensure!(
            self.version == "leave-opening-preview-v1"
                && self.company_id == data.company.id
                && self.user_id == data.access.user_id
                && self.as_of == data.company.today()
                && self.record == *record
                && record.matches(&self.opening)
                && self.fingerprint.len() == 32
                && self
                    .fingerprint
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
                && self.basis.known_from <= record.effective_on
                && self.basis.known_from.year() == record.effective_on.year()
                && self.type_snapshot.code == record.type_code,
            "Opening preview does not match this employee/draft/access."
        );
        let credits = data.leave_setup.credits.as_ref().map_or(0, |c| {
            c.units(
                record.employee_id,
                &record.type_code,
                self.as_of.year(),
                self.as_of,
            )
        });
        ensure!(
            self.basis.base_with_credits(self.as_of, credits)?
                == self.opening.base_earned_at_opening,
            "Opening preview has a different credit/rate basis. Refresh."
        );
        let rows = bookings(
            data,
            record.employee_id,
            &record.type_code,
            self.as_of.year(),
            self.opening.carry_expires_on,
            None,
        );
        ensure!(
            rows == self.opening.baseline
                && self
                    .opening
                    .project(self.as_of, self.opening.base_earned_at_opening, &rows)?
                    .balance
                    == self.after,
            "Opening preview has different saved reservations. Refresh."
        );
        let expected = preview_demo(data, record)?;
        ensure!(
            expected.opening == self.opening
                && expected.basis == self.basis
                && expected.type_snapshot == self.type_snapshot
                && expected.before == self.before
                && expected.after == self.after,
            "Opening preview does not match the independently calculated employee, carry and reservation basis. Refresh."
        );
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyRecord {
    pub id: Uuid,
    pub effective_on: NaiveDate,
    pub expected_revision: i64,
    pub policy: LeavePolicy,
    pub reason: String,
    pub reference: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WithdrawRecord {
    pub id: Uuid,
    pub schedule_id: Uuid,
    pub expected_revision: i64,
    pub reason: String,
    pub reference: String,
}
#[derive(Clone, Debug)]
pub enum Change {
    Opening(Box<Preview>),
    OpeningBatch(Box<super::opening_import::Post>),
    Schedule(PolicyRecord),
    Withdraw(WithdrawRecord),
}
impl Change {
    pub fn id(&self) -> Uuid {
        match self {
            Self::Opening(p) => p.record.id,
            Self::OpeningBatch(p) => p.id,
            Self::Schedule(r) => r.id,
            Self::Withdraw(r) => r.id,
        }
    }
    pub fn revision(&self) -> i64 {
        match self {
            Self::Opening(p) => p.record.expected_revision,
            Self::OpeningBatch(p) => p.preview.expected_revision(),
            Self::Schedule(r) => r.expected_revision,
            Self::Withdraw(r) => r.expected_revision,
        }
    }
    pub fn posted_revision(&self) -> i64 {
        self.revision()
            + match self {
                Self::OpeningBatch(p) => p.preview.rows.len() as i64,
                _ => 1,
            }
    }
    pub fn matches(&self, c: &Context) -> bool {
        match self {
            Self::Opening(p) => c.openings.iter().any(|o| p.record.matches(o)),
            Self::OpeningBatch(p) => p.matches(c),
            Self::Schedule(r) => c.timeline.versions.iter().any(|v| {
                v.id == r.id && v.effective_on == r.effective_on && {
                    let mut p = r.policy.clone();
                    p.revision = v.policy.revision;
                    p == v.policy
                }
            }),
            Self::Withdraw(r) => c
                .withdrawals
                .iter()
                .any(|w| w.id == r.id && w.schedule_id == r.schedule_id),
        }
    }
    pub fn payload(&self, data: &Snapshot) -> Result<(&'static str, serde_json::Value)> {
        ensure!(
            data.access.role.manages_accounts(),
            "Only an Administrator posts opening balances or policy versions."
        );
        let c = data
            .leave_setup
            .dated
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Refresh dated leave setup."))?;
        c.validate(&data.access, data.company.today())?;
        ensure!(
            self.revision() == c.revision || self.matches(c),
            "Leave setup changed. Refresh before posting."
        );
        match self {
            Self::Opening(p) => {
                if !self.matches(c) {
                    p.validate(data, &p.record)?;
                }
                Ok((
                    "rpc/post_leave_opening",
                    serde_json::json!({"p_record":p.record,"p_fingerprint":p.fingerprint}),
                ))
            }
            Self::OpeningBatch(p) => Ok(("rpc/post_leave_openings", p.payload(data)?)),
            Self::Schedule(r) => {
                ensure!(
                    !r.id.is_nil() && clean(r.reason.trim(), 5, 500) && clean(&r.reference, 0, 160),
                    "Enter a private reason and optional reference."
                );
                if !self.matches(c) {
                    let mut p = r.policy.clone();
                    p.revision = c.next_policy_revision();
                    c.timeline.validate_schedule(
                        &Scheduled {
                            id: r.id,
                            effective_on: r.effective_on,
                            policy: p,
                            withdrawn: false,
                        },
                        c.as_of,
                    )?;
                }
                Ok((
                    "rpc/save_dated_leave_policy",
                    serde_json::json!({"p_record":r}),
                ))
            }
            Self::Withdraw(r) => {
                ensure!(
                    !r.id.is_nil() && clean(r.reason.trim(), 5, 500) && clean(&r.reference, 0, 160),
                    "Enter a private withdrawal reason and optional reference."
                );
                if !self.matches(c) {
                    let active = data
                        .leave
                        .iter()
                        .filter(|r| r.live())
                        .map(|r| r.policy.policy_revision)
                        .collect::<Vec<_>>();
                    ensure!(
                        c.timeline.can_withdraw(r.schedule_id, c.as_of, &active),
                        "Only the unactivated future tail without active request references can be withdrawn."
                    );
                    let mut remaining = c.timeline.clone();
                    let version = remaining
                        .versions
                        .iter_mut()
                        .find(|v| v.id == r.schedule_id)
                        .unwrap();
                    let on = version.effective_on;
                    version.withdrawn = true;
                    if let Some(credits) = &data.leave_setup.credits {
                        for e in credits.entries.iter().filter(|e| e.effective_on >= on) {
                            ensure!(
                                credits.units(
                                    e.employee_id,
                                    &e.type_code,
                                    e.effective_on.year(),
                                    e.effective_on
                                ) == 0
                                    || remaining
                                        .policy_on(e.effective_on)
                                        .leave_type(&e.type_code)
                                        .is_some(),
                                "Scheduled credits need this leave type. Reverse their dated effect before withdrawing its version."
                            );
                        }
                    }
                }
                Ok((
                    "rpc/withdraw_dated_leave_policy",
                    serde_json::json!({"p_record":r}),
                ))
            }
        }
    }
}
pub fn preview_demo(data: &Snapshot, record: &OpeningRecord) -> Result<Preview> {
    record.validate(data)?;
    let c = data.leave_setup.dated.as_ref().unwrap();
    let e = data.active().find(|e| e.id == record.employee_id).unwrap();
    ensure!(
        !c.openings.iter().any(|o| o.employee_id == e.id
            && o.type_code == record.type_code
            && o.effective_on.year() == record.effective_on.year()),
        "This year already has an opening. Use an audited credit for a later correction."
    );
    ensure!(
        !data.leave.iter().any(|r| r.employee_id == e.id
            && r.type_code() == record.type_code
            && r.live()
            && r.starts_on.year() > record.effective_on.year()),
        "Review later-year reservations before importing an opening."
    );
    let existing = c.anchors.iter().find(|a| {
        a.employee_id == e.id && a.type_code == record.type_code && a.year == c.as_of.year()
    });
    let t = existing.map(|a| &a.type_snapshot).unwrap_or_else(|| {
        c.timeline
            .policy_on(NaiveDate::from_ymd_opt(c.as_of.year(), 1, 1).unwrap())
            .leave_type(&record.type_code)
            .unwrap()
    });
    let basis = existing
        .map(|a| a.basis.clone())
        .map_or_else(|| FrozenBasis::capture(e, t, c.as_of), Ok)?;
    let boundary = super::expiry(c.as_of.year(), t.carryover_expiry_month);
    let rows = bookings(data, e.id, &t.code, c.as_of.year(), boundary, None);
    let (before, gross) = balance_detail(data, e, &t.code, c.as_of.year(), c.as_of, None)?;
    let credits = data.leave_setup.credits.as_ref().map_or(0, |c| {
        c.units(
            e.id,
            &t.code,
            record.effective_on.year(),
            record.effective_on,
        )
    });
    let opening = Opening::capture(Capture {
        id: record.id,
        employee_id: e.id,
        type_code: &t.code,
        effective_on: c.as_of,
        carry_expires_on: boundary,
        joined_on: basis.joined_on,
        base_earned_units: basis.base_with_credits(c.as_of, credits)?,
        legacy_gross_carry_units: gross,
        carry_cap_units: t.carryover_units,
        target_available_units: record.target_available_units,
        remaining_carry_units: record.remaining_carry_units,
        source_outstanding_requests_reviewed: record.source_requests_reviewed,
        bookings: &rows,
    })?;
    let after = opening
        .project(c.as_of, opening.base_earned_at_opening, &rows)?
        .balance;
    let fingerprint = format!(
        "{:032x}",
        record.id.as_u128() ^ u128::try_from(c.revision).unwrap_or(0)
    );
    Ok(Preview {
        version: "leave-opening-preview-v1".into(),
        company_id: data.company.id,
        user_id: data.access.user_id,
        as_of: c.as_of,
        record: record.clone(),
        fingerprint,
        opening,
        basis,
        type_snapshot: t.clone(),
        before,
        after,
    })
}
fn capture_anchors(data: &Snapshot, people: &[Uuid]) -> Result<Vec<Anchor>> {
    let c = data.leave_setup.dated.as_ref().unwrap();
    let today = data.company.today();
    let mut rows = Vec::new();
    for e in data.active().filter(|e| people.contains(&e.id)) {
        for t in &c
            .timeline
            .policy_on(NaiveDate::from_ymd_opt(today.year(), 1, 1).unwrap())
            .types
        {
            if t.accrual == Accrual::Unlimited
                || (t.accrual == Accrual::Monthly && e.joined_on.is_none())
                || e.joined_on.is_some_and(|j| j > today)
                || c.anchors.iter().any(|a| {
                    a.employee_id == e.id && a.type_code == t.code && a.year == today.year()
                })
            {
                continue;
            }
            rows.push(Anchor {
                employee_id: e.id,
                type_code: t.code.clone(),
                year: today.year(),
                basis: FrozenBasis::capture(e, t, today)?,
                type_snapshot: t.clone(),
                incoming_carry_units: balance_detail(data, e, &t.code, today.year(), today, None)?
                    .1,
            });
        }
    }
    Ok(rows)
}
pub fn apply_demo(data: &mut Snapshot, change: &Change) -> Result<()> {
    if data.leave_setup.dated.is_none() {
        data.leave_setup.dated = Some(Context::empty(data));
    }
    change.payload(data)?;
    if change.matches(data.leave_setup.dated.as_ref().unwrap()) {
        return Ok(());
    }
    if let Change::OpeningBatch(p) = change {
        let records = p.preview.records();
        let fresh = super::opening_import::BatchPreview::demo(data, &records);
        ensure!(
            serde_json::to_value(&fresh)? == serde_json::to_value(&p.preview)?,
            "An opening import input changed. Review all rows again."
        );
        let mut next = data.clone();
        for mut r in records {
            r.expected_revision = next.leave_setup.dated.as_ref().unwrap().revision;
            let fresh = preview_demo(&next, &r)?;
            apply_demo(&mut next, &Change::Opening(Box::new(fresh)))?;
        }
        *data = next;
        return Ok(());
    }
    let mut next = data.clone();
    let today = next.company.today();
    match change {
        Change::Opening(p) => {
            let fresh = preview_demo(&next, &p.record)?;
            ensure!(
                serde_json::to_value(&fresh)? == serde_json::to_value(p)?,
                "Opening inputs changed. Review a new preview."
            );
            let anchors = capture_anchors(&next, &[p.record.employee_id])?;
            let c = next.leave_setup.dated.as_mut().unwrap();
            c.anchors.extend(anchors);
            c.openings.push(p.opening.clone());
        }
        Change::OpeningBatch(_) => unreachable!("Batch openings return above"),
        Change::Schedule(r) => {
            let people = next.active().map(|e| e.id).collect::<Vec<_>>();
            let anchors = capture_anchors(&next, &people)?;
            let c = next.leave_setup.dated.as_mut().unwrap();
            c.anchors.extend(anchors);
            let mut p = r.policy.clone();
            p.revision = c.next_policy_revision();
            c.timeline.versions.push(Scheduled {
                id: r.id,
                effective_on: r.effective_on,
                policy: p,
                withdrawn: false,
            });
        }
        Change::Withdraw(r) => {
            let c = next.leave_setup.dated.as_mut().unwrap();
            c.timeline
                .versions
                .iter_mut()
                .find(|v| v.id == r.schedule_id)
                .unwrap()
                .withdrawn = true;
            c.withdrawals.push(Withdrawal {
                id: r.id,
                schedule_id: r.schedule_id,
            });
        }
    }
    let (kind, reason, reference) = match change {
        Change::Opening(p) => (
            "opening",
            p.record.reason.clone(),
            p.record.reference.clone(),
        ),
        Change::Schedule(r) => ("policy", r.reason.clone(), r.reference.clone()),
        Change::Withdraw(r) => ("withdrawal", r.reason.clone(), r.reference.clone()),
        Change::OpeningBatch(_) => unreachable!("Batch openings return above"),
    };
    let c = next.leave_setup.dated.as_mut().unwrap();
    c.enabled = true;
    c.revision += 1;
    c.audit.push(Audit {
        id: change.id(),
        kind: kind.into(),
        reason,
        reference,
        created_by_name: "Demo Administrator".into(),
        posted_at: Utc::now(),
        company_revision: c.revision,
    });
    c.validate(&next.access, today)?;
    next.leave_setup.policy = c.timeline.policy_on(today).clone();
    for r in next
        .leave
        .iter()
        .filter(|r| r.live() && r.starts_on >= today)
    {
        if let Some(e) = next.employees.iter().find(|e| e.id == r.employee_id) {
            let p = policy_on(&next, r.starts_on);
            if p.leave_type(r.type_code())
                .is_some_and(|t| t.accrual != Accrual::Unlimited)
            {
                ensure!(
                    balance(
                        &next,
                        e,
                        r.type_code(),
                        r.starts_on.year(),
                        r.starts_on,
                        None
                    )?
                    .available_units
                        >= 0,
                    "This change would underfund existing reservations."
                );
            }
        }
    }
    *data = next;
    Ok(())
}
