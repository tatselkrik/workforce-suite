use crate::{
    access::{Access, Role},
    domain::{Attendance, EmployeeStatus, Snapshot, parse_date},
};
use anyhow::{Result, ensure};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BreakRule {
    pub id: Uuid,
    pub name: String,
    pub minutes: u32,
    pub paid: bool,
    pub required: bool,
}
impl Default for BreakRule {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            name: String::new(),
            minutes: 15,
            paid: true,
            required: false,
        }
    }
}
impl BreakRule {
    pub fn label(&self) -> String {
        format!(
            "{} · {} min · {}{}",
            self.name,
            self.minutes,
            if self.paid { "Paid" } else { "Unpaid" },
            if self.required { " · Required" } else { "" }
        )
    }
}
pub fn validate_rules(rules: &[BreakRule], shift_minutes: u32) -> Result<()> {
    ensure!(
        rules.len() <= 8,
        "A shift supports up to eight break rules."
    );
    let mut ids = std::collections::HashSet::new();
    let mut names = std::collections::HashSet::new();
    let mut total = 0;
    for r in rules {
        ensure!(
            !r.id.is_nil() && ids.insert(r.id),
            "Each break rule needs a unique identity."
        );
        ensure!(
            !r.name.trim().is_empty()
                && r.name.trim().chars().count() <= 80
                && !r.name.chars().any(char::is_control)
                && names.insert(r.name.trim().to_lowercase()),
            "Use unique break names of 1–80 characters without control characters."
        );
        ensure!(
            (1..=720).contains(&r.minutes),
            "Break duration must be 1–720 whole minutes."
        );
        total += r.minutes;
    }
    ensure!(
        rules.is_empty() || total < shift_minutes,
        "Combined break durations must be shorter than the shift."
    );
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttendanceBreak {
    pub id: Uuid,
    pub company_id: Uuid,
    pub attendance_id: Uuid,
    pub employee_id: Uuid,
    pub rule_id: Option<Uuid>,
    pub name: String,
    pub paid: bool,
    pub required: bool,
    pub expected_minutes: u32,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub voided: bool,
    pub revision: i64,
}
impl AttendanceBreak {
    pub fn seconds(&self) -> i64 {
        self.ended_at
            .map(|t| (t - self.started_at).num_seconds().max(0))
            .unwrap_or(0)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BreakEvent {
    pub id: Uuid,
    pub company_id: Uuid,
    pub attendance_id: Uuid,
    pub employee_id: Uuid,
    pub break_id: Uuid,
    pub actor_id: Uuid,
    pub actor_name: String,
    pub actor_role: Role,
    pub action: String,
    pub reason: String,
    pub before_record: Option<AttendanceBreak>,
    pub after_record: AttendanceBreak,
    pub created_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Default)]
pub struct BreakSetup {
    pub records: Vec<AttendanceBreak>,
    pub events: Vec<BreakEvent>,
}
impl BreakSetup {
    pub fn for_attendance(&self, id: Uuid) -> impl Iterator<Item = &AttendanceBreak> {
        self.records
            .iter()
            .filter(move |b| b.attendance_id == id && !b.voided)
    }
    pub fn open(&self, id: Uuid) -> Option<&AttendanceBreak> {
        self.for_attendance(id).find(|b| b.ended_at.is_none())
    }
    pub fn totals(&self, id: Uuid) -> BreakTotals {
        let mut total = BreakTotals::default();
        for r in self.for_attendance(id) {
            if r.ended_at.is_none() {
                total.open = true;
            } else if r.paid {
                total.paid_seconds += r.seconds();
            } else {
                total.unpaid_seconds += r.seconds();
            }
        }
        total
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BreakTotals {
    pub paid_seconds: i64,
    pub unpaid_seconds: i64,
    pub open: bool,
}
impl BreakTotals {
    pub fn time_excluding_unpaid(self, attendance: &Attendance) -> Option<i64> {
        attendance.checked_out_at.filter(|_| !self.open).map(|end| {
            ((end - attendance.checked_in_at).num_seconds() - self.unpaid_seconds).max(0)
        })
    }
}
pub fn duration(seconds: i64) -> String {
    format!(
        "{} h {:02} min",
        seconds.max(0) / 3600,
        seconds.max(0) % 3600 / 60
    )
}
pub fn rule_status(data: &Snapshot, attendance: Uuid, rule: &BreakRule) -> &'static str {
    match data
        .breaks
        .for_attendance(attendance)
        .find(|b| b.rule_id == Some(rule.id))
    {
        None => {
            if rule.required {
                "Required · not recorded"
            } else {
                "Not recorded"
            }
        }
        Some(b) if b.ended_at.is_none() => "In progress",
        Some(b) if b.seconds() < (rule.minutes * 60) as i64 => "Recorded · shorter than rule",
        Some(_) => "Recorded",
    }
}

#[derive(Clone, Debug)]
pub struct BreakDraft {
    pub attendance_id: Uuid,
    pub original: Option<AttendanceBreak>,
    pub expected_parent_revision: i64,
    pub rule_id: Option<Uuid>,
    pub name: String,
    pub paid: bool,
    pub started_date: String,
    pub started_time: String,
    pub ended_date: String,
    pub ended_time: String,
    pub reason: String,
}
impl BreakDraft {
    pub fn new(
        data: &Snapshot,
        attendance: &Attendance,
        original: Option<AttendanceBreak>,
    ) -> Self {
        let zone = data.company.tz();
        Self {
            attendance_id: attendance.id,
            expected_parent_revision: attendance.revision,
            rule_id: original.as_ref().and_then(|b| b.rule_id),
            name: original
                .as_ref()
                .map(|b| b.name.clone())
                .unwrap_or_else(|| "Unscheduled break".into()),
            paid: original.as_ref().is_some_and(|b| b.paid),
            started_date: original
                .as_ref()
                .map(|b| b.started_at)
                .unwrap_or(attendance.checked_in_at)
                .with_timezone(&zone)
                .date_naive()
                .to_string(),
            started_time: original
                .as_ref()
                .map(|b| {
                    b.started_at
                        .with_timezone(&zone)
                        .format("%H:%M:%S%.f")
                        .to_string()
                })
                .unwrap_or_default(),
            ended_date: original
                .as_ref()
                .and_then(|b| b.ended_at)
                .unwrap_or(attendance.checked_in_at)
                .with_timezone(&zone)
                .date_naive()
                .to_string(),
            ended_time: original
                .as_ref()
                .and_then(|b| b.ended_at)
                .map(|at| at.with_timezone(&zone).format("%H:%M:%S%.f").to_string())
                .unwrap_or_default(),
            reason: String::new(),
            original,
        }
    }
}
#[derive(Clone, Debug)]
pub enum BreakAction {
    Start {
        attendance_id: Uuid,
        parent_revision: i64,
        rule_id: Uuid,
    },
    End {
        id: Uuid,
        revision: i64,
        parent_revision: i64,
    },
    Save(BreakDraft),
    Void {
        id: Uuid,
        revision: i64,
        parent_revision: i64,
        reason: String,
    },
}
impl BreakAction {
    pub fn parent_id(&self, data: &Snapshot) -> Option<Uuid> {
        match self {
            Self::Start { attendance_id, .. } => Some(*attendance_id),
            Self::Save(d) => Some(d.attendance_id),
            Self::End { id, .. } | Self::Void { id, .. } => data
                .breaks
                .records
                .iter()
                .find(|b| b.id == *id)
                .map(|b| b.attendance_id),
        }
    }
    pub fn allowed(&self, access: &Access, data: &Snapshot) -> bool {
        self.parent_id(data)
            .and_then(|id| data.attendance.iter().find(|a| a.id == id))
            .is_some_and(|a| {
                a.company_id == access.company_id
                    && match self {
                        Self::Start { .. } | Self::End { .. } => {
                            access.employee_id == Some(a.employee_id)
                        }
                        Self::Save(_) | Self::Void { .. } => access.allows(crate::staff_roles::Capability::AttendanceAdministration),
                    }
            })
    }
    pub fn prepare(
        &self,
        data: &Snapshot,
        now: DateTime<Utc>,
    ) -> Result<(String, serde_json::Value, AttendanceBreak)> {
        ensure!(
            self.allowed(&data.access, data),
            "Your role does not permit this break action."
        );
        let parent = self
            .parent_id(data)
            .and_then(|id| data.attendance.iter().find(|a| a.id == id))
            .ok_or_else(|| {
                anyhow::anyhow!("Attendance unavailable. Refresh before recording a break.")
            })?;
        let parent_revision = match self {
            Self::Start {
                parent_revision, ..
            }
            | Self::End {
                parent_revision, ..
            }
            | Self::Void {
                parent_revision, ..
            } => *parent_revision,
            Self::Save(d) => d.expected_parent_revision,
        };
        ensure!(
            parent_revision == parent.revision,
            "Attendance changed. Refresh and reopen its breaks."
        );
        let original = match self {
            Self::End { id, revision, .. } | Self::Void { id, revision, .. } => {
                Some((*id, *revision))
            }
            Self::Save(d) => d.original.as_ref().map(|b| (b.id, b.revision)),
            Self::Start { .. } => None,
        };
        let old = original
            .map(|(id, revision)| {
                let b = data
                    .breaks
                    .records
                    .iter()
                    .find(|b| b.id == id && b.attendance_id == parent.id && !b.voided)
                    .ok_or_else(|| {
                        anyhow::anyhow!("Break changed or is unavailable. Refresh before editing.")
                    })?;
                ensure!(
                    b.revision == revision,
                    "This break changed. Refresh and reopen it."
                );
                Ok::<_, anyhow::Error>(b.clone())
            })
            .transpose()?;
        let (operation, reason, mut saved): (&str, String, AttendanceBreak) = match self {
            Self::Start { rule_id, .. } => {
                ensure!(
                    parent.checked_out_at.is_none(),
                    "Only an open attendance record can start a break."
                );
                ensure!(
                    data.employees
                        .iter()
                        .any(|e| e.id == parent.employee_id && e.status == EmployeeStatus::Active),
                    "Archived employees cannot start a break."
                );
                let rule = parent
                    .planned_shift
                    .as_ref()
                    .and_then(|p| p.break_rules.iter().find(|r| r.id == *rule_id))
                    .ok_or_else(|| {
                        anyhow::anyhow!("Choose a break from this attendance record's saved rules.")
                    })?;
                (
                    "start",
                    "Employee started break".into(),
                    new_record(parent, Some(rule), &rule.name, rule.paid, now, None),
                )
            }
            Self::End { .. } => {
                let mut b = old.clone().unwrap();
                ensure!(
                    parent.checked_out_at.is_none() && b.ended_at.is_none(),
                    "Only an open break on an open attendance record can be ended."
                );
                b.ended_at = Some(now);
                ("end", "Employee ended break".into(), b)
            }
            Self::Save(d) => {
                validate_reason(&d.reason)?;
                let start = crate::corrections::local_time(
                    parse_date(&d.started_date)?,
                    &d.started_time,
                    data.company.tz(),
                )?;
                let end = if d.ended_time.trim().is_empty() {
                    None
                } else {
                    Some(crate::corrections::local_time(
                        parse_date(&d.ended_date)?,
                        &d.ended_time,
                        data.company.tz(),
                    )?)
                };
                let b = if let Some(mut b) = old.clone() {
                    b.started_at = start;
                    b.ended_at = end;
                    b
                } else {
                    let rule = d
                        .rule_id
                        .map(|id| {
                            parent
                                .planned_shift
                                .as_ref()
                                .and_then(|p| p.break_rules.iter().find(|r| r.id == id))
                                .ok_or_else(|| {
                                    anyhow::anyhow!(
                                        "Choose a saved break rule or an unscheduled break."
                                    )
                                })
                        })
                        .transpose()?;
                    new_record(parent, rule, &d.name, d.paid, start, end)
                };
                (
                    if old.is_some() { "correct" } else { "record" },
                    d.reason.trim().into(),
                    b,
                )
            }
            Self::Void { reason, .. } => {
                validate_reason(reason)?;
                let mut b = old.clone().unwrap();
                b.voided = true;
                ("void", reason.trim().into(), b)
            }
        };
        saved.revision = old.as_ref().map(|b| b.revision + 1).unwrap_or(0);
        if !saved.voided {
            ensure!(
                !saved.name.trim().is_empty()
                    && saved.name.trim().chars().count() <= 80
                    && !saved.name.chars().any(char::is_control),
                "Enter a break name of 1–80 characters without control characters."
            );
            ensure!(
                saved.started_at >= parent.checked_in_at
                    && saved.started_at <= now
                    && saved
                        .ended_at
                        .is_none_or(|end| end >= saved.started_at && end <= now),
                "Use past or current break times inside the recorded attendance period."
            );
            ensure!(
                parent
                    .checked_out_at
                    .is_none_or(|end| saved.ended_at.is_some_and(|e| e <= end)),
                "Finish the break inside the attendance period, or correct attendance first."
            );
            ensure!(
                !data
                    .breaks
                    .for_attendance(parent.id)
                    .any(|b| b.id != saved.id
                        && saved.rule_id.is_some()
                        && b.rule_id == saved.rule_id),
                "That saved break rule is already recorded. Correct it or void it before recording a replacement."
            );
            ensure!(
                !data
                    .breaks
                    .for_attendance(parent.id)
                    .any(|b| b.id != saved.id && b.ended_at.is_none() && saved.ended_at.is_none()),
                "End or correct the open break first."
            );
            ensure!(
                !data
                    .breaks
                    .for_attendance(parent.id)
                    .any(|b| b.id != saved.id
                        && b.ended_at != Some(b.started_at)
                        && saved.ended_at != Some(saved.started_at)
                        && saved.ended_at.is_none_or(|e| b.started_at < e)
                        && b.ended_at.is_none_or(|e| saved.started_at < e)),
                "These break times overlap another break."
            );
        }
        let payload = serde_json::json!({"attendance_id":parent.id,"expected_parent_revision":parent_revision,"id":original.map(|o|o.0),"expected_revision":original.map(|o|o.1),"rule_id":saved.rule_id,"name":saved.name,"paid":saved.paid,"started_at":saved.started_at,"ended_at":saved.ended_at,"reason":reason});
        Ok((operation.into(), payload, saved))
    }
}
fn new_record(
    a: &Attendance,
    rule: Option<&BreakRule>,
    name: &str,
    paid: bool,
    start: DateTime<Utc>,
    end: Option<DateTime<Utc>>,
) -> AttendanceBreak {
    AttendanceBreak {
        id: Uuid::new_v4(),
        company_id: a.company_id,
        attendance_id: a.id,
        employee_id: a.employee_id,
        rule_id: rule.map(|r| r.id),
        name: rule
            .map(|r| r.name.clone())
            .unwrap_or_else(|| name.trim().into()),
        paid: rule.map(|r| r.paid).unwrap_or(paid),
        required: rule.is_some_and(|r| r.required),
        expected_minutes: rule.map(|r| r.minutes).unwrap_or(0),
        started_at: start,
        ended_at: end,
        voided: false,
        revision: 0,
    }
}
fn validate_reason(reason: &str) -> Result<()> {
    ensure!(
        !reason.trim().is_empty()
            && reason.trim().chars().count() <= 500
            && !reason.chars().any(char::is_control),
        "Enter a reason of 1–500 characters without control characters."
    );
    Ok(())
}
pub fn validate_attendance(data: &Snapshot, a: &Attendance) -> Result<()> {
    for b in data.breaks.for_attendance(a.id) {
        ensure!(
            b.started_at >= a.checked_in_at
                && a.checked_out_at
                    .is_none_or(|e| b.ended_at.is_some_and(|end| end <= e)),
            "End the open break or correct its times before changing attendance."
        );
    }
    Ok(())
}
pub fn apply_demo(data: &mut Snapshot, action: BreakAction) -> Result<()> {
    let (operation, payload, saved) = action.prepare(data, Utc::now())?;
    let before = data
        .breaks
        .records
        .iter()
        .find(|b| b.id == saved.id)
        .cloned();
    data.breaks.events.push(BreakEvent {
        id: Uuid::new_v4(),
        company_id: data.company.id,
        attendance_id: saved.attendance_id,
        employee_id: saved.employee_id,
        break_id: saved.id,
        actor_id: data.access.user_id,
        actor_name: data
            .own_employee()
            .map(|e| e.full_name.clone())
            .unwrap_or_else(|| "HR staff".into()),
        actor_role: data.access.role,
        action: operation,
        reason: payload["reason"].as_str().unwrap_or("").into(),
        before_record: before,
        after_record: saved.clone(),
        created_at: Utc::now(),
    });
    if let Some(b) = data.breaks.records.iter_mut().find(|b| b.id == saved.id) {
        *b = saved
    } else {
        data.breaks.records.push(saved)
    }
    Ok(())
}
