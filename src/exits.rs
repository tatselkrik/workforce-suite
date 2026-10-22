//! Private employee exits and scoped clearance tasks. No payroll calculation or Auth deletion.
use crate::{
    access::Role,
    domain::{EmployeeStatus, Mode, Snapshot, parse_date},
    hiring,
};
use anyhow::{Context, Result, ensure};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Case {
    pub id: Uuid,
    pub company_id: Uuid,
    pub employee_id: Uuid,
    pub employee_name: String,
    pub employee_code: String,
    pub last_working_on: NaiveDate,
    pub category: String,
    pub private_notes: String,
    pub status: String,
    pub revision: i64,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Task {
    pub id: Uuid,
    pub company_id: Uuid,
    pub case_id: Uuid,
    pub employee_id: Uuid,
    pub employee_name: String,
    pub employee_code: String,
    pub last_working_on: NaiveDate,
    pub case_status: String,
    pub assigned_employee_id: Option<Uuid>,
    pub title: String,
    pub instructions: String,
    pub due_on: Option<NaiveDate>,
    pub required: bool,
    pub status: String,
    pub revision: i64,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
pub type Event = crate::onboarding::Event;
#[derive(Clone, Debug, Default)]
pub struct Setup {
    pub cases: Vec<Case>,
    pub tasks: Vec<Task>,
    pub events: Vec<Event>,
    pub readiness: Vec<Readiness>,
    pub(crate) retries: std::collections::HashMap<Uuid, Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Readiness {
    pub case_id: Uuid,
    pub company_id: Uuid,
    pub case_revision: i64,
    pub last_day_reached: bool,
    pub employee_active: bool,
    pub open_required_tasks: u64,
    pub active_direct_reports: u64,
    pub open_attendance: u64,
    pub pending_leave: u64,
    pub pending_attendance_requests: u64,
    pub pending_overtime: u64,
    pub pending_assigned_leave: u64,
    pub named_leave_policies: u64,
    pub open_onboarding_tasks: u64,
    pub open_clearance_tasks: u64,
    pub scheduled_interviews: u64,
    pub installation_owner: bool,
    pub own_account: bool,
}
impl Readiness {
    pub fn blockers(&self) -> Vec<String> {
        let mut v = Vec::new();
        if !self.last_day_reached {
            v.push("The last working date has not arrived.".into());
        }
        if !self.employee_active {
            v.push("The employee is already archived; review the record before finalizing.".into());
        }
        if self.installation_owner {
            v.push(
                "The installation owner is protected. Transfer deployment ownership separately."
                    .into(),
            );
        }
        if self.own_account {
            v.push("Another Administrator must finalize your exit.".into());
        }
        for (n, label) in [
            (
                self.open_required_tasks,
                "required clearance tasks to finish or waive",
            ),
            (
                self.active_direct_reports,
                "active direct reports to reassign",
            ),
            (self.open_attendance, "open attendance records to complete"),
            (
                self.pending_leave,
                "pending leave requests to resolve or cancel",
            ),
            (
                self.pending_attendance_requests,
                "pending attendance correction requests to resolve or cancel",
            ),
            (
                self.pending_overtime,
                "pending overtime requests to resolve or cancel",
            ),
            (
                self.pending_assigned_leave,
                "filed leave requests still awaiting this named approver; resolve, or cancel and resubmit with a corrected chain",
            ),
            (
                self.named_leave_policies,
                "active leave types naming this approver; update their approval chains",
            ),
            (
                self.open_onboarding_tasks,
                "assigned onboarding tasks to complete or reassign",
            ),
            (
                self.open_clearance_tasks,
                "assigned clearance tasks to complete or reassign",
            ),
            (
                self.scheduled_interviews,
                "scheduled interviews to complete, cancel or reassign",
            ),
        ] {
            if n > 0 {
                v.push(format!("{n} {label}."));
            }
        }
        v
    }
    pub fn ready(&self) -> bool {
        self.blockers().is_empty()
    }
}
#[derive(Clone, Debug)]
pub struct CaseDraft {
    pub id: Uuid,
    pub original: Option<Case>,
    pub employee_id: Option<Uuid>,
    pub last_working_on: String,
    pub category: String,
    pub private_notes: String,
    pub reason: String,
}
impl CaseDraft {
    pub fn new(data: &Snapshot, employee_id: Option<Uuid>) -> Self {
        Self {
            id: Uuid::new_v4(),
            original: None,
            employee_id,
            last_working_on: data.company.today().to_string(),
            category: "resignation".into(),
            private_notes: String::new(),
            reason: String::new(),
        }
    }
    pub fn existing(c: &Case) -> Self {
        Self {
            id: c.id,
            original: Some(c.clone()),
            employee_id: Some(c.employee_id),
            last_working_on: c.last_working_on.to_string(),
            category: c.category.clone(),
            private_notes: c.private_notes.clone(),
            reason: String::new(),
        }
    }
}
#[derive(Clone, Debug)]
pub struct TaskDraft {
    pub id: Uuid,
    pub case_id: Uuid,
    pub original: Option<Task>,
    pub assigned_employee_id: Option<Uuid>,
    pub title: String,
    pub instructions: String,
    pub due_on: String,
    pub required: bool,
    pub reason: String,
}
impl TaskDraft {
    pub fn new(case: Uuid) -> Self {
        Self {
            id: Uuid::new_v4(),
            case_id: case,
            original: None,
            assigned_employee_id: None,
            title: String::new(),
            instructions: String::new(),
            due_on: String::new(),
            required: true,
            reason: String::new(),
        }
    }
    pub fn existing(t: &Task) -> Self {
        Self {
            id: t.id,
            case_id: t.case_id,
            original: Some(t.clone()),
            assigned_employee_id: t.assigned_employee_id,
            title: t.title.clone(),
            instructions: t.instructions.clone(),
            due_on: t.due_on.map(|d| d.to_string()).unwrap_or_default(),
            required: t.required,
            reason: String::new(),
        }
    }
}
#[derive(Clone, Debug)]
pub enum ExitAction {
    CaseSave(CaseDraft),
    TaskSave(TaskDraft),
    CaseStatus {
        id: Uuid,
        revision: i64,
        status: String,
        reason: String,
    },
    TaskStatus {
        id: Uuid,
        revision: i64,
        status: String,
        reason: String,
    },
}
pub fn case(data: &Snapshot, id: Uuid) -> Result<&Case> {
    ensure!(
        data.access.role.manages_hr(),
        "Only HR and Administrators can see private exit records."
    );
    data.exits
        .cases
        .iter()
        .find(|c| c.id == id && c.company_id == data.company.id)
        .context("Exit record not found. Refresh.")
}
fn task(data: &Snapshot, id: Uuid) -> Result<&Task> {
    let t = data
        .exits
        .tasks
        .iter()
        .find(|t| t.id == id && t.company_id == data.company.id)
        .context("Clearance task not found. Refresh.")?;
    ensure!(
        data.access.role.manages_hr()
            || (data.access.role == Role::Manager
                && t.assigned_employee_id.is_some()
                && t.assigned_employee_id == data.access.employee_id),
        "This clearance task is not assigned to you."
    );
    Ok(t)
}
fn text(s: &str, max: usize, required: bool, multiline: bool) -> Result<()> {
    ensure!(
        (!required || !s.trim().is_empty())
            && s.trim().chars().count() <= max
            && (multiline || !s.chars().any(char::is_control)),
        "Check required text and the maximum length ({max} characters)."
    );
    Ok(())
}
impl ExitAction {
    pub fn payload(&self, data: &Snapshot) -> Result<Value> {
        ensure!(
            data.mode != Mode::SignedOut
                && data.access.company_id == data.company.id
                && data.access.role.uses_desktop(),
            "Active desktop staff access is required."
        );
        let (op, p) = match self {
            Self::CaseSave(d) => {
                ensure!(
                    data.access.role.manages_hr(),
                    "Only HR manages exit details."
                );
                let employee = d.employee_id.context("Choose an active employee.")?;
                let retry = d.original.is_none() && data.exits.cases.iter().any(|c| c.id == d.id);
                if !retry {
                    let e = data
                        .employees
                        .iter()
                        .find(|e| e.id == employee && e.status == EmployeeStatus::Active)
                        .context("Choose an active employee.")?;
                    ensure!(
                        e.joined_on
                            .is_none_or(|start| parse_date(&d.last_working_on)
                                .is_ok_and(|date| date >= start)),
                        "The last working date cannot precede joining."
                    );
                    ensure!(
                        !data.exits.cases.iter().any(|c| c.employee_id == employee
                            && c.id != d.id
                            && ["open", "cleared"].contains(&c.status.as_str())),
                        "This employee already has an active exit. Open that record."
                    );
                }
                if let Some(old) = &d.original {
                    let c = case(data, d.id)?;
                    ensure!(
                        c.revision == old.revision
                            && c.status == "open"
                            && c.employee_id == employee,
                        "This exit changed or is closed. Refresh and reopen it."
                    );
                }
                ensure!(
                    ["resignation", "end_of_contract", "retirement", "other"]
                        .contains(&d.category.as_str()),
                    "Choose an available separation category."
                );
                text(&d.private_notes, 2000, false, true)?;
                (
                    "case_save",
                    json!({"id":d.id,"expected_revision":d.original.as_ref().map(|c|c.revision),"employee_id":employee,"last_working_on":parse_date(&d.last_working_on)?,"category":d.category,"private_notes":d.private_notes.trim(),"reason":d.reason.trim()}),
                )
            }
            Self::TaskSave(d) => {
                ensure!(
                    data.access.role.manages_hr(),
                    "Only HR assigns clearance tasks."
                );
                ensure!(
                    case(data, d.case_id)?.status == "open",
                    "Reopen clearance before editing its checklist."
                );
                if let Some(old) = &d.original {
                    let t = task(data, d.id)?;
                    ensure!(
                        t.revision == old.revision && t.case_id == d.case_id && t.status == "open",
                        "This task changed or is finished. Refresh and reopen it."
                    );
                }
                if let Some(id) = d.assigned_employee_id {
                    ensure!(
                        data.recruiting
                            .interviewers
                            .iter()
                            .any(|s| s.employee_id == id),
                        "Assign active staff with desktop access."
                    );
                }
                text(&d.title, 120, true, false)?;
                text(&d.instructions, 2000, false, true)?;
                (
                    "task_save",
                    json!({"id":d.id,"expected_revision":d.original.as_ref().map(|t|t.revision),"case_id":d.case_id,"assigned_employee_id":d.assigned_employee_id,"title":d.title.trim(),"instructions":d.instructions.trim(),"due_on":if d.due_on.trim().is_empty(){None}else{Some(parse_date(&d.due_on)?)},"required":d.required,"reason":d.reason.trim()}),
                )
            }
            Self::TaskStatus {
                id,
                revision,
                status,
                reason,
            } => {
                let t = task(data, *id)?;
                ensure!(
                    t.revision == *revision
                        && t.case_status == "open"
                        && status != &t.status
                        && ["open", "done", "waived"].contains(&status.as_str()),
                    "This task changed, or the action is unavailable. Refresh."
                );
                ensure!(
                    data.access.role.manages_hr() || (status != "waived" && t.status != "waived"),
                    "HR handles clearance waivers."
                );
                (
                    "task_status",
                    json!({"id":id,"expected_revision":revision,"status":status,"reason":reason.trim()}),
                )
            }
            Self::CaseStatus {
                id,
                revision,
                status,
                reason,
            } => {
                let c = case(data, *id)?;
                ensure!(
                    c.revision == *revision
                        && ((c.status == "open"
                            && ["cleared", "cancelled"].contains(&status.as_str()))
                            || (c.status == "cleared"
                                && ["open", "cancelled", "finalized"].contains(&status.as_str()))),
                    "This exit changed, or the action is unavailable. Refresh."
                );
                if ["cleared", "finalized"].contains(&status.as_str()) {
                    ensure!(
                        !data
                            .exits
                            .tasks
                            .iter()
                            .any(|t| t.case_id == *id && t.required && t.status == "open"),
                        "Finish or waive every required clearance task first."
                    );
                }
                if status == "finalized" {
                    ensure!(
                        data.access.role == Role::Administrator,
                        "An Administrator must finalize exit and disable sign-in."
                    );
                    let r = readiness(data, *id)
                        .context("Refresh exit readiness before finalizing.")?;
                    ensure!(
                        r.case_revision == *revision && r.ready(),
                        "Exit is not ready. Resolve the readiness items and Refresh."
                    );
                }
                (
                    "case_status",
                    json!({"id":id,"expected_revision":revision,"status":status,"reason":reason.trim()}),
                )
            }
        };
        hiring::reason(p["reason"].as_str().unwrap_or_default())?;
        Ok(json!({"p_operation":op,"p_record":p}))
    }
    pub fn saved(&self, data: &Snapshot) -> bool {
        let Ok(p) = self.receipt_payload() else {
            return false;
        };
        let id = p["id"].as_str().and_then(|s| Uuid::parse_str(s).ok());
        let record = match self {
            Self::CaseSave(_) | Self::CaseStatus { .. } => data
                .exits
                .cases
                .iter()
                .find(|c| Some(c.id) == id)
                .and_then(|c| serde_json::to_value(c).ok()),
            _ => data
                .exits
                .tasks
                .iter()
                .find(|t| Some(t.id) == id)
                .and_then(|t| serde_json::to_value(t).ok()),
        };
        record.is_some_and(|r| {
            r["company_id"] == json!(data.company.id)
                && r["revision"] == json!(p["expected_revision"].as_i64().map_or(0, |n| n + 1))
                && p.as_object().unwrap().iter().all(|(k, v)| {
                    ["expected_revision", "reason"].contains(&k.as_str()) || r.get(k) == Some(v)
                })
        })
    }
    fn receipt_payload(&self) -> Result<Value> {
        Ok(match self {
            Self::CaseSave(d) => {
                json!({"id":d.id,"expected_revision":d.original.as_ref().map(|c|c.revision),"employee_id":d.employee_id,"last_working_on":parse_date(&d.last_working_on)?,"category":d.category,"private_notes":d.private_notes.trim()})
            }
            Self::TaskSave(d) => {
                json!({"id":d.id,"expected_revision":d.original.as_ref().map(|t|t.revision),"case_id":d.case_id,"assigned_employee_id":d.assigned_employee_id,"title":d.title.trim(),"instructions":d.instructions.trim(),"due_on":if d.due_on.trim().is_empty(){None}else{Some(parse_date(&d.due_on)?)},"required":d.required})
            }
            Self::CaseStatus {
                id,
                revision,
                status,
                ..
            }
            | Self::TaskStatus {
                id,
                revision,
                status,
                ..
            } => json!({"id":id,"expected_revision":revision,"status":status}),
        })
    }
}
pub const DEFAULT_TASKS: [&str; 5] = [
    "Complete work handover",
    "Return company equipment and property",
    "Review system access and credentials",
    "Complete HR documents and records",
    "Record payroll and final-pay review",
];
pub fn readiness(data: &Snapshot, id: Uuid) -> Option<Readiness> {
    if !data.access.role.manages_hr() {
        return None;
    }
    if data.mode != Mode::Demo {
        return data
            .exits
            .readiness
            .iter()
            .find(|r| r.case_id == id && r.company_id == data.company.id)
            .cloned();
    }
    let c = case(data, id).ok()?;
    let person = c.employee_id;
    Some(Readiness {
        case_id: id,
        company_id: data.company.id,
        case_revision: c.revision,
        last_day_reached: c.last_working_on <= data.company.today(),
        employee_active: data
            .employees
            .iter()
            .any(|e| e.id == person && e.status == EmployeeStatus::Active),
        open_required_tasks: data
            .exits
            .tasks
            .iter()
            .filter(|t| t.case_id == id && t.required && t.status == "open")
            .count() as u64,
        active_direct_reports: data
            .employees
            .iter()
            .filter(|e| e.reports_to == Some(person) && e.status == EmployeeStatus::Active)
            .count() as u64,
        open_attendance: data
            .attendance
            .iter()
            .filter(|a| a.employee_id == person && a.checked_out_at.is_none())
            .count() as u64,
        pending_leave: data
            .leave
            .iter()
            .filter(|r| r.employee_id == person && r.status == crate::domain::LeaveStatus::Pending)
            .count() as u64,
        pending_attendance_requests: data
            .requests
            .records
            .iter()
            .filter(|r| r.employee_id == person && r.status == crate::requests::Status::Pending)
            .count() as u64,
        pending_overtime: data
            .overtime
            .records
            .iter()
            .filter(|r| r.employee_id == person && r.status == crate::overtime::Status::Pending)
            .count() as u64,
        pending_assigned_leave: data
            .leave
            .iter()
            .filter(|r| {
                r.status == crate::domain::LeaveStatus::Pending
                    && r.policy.approval_steps.iter().any(|s| {
                        s.approved_by.is_none()
                            && s.employee_id == Some(person)
                            && matches!(
                                s.kind,
                                crate::leave::ApproverKind::Manager
                                    | crate::leave::ApproverKind::Person
                            )
                    })
            })
            .count() as u64,
        named_leave_policies: data
            .leave_setup
            .policy
            .types
            .iter()
            .filter(|t| {
                t.active
                    && t.approval_chain.iter().any(|s| {
                        s.kind == crate::leave::ApproverKind::Person
                            && s.employee_id == Some(person)
                    })
            })
            .count() as u64,
        open_onboarding_tasks: data
            .onboarding
            .tasks
            .iter()
            .filter(|t| {
                t.assigned_employee_id == Some(person)
                    && t.case_status == "open"
                    && t.status == "open"
            })
            .count() as u64,
        open_clearance_tasks: data
            .exits
            .tasks
            .iter()
            .filter(|t| {
                t.assigned_employee_id == Some(person)
                    && t.case_status == "open"
                    && t.status == "open"
            })
            .count() as u64,
        scheduled_interviews: data
            .recruiting
            .interviews
            .iter()
            .filter(|i| {
                i.interviewer_employee_id == person
                    && i.status == crate::recruiting::InterviewStatus::Scheduled
            })
            .count() as u64,
        installation_owner: data
            .accounts
            .iter()
            .any(|a| a.employee_id == Some(person) && a.is_owner),
        own_account: data.access.employee_id == Some(person),
    })
}
pub fn apply_demo(data: &mut Snapshot, action: &ExitAction, now: DateTime<Utc>) -> Result<()> {
    let payload = action.payload(data)?;
    let p = &payload["p_record"];
    let id: Uuid = serde_json::from_value(p["id"].clone())?;
    let create = matches!(action,ExitAction::CaseSave(d)if d.original.is_none())
        || matches!(action,ExitAction::TaskSave(d)if d.original.is_none());
    if create {
        if let Some(old) = data.exits.retries.get(&id) {
            ensure!(old == &payload, "This record ID already exists.");
            return Ok(());
        }
    }
    let mut next = data.clone();
    let before;
    let after;
    let case_id;
    let task_id;
    let kind;
    match action {
        ExitAction::CaseSave(d) => {
            let old = next.exits.cases.iter().find(|c| c.id == id).cloned();
            let e = next
                .employees
                .iter()
                .find(|e| Some(e.id) == d.employee_id)
                .context("Employee not found.")?;
            let c = Case {
                id,
                company_id: next.company.id,
                employee_id: e.id,
                employee_name: old
                    .as_ref()
                    .map_or(e.full_name.clone(), |c| c.employee_name.clone()),
                employee_code: old
                    .as_ref()
                    .map_or(e.employee_code.clone(), |c| c.employee_code.clone()),
                last_working_on: parse_date(&d.last_working_on)?,
                category: d.category.clone(),
                private_notes: d.private_notes.trim().into(),
                status: "open".into(),
                revision: old.as_ref().map_or(0, |c| c.revision + 1),
                created_by: old.as_ref().map_or(data.access.user_id, |c| c.created_by),
                created_at: old.as_ref().map_or(now, |c| c.created_at),
                updated_at: now,
            };
            before = old.as_ref().map(serde_json::to_value).transpose()?;
            after = serde_json::to_value(&c)?;
            case_id = id;
            task_id = None;
            kind = if old.is_some() { "edited" } else { "started" };
            for t in next
                .exits
                .tasks
                .iter_mut()
                .filter(|t| t.case_id == id && t.last_working_on != c.last_working_on)
            {
                t.last_working_on = c.last_working_on;
                t.revision += 1;
                t.updated_at = now;
            }
            if old.is_none() {
                for title in DEFAULT_TASKS {
                    next.exits.tasks.push(Task {
                        id: Uuid::new_v4(),
                        company_id: c.company_id,
                        case_id: id,
                        employee_id: c.employee_id,
                        employee_name: c.employee_name.clone(),
                        employee_code: c.employee_code.clone(),
                        last_working_on: c.last_working_on,
                        case_status: "open".into(),
                        assigned_employee_id: None,
                        title: title.into(),
                        instructions: String::new(),
                        due_on: Some(c.last_working_on),
                        required: true,
                        status: "open".into(),
                        revision: 0,
                        created_by: data.access.user_id,
                        created_at: now,
                        updated_at: now,
                    });
                }
            }
            next.exits.cases.retain(|r| r.id != id);
            next.exits.cases.push(c);
        }
        ExitAction::TaskSave(d) => {
            let old = next.exits.tasks.iter().find(|t| t.id == id).cloned();
            let c = case(data, d.case_id)?;
            let t = Task {
                id,
                company_id: c.company_id,
                case_id: c.id,
                employee_id: c.employee_id,
                employee_name: c.employee_name.clone(),
                employee_code: c.employee_code.clone(),
                last_working_on: c.last_working_on,
                case_status: c.status.clone(),
                assigned_employee_id: d.assigned_employee_id,
                title: d.title.trim().into(),
                instructions: d.instructions.trim().into(),
                due_on: serde_json::from_value(p["due_on"].clone())?,
                required: d.required,
                status: "open".into(),
                revision: old.as_ref().map_or(0, |t| t.revision + 1),
                created_by: old.as_ref().map_or(data.access.user_id, |t| t.created_by),
                created_at: old.as_ref().map_or(now, |t| t.created_at),
                updated_at: now,
            };
            before = old.as_ref().map(serde_json::to_value).transpose()?;
            after = serde_json::to_value(&t)?;
            case_id = c.id;
            task_id = Some(id);
            kind = if old.is_some() {
                "task_edited"
            } else {
                "task_created"
            };
            next.exits.tasks.retain(|r| r.id != id);
            next.exits.tasks.push(t);
        }
        ExitAction::TaskStatus { status, .. } => {
            let t = next
                .exits
                .tasks
                .iter_mut()
                .find(|t| t.id == id)
                .context("Task not found.")?;
            before = Some(serde_json::to_value(&*t)?);
            t.status = status.clone();
            t.revision += 1;
            t.updated_at = now;
            after = serde_json::to_value(&*t)?;
            case_id = t.case_id;
            task_id = Some(id);
            kind = match status.as_str() {
                "done" => "task_done",
                "waived" => "task_waived",
                _ => "task_open",
            };
        }
        ExitAction::CaseStatus { status, .. } => {
            let c = next
                .exits
                .cases
                .iter_mut()
                .find(|c| c.id == id)
                .context("Exit not found.")?;
            before = Some(serde_json::to_value(&*c)?);
            c.status = status.clone();
            c.revision += 1;
            c.updated_at = now;
            after = serde_json::to_value(&*c)?;
            case_id = id;
            task_id = None;
            kind = match status.as_str() {
                "cleared" => "cleared",
                "finalized" => "finalized",
                "cancelled" => "cancelled",
                _ => "open",
            };
            for t in next.exits.tasks.iter_mut().filter(|t| t.case_id == id) {
                t.case_status = status.clone();
                t.revision += 1;
                t.updated_at = now;
            }
            if status == "finalized" {
                let person = c.employee_id;
                for a in next
                    .accounts
                    .iter_mut()
                    .filter(|a| a.employee_id == Some(person))
                {
                    a.active = false;
                }
                next.employees
                    .iter_mut()
                    .find(|e| e.id == person)
                    .context("Employee not found.")?
                    .status = EmployeeStatus::Archived;
            }
        }
    }
    let event = Event {
        id: Uuid::new_v4(),
        company_id: next.company.id,
        offer_id: None,
        case_id: Some(case_id),
        task_id,
        kind: kind.into(),
        reason: p["reason"].as_str().unwrap().into(),
        actor_id: data.access.user_id,
        actor_name: data
            .access
            .employee_id
            .map(|id| data.employee_name(id))
            .unwrap_or("Demo HR")
            .into(),
        actor_role: data.access.role,
        before_record: before,
        after_record: after,
        recorded_at: now,
    };
    next.exits.events.push(event);
    if create {
        next.exits.retries.insert(id, payload);
    }
    *data = next;
    Ok(())
}
