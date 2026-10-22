//! Self-service attendance proposals, reviewed atomically before changing facts.
use crate::{
    access::{Access, Role},
    corrections::{CorrectionChange, CorrectionDraft},
    domain::*,
};
use anyhow::{Result, ensure};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    #[default]
    Pending,
    Approved,
    Rejected,
    Cancelled,
}
impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pending => "Pending",
            Self::Approved => "Approved",
            Self::Rejected => "Rejected",
            Self::Cancelled => "Cancelled",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request {
    pub id: Uuid,
    pub company_id: Uuid,
    pub employee_id: Uuid,
    pub employee_name: String,
    pub employee_code: String,
    pub submitted_by: Uuid,
    pub work_date: NaiveDate,
    pub timezone: String,
    pub base_record: Option<Attendance>,
    pub proposed_in: DateTime<Utc>,
    pub proposed_out: Option<DateTime<Utc>>,
    pub planned_shift: Option<crate::shifts::ShiftPlan>,
    pub reason: String,
    pub status: Status,
    pub revision: i64,
    pub applied_attendance_id: Option<Uuid>,
    pub correction_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub id: Uuid,
    pub company_id: Uuid,
    pub request_id: Uuid,
    pub employee_id: Uuid,
    pub actor_id: Uuid,
    pub actor_name: String,
    pub actor_role: Role,
    pub status: Status,
    pub reason: String,
    pub before_record: Option<Attendance>,
    pub after_record: Option<Attendance>,
    pub created_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Default)]
