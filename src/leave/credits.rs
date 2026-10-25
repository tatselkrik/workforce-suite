//! Dated base-credit deltas. Opening balances need a separate takeover basis.
use crate::access::Access;
use anyhow::{Result, ensure};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub id: Uuid,
    pub employee_id: Uuid,
    pub type_code: String,
    pub effective_on: NaiveDate,
    pub units: i32,
    pub reverses_id: Option<Uuid>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Audit {
    pub id: Uuid,
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
    pub role: crate::access::Role,
    pub as_of: NaiveDate,
    pub revision: i64,
    pub entries: Vec<Entry>,
    pub audit: Vec<Audit>,
}
fn clean(s: &str, min: usize, max: usize) -> bool {
    (min..=max).contains(&s.chars().count()) && !s.chars().any(char::is_control)
}
fn type_code(s: &str) -> bool {
    (1..=30).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}
impl Context {
    pub fn empty(data: &crate::domain::Snapshot) -> Self {
        Self {
            version: "leave-credits-v1".into(),
            company_id: data.company.id,
            user_id: data.access.user_id,
            employee_id: data.access.employee_id,
            role: data.access.role,
            as_of: data.company.today(),
            revision: 0,
            entries: vec![],
            audit: vec![],
        }
    }
    pub fn validate(&self, access: &Access, today: NaiveDate) -> Result<()> {
        ensure!(
            self.version == "leave-credits-v1"
                && self.company_id == access.company_id
                && self.user_id == access.user_id
                && self.employee_id == access.employee_id
                && self.role == access.role
                && self.as_of == today
                && access.role.uses_desktop(),
            "Leave credits do not match current staff access/date."
        );
        ensure!(
            (0..=5000).contains(&self.revision) && self.entries.len() <= 5000,
            "Leave credit history exceeds the supported limit."
        );
        let rows: HashMap<_, _> = self.entries.iter().map(|e| (e.id, e)).collect();
        ensure!(
            rows.len() == self.entries.len(),
            "Duplicate leave credit identity."
        );
        let mut reversals = HashSet::new();
        for e in &self.entries {
            ensure!(
                !e.id.is_nil()
                    && !e.employee_id.is_nil()
                    && type_code(&e.type_code)
                    && (-732..=732).contains(&e.units)
                    && e.units != 0
                    && (1900..=2200).contains(&e.effective_on.year()),
                "Invalid dated leave credit."
            );
            ensure!(
                access.role.manages_hr()
                    || access.employee_id == Some(e.employee_id)
                    || access.direct_reports.contains(&e.employee_id),
                "Leave credit is outside current reporting scope."
            );
            if let Some(id) = e.reverses_id {
                let original = rows
                    .get(&id)
                    .ok_or_else(|| anyhow::anyhow!("Reversal is missing its original credit."))?;
                ensure!(
                    original.reverses_id.is_none()
                        && original.id != e.id
                        && original.employee_id == e.employee_id
                        && original.type_code == e.type_code
                        && e.units == -original.units
                        && e.effective_on >= original.effective_on
                        && e.effective_on.year() == original.effective_on.year()
                        && reversals.insert(id),
                    "Invalid linked credit reversal."
                );
            }
        }
        if access.role.manages_hr() {
            ensure!(
                self.audit.len() == self.entries.len(),
                "Credit audit history is incomplete."
            );
        } else {
            ensure!(
                self.audit.is_empty(),
                "Private credit audit is outside current access."
            );
        }
        let mut audit_ids = HashSet::new();
        let mut revisions = HashSet::new();
        for a in &self.audit {
            ensure!(
                rows.contains_key(&a.id)
                    && audit_ids.insert(a.id)
                    && revisions.insert(a.company_revision)
                    && a.company_revision > 0
                    && a.company_revision <= self.revision
                    && clean(a.reason.trim(), 5, 500)
                    && clean(&a.reference, 0, 160)
                    && clean(&a.created_by_name, 1, 200),
                "Invalid private credit audit."
            );
        }
        Ok(())
    }
    pub fn units(&self, employee: Uuid, code: &str, year: i32, as_of: NaiveDate) -> i32 {
        self.entries
            .iter()
            .filter(|e| {
                e.employee_id == employee
                    && e.type_code == code
                    && e.effective_on.year() == year
                    && e.effective_on <= as_of
            })
            .map(|e| e.units)
            .sum()
    }
}
/// Parse an explicit signed number of days without floating-point rounding.
pub fn parse_delta(input: &str) -> Result<i32> {
    let text = input.trim();
    let (sign, value) = if let Some(v) = text.strip_prefix('-') {
        (-1, v)
    } else {
        (1, text.strip_prefix('+').unwrap_or(text))
    };
    let (whole, fraction) = value.split_once('.').unwrap_or((value, "0"));
    ensure!(
        !whole.is_empty()
            && whole.bytes().all(|b| b.is_ascii_digit())
            && matches!(fraction, "0" | "5"),
        "Enter whole or half days, such as 2, -1 or 0.5."
    );
    let whole: i32 = whole.parse()?;
    ensure!(whole <= 366, "Use no more than 366 days per adjustment.");
    let units = sign * (whole * 2 + if fraction == "5" { 1 } else { 0 });
    ensure!(
        (-732..=732).contains(&units) && units != 0,
        "Use a nonzero adjustment through 366 days."
    );
    Ok(units)
}
#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub id: Uuid,
    pub employee_id: Uuid,
    pub type_code: String,
    pub effective_on: NaiveDate,
    pub units: i32,
    pub reverses_id: Option<Uuid>,
    pub expected_revision: i64,
    pub reason: String,
    pub reference: String,
}
impl Record {
    pub fn matches_entry(&self, e: &Entry) -> bool {
        e.id == self.id
            && e.employee_id == self.employee_id
            && e.type_code == self.type_code
            && e.effective_on == self.effective_on
            && e.units == self.units
            && e.reverses_id == self.reverses_id
    }
    pub fn validate(&self, data: &crate::domain::Snapshot) -> Result<()> {
        ensure!(
            data.access.role.manages_accounts(),
            "Only an Administrator can post credit adjustments."
        );
        let context = data
            .leave_setup
            .credits
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Refresh credit history before posting."))?;
        context.validate(&data.access, data.company.today())?;
        ensure!(
            self.expected_revision == context.revision
                || context.entries.iter().any(|e| e.id == self.id),
            "Credit history changed. Refresh before posting."
        );
        let person = data
            .employees
            .iter()
            .find(|e| e.id == self.employee_id)
            .ok_or_else(|| anyhow::anyhow!("Choose an employee."))?;
        let kind = super::dated::policy_on(data,self.effective_on)
            .leave_type(&self.type_code)
            .ok_or_else(|| anyhow::anyhow!("Choose a leave type."))?;
        ensure!(
            person.status == crate::domain::EmployeeStatus::Active
                && kind.accrual != super::Accrual::Unlimited,
            "Use an active employee and a finite leave type."
        );
        ensure!(
            !self.id.is_nil()
                && (-732..=732).contains(&self.units)
                && self.units != 0
                && self.effective_on >= context.as_of
                && self.effective_on.year() <= (context.as_of.year() + 1).min(2200)
                && self.effective_on.year() >= data.leave_setup.policy.tracking_starts_year
                && person.joined_on.is_none_or(|d| self.effective_on >= d)
                && clean(self.reason.trim(), 5, 500)
                && clean(&self.reference, 0, 160),
            "Use a dated half-day adjustment with a reason. Prior balances cannot be backdated."
        );
        if let Some(id) = self.reverses_id {
            let original = context
                .entries
                .iter()
                .find(|e| e.id == id)
                .ok_or_else(|| anyhow::anyhow!("Refresh the original credit."))?;
            ensure!(
                original.reverses_id.is_none()
                    && !context
                        .entries
                        .iter()
                        .any(|e| e.reverses_id == Some(id) && e.id != self.id)
                    && original.employee_id == self.employee_id
                    && original.type_code == self.type_code
                    && self.units == -original.units
                    && self.effective_on >= original.effective_on
                    && self.effective_on.year() == original.effective_on.year(),
                "Reverse one full original entry in the same year."
            );
        }
        Ok(())
    }
}
pub fn apply_demo(data: &mut crate::domain::Snapshot, record: &Record) -> Result<()> {
    if data.leave_setup.credits.is_none() {
        data.leave_setup.credits = Some(Context::empty(data));
    }
    record.validate(data)?;
    let context = data.leave_setup.credits.as_ref().unwrap();
    if let Some(old) = context.entries.iter().find(|e| e.id == record.id) {
        let audit = context.audit.iter().find(|a| a.id == record.id).unwrap();
        ensure!(
            old.employee_id == record.employee_id
                && old.type_code == record.type_code
                && old.effective_on == record.effective_on
                && old.units == record.units
                && old.reverses_id == record.reverses_id
                && audit.reason == record.reason.trim()
                && audit.reference == record.reference
                && audit.company_revision == record.expected_revision + 1,
            "That credit request identity was already used."
        );
        return Ok(());
    }
    ensure!(
        context.revision < 5000,
        "Credit history exceeds the supported review limit."
    );
    if record.units < 0 {
        ensure!(
            !data
                .leave
                .iter()
                .any(|r| r.employee_id == record.employee_id
                    && r.type_code() == record.type_code
                    && matches!(
                        r.status,
                        crate::domain::LeaveStatus::Pending | crate::domain::LeaveStatus::Approved
                    )
                    && r.starts_on.year() > record.effective_on.year()),
            "Review later-year leave before reducing credit."
        );
    }
    let mut proposed = data.clone();
    let context = proposed.leave_setup.credits.as_mut().unwrap();
    context.revision += 1;
    context.entries.push(Entry {
        id: record.id,
        employee_id: record.employee_id,
        type_code: record.type_code.clone(),
        effective_on: record.effective_on,
        units: record.units,
        reverses_id: record.reverses_id,
    });
    context.audit.push(Audit {
        id: record.id,
        reason: record.reason.trim().into(),
        reference: record.reference.clone(),
        created_by_name: data
            .own_employee()
            .map(|e| e.full_name.clone())
            .unwrap_or_else(|| "Demo Administrator".into()),
        posted_at: Utc::now(),
        company_revision: context.revision,
    });
    if record.units < 0 {
        let person = proposed
            .employees
            .iter()
            .find(|e| e.id == record.employee_id)
            .unwrap();
        let kind = super::dated::policy_on(&proposed,record.effective_on)
            .leave_type(&record.type_code)
            .unwrap();
        let mut day = record.effective_on;
        while day.year() == record.effective_on.year() {
            ensure!(
                super::balance_checked(&proposed, person, kind, day.year(), day, None)?.available_units >= 0,
                "This reduction would exceed available leave or existing reservations."
            );
            let Some(next) = day.succ_opt() else { break };
            day = next;
        }
    }
    data.leave_setup.credits = proposed.leave_setup.credits;
    Ok(())
}
