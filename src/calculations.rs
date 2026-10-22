use crate::{
    access::Role,
    domain::{Attendance, LeaveStatus, Snapshot},
};
use anyhow::{Result, ensure};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalculationPolicy {
    pub company_id: Uuid,
    pub revision: i64,
    pub late_grace_minutes: u32,
    pub undertime_grace_minutes: u32,
    pub overtime_min_minutes: u32,
    pub overtime_round_minutes: u32,
    pub include_pre_shift_overtime: bool,
}
impl Default for CalculationPolicy {
    fn default() -> Self {
        Self {
            company_id: Uuid::nil(),
            revision: 0,
            late_grace_minutes: 0,
            undertime_grace_minutes: 0,
            overtime_min_minutes: 0,
            overtime_round_minutes: 1,
            include_pre_shift_overtime: false,
        }
    }
}
impl CalculationPolicy {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.late_grace_minutes <= 120 && self.undertime_grace_minutes <= 120,
            "Grace allowances must be 0–120 minutes."
        );
        ensure!(
            self.overtime_min_minutes <= 240,
            "Minimum overtime must be 0–240 minutes."
        );
        ensure!(
            [1, 5, 10, 15, 30, 60].contains(&self.overtime_round_minutes),
            "Choose an overtime rounding interval of 1, 5, 10, 15, 30 or 60 minutes."
        );
        Ok(())
    }
    pub fn same_rules(&self, other: &Self) -> bool {
        let mut a = self.clone();
        a.company_id = other.company_id;
        a.revision = other.revision;
        a == *other
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowStatus {
    Ready,
    NoShift,
    DstGap,
    InvalidWindow,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalculationSnapshot {
    pub version: u32,
    pub policy: CalculationPolicy,
    pub timezone: String,
    pub scheduled_workday: bool,
    pub planned_start: Option<DateTime<Utc>>,
    pub planned_end: Option<DateTime<Utc>>,
    pub window_status: WindowStatus,
}
impl CalculationSnapshot {
    pub fn capture(data: &Snapshot, a: &Attendance) -> Self {
        let mut result = Self {
            version: 1,
            policy: data.calculations.policy.clone(),
            timezone: data.company.timezone.clone(),
            scheduled_workday: false,
            planned_start: None,
            planned_end: None,
            window_status: WindowStatus::NoShift,
        };
        if let Some(plan) = &a.planned_shift {
            result.timezone = plan.timezone.clone();
            result.scheduled_workday = plan.working(data, a.work_date);
            match (
                plan.boundary(a.work_date, false),
                plan.boundary(a.work_date, true),
            ) {
                (Ok(start), Ok(end)) if end > start => {
                    result.planned_start = Some(start);
                    result.planned_end = Some(end);
                    result.window_status = WindowStatus::Ready;
                }
                (Ok(_), Ok(_)) => result.window_status = WindowStatus::InvalidWindow,
                _ => result.window_status = WindowStatus::DstGap,
            }
        }
        result
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CalculationEvent {
    pub id: Uuid,
    pub company_id: Uuid,
    pub attendance_id: Option<Uuid>,
    pub employee_id: Option<Uuid>,
    pub actor_id: Uuid,
    pub actor_name: String,
    pub actor_role: Role,
    pub reason: String,
    pub before_snapshot: Option<serde_json::Value>,
    pub after_snapshot: serde_json::Value,
    pub created_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Default)]
pub struct CalculationSetup {
    pub policy: CalculationPolicy,
    pub policy_events: Vec<CalculationEvent>,
    pub attendance_events: Vec<CalculationEvent>,
}
#[derive(Clone, Debug)]
pub struct PolicyDraft {
    pub policy: CalculationPolicy,
    pub expected_revision: i64,
    pub reason: String,
}
impl PolicyDraft {
    pub fn new(policy: &CalculationPolicy) -> Self {
        Self {
            policy: policy.clone(),
            expected_revision: policy.revision,
            reason: String::new(),
        }
    }
    pub fn payload(&self, data: &Snapshot) -> Result<serde_json::Value> {
        ensure!(
            data.access.role.manages_accounts(),
            "Only Administrators can change calculation rules."
        );
        ensure!(
            self.policy.company_id == data.company.id
                && self.expected_revision == data.calculations.policy.revision,
            "Calculation rules changed. Refresh and reopen the editor."
        );
        self.policy.validate()?;
        reason(&self.reason)?;
        ensure!(
            !self.policy.same_rules(&data.calculations.policy),
            "Change a calculation rule before saving."
        );
        let mut value = serde_json::to_value(&self.policy)?;
        value["expected_revision"] = self.expected_revision.into();
        value["reason"] = self.reason.trim().into();
        Ok(value)
    }
}
#[derive(Clone, Debug)]
pub struct ApplyPolicy {
    pub attendance_id: Uuid,
    pub expected_revision: i64,
    pub expected_policy_revision: i64,
    pub reason: String,
}
impl ApplyPolicy {
    pub fn payload(&self, data: &Snapshot) -> Result<serde_json::Value> {
        ensure!(
            data.access.role.manages_hr(),
            "Only HR and Administrators can apply calculation rules to a saved record."
        );
        let row = data
            .attendance
            .iter()
            .find(|a| a.id == self.attendance_id && a.company_id == data.company.id)
            .ok_or_else(|| anyhow::anyhow!("Attendance unavailable. Refresh and reopen it."))?;
        ensure!(
            row.revision == self.expected_revision
                && data.calculations.policy.revision == self.expected_policy_revision,
            "Attendance or calculation rules changed. Refresh and reopen this record."
        );
        ensure!(
            row.planned_shift.is_some(),
            "This attendance has no saved shift. Earlier shift hours cannot be reconstructed."
        );
        reason(&self.reason)?;
        ensure!(
            row.calculation_snapshot.as_ref() != Some(&CalculationSnapshot::capture(data, row)),
            "This record already has the current rules and calendar decision."
        );
        Ok(
            serde_json::json!({"attendance_id":row.id,"expected_revision":self.expected_revision,"expected_policy_revision":self.expected_policy_revision,"reason":self.reason.trim()}),
        )
    }
}
fn reason(value: &str) -> Result<()> {
    ensure!(
        !value.trim().is_empty()
            && value.trim().chars().count() <= 500
            && !value.chars().any(char::is_control),
        "Enter a reason of 1–500 characters without control characters."
    );
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalculationStatus {
    Ready,
    InProgress,
    BreakInProgress,
    MissingPolicy,
    MissingShift,
    Unscheduled,
    LeaveReview,
    BoundaryReview,
}
impl CalculationStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ready => "Calculated",
            Self::InProgress => "Attendance in progress",
            Self::BreakInProgress => "Break in progress",
            Self::MissingPolicy => "No saved calculation rules",
            Self::MissingShift => "No saved shift hours",
            Self::Unscheduled => "Unscheduled work · HR review",
            Self::LeaveReview => "Approved leave · HR review",
            Self::BoundaryReview => "Schedule boundary · HR review",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimeCalculation {
    pub status: CalculationStatus,
    pub net_seconds: Option<i64>,
    pub late_seconds: Option<i64>,
    pub undertime_seconds: Option<i64>,
    pub overtime_minutes: Option<i64>,
    pub raw_overtime_seconds: Option<i64>,
}
pub fn calculate(data: &Snapshot, a: &Attendance) -> TimeCalculation {
    let totals = data.breaks.totals(a.id);
    let mut result = TimeCalculation {
        status: CalculationStatus::MissingPolicy,
        net_seconds: totals.time_excluding_unpaid(a),
        late_seconds: None,
        undertime_seconds: None,
        overtime_minutes: None,
        raw_overtime_seconds: None,
    };
    let Some(s) = &a.calculation_snapshot else {
        return result;
    };
    if s.version != 1 || s.policy.company_id != a.company_id || s.policy.validate().is_err() {
        result.status = CalculationStatus::BoundaryReview;
        return result;
    }
    if a.planned_shift.is_none() || s.window_status == WindowStatus::NoShift {
        result.status = CalculationStatus::MissingShift;
        return result;
    }
    let Some((start, end)) = s.planned_start.zip(s.planned_end).filter(|(b, e)| e > b) else {
        result.status = CalculationStatus::BoundaryReview;
        return result;
    };
    if s.window_status != WindowStatus::Ready {
        result.status = CalculationStatus::BoundaryReview;
        return result;
    }
    if !s.scheduled_workday {
        result.status = CalculationStatus::Unscheduled;
        return result;
    }
    if data.leave.iter().any(|r| {
        r.employee_id == a.employee_id
            && r.status == LeaveStatus::Approved
            && r.charges()
                .iter()
                .any(|c| c.date == a.work_date && c.units > 0)
    }) {
        result.status = CalculationStatus::LeaveReview;
        return result;
    }
    result.late_seconds = Some(
        ((a.checked_in_at.min(end) - start).num_seconds()
            - i64::from(s.policy.late_grace_minutes) * 60)
            .max(0)
            / 60
            * 60,
    );
    let Some(checkout) = a.checked_out_at else {
        result.status = if totals.open {
            CalculationStatus::BreakInProgress
        } else {
            CalculationStatus::InProgress
        };
        return result;
    };
    if totals.open || checkout < a.checked_in_at {
        result.status = CalculationStatus::BoundaryReview;
        return result;
    }
    result.undertime_seconds = Some(
        ((end - checkout.max(start)).num_seconds()
            - i64::from(s.policy.undertime_grace_minutes) * 60)
            .max(0)
            / 60
            * 60,
    );
    let worked = |left: DateTime<Utc>, right: DateTime<Utc>| {
        let lo = a.checked_in_at.max(left);
        let hi = checkout.min(right);
        if hi <= lo {
            return 0;
        }
        let unpaid: i64 = data
            .breaks
            .for_attendance(a.id)
            .filter(|b| !b.paid)
            .filter_map(|b| {
                b.ended_at
                    .map(|e| (e.min(hi) - b.started_at.max(lo)).num_seconds().max(0))
            })
            .sum();
        ((hi - lo).num_seconds() - unpaid).max(0)
    };
    let raw = worked(end, checkout)
        + if s.policy.include_pre_shift_overtime {
            worked(a.checked_in_at, start)
        } else {
            0
        };
    result.raw_overtime_seconds = Some(raw);
    let minutes = raw / 60;
    let step = i64::from(s.policy.overtime_round_minutes);
    result.overtime_minutes = Some(if minutes < i64::from(s.policy.overtime_min_minutes) {
        0
    } else {
        minutes / step * step
    });
    result.status = CalculationStatus::Ready;
    result
}
pub fn minutes(seconds: Option<i64>) -> String {
    seconds
        .map(|s| format!("{} min", s.max(0) / 60))
        .unwrap_or_else(|| "—".into())
}

pub fn save_demo_policy(data: &mut Snapshot, draft: PolicyDraft) -> Result<()> {
    draft.payload(data)?;
    let before = serde_json::to_value(&data.calculations.policy)?;
    let mut saved = draft.policy;
    saved.revision = data.calculations.policy.revision + 1;
    let event = event(
        data,
        None,
        None,
        draft.reason,
        Some(before),
        serde_json::to_value(&saved)?,
    );
    data.calculations.policy = saved;
    data.calculations.policy_events.push(event);
    Ok(())
}
pub fn apply_demo_policy(data: &mut Snapshot, change: ApplyPolicy) -> Result<()> {
    change.payload(data)?;
    let i = data
        .attendance
        .iter()
        .position(|a| a.id == change.attendance_id)
        .unwrap();
    let before = data.attendance[i].calculation_snapshot.clone();
    let saved = CalculationSnapshot::capture(data, &data.attendance[i]);
    let e = event(
        data,
        Some(change.attendance_id),
        Some(data.attendance[i].employee_id),
        change.reason,
        before.map(serde_json::to_value).transpose()?,
        serde_json::to_value(&saved)?,
    );
    data.attendance[i].calculation_snapshot = Some(saved);
    data.attendance[i].revision += 1;
    data.calculations.attendance_events.push(e);
    Ok(())
}
fn event(
    data: &Snapshot,
    attendance_id: Option<Uuid>,
    employee_id: Option<Uuid>,
    reason: String,
    before_snapshot: Option<serde_json::Value>,
    after_snapshot: serde_json::Value,
) -> CalculationEvent {
    CalculationEvent {
        id: Uuid::new_v4(),
        company_id: data.company.id,
        attendance_id,
        employee_id,
        actor_id: data.access.user_id,
        actor_name: data
            .own_employee()
            .map(|e| e.full_name.clone())
            .unwrap_or_else(|| "HR staff".into()),
        actor_role: data.access.role,
        reason: reason.trim().into(),
        before_snapshot,
        after_snapshot,
        created_at: Utc::now(),
    }
}