pub struct Setup {
    pub records: Vec<Request>,
    pub events: Vec<Event>,
}
#[derive(Clone, Debug)]
pub enum RequestAction {
    Submit(CorrectionDraft),
    Resolve {
        id: Uuid,
        expected_revision: i64,
        status: Status,
        reason: String,
    },
}
pub fn can_review(access: &Access, r: &Request) -> bool {
    r.status == Status::Pending
        && r.submitted_by != access.user_id
        && access.can_review(r.employee_id)
}
pub fn allowed(access: &Access, action: &RequestAction, data: &Snapshot) -> bool {
    match action {
        RequestAction::Submit(d) => access.employee_id == Some(d.employee_id),
        RequestAction::Resolve { id, status, .. } => data
            .requests
            .records
            .iter()
            .find(|r| r.id == *id)
            .is_some_and(|r| match status {
                Status::Cancelled => {
                    r.status == Status::Pending
                        && r.submitted_by == access.user_id
                        && access.employee_id == Some(r.employee_id)
                }
                Status::Approved | Status::Rejected => can_review(access, r),
                Status::Pending => false,
            }),
    }
}
pub fn note(s: &str) -> Result<()> {
    ensure!(
        !s.trim().is_empty() && s.trim().chars().count() <= 500 && !s.chars().any(char::is_control),
        "Enter a reason of 1–500 characters without control characters."
    );
    Ok(())
}
pub fn payload(
    action: &RequestAction,
    data: &Snapshot,
    now: DateTime<Utc>,
) -> Result<serde_json::Value> {
    ensure!(
        allowed(&data.access, action, data),
        "Your role does not permit this attendance request action."
    );
    match action {
        RequestAction::Submit(d) => {
            ensure!(
                data.employees
                    .iter()
                    .any(|e| e.id == d.employee_id && e.status == EmployeeStatus::Active),
                "Only active employees can submit their own attendance requests."
            );
            ensure!(
                !data
                    .requests
                    .records
                    .iter()
                    .any(|r| r.employee_id == d.employee_id
                        && r.work_date == d.work_date
                        && r.status == Status::Pending),
                "A pending request already exists for this date. Review or cancel it first."
            );
            let change = d.proposal(data, now)?;
            Ok(
                serde_json::json!({"operation":"submit","work_date":d.work_date,"attendance_id":change.expected_revision.map(|_|change.record.id),"expected_revision":change.expected_revision,"checked_in_at":change.record.checked_in_at,"checked_out_at":change.record.checked_out_at,"expected_timezone":data.company.timezone,"reason":d.reason.trim()}),
            )
        }
        RequestAction::Resolve {
            id,
            expected_revision,
            status,
            reason,
        } => {
            note(reason)?;
            let r = data.requests.records.iter().find(|r| r.id == *id).unwrap();
            ensure!(
                r.revision == *expected_revision,
                "This request changed. Refresh and reopen it."
            );
            if *status == Status::Approved {
                approval_change(data, r, now)?;
            }
            Ok(
                serde_json::json!({"operation":match status {Status::Approved=>"approve",Status::Rejected=>"reject",Status::Cancelled=>"cancel",Status::Pending=>unreachable!()},"request_id":id,"expected_revision":expected_revision,"reason":reason.trim()}),
            )
        }
    }
}
pub fn stale(data: &Snapshot, r: &Request) -> bool {
    if r.timezone != data.company.timezone {
        return true;
    }
    let current = data
        .attendance
        .iter()
        .find(|a| a.employee_id == r.employee_id && a.work_date == r.work_date);
    match (&r.base_record, current) {
        (Some(old), Some(now)) => old != now,
        (None, None) => data.shifts.plan(r.employee_id, r.work_date) != r.planned_shift.as_ref(),
        _ => true,
    }
}
fn approval_change(data: &Snapshot, r: &Request, now: DateTime<Utc>) -> Result<CorrectionChange> {
    ensure!(
        !stale(data, r),
        "Attendance, company time zone or the missing record's schedule changed. Reject or cancel this request and submit a fresh one."
    );
    let mut record = r.base_record.clone().unwrap_or(Attendance {
        id: Uuid::new_v4(),
        company_id: r.company_id,
        employee_id: r.employee_id,
        work_date: r.work_date,
        checked_in_at: r.proposed_in,
        checked_out_at: r.proposed_out,
        revision: 0,
        planned_shift: r.planned_shift.clone(),
        calculation_snapshot: None,
    });
    record.checked_in_at = r.proposed_in;
    record.checked_out_at = r.proposed_out;
    record.revision = r.base_record.as_ref().map_or(0, |a| a.revision + 1);
    let change = CorrectionChange {
        record,
        expected_revision: r.base_record.as_ref().map(|a| a.revision),
        reason: r.reason.clone(),
    };
    change.validate_record(data, now)?;
    Ok(change)
}
pub fn apply_demo(data: &mut Snapshot, action: RequestAction) -> Result<()> {
    let now = Utc::now();
    payload(&action, data, now)?;
    let (id, status, reason, before, after) = match action {
        RequestAction::Submit(d) => {
            let c = d.proposal(data, now)?;
            let person = data
                .employees
                .iter()
                .find(|e| e.id == d.employee_id)
                .unwrap();
            let id = Uuid::new_v4();
            data.requests.records.push(Request {
                id,
                company_id: data.company.id,
                employee_id: person.id,
                employee_name: person.full_name.clone(),
                employee_code: person.employee_code.clone(),
                submitted_by: data.access.user_id,
                work_date: d.work_date,
                timezone: data.company.timezone.clone(),
                base_record: d.original,
                proposed_in: c.record.checked_in_at,
                proposed_out: c.record.checked_out_at,
                planned_shift: c.record.planned_shift,
                reason: d.reason.trim().into(),
                status: Status::Pending,
                revision: 0,
                applied_attendance_id: None,
                correction_id: None,
                created_at: now,
            });
            (id, Status::Pending, d.reason, None, None)
        }
        RequestAction::Resolve {
            id, status, reason, ..
        } => {
            let i = data
                .requests
                .records
                .iter()
                .position(|r| r.id == id)
                .unwrap();
            let r = data.requests.records[i].clone();
            let mut before = None;
            let mut after = None;
            if status == Status::Approved {
                let mut c = approval_change(data, &r, now)?;
                before = r.base_record.clone();
                if c.expected_revision.is_none() {
                    c.record.calculation_snapshot = Some(
                        crate::calculations::CalculationSnapshot::capture(data, &c.record),
                    );
                }
                let audit_id = Uuid::new_v4();
                data.corrections.push(crate::corrections::CorrectionEvent {
                    id: audit_id,
                    company_id: r.company_id,
                    attendance_id: c.record.id,
                    employee_id: r.employee_id,
                    work_date: r.work_date,
                    actor_id: data.access.user_id,
                    actor_name: data
                        .own_employee()
                        .map(|e| e.full_name.clone())
                        .unwrap_or_else(|| "Reviewer".into()),
                    actor_role: data.access.role,
                    reason: format!("Approved attendance request {}", id),
                    before_record: before.clone(),
                    after_record: c.record.clone(),
                    created_at: now,
                });
                data.requests.records[i].applied_attendance_id = Some(c.record.id);
                data.requests.records[i].correction_id = Some(audit_id);
                after = Some(c.record.clone());
                if let Some(old) = data.attendance.iter_mut().find(|a| a.id == c.record.id) {
                    *old = c.record;
                } else {
                    data.attendance.push(c.record);
                }
            }
            data.requests.records[i].status = status;
            data.requests.records[i].revision += 1;
            (id, status, reason, before, after)
        }
    };
    let employee_id = data
        .requests
        .records
        .iter()
        .find(|r| r.id == id)
        .unwrap()
        .employee_id;
    data.requests.events.push(Event {
        id: Uuid::new_v4(),
        company_id: data.company.id,
        request_id: id,
        employee_id,
        actor_id: data.access.user_id,
        actor_name: data
            .own_employee()
            .map(|e| e.full_name.clone())
            .unwrap_or_else(|| "Staff".into()),
        actor_role: data.access.role,
        status,
        reason: reason.trim().into(),
        before_record: before,
        after_record: after,
        created_at: now,
    });
    Ok(())
}
