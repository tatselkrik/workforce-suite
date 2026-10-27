//! Shared career aspirations and independently reviewed milestones, separate from employment actions.
use crate::{
    access::Role,
    domain::{Employee, EmployeeStatus, Mode, Snapshot, parse_date},
    hiring,
};
use anyhow::{Context, Result, ensure};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Plan {
    pub id: Uuid,
    pub company_id: Uuid,
    pub employee_id: Uuid,
    pub employee_name: String,
    pub employee_code: String,
    pub title: String,
    pub target_role: String,
    pub target_department: String,
    pub skills_focus: String,
    pub shared_notes: String,
    pub starts_on: NaiveDate,
    pub target_on: Option<NaiveDate>,
    pub status: String,
    pub completed_on: Option<NaiveDate>,
    pub completion_summary: String,
    pub completed_by: Option<Uuid>,
    pub milestones_snapshot: Vec<Value>,
    pub revision: i64,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Milestone {
    pub id: Uuid,
    pub company_id: Uuid,
    pub employee_id: Uuid,
    pub plan_id: Uuid,
    pub title: String,
    pub action_steps: String,
    pub due_on: Option<NaiveDate>,
    pub required: bool,
    pub goal_id: Option<Uuid>,
    pub training_id: Option<Uuid>,
    pub certificate_id: Option<Uuid>,
    pub status: String,
    pub progress_note: String,
    pub review_result: String,
    pub completed_on: Option<NaiveDate>,
    pub reviewed_by: Option<Uuid>,
    pub revision: i64,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub id: Uuid,
    pub company_id: Uuid,
    pub plan_id: Uuid,
    pub milestone_id: Option<Uuid>,
    pub kind: String,
    pub reason: String,
    pub actor_id: Uuid,
    pub actor_name: String,
    pub actor_role: Role,
    pub before_record: Option<Value>,
    pub after_record: Value,
    pub recorded_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Default)]
