use crate::{
    access::{Access, Role},
    breaks::AttendanceBreak,
    calculations::{self, CalculationStatus},
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
    Revoked,
}
impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pending => "Pending",
            Self::Approved => "Approved",
            Self::Rejected => "Rejected",
            Self::Cancelled => "Cancelled",
            Self::Revoked => "Revoked",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Basis {
    pub attendance: Attendance,
    pub breaks: Vec<AttendanceBreak>,
    pub minutes: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request {
    pub id: Uuid,
    pub company_id: Uuid,
    pub attendance_id: Uuid,
    pub employee_id: Uuid,
    pub employee_name: String,
    pub employee_code: String,
    pub submitted_by: Uuid,
    pub work_date: NaiveDate,
    pub requested_minutes: u32,
    pub basis: Basis,
    pub status: Status,
    pub approved_minutes: Option<u32>,
    pub reason: String,
    pub revision: i64,
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
    pub before_status: Option<Status>,
    pub before_approved_minutes: Option<u32>,
    pub approved_minutes: Option<u32>,
    pub created_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Default)]
pub struct Setup {
    pub records: Vec<Request>,
    pub events: Vec<Event>,
}
#[derive(Clone, Debug)]
pub struct Draft {
    pub basis: Basis,
    pub minutes: u32,
    pub reason: String,
}
impl Draft {
    pub fn new(data: &Snapshot, a: &Attendance) -> Result<Self> {
        let basis=capture(data,a).ok_or_else(||anyhow::anyhow!("This completed attendance has no available overtime candidate. Review its saved shift, rules, breaks and leave first."))?;
        Ok(Self {
            minutes: basis.minutes as u32,
            basis,
            reason: String::new(),
        })
    }
}
#[derive(Clone, Debug)]
pub enum OvertimeAction {
    Submit(Draft),
    Resolve {
        id: Uuid,
        expected_revision: i64,
        status: Status,
        minutes: Option<u32>,
        reason: String,
    },
}
pub fn capture(data: &Snapshot, a: &Attendance) -> Option<Basis> {
    let calc = calculations::calculate(data, a);
    let minutes = calc.overtime_minutes?;
    if calc.status != CalculationStatus::Ready || minutes <= 0 {
        return None;
    }
    let mut breaks: Vec<_> = data.breaks.for_attendance(a.id).cloned().collect();
    breaks.sort_by_key(|b| b.id);
    Some(Basis {
        attendance: a.clone(),
        breaks,
        minutes,
    })
}
pub fn matches_current(data: &Snapshot, r: &Request) -> bool {
    if let Some(current) = crate::approvals::delegated_source(
        data,
        crate::approvals::Scope::Overtime,
        r.id,
        r.revision,
    ) {
        return current;
    }
    data.attendance
        .iter()
        .find(|a| a.id == r.attendance_id)
        .and_then(|a| capture(data, a))
        .as_ref()
        == Some(&r.basis)
}
pub fn effective_minutes(data: &Snapshot, r: &Request) -> Option<u32> {
    if r.status == Status::Approved && matches_current(data, r) {
        r.approved_minutes
    } else {
        None
    }
}
pub fn can_review(access: &Access, r: &Request) -> bool {
    r.submitted_by != access.user_id && access.can_review(r.employee_id)
}
pub fn can_review_in(data: &Snapshot, r: &Request) -> bool {
    can_review(&data.access, r)
        || (r.submitted_by != data.access.user_id
            && Some(r.employee_id) != data.access.employee_id
            && crate::approvals::permitted(
                data,
                crate::approvals::Scope::Overtime,
                r.id,
                r.revision,
            ))
}
pub fn allowed(access: &Access, action: &OvertimeAction, data: &Snapshot) -> bool {
    match action {
        OvertimeAction::Submit(d) => access.employee_id == Some(d.basis.attendance.employee_id),
        OvertimeAction::Resolve { id, status, .. } => data
            .overtime
            .records
            .iter()
            .find(|r| r.id == *id)
            .is_some_and(|r| match status {
                Status::Cancelled => {
                    r.status == Status::Pending
                        && r.submitted_by == access.user_id
                        && access.employee_id == Some(r.employee_id)
                }
                Status::Approved | Status::Rejected => {
                    r.status == Status::Pending
                        && (can_review(access, r)
                            || (access == &data.access && can_review_in(data, r)))
                }
                Status::Revoked => {
                    r.status == Status::Approved
                        && (can_review(access, r)
                            || (access == &data.access && can_review_in(data, r)))
                }
                Status::Pending => false,
            }),
    }
}
pub fn payload(action: &OvertimeAction, data: &Snapshot) -> Result<serde_json::Value> {
    ensure!(
        allowed(&data.access, action, data),
        "Your role does not permit this overtime action."
    );
    match action {
        OvertimeAction::Submit(d) => {
            crate::requests::note(&d.reason)?;
            let a = &d.basis.attendance;
            ensure!(
                data.employees
                    .iter()
                    .any(|e| e.id == a.employee_id && e.status == EmployeeStatus::Active),
                "Only active employees can submit their own overtime requests."
            );
            ensure!(
                capture(data, a).as_ref() == Some(&d.basis)
                    && data.attendance.iter().any(|now| now == a),
                "Attendance or breaks changed. Refresh and reopen the overtime request."
            );
            ensure!(
                d.minutes > 0 && i64::from(d.minutes) <= d.basis.minutes,
                "Request between one minute and the calculated candidate."
            );
            ensure!(
                !data.overtime.records.iter().any(|r| r.attendance_id == a.id
                    && matches!(r.status, Status::Pending | Status::Approved)),
                "A pending or approved overtime request already exists. Cancel the pending request or have an authorized reviewer revoke the approval before resubmitting."
            );
            Ok(
                serde_json::json!({"operation":"submit","attendance_id":a.id,"expected_revision":a.revision,"expected_breaks":d.basis.breaks.iter().map(|b|serde_json::json!({"id":b.id,"revision":b.revision})).collect::<Vec<_>>(),"expected_candidate_minutes":d.basis.minutes,"minutes":d.minutes,"reason":d.reason.trim()}),
            )
        }
        OvertimeAction::Resolve {
            id,
            expected_revision,
            status,
            minutes,
            reason,
        } => {
            crate::requests::note(reason)?;
            let r = data.overtime.records.iter().find(|r| r.id == *id).unwrap();
            ensure!(
                r.revision == *expected_revision,
                "This overtime request changed. Refresh and reopen it."
            );
            if *status == Status::Approved {
                ensure!(
                    matches_current(data, r),
                    "Attendance, breaks or leave changed. Reject or cancel and submit a fresh overtime request."
                );
                ensure!(
                    minutes.is_some_and(|m| m > 0
                        && m <= r.requested_minutes
                        && i64::from(m) <= r.basis.minutes),
                    "Approve between one minute and the requested amount."
                );
            }
            Ok(
                serde_json::json!({"operation":match status{Status::Approved=>"approve",Status::Rejected=>"reject",Status::Cancelled=>"cancel",Status::Revoked=>"revoke",Status::Pending=>unreachable!()},"request_id":id,"expected_revision":expected_revision,"minutes":minutes,"reason":reason.trim()}),
            )
        }
    }
}
pub fn apply_demo(data: &mut Snapshot, action: OvertimeAction) -> Result<()> {
    payload(&action, data)?;
    let now = Utc::now();
    let (id, status, reason, before_status, before_minutes, after_minutes) = match action {
        OvertimeAction::Submit(d) => {
            let a = &d.basis.attendance;
            let e = data
                .employees
                .iter()
                .find(|e| e.id == a.employee_id)
                .unwrap();
            let id = Uuid::new_v4();
            data.overtime.records.push(Request {
                id,
                company_id: data.company.id,
                attendance_id: a.id,
                employee_id: a.employee_id,
                employee_name: e.full_name.clone(),
                employee_code: e.employee_code.clone(),
                submitted_by: data.access.user_id,
                work_date: a.work_date,
                requested_minutes: d.minutes,
                basis: d.basis,
                status: Status::Pending,
                approved_minutes: None,
                reason: d.reason.trim().into(),
                revision: 0,
                created_at: now,
            });
            (id, Status::Pending, d.reason, None, None, None)
        }
        OvertimeAction::Resolve {
            id,
            status,
            minutes,
            reason,
            ..
        } => {
            let r = data
                .overtime
                .records
                .iter_mut()
                .find(|r| r.id == id)
                .unwrap();
            let before_status = r.status;
            let before_minutes = r.approved_minutes;
            r.status = status;
            r.revision += 1;
            if status == Status::Approved {
                r.approved_minutes = minutes;
            }
            (
                id,
                status,
                reason,
                Some(before_status),
                before_minutes,
                r.approved_minutes,
            )
        }
    };
    let employee_id = data
        .overtime
        .records
        .iter()
        .find(|r| r.id == id)
        .unwrap()
        .employee_id;
    data.overtime.events.push(Event {
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
        before_status,
        before_approved_minutes: before_minutes,
        approved_minutes: after_minutes,
        created_at: now,
    });
    Ok(())
}
