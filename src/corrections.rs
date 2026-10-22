use crate::domain::{Attendance, EmployeeStatus, Snapshot, parse_date};
use anyhow::{Result, bail, ensure};
use chrono::{DateTime, LocalResult, NaiveDate, NaiveTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CorrectionEvent {
    pub id: Uuid,
    pub company_id: Uuid,
    pub attendance_id: Uuid,
    pub employee_id: Uuid,
    pub work_date: NaiveDate,
    pub actor_id: Uuid,
    pub actor_name: String,
    pub actor_role: crate::access::Role,
    pub reason: String,
    pub before_record: Option<Attendance>,
    pub after_record: Attendance,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct CorrectionDraft {
    pub original: Option<Attendance>,
    pub employee_id: Uuid,
    pub work_date: NaiveDate,
    pub checked_in: String,
    pub checked_in_date: String,
    pub checked_out_date: String,
    pub checked_out: String,
    pub reason: String,
}

#[derive(Clone, Debug)]
pub struct CorrectionChange {
    pub record: Attendance,
    pub expected_revision: Option<i64>,
    pub reason: String,
}

impl CorrectionDraft {
    pub fn new(
        employee_id: Uuid,
        work_date: NaiveDate,
        original: Option<Attendance>,
        data: &Snapshot,
    ) -> Self {
        let tz = data.company.tz();
        Self {
            employee_id,
            work_date,
            checked_in_date: original
                .as_ref()
                .map(|a| a.checked_in_at.with_timezone(&tz).date_naive())
                .unwrap_or(work_date)
                .to_string(),
            checked_in: original
                .as_ref()
                .map(|a| {
                    a.checked_in_at
                        .with_timezone(&tz)
                        .format("%H:%M:%S%.f")
                        .to_string()
                })
                .unwrap_or_default(),
            checked_out_date: original
                .as_ref()
                .and_then(|a| a.checked_out_at)
                .map(|t| t.with_timezone(&tz).date_naive())
                .unwrap_or(work_date)
                .to_string(),
            checked_out: original
                .as_ref()
                .and_then(|a| a.checked_out_at)
                .map(|t| t.with_timezone(&tz).format("%H:%M:%S%.f").to_string())
                .unwrap_or_default(),
            original,
            reason: String::new(),
        }
    }

    pub fn change(&self, data: &Snapshot, now: DateTime<Utc>) -> Result<CorrectionChange> {
        let change = self.proposal(data, now)?;
        change.validate(data, now)?;
        Ok(change)
    }

    pub fn proposal(&self, data: &Snapshot, now: DateTime<Utc>) -> Result<CorrectionChange> {
        let time_in = local_time(
            parse_date(&self.checked_in_date)?,
            &self.checked_in,
            data.company.tz(),
        )?;
        let time_out = if self.checked_out.trim().is_empty() {
            None
        } else {
            Some(local_time(
                parse_date(&self.checked_out_date)?,
                &self.checked_out,
                data.company.tz(),
            )?)
        };
        let mut change = CorrectionChange {
            record: Attendance {
                calculation_snapshot: self
                    .original
                    .as_ref()
                    .and_then(|a| a.calculation_snapshot.clone()),
                planned_shift: self
                    .original
                    .as_ref()
                    .and_then(|a| a.planned_shift.clone())
                    .or_else(|| {
                        self.original
                            .is_none()
                            .then(|| data.shifts.plan(self.employee_id, self.work_date).cloned())
                            .flatten()
                    }),
                id: self
                    .original
                    .as_ref()
                    .map(|a| a.id)
                    .unwrap_or_else(Uuid::new_v4),
                company_id: data.company.id,
                employee_id: self.employee_id,
                work_date: self.work_date,
                checked_in_at: time_in,
                checked_out_at: time_out,
                revision: self.original.as_ref().map(|a| a.revision + 1).unwrap_or(0),
            },
            expected_revision: self.original.as_ref().map(|a| a.revision),
            reason: self.reason.trim().into(),
        };
        if self.original.is_none() {
            change.record.calculation_snapshot = Some(
                crate::calculations::CalculationSnapshot::capture(data, &change.record),
            );
        }
        change.validate_record(data, now)?;
        Ok(change)
    }
}

pub(crate) fn local_time(day: NaiveDate, value: &str, tz: chrono_tz::Tz) -> Result<DateTime<Utc>> {
    let time = NaiveTime::parse_from_str(value.trim(), "%H:%M:%S%.f")
        .or_else(|_| NaiveTime::parse_from_str(value.trim(), "%H:%M"))
        .map_err(|_| anyhow::anyhow!("Enter a valid 24-hour time, such as 08:30 or 17:00."))?;
    match tz.from_local_datetime(&day.and_time(time)) {
        LocalResult::Single(t) => Ok(t.with_timezone(&Utc)),
        LocalResult::Ambiguous(_, _) => bail!(
            "This time occurs twice when daylight saving ends. Choose an unambiguous local time."
        ),
        LocalResult::None => bail!(
            "This local time does not exist when daylight saving starts. Choose another time."
        ),
    }
}

impl CorrectionChange {
    pub fn validate(&self, data: &Snapshot, now: DateTime<Utc>) -> Result<()> {
        ensure!(
            data.access.role.manages_hr(),
            "Only HR staff and administrators can correct attendance."
        );
        self.validate_record(data, now)
    }

    pub(crate) fn validate_record(&self, data: &Snapshot, now: DateTime<Utc>) -> Result<()> {
        let a = &self.record;
        crate::breaks::validate_attendance(data, a)?;
        ensure!(
            a.company_id == data.company.id,
            "Choose an employee in this company."
        );
        let person = data
            .employees
            .iter()
            .find(|e| e.id == a.employee_id)
            .ok_or_else(|| anyhow::anyhow!("Choose an employee in this company."))?;
        let today = now.with_timezone(&data.company.tz()).date_naive();
        ensure!(
            !self.reason.trim().is_empty()
                && self.reason.trim().chars().count() <= 500
                && !self.reason.chars().any(char::is_control),
            "Enter a correction reason of 1–500 characters without control characters."
        );
        ensure!(
            a.work_date >= NaiveDate::from_ymd_opt(1900, 1, 1).unwrap()
                && a.work_date <= today
                && a.checked_in_at <= now
                && (a
                    .checked_in_at
                    .with_timezone(&data.company.tz())
                    .date_naive()
                    == a.work_date
                    || a.planned_shift.as_ref().is_some_and(
                        |p| p.overnight() && p.contains_clock(a.work_date, a.checked_in_at)
                    ))
                && a.checked_out_at
                    .is_none_or(|t| t <= now && t >= a.checked_in_at),
            "Use past or current times; check-out must be on or after check-in."
        );
        ensure!(
            person.joined_on.is_none_or(|d| a.work_date >= d),
            "Attendance cannot be recorded before the employee hire date."
        );
        ensure!(
            person.status != EmployeeStatus::Archived || a.work_date < today,
            "Archived employees can only have historical attendance corrected."
        );
        if let Some(revision) = self.expected_revision {
            let previous = data
                .attendance
                .iter()
                .find(|r| {
                    r.id == a.id && r.employee_id == a.employee_id && r.work_date == a.work_date
                })
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Attendance changed or is unavailable. Refresh before correcting it."
                    )
                })?;
            ensure!(
                previous.revision == revision,
                "Someone changed this attendance record. Refresh, reopen it and review their changes."
            );
            ensure!(
                previous.checked_in_at != a.checked_in_at
                    || previous.checked_out_at != a.checked_out_at,
                "Change the check-in or check-out time before saving a correction."
            );
        } else {
            ensure!(
                !data.attendance.iter().any(|r| r.id == a.id),
                "Refresh before recording attendance."
            );
        }
        ensure!(
            !data.attendance.iter().any(|r| r.id != a.id
                && r.employee_id == a.employee_id
                && r.work_date == a.work_date),
            "Attendance already exists for this date. Open that record to correct it."
        );
        ensure!(
            !data.attendance.iter().any(|r| r.id != a.id
                && r.employee_id == a.employee_id
                && r.checked_out_at.is_none()
                && a.checked_out_at.is_none()),
            "Another shift is still open. Correct or check out that shift first."
        );
        ensure!(
            !data.attendance.iter().any(|r| r.id != a.id
                && r.employee_id == a.employee_id
                && r.checked_out_at != Some(r.checked_in_at)
                && a.checked_out_at != Some(a.checked_in_at)
                && a.checked_out_at.is_none_or(|end| r.checked_in_at < end)
                && r.checked_out_at.is_none_or(|end| a.checked_in_at < end)),
            "These times overlap another attendance record. Review both shifts before saving."
        );
        Ok(())
    }

    pub fn payload(&self) -> serde_json::Value {
        serde_json::json!({"attendance_id":self.expected_revision.map(|_| self.record.id),
            "employee_id":self.record.employee_id,"work_date":self.record.work_date,
            "checked_in_at":self.record.checked_in_at,"checked_out_at":self.record.checked_out_at,
            "expected_revision":self.expected_revision,"reason":self.reason.trim()})
    }
}