pub struct Setup {
    pub plans: Vec<Plan>,
    pub milestones: Vec<Milestone>,
    pub events: Vec<Event>,
    pub(crate) retries: std::collections::HashMap<String, (Uuid, Value)>,
}
#[derive(Clone, Debug)]
pub struct PlanDraft {
    pub id: Uuid,
    pub original: Option<Plan>,
    pub employee_id: Option<Uuid>,
    pub title: String,
    pub target_role: String,
    pub target_department: String,
    pub skills_focus: String,
    pub shared_notes: String,
    pub starts_on: String,
    pub target_on: String,
    pub reason: String,
}
impl PlanDraft {
    pub fn new(employee_id: Option<Uuid>, today: NaiveDate) -> Self {
        Self {
            id: Uuid::new_v4(),
            original: None,
            employee_id,
            title: String::new(),
            target_role: String::new(),
            target_department: String::new(),
            skills_focus: String::new(),
            shared_notes: String::new(),
            starts_on: today.to_string(),
            target_on: String::new(),
            reason: String::new(),
        }
    }
    pub fn existing(p: &Plan) -> Self {
        Self {
            id: p.id,
            original: Some(p.clone()),
            employee_id: Some(p.employee_id),
            title: p.title.clone(),
            target_role: p.target_role.clone(),
            target_department: p.target_department.clone(),
            skills_focus: p.skills_focus.clone(),
            shared_notes: p.shared_notes.clone(),
            starts_on: p.starts_on.to_string(),
            target_on: crate::learning::date_text(p.target_on),
            reason: String::new(),
        }
    }
}
#[derive(Clone, Debug)]
pub struct MilestoneDraft {
    pub id: Uuid,
    pub original: Option<Milestone>,
    pub plan_id: Uuid,
    pub title: String,
    pub action_steps: String,
    pub due_on: String,
    pub required: bool,
    pub goal_id: Option<Uuid>,
    pub training_id: Option<Uuid>,
    pub certificate_id: Option<Uuid>,
    pub reason: String,
}
impl MilestoneDraft {
    pub fn new(plan_id: Uuid) -> Self {
        Self {
            id: Uuid::new_v4(),
            original: None,
            plan_id,
            title: String::new(),
            action_steps: String::new(),
            due_on: String::new(),
            required: true,
            goal_id: None,
            training_id: None,
            certificate_id: None,
            reason: String::new(),
        }
    }
    pub fn existing(t: &Milestone) -> Self {
        Self {
            id: t.id,
            original: Some(t.clone()),
            plan_id: t.plan_id,
            title: t.title.clone(),
            action_steps: t.action_steps.clone(),
            due_on: crate::learning::date_text(t.due_on),
            required: t.required,
            goal_id: t.goal_id,
            training_id: t.training_id,
            certificate_id: t.certificate_id,
            reason: String::new(),
        }
    }
}
#[derive(Clone, Debug)]
pub enum CareerAction {
    PlanSave(PlanDraft),
    PlanStatus {
        id: Uuid,
        revision: i64,
        status: String,
        completed_on: String,
        summary: String,
        reason: String,
    },
    MilestoneSave(MilestoneDraft),
    Progress {
        id: Uuid,
        revision: i64,
        note: String,
        reason: String,
    },
    MilestoneStatus {
        id: Uuid,
        revision: i64,
        status: String,
        completed_on: String,
        result: String,
        reason: String,
    },
}
fn optional(s: &str) -> Result<Option<NaiveDate>> {
    if s.trim().is_empty() {
        Ok(None)
    } else {
        Ok(Some(parse_date(s)?))
    }
}
fn text(s: &str, max: usize, required: bool, multiline: bool) -> Result<()> {
    ensure!(
        (!required || !s.trim().is_empty())
            && s.trim().chars().count() <= max
            && (multiline || !s.chars().any(char::is_control)),
        "Check required text and maximum length ({max} characters)."
    );
    Ok(())
}
fn person(data: &Snapshot, id: Uuid) -> Result<&Employee> {
    data.employees
        .iter()
        .find(|e| e.id == id && e.company_id == data.company.id)
        .context("Employee not found. Refresh.")
}
pub fn in_scope(data: &Snapshot, id: Uuid) -> bool {
    person(data, id).is_ok_and(|e| {
        (data.access.allows(crate::staff_roles::Capability::DevelopmentAdministration) || data.access.allows(crate::staff_roles::Capability::PrivateHrNotes))
            || Some(e.id) == data.access.employee_id
            || (data.access.allows(crate::staff_roles::Capability::TeamReview)
                && data.access.employee_id.is_some()
                && e.reports_to == data.access.employee_id
                && e.status == EmployeeStatus::Active)
    })
}
pub fn can_manage(data: &Snapshot, id: Uuid) -> bool {
    data.access.employee_id.is_some()
        && data.access.employee_id != Some(id)
        && person(data, id).is_ok_and(|e| {
            data.access.allows(crate::staff_roles::Capability::DevelopmentAdministration)
                || (data.access.allows(crate::staff_roles::Capability::TeamReview)
                    && e.reports_to == data.access.employee_id
                    && e.status == EmployeeStatus::Active)
        })
}
pub fn plan(data: &Snapshot, id: Uuid) -> Result<&Plan> {
    let p = data
        .career
        .plans
        .iter()
        .find(|p| p.id == id && p.company_id == data.company.id)
        .context("Plan not found. Refresh.")?;
    ensure!(
        in_scope(data, p.employee_id),
        "Plan is outside your current access."
    );
    Ok(p)
}
pub fn milestone(data: &Snapshot, id: Uuid) -> Result<&Milestone> {
    let t = data
        .career
        .milestones
        .iter()
        .find(|t| t.id == id && t.company_id == data.company.id)
        .context("Milestone not found. Refresh.")?;
    ensure!(
        plan(data, t.plan_id)?.employee_id == t.employee_id,
        "Milestone is outside your current plan."
    );
    Ok(t)
}
fn reference(
    data: &Snapshot,
    employee: Uuid,
    goal: Option<Uuid>,
    training: Option<Uuid>,
    certificate: Option<Uuid>,
) -> Result<Value> {
    ensure!(
        [goal, training, certificate]
            .iter()
            .filter(|v| v.is_some())
            .count()
            <= 1,
        "Choose at most one related goal, training or certificate."
    );
    if let Some(id) = goal {
        let g = data
            .performance
            .goals
            .iter()
            .find(|g| g.id == id && g.company_id == data.company.id && g.employee_id == employee)
            .context("Choose a goal for this employee.")?;
        return Ok(
            json!({"kind":"goal","id":g.id,"title":g.title,"status":g.status,"progress":g.progress,"due_on":g.due_on}),
        );
    }
    if let Some(id) = training {
        let t = data
            .learning
            .training
            .iter()
            .find(|t| t.id == id && t.company_id == data.company.id && t.employee_id == employee)
            .context("Choose training for this employee.")?;
        return Ok(
            json!({"kind":"training","id":t.id,"title":t.title,"status":t.status,"completed_on":t.completed_on,"learning_minutes":t.learning_minutes}),
        );
    }
    if let Some(id) = certificate {
        let c = data
            .learning
            .certificates
            .iter()
            .find(|c| c.id == id && c.company_id == data.company.id && c.employee_id == employee)
            .context("Choose a certificate for this employee.")?;
        return Ok(
            json!({"kind":"certificate","id":c.id,"title":c.title,"issuer":c.issuer,"status":c.status,"archived":c.archived,"issued_on":c.issued_on,"expires_on":c.expires_on}),
        );
    }
    Ok(Value::Null)
}
pub fn closure_snapshot(data: &Snapshot, plan_id: Uuid) -> Result<Vec<Value>> {
    let p = plan(data, plan_id)?;
    let mut rows: Vec<_> = data
        .career
        .milestones
        .iter()
        .filter(|t| t.plan_id == p.id && t.company_id == data.company.id)
        .collect();
    rows.sort_by_key(|t| (t.due_on.is_none(), t.due_on, t.created_at, t.id));
    rows.into_iter().map(|t|Ok(json!({"id":t.id,"title":t.title,"action_steps":t.action_steps,"due_on":t.due_on,"required":t.required,"status":t.status,"progress_note":t.progress_note,"review_result":t.review_result,"completed_on":t.completed_on,"revision":t.revision,"reference":reference(data,t.employee_id,t.goal_id,t.training_id,t.certificate_id)?}))).collect()
}
impl CareerAction {
    pub fn reason_mut(&mut self) -> &mut String {
        match self {
            Self::PlanSave(d) => &mut d.reason,
            Self::MilestoneSave(d) => &mut d.reason,
            Self::PlanStatus { reason, .. }
            | Self::Progress { reason, .. }
            | Self::MilestoneStatus { reason, .. } => reason,
        }
    }
    pub fn id(&self) -> Uuid {
        match self {
            Self::PlanSave(d) => d.id,
            Self::MilestoneSave(d) => d.id,
            Self::PlanStatus { id, .. }
            | Self::Progress { id, .. }
            | Self::MilestoneStatus { id, .. } => *id,
        }
    }
    pub fn is_plan(&self) -> bool {
        matches!(self, Self::PlanSave(_) | Self::PlanStatus { .. })
    }
    fn record(&self) -> Result<(&'static str, Value)> {
        Ok(match self {
            Self::PlanSave(d) => {
                text(&d.title, 120, true, false)?;
                text(&d.target_role, 120, false, false)?;
                text(&d.target_department, 120, false, false)?;
                text(&d.skills_focus, 2000, false, true)?;
                text(&d.shared_notes, 2000, false, true)?;
                let start = parse_date(&d.starts_on)?;
                let target = optional(&d.target_on)?;
                ensure!(
                    target.is_none_or(|d| d >= start),
                    "Target date cannot precede the start."
                );
                (
                    "plan_save",
                    json!({"id":d.id,"expected_revision":d.original.as_ref().map(|p|p.revision),"employee_id":d.employee_id.context("Choose an employee.")?,"title":d.title.trim(),"target_role":d.target_role.trim(),"target_department":d.target_department.trim(),"skills_focus":d.skills_focus.trim(),"shared_notes":d.shared_notes.trim(),"starts_on":start,"target_on":target,"reason":d.reason.trim()}),
                )
            }
            Self::PlanStatus {
                id,
                revision,
                status,
                completed_on,
                summary,
                reason,
            } => {
                text(summary, 2000, false, true)?;
                (
                    "plan_status",
                    json!({"id":id,"expected_revision":revision,"status":status,"completed_on":optional(completed_on)?,"completion_summary":summary.trim(),"reason":reason.trim()}),
                )
            }
            Self::MilestoneSave(d) => {
                text(&d.title, 120, true, false)?;
                text(&d.action_steps, 2000, false, true)?;
                (
                    "milestone_save",
                    json!({"id":d.id,"expected_revision":d.original.as_ref().map(|t|t.revision),"plan_id":d.plan_id,"title":d.title.trim(),"action_steps":d.action_steps.trim(),"due_on":optional(&d.due_on)?,"required":d.required,"goal_id":d.goal_id,"training_id":d.training_id,"certificate_id":d.certificate_id,"reason":d.reason.trim()}),
                )
            }
            Self::Progress {
                id,
                revision,
                note,
                reason,
            } => {
                text(note, 1000, false, true)?;
                (
                    "milestone_progress",
                    json!({"id":id,"expected_revision":revision,"progress_note":note.trim(),"reason":reason.trim()}),
                )
            }
            Self::MilestoneStatus {
                id,
                revision,
                status,
                completed_on,
                result,
                reason,
            } => {
                text(result, 2000, false, true)?;
                (
                    "milestone_status",
                    json!({"id":id,"expected_revision":revision,"status":status,"completed_on":optional(completed_on)?,"review_result":result.trim(),"reason":reason.trim()}),
                )
            }
        })
    }
    pub fn payload(&self, data: &Snapshot) -> Result<Value> {
        ensure!(
            data.mode != Mode::SignedOut
                && data.access.company_id == data.company.id
                && data.access.employee_id.is_some()
                && data.access.uses_desktop(),
            "Active desktop staff access is required."
        );
        let (op, payload) = self.record()?;
        hiring::reason(payload["reason"].as_str().unwrap_or_default())?;
        match self {
            Self::PlanSave(d) => {
                let employee = d.employee_id.unwrap();
                ensure!(
                    can_manage(data, employee),
                    "Another HR user or the current reporting Manager manages this plan."
                );
                let e = person(data, employee)?;
                let current = data
                    .career
                    .plans
                    .iter()
                    .find(|p| p.id == d.id && p.company_id == data.company.id);
                if !(d.original.is_none() && current.is_some()) {
                    ensure!(
                        e.status == EmployeeStatus::Active,
                        "Plan changes require an active employee."
                    );
                    let start = parse_date(&d.starts_on)?;
                    let target = optional(&d.target_on)?;
                    ensure!(
                        e.joined_on.is_none_or(|j| start >= j),
                        "Plan start cannot precede joining."
                    );
                    if let Some(old) = &d.original {
                        let p = plan(data, d.id)?;
                        ensure!(
                            p.revision == old.revision
                                && p.employee_id == employee
                                && p.status == "active",
                            "Plan changed or is closed. Refresh/reopen it."
                        );
                        ensure!(
                            data.career
                                .milestones
                                .iter()
                                .filter(|t| t.company_id == data.company.id && t.plan_id == p.id)
                                .all(|t| t.due_on.is_none_or(
                                    |due| due >= start && target.is_none_or(|end| due <= end)
                                ) && t.completed_on.is_none_or(|c| c >= start)),
                            "Plan dates must include all saved milestone dates."
                        );
                    }
                }
            }
            Self::PlanStatus {
                id,
                revision,
                status,
                completed_on,
                summary,
                ..
            } => {
                let p = plan(data, *id)?;
                ensure!(
                    can_manage(data, p.employee_id),
                    "Another HR user or current reporting Manager reviews this plan."
                );
                ensure!(p.revision == *revision, "Plan changed. Refresh.");
                ensure!(
                    (p.status == "active" && ["completed", "cancelled"].contains(&status.as_str()))
                        || (["completed", "cancelled"].contains(&p.status.as_str())
                            && status == "active"),
                    "Choose an available plan transition."
                );
                ensure!(
                    status == "cancelled"
                        || person(data, p.employee_id)?.status == EmployeeStatus::Active,
                    "Archived employees cannot complete or reopen plans."
                );
                let date = optional(completed_on)?;
                if status == "completed" {
                    ensure!(
                        date.is_some_and(|d| d >= p.starts_on && d <= data.company.today())
                            && !summary.trim().is_empty(),
                        "Completion needs a factual date on/before today and a review summary."
                    );
                    let tasks: Vec<_> = data
                        .career
                        .milestones
                        .iter()
                        .filter(|t| t.company_id == data.company.id && t.plan_id == p.id)
                        .collect();
                    ensure!(
                        tasks.len() <= 50
                            && tasks.iter().any(|t| t.status == "done")
                            && !tasks.iter().any(|t| t.required && t.status == "open"),
                        "Resolve required milestones and record at least one completed milestone."
                    );
                    ensure!(
                        tasks
                            .iter()
                            .all(|t| t.completed_on.is_none_or(|d| Some(d) <= date)),
                        "Plan completion cannot precede a completed milestone."
                    );
                    closure_snapshot(data, p.id)?;
                } else {
                    ensure!(
                        date.is_none() && summary.is_empty(),
                        "Completion fields belong only to completed plans."
                    );
                }
            }
            Self::MilestoneSave(d) => {
                let p = plan(data, d.plan_id)?;
                ensure!(
                    can_manage(data, p.employee_id),
                    "Another HR user or current Manager manages these milestones."
                );
                let current = data
                    .career
                    .milestones
                    .iter()
                    .find(|t| t.id == d.id && t.company_id == data.company.id);
                if !(d.original.is_none() && current.is_some()) {
                    ensure!(
                        p.status == "active"
                            && person(data, p.employee_id)?.status == EmployeeStatus::Active,
                        "Reopen an active employee's plan before changing milestones."
                    );
                    let due = optional(&d.due_on)?;
                    ensure!(
                        due.is_none_or(
                            |d| d >= p.starts_on && p.target_on.is_none_or(|end| d <= end)
                        ),
                        "Milestone due date must fall within the plan horizon."
                    );
                    reference(
                        data,
                        p.employee_id,
                        d.goal_id,
                        d.training_id,
                        d.certificate_id,
                    )?;
                    if let Some(old) = &d.original {
                        let t = milestone(data, d.id)?;
                        ensure!(
                            t.revision == old.revision && t.plan_id == p.id && t.status == "open",
                            "Milestone changed or is closed. Refresh/reopen it."
                        );
                    } else {
                        ensure!(
                            data.career
                                .milestones
                                .iter()
                                .filter(|t| t.company_id == data.company.id && t.plan_id == p.id)
                                .count()
                                < 50,
                            "A plan can hold at most 50 milestones."
                        );
                    }
                }
            }
            Self::Progress { id, revision, .. } | Self::MilestoneStatus { id, revision, .. } => {
                let t = milestone(data, *id)?;
                let p = plan(data, t.plan_id)?;
                ensure!(
                    t.revision == *revision
                        && p.status == "active"
                        && person(data, t.employee_id)?.status == EmployeeStatus::Active,
                    "Milestone changed, plan is closed or employee is archived. Refresh."
                );
                match self {
                    Self::Progress { .. } => {
                        ensure!(data.access.allows(crate::staff_roles::Capability::DevelopmentAdministration) || data.access.employee_id==Some(t.employee_id) || (data.access.allows(crate::staff_roles::Capability::TeamReview) && person(data,t.employee_id)?.reports_to==data.access.employee_id), "Only the employee, current Manager or development staff update progress.");
                        ensure!(t.status == "open", "Only open milestones accept progress.")
                    }
                    Self::MilestoneStatus {
                        status,
                        completed_on,
                        result,
                        ..
                    } => {
                        ensure!(
                            can_manage(data, t.employee_id) || (data.access.allows(crate::staff_roles::Capability::PrivateHrNotes) && data.access.employee_id!=Some(t.employee_id) && (status=="waived" || (t.status=="waived" && status=="open"))),
                            "Another HR user or current Manager reviews your milestone."
                        );
                        ensure!(
                            (t.status == "open" && ["done", "waived"].contains(&status.as_str()))
                                || (["done", "waived"].contains(&t.status.as_str())
                                    && status == "open"),
                            "Choose an available milestone transition."
                        );
                        ensure!(
                            status != "waived" || data.access.allows(crate::staff_roles::Capability::PrivateHrNotes),
                            "Only another HR user can waive a milestone."
                        );
                        let date = optional(completed_on)?;
                        if status == "done" {
                            ensure!(
                                date.is_some_and(|d| d >= p.starts_on && d <= data.company.today())
                                    && !result.trim().is_empty(),
                                "Completion needs a factual date and a review result."
                            );
                        } else {
                            ensure!(
                                date.is_none(),
                                "Completion date belongs only to a done milestone."
                            );
                            if status == "waived" {
                                ensure!(!result.trim().is_empty(), "Record the waiver result.");
                            } else {
                                ensure!(result.is_empty(), "Clear review details when reopening.");
                            }
                        }
                    }
                    _ => unreachable!(),
                }
            }
        }
        Ok(json!({"p_operation":op,"p_record":payload}))
    }
    pub fn saved(&self, data: &Snapshot) -> bool {
        let Ok((_, p)) = self.record() else {
            return false;
        };
        let id = self.id();
        let row = if self.is_plan() {
            data.career
                .plans
                .iter()
                .find(|r| r.id == id && r.company_id == data.company.id)
                .and_then(|r| serde_json::to_value(r).ok())
        } else {
            data.career
                .milestones
                .iter()
                .find(|r| r.id == id && r.company_id == data.company.id)
                .and_then(|r| serde_json::to_value(r).ok())
        };
        let Some(row) = row else {
            return false;
        };
        if p["expected_revision"]
            .as_i64()
            .is_some_and(|r| row["revision"].as_i64() != r.checked_add(1))
        {
            return false;
        }
        if !p
            .as_object()
            .unwrap()
            .iter()
            .filter(|(k, _)| !matches!(k.as_str(), "reason" | "expected_revision"))
            .all(|(k, v)| row.get(k) == Some(v))
        {
            return false;
        }
        match self {
            Self::PlanStatus { status, .. } if status == "completed" => {
                row["completed_by"] == json!(data.access.user_id)
                    && row["milestones_snapshot"].as_array().is_some_and(|s| {
                        !s.is_empty()
                            && s.len() <= 50
                            && s.iter().any(|m| m["status"] == "done")
                            && !s
                                .iter()
                                .any(|m| m["required"] == true && m["status"] == "open")
                    })
            }
            Self::MilestoneStatus { status, .. } if status != "open" => {
                row["reviewed_by"] == json!(data.access.user_id)
            }
            _ => true,
        }
    }
}
pub fn visible_events(data: &Snapshot, plan_id: Uuid) -> Vec<&Event> {
    if plan(data, plan_id).is_err() {
        return Vec::new();
    }
    data.career
        .events
        .iter()
        .filter(|e| e.company_id == data.company.id && e.plan_id == plan_id)
        .collect()
}
pub fn apply_demo(data: &mut Snapshot, action: &CareerAction, now: DateTime<Utc>) -> Result<()> {
    let payload = action.payload(data)?;
    let p = &payload["p_record"];
    let id = action.id();
    let is_plan = action.is_plan();
    let birth = match action {
        CareerAction::PlanSave(d) if d.original.is_none() => Some("plan"),
        CareerAction::MilestoneSave(d) if d.original.is_none() => Some("milestone"),
        _ => None,
    };
    let key = birth.map(|k| format!("{k}:{id}"));
    if let Some(key) = &key {
        if let Some((actor, old)) = data.career.retries.get(key) {
            ensure!(
                *actor == data.access.user_id && old == &payload,
                "This record ID exists. Refresh."
            );
            return Ok(());
        }
    }
    let before = if is_plan {
        data.career
            .plans
            .iter()
            .find(|r| r.id == id)
            .map(serde_json::to_value)
            .transpose()?
    } else {
        data.career
            .milestones
            .iter()
            .find(|r| r.id == id)
            .map(serde_json::to_value)
            .transpose()?
    };
    ensure!(
        birth.is_none() || before.is_none(),
        "This record ID exists. Refresh."
    );
    let mut after=before.clone().unwrap_or_else(||json!({"id":id,"company_id":data.company.id,"revision":0,"created_by":data.access.user_id,"created_at":now,"updated_at":now}));
    if let Some(b) = &before {
        after["revision"] = json!(
            b["revision"]
                .as_i64()
                .context("Missing revision")?
                .checked_add(1)
                .context("Revision limit")?
        );
        after["updated_at"] = json!(now);
    }
    let plan_id;
    let kind = match action {
        CareerAction::PlanSave(d) => {
            plan_id = id;
            let e = person(data, d.employee_id.unwrap())?;
            if before.is_none() {
                for(k,v)in json!({"employee_id":e.id,"employee_name":e.full_name,"employee_code":e.employee_code,"status":"active","completed_on":null,"completion_summary":"","completed_by":null,"milestones_snapshot":[]}).as_object().unwrap(){after[k]=v.clone();}
            }
            for k in [
                "title",
                "target_role",
                "target_department",
                "skills_focus",
                "shared_notes",
                "starts_on",
                "target_on",
            ] {
                after[k] = p[k].clone();
            }
            if before.is_none() {
                "plan_created".into()
            } else {
                "plan_edited".into()
            }
        }
        CareerAction::PlanStatus { status, .. } => {
            plan_id = id;
            for k in ["status", "completed_on", "completion_summary"] {
                after[k] = p[k].clone();
            }
            after["completed_by"] = if status == "completed" {
                json!(data.access.user_id)
            } else {
                Value::Null
            };
            after["milestones_snapshot"] = if status == "completed" {
                json!(closure_snapshot(data, id)?)
            } else {
                json!([])
            };
            format!("plan_{status}")
        }
        CareerAction::MilestoneSave(d) => {
            plan_id = d.plan_id;
            if before.is_none() {
                for(k,v)in json!({"plan_id":d.plan_id,"employee_id":plan(data,d.plan_id)?.employee_id,"status":"open","progress_note":"","review_result":"","completed_on":null,"reviewed_by":null}).as_object().unwrap(){after[k]=v.clone();}
            }
            for k in [
                "title",
                "action_steps",
                "due_on",
                "required",
                "goal_id",
                "training_id",
                "certificate_id",
            ] {
                after[k] = p[k].clone();
            }
            if before.is_none() {
                "milestone_created".into()
            } else {
                "milestone_edited".into()
            }
        }
        CareerAction::Progress { .. } => {
            plan_id = milestone(data, id)?.plan_id;
            after["progress_note"] = p["progress_note"].clone();
            "milestone_progress".into()
        }
        CareerAction::MilestoneStatus { status, .. } => {
            plan_id = milestone(data, id)?.plan_id;
            for k in ["status", "completed_on", "review_result"] {
                after[k] = p[k].clone();
            }
            after["reviewed_by"] = if status == "open" {
                Value::Null
            } else {
                json!(data.access.user_id)
            };
            format!("milestone_{status}")
        }
    };
    let mut next = data.clone();
    if is_plan {
        let row: Plan = serde_json::from_value(after.clone())?;
        if let Some(pos) = next.career.plans.iter().position(|r| r.id == id) {
            next.career.plans[pos] = row;
        } else {
            next.career.plans.push(row);
        }
    } else {
        let row: Milestone = serde_json::from_value(after.clone())?;
        if let Some(pos) = next.career.milestones.iter().position(|r| r.id == id) {
            next.career.milestones[pos] = row;
        } else {
            next.career.milestones.push(row);
        }
    }
    next.career.events.push(Event {
        id: Uuid::new_v4(),
        company_id: data.company.id,
        plan_id,
        milestone_id: if is_plan { None } else { Some(id) },
        kind,
        reason: p["reason"].as_str().unwrap().into(),
        actor_id: data.access.user_id,
        actor_name: data
            .access
            .employee_id
            .map(|id| data.employee_name(id).to_owned())
            .unwrap_or_else(|| "Demo staff".into()),
        actor_role: data.access.role,
        before_record: before,
        after_record: after,
        recorded_at: now,
    });
    if let Some(k) = key {
        next.career
            .retries
            .insert(k, (data.access.user_id, payload));
    }
    *data = next;
    Ok(())
}
