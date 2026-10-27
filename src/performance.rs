//! Company goals and protected appraisals. Private HR notes never enter published review rows.
use crate::{
    access::Role,
    domain::{Employee, EmployeeStatus, Mode, Snapshot, parse_date},
    hiring,
};
use anyhow::{Context, Result, ensure};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Goal {
    pub id: Uuid,
    pub company_id: Uuid,
    pub employee_id: Uuid,
    pub employee_name: String,
    pub employee_code: String,
    pub title: String,
    pub description: String,
    pub measure: String,
    pub starts_on: NaiveDate,
    pub due_on: NaiveDate,
    pub progress: u8,
    pub progress_note: String,
    pub status: String,
    pub revision: i64,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Cycle {
    pub id: Uuid,
    pub company_id: Uuid,
    pub name: String,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    pub due_on: NaiveDate,
    pub status: String,
    pub revision: i64,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Review {
    pub id: Uuid,
    pub company_id: Uuid,
    pub cycle_id: Uuid,
    pub employee_id: Uuid,
    pub employee_name: String,
    pub employee_code: String,
    pub cycle_name: String,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    pub due_on: NaiveDate,
    pub reviewer_employee_id: Uuid,
    pub reviewer_name: String,
    pub rating: Option<u8>,
    pub assessment: String,
    pub strengths: String,
    pub development: String,
    pub goals_snapshot: Vec<GoalSnapshot>,
    pub status: String,
    pub submitted_at: Option<DateTime<Utc>>,
    pub published_at: Option<DateTime<Utc>>,
    pub published_by: Option<Uuid>,
    pub revision: i64,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Note {
    pub id: Uuid,
    pub company_id: Uuid,
    pub review_id: Uuid,
    pub note: String,
    pub revision: i64,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GoalSnapshot {
    pub id: Uuid,
    pub title: String,
    pub measure: String,
    pub starts_on: NaiveDate,
    pub due_on: NaiveDate,
    pub progress: u8,
    pub status: String,
    pub revision: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub id: Uuid,
    pub company_id: Uuid,
    pub goal_id: Option<Uuid>,
    pub cycle_id: Option<Uuid>,
    pub review_id: Option<Uuid>,
    pub note_id: Option<Uuid>,
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
    pub goals: Vec<Goal>,
    pub cycles: Vec<Cycle>,
    pub reviews: Vec<Review>,
    pub notes: Vec<Note>,
    pub events: Vec<Event>,
    pub(crate) retries: std::collections::HashMap<String, (Uuid, Value)>,
}
#[derive(Clone, Debug)]
pub struct GoalDraft {
    pub id: Uuid,
    pub original: Option<Goal>,
    pub employee_id: Option<Uuid>,
    pub title: String,
    pub description: String,
    pub measure: String,
    pub starts_on: String,
    pub due_on: String,
    pub reason: String,
}
impl GoalDraft {
    pub fn new(data: &Snapshot, employee_id: Option<Uuid>) -> Self {
        Self {
            id: Uuid::new_v4(),
            original: None,
            employee_id,
            title: String::new(),
            description: String::new(),
            measure: String::new(),
            starts_on: data.company.today().to_string(),
            due_on: data.company.today().to_string(),
            reason: String::new(),
        }
    }
    pub fn existing(g: &Goal) -> Self {
        Self {
            id: g.id,
            original: Some(g.clone()),
            employee_id: Some(g.employee_id),
            title: g.title.clone(),
            description: g.description.clone(),
            measure: g.measure.clone(),
            starts_on: g.starts_on.to_string(),
            due_on: g.due_on.to_string(),
            reason: String::new(),
        }
    }
}
#[derive(Clone, Debug)]
pub struct CycleDraft {
    pub id: Uuid,
    pub original: Option<Cycle>,
    pub name: String,
    pub starts_on: String,
    pub ends_on: String,
    pub due_on: String,
    pub reason: String,
}
impl CycleDraft {
    pub fn new(data: &Snapshot) -> Self {
        let year = data.company.today().year();
        Self {
            id: Uuid::new_v4(),
            original: None,
            name: String::new(),
            starts_on: format!("{year}-01-01"),
            ends_on: format!("{year}-12-31"),
            due_on: format!("{}-01-15", year + 1),
            reason: String::new(),
        }
    }
    pub fn existing(c: &Cycle) -> Self {
        Self {
            id: c.id,
            original: Some(c.clone()),
            name: c.name.clone(),
            starts_on: c.starts_on.to_string(),
            ends_on: c.ends_on.to_string(),
            due_on: c.due_on.to_string(),
            reason: String::new(),
        }
    }
}
#[derive(Clone, Debug)]
pub struct AssignmentDraft {
    pub id: Uuid,
    pub original: Option<Review>,
    pub cycle_id: Option<Uuid>,
    pub employee_id: Option<Uuid>,
    pub reviewer_employee_id: Option<Uuid>,
    pub reason: String,
}
impl AssignmentDraft {
    pub fn new(employee_id: Option<Uuid>, cycle_id: Option<Uuid>) -> Self {
        Self {
            id: Uuid::new_v4(),
            original: None,
            cycle_id,
            employee_id,
            reviewer_employee_id: None,
            reason: String::new(),
        }
    }
    pub fn existing(r: &Review) -> Self {
        Self {
            id: r.id,
            original: Some(r.clone()),
            cycle_id: Some(r.cycle_id),
            employee_id: Some(r.employee_id),
            reviewer_employee_id: Some(r.reviewer_employee_id),
            reason: String::new(),
        }
    }
}
#[derive(Clone, Debug)]
pub struct AssessmentDraft {
    pub id: Uuid,
    pub revision: i64,
    pub rating: Option<u8>,
    pub assessment: String,
    pub strengths: String,
    pub development: String,
    pub reason: String,
}
impl AssessmentDraft {
    pub fn existing(r: &Review) -> Self {
        Self {
            id: r.id,
            revision: r.revision,
            rating: r.rating,
            assessment: r.assessment.clone(),
            strengths: r.strengths.clone(),
            development: r.development.clone(),
            reason: String::new(),
        }
    }
}
#[derive(Clone, Debug)]
pub struct NoteDraft {
    pub id: Uuid,
    pub original: Option<Note>,
    pub review_id: Uuid,
    pub note: String,
    pub reason: String,
}
impl NoteDraft {
    pub fn new(review_id: Uuid) -> Self {
        Self {
            id: Uuid::new_v4(),
            original: None,
            review_id,
            note: String::new(),
            reason: String::new(),
        }
    }
    pub fn existing(n: &Note) -> Self {
        Self {
            id: n.id,
            original: Some(n.clone()),
            review_id: n.review_id,
            note: n.note.clone(),
            reason: String::new(),
        }
    }
}
#[derive(Clone, Debug)]
pub enum PerformanceAction {
    GoalSave(GoalDraft),
    Progress {
        id: Uuid,
        revision: i64,
        progress: u8,
        note: String,
        reason: String,
    },
    GoalStatus {
        id: Uuid,
        revision: i64,
        status: String,
        reason: String,
    },
    CycleSave(CycleDraft),
    CycleStatus {
        id: Uuid,
        revision: i64,
        status: String,
        reason: String,
    },
    Assign(AssignmentDraft),
    Assessment(AssessmentDraft),
    ReviewStatus {
        id: Uuid,
        revision: i64,
        status: String,
        reason: String,
    },
    NoteSave(NoteDraft),
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
fn employee(data: &Snapshot, id: Uuid) -> Result<&Employee> {
    data.employees
        .iter()
        .find(|e| e.id == id && e.company_id == data.company.id)
        .context("Employee not found in your current access. Refresh.")
}
pub fn manages_goal(data: &Snapshot, person: Uuid) -> bool {
    data.access.allows(crate::staff_roles::Capability::DevelopmentAdministration)
        || (data.access.allows(crate::staff_roles::Capability::TeamReview)
            && data.access.employee_id.is_some()
            && employee(data, person).is_ok_and(|e| e.reports_to == data.access.employee_id))
}
pub fn goal(data: &Snapshot, id: Uuid) -> Result<&Goal> {
    let g = data
        .performance
        .goals
        .iter()
        .find(|g| g.id == id && g.company_id == data.company.id)
        .context("Goal not found. Refresh.")?;
    ensure!(
        manages_goal(data, g.employee_id) || data.access.allows(crate::staff_roles::Capability::PrivateHrNotes) || Some(g.employee_id) == data.access.employee_id,
        "Your role cannot read this goal."
    );
    Ok(g)
}
pub fn review(data: &Snapshot, id: Uuid) -> Result<&Review> {
    let r = data
        .performance
        .reviews
        .iter()
        .find(|r| r.id == id && r.company_id == data.company.id)
        .context("Appraisal not found. Refresh.")?;
    ensure!(
        (data.access.allows(crate::staff_roles::Capability::DevelopmentAdministration) || data.access.allows(crate::staff_roles::Capability::PrivateHrNotes))
            || (Some(r.employee_id) == data.access.employee_id && r.status == "published")
            || (data.access.allows(crate::staff_roles::Capability::TeamReview)
                && Some(r.reviewer_employee_id) == data.access.employee_id
                && manages_goal(data, r.employee_id)),
        "This appraisal is outside your current access."
    );
    Ok(r)
}
pub fn cycle(data: &Snapshot, id: Uuid) -> Result<&Cycle> {
    data.performance
        .cycles
        .iter()
        .find(|c| c.id == id && c.company_id == data.company.id)
        .context("Review cycle not found. Refresh.")
}
pub fn eligible_reviewer(data: &Snapshot, person: Uuid, reviewer: Uuid) -> bool {
    person != reviewer
        && data.recruiting.interviewers.iter().any(|s| {
            s.employee_id == reviewer
                && (s.allows(crate::staff_roles::Capability::DevelopmentAdministration)
                    || (s.allows(crate::staff_roles::Capability::TeamReview)
                        && employee(data, person).is_ok_and(|e| e.reports_to == Some(reviewer))))
        })
}
fn hr(data: &Snapshot) -> Result<()> {
    ensure!(
        data.access.allows(crate::staff_roles::Capability::DevelopmentAdministration),
        "Only HR and Administrators manage appraisal cycles and assignments."
    );
    Ok(())
}
fn active(data: &Snapshot, person: Uuid) -> Result<()> {
    ensure!(
        employee(data, person)?.status == EmployeeStatus::Active,
        "Choose an active employee."
    );
    Ok(())
}
fn independent(data: &Snapshot, person: Uuid) -> Result<()> {
    ensure!(
        data.access.employee_id != Some(person),
        "Another HR or Administrator must manage your appraisal."
    );
    Ok(())
}
impl PerformanceAction {
    pub fn reason_mut(&mut self) -> &mut String {
        match self {
            Self::GoalSave(d) => &mut d.reason,
            Self::CycleSave(d) => &mut d.reason,
            Self::Assign(d) => &mut d.reason,
            Self::Assessment(d) => &mut d.reason,
            Self::NoteSave(d) => &mut d.reason,
            Self::Progress { reason, .. }
            | Self::GoalStatus { reason, .. }
            | Self::CycleStatus { reason, .. }
            | Self::ReviewStatus { reason, .. } => reason,
        }
    }
    fn record(&self) -> Result<(&'static str, Value)> {
        Ok(match self {
            Self::GoalSave(d) => {
                text(&d.title, 120, true, false)?;
                text(&d.description, 3000, false, true)?;
                text(&d.measure, 500, false, true)?;
                let start = parse_date(&d.starts_on)?;
                let due = parse_date(&d.due_on)?;
                ensure!(due >= start, "Goal due date cannot precede its start.");
                (
                    "goal_save",
                    json!({"id":d.id,"expected_revision":d.original.as_ref().map(|g|g.revision),"employee_id":d.employee_id.context("Choose an employee.")?,"title":d.title.trim(),"description":d.description.trim(),"measure":d.measure.trim(),"starts_on":start,"due_on":due,"reason":d.reason.trim()}),
                )
            }
            Self::Progress {
                id,
                revision,
                progress,
                note,
                reason,
            } => {
                ensure!(*progress <= 100, "Use whole-number progress from 0 to 100.");
                text(note, 1000, false, true)?;
                (
                    "goal_progress",
                    json!({"id":id,"expected_revision":revision,"progress":progress,"progress_note":note.trim(),"reason":reason.trim()}),
                )
            }
            Self::GoalStatus {
                id,
                revision,
                status,
                reason,
            } => (
                "goal_status",
                json!({"id":id,"expected_revision":revision,"status":status,"reason":reason.trim()}),
            ),
            Self::CycleSave(d) => {
                text(&d.name, 120, true, false)?;
                let start = parse_date(&d.starts_on)?;
                let end = parse_date(&d.ends_on)?;
                let due = parse_date(&d.due_on)?;
                ensure!(
                    end >= start && due >= end,
                    "Use an ordered period and a review deadline on/after its end."
                );
                (
                    "cycle_save",
                    json!({"id":d.id,"expected_revision":d.original.as_ref().map(|c|c.revision),"name":d.name.trim(),"starts_on":start,"ends_on":end,"due_on":due,"reason":d.reason.trim()}),
                )
            }
            Self::CycleStatus {
                id,
                revision,
                status,
                reason,
            } => (
                "cycle_status",
                json!({"id":id,"expected_revision":revision,"status":status,"reason":reason.trim()}),
            ),
            Self::Assign(d) => (
                "review_assign",
                json!({"id":d.id,"expected_revision":d.original.as_ref().map(|r|r.revision),"cycle_id":d.cycle_id.context("Choose a review cycle.")?,"employee_id":d.employee_id.context("Choose an employee.")?,"reviewer_employee_id":d.reviewer_employee_id.context("Choose an eligible reviewer.")?,"reason":d.reason.trim()}),
            ),
            Self::Assessment(d) => {
                ensure!(
                    d.rating.is_none_or(|n| (1..=5).contains(&n)),
                    "Use a whole-number rating from 1 to 5."
                );
                text(&d.assessment, 4000, false, true)?;
                text(&d.strengths, 2000, false, true)?;
                text(&d.development, 2000, false, true)?;
                (
                    "review_save",
                    json!({"id":d.id,"expected_revision":d.revision,"rating":d.rating,"assessment":d.assessment.trim(),"strengths":d.strengths.trim(),"development":d.development.trim(),"reason":d.reason.trim()}),
                )
            }
            Self::ReviewStatus {
                id,
                revision,
                status,
                reason,
            } => (
                "review_status",
                json!({"id":id,"expected_revision":revision,"status":status,"reason":reason.trim()}),
            ),
            Self::NoteSave(d) => {
                text(&d.note, 2000, false, true)?;
                (
                    "note_save",
                    json!({"id":d.id,"expected_revision":d.original.as_ref().map(|n|n.revision),"review_id":d.review_id,"note":d.note.trim(),"reason":d.reason.trim()}),
                )
            }
        })
    }
    pub fn payload(&self, data: &Snapshot) -> Result<Value> {
        ensure!(
            data.mode != Mode::SignedOut
                && data.access.company_id == data.company.id
                && data.access.uses_desktop(),
            "Active desktop staff access is required."
        );
        let (op, p) = self.record()?;
        hiring::reason(p["reason"].as_str().unwrap_or_default())?;
        match self {
            Self::GoalSave(d) => {
                let person = d.employee_id.unwrap();
                ensure!(
                    manages_goal(data, person),
                    "Only HR or the current reporting Manager assigns goals."
                );
                let retry =
                    d.original.is_none() && data.performance.goals.iter().any(|g| g.id == d.id);
                if !retry {
                    active(data, person)?;
                    ensure!(
                        employee(data, person)?
                            .joined_on
                            .is_none_or(|start| parse_date(&d.starts_on).is_ok_and(|d| d >= start)),
                        "Goals cannot precede joining."
                    );
                }
                if let Some(old) = &d.original {
                    let g = goal(data, d.id)?;
                    ensure!(
                        g.revision == old.revision
                            && g.employee_id == person
                            && g.status == "active",
                        "This goal changed or is closed. Refresh and reopen it."
                    );
                }
            }
            Self::Progress { id, revision, .. } => {
                let g = goal(data, *id)?;
                ensure!(
                    manages_goal(data, g.employee_id)
                        || Some(g.employee_id) == data.access.employee_id,
                    "Your role cannot update this goal."
                );
                active(data, g.employee_id)?;
                ensure!(
                    g.revision == *revision && g.status == "active",
                    "This goal changed or is closed. Refresh."
                );
            }
            Self::GoalStatus {
                id,
                revision,
                status,
                ..
            } => {
                let g = goal(data, *id)?;
                ensure!(
                    manages_goal(data, g.employee_id),
                    "HR or the current reporting Manager manages goal status."
                );
                ensure!(
                    g.revision == *revision
                        && status != &g.status
                        && ["active", "completed", "cancelled"].contains(&status.as_str()),
                    "Choose an available goal status after Refresh."
                );
                ensure!(
                    status != "completed" || g.progress == 100,
                    "Record 100% progress before completing the goal."
                );
                if !(data.access.allows(crate::staff_roles::Capability::DevelopmentAdministration) && status == "cancelled") {
                    active(data, g.employee_id)?;
                }
            }
            Self::CycleSave(d) => {
                hr(data)?;
                if let Some(old) = &d.original {
                    let c = cycle(data, d.id)?;
                    ensure!(
                        c.revision == old.revision
                            && c.status == "open"
                            && !data.performance.reviews.iter().any(|r| r.cycle_id == c.id),
                        "Cycle labels/dates freeze when a review is assigned. Refresh."
                    );
                }
            }
            Self::CycleStatus {
                id,
                revision,
                status,
                ..
            } => {
                hr(data)?;
                let c = cycle(data, *id)?;
                ensure!(
                    c.revision == *revision
                        && status != &c.status
                        && ["open", "closed"].contains(&status.as_str()),
                    "Choose an available cycle status after Refresh."
                );
                ensure!(
                    status != "closed"
                        || !data.performance.reviews.iter().any(|r| r.cycle_id == *id
                            && ["draft", "submitted"].contains(&r.status.as_str())),
                    "Publish or cancel every unfinished appraisal first."
                );
            }
            Self::Assign(d) => {
                hr(data)?;
                let retry =
                    d.original.is_none() && data.performance.reviews.iter().any(|r| r.id == d.id);
                if !retry {
                    let person = d.employee_id.unwrap();
                    active(data, person)?;
                    independent(data, person)?;
                    let c = cycle(data, d.cycle_id.unwrap())?;
                    ensure!(
                        c.status == "open"
                            && employee(data, person)?
                                .joined_on
                                .is_none_or(|date| date <= c.ends_on),
                        "Choose an open period in which the employee was employed."
                    );
                    ensure!(
                        eligible_reviewer(data, person, d.reviewer_employee_id.unwrap()),
                        "Choose another active HR/Administrator or the employee's reporting Manager."
                    );
                    ensure!(
                        !data
                            .performance
                            .reviews
                            .iter()
                            .any(|r| r.id != d.id && r.employee_id == person && r.cycle_id == c.id),
                        "An appraisal already exists for this employee and cycle."
                    );
                    if let Some(old) = &d.original {
                        let r = review(data, d.id)?;
                        ensure!(
                            r.revision == old.revision
                                && r.status != "published"
                                && r.employee_id == person
                                && r.cycle_id == c.id
                                && Some(r.reviewer_employee_id) != d.reviewer_employee_id,
                            "Choose a new reviewer for the same unpublished appraisal."
                        );
                    }
                }
            }
            Self::Assessment(d) => {
                let r = review(data, d.id)?;
                ensure!(
                    Some(r.reviewer_employee_id) == data.access.employee_id && can_assess(data, r),
                    "Only the currently assigned eligible reviewer can assess this employee."
                );
                ensure!(
                    r.revision == d.revision
                        && r.status == "draft"
                        && cycle(data, r.cycle_id)?.status == "open",
                    "This appraisal changed or is no longer a draft. Refresh."
                );
                active(data, r.employee_id)?;
            }
            Self::ReviewStatus {
                id,
                revision,
                status,
                ..
            } => {
                let r = review(data, *id)?;
                ensure!(
                    r.revision == *revision
                        && cycle(data, r.cycle_id)?.status == "open"
                        && ((r.status == "draft"
                            && ["submitted", "cancelled"].contains(&status.as_str()))
                            || (r.status == "submitted"
                                && ["draft", "published", "cancelled"].contains(&status.as_str()))
                            || (r.status == "cancelled" && status == "draft")),
                    "Choose an available appraisal transition after Refresh."
                );
                if status == "submitted" {
                    ensure!(
                        can_assess(data, r),
                        "Only the currently assigned reviewer submits this appraisal."
                    );
                    active(data, r.employee_id)?;
                    ensure!(
                        r.rating.is_some() && !r.assessment.trim().is_empty(),
                        "Record a rating and assessment before submitting."
                    );
                    ensure!(
                        saved_goals(data, r).len() <= 100,
                        "The review period contains more than 100 goals."
                    );
                } else {
                    hr(data)?;
                    independent(data, r.employee_id)?;
                    if status != "cancelled" {
                        active(data, r.employee_id)?;
                        ensure!(
                            reviewer_current(data, r),
                            "An active eligible reviewer is required. Reassign if needed."
                        );
                    }
                }
            }
            Self::NoteSave(d) => {
                ensure!(data.access.allows(crate::staff_roles::Capability::PrivateHrNotes), "HR notes are private to HR and Administrators.");
                let r = review(data, d.review_id)?;
                independent(data, r.employee_id)?;
                if let Some(old) = &d.original {
                    let n = data
                        .performance
                        .notes
                        .iter()
                        .find(|n| n.id == d.id && n.company_id == data.company.id)
                        .context("Private note not found. Refresh.")?;
                    ensure!(
                        n.revision == old.revision && n.review_id == d.review_id,
                        "This private note changed. Refresh."
                    );
                } else {
                    ensure!(
                        !data
                            .performance
                            .notes
                            .iter()
                            .any(|n| n.review_id == d.review_id && n.id != d.id),
                        "A private note already exists. Refresh and reopen it."
                    );
                }
            }
        }
        Ok(json!({"p_operation":op,"p_record":p}))
    }
    pub fn saved(&self, data: &Snapshot) -> bool {
        let Ok((op, p)) = self.record() else {
            return false;
        };
        let id = p["id"].as_str().and_then(|s| s.parse::<Uuid>().ok());
        let row = if op.starts_with("goal_") {
            data.performance
                .goals
                .iter()
                .find(|g| Some(g.id) == id)
                .and_then(|g| serde_json::to_value(g).ok())
        } else if op.starts_with("cycle_") {
            data.performance
                .cycles
                .iter()
                .find(|c| Some(c.id) == id)
                .and_then(|c| serde_json::to_value(c).ok())
        } else if op == "note_save" {
            data.performance
                .notes
                .iter()
                .find(|n| Some(n.id) == id)
                .and_then(|n| serde_json::to_value(n).ok())
        } else {
            data.performance
                .reviews
                .iter()
                .find(|r| Some(r.id) == id)
                .and_then(|r| serde_json::to_value(r).ok())
        };
        row.is_some_and(|r| {
            r["company_id"] == json!(data.company.id)
                && r["revision"] == json!(p["expected_revision"].as_i64().map_or(0, |n| n + 1))
                && p.as_object().unwrap().iter().all(|(key, v)| {
                    ["expected_revision", "reason"].contains(&key.as_str()) || r.get(key) == Some(v)
                })
        })
    }
}
pub fn can_assess(data: &Snapshot, r: &Review) -> bool {
    Some(r.reviewer_employee_id) == data.access.employee_id
        && r.reviewer_employee_id != r.employee_id
        && (data.access.allows(crate::staff_roles::Capability::DevelopmentAdministration)
            || (data.access.allows(crate::staff_roles::Capability::TeamReview) && manages_goal(data, r.employee_id)))
}
fn reviewer_current(data: &Snapshot, r: &Review) -> bool {
    if data.access.allows(crate::staff_roles::Capability::DevelopmentAdministration) {
        eligible_reviewer(data, r.employee_id, r.reviewer_employee_id)
    } else {
        can_assess(data, r)
    }
}
pub fn saved_goals(data: &Snapshot, r: &Review) -> Vec<GoalSnapshot> {
    let mut rows: Vec<_> = data
        .performance
        .goals
        .iter()
        .filter(|g| {
            g.company_id == data.company.id
                && g.employee_id == r.employee_id
                && g.status != "cancelled"
                && g.starts_on <= r.ends_on
                && g.due_on >= r.starts_on
        })
        .map(|g| GoalSnapshot {
            id: g.id,
            title: g.title.clone(),
            measure: g.measure.clone(),
            starts_on: g.starts_on,
            due_on: g.due_on,
            progress: g.progress,
            status: g.status.clone(),
            revision: g.revision,
        })
        .collect();
    rows.sort_by_key(|g| (g.due_on, g.id));
    rows
}
pub fn unfinished_reviews(data: &Snapshot, person: Uuid) -> u64 {
    data.performance
        .reviews
        .iter()
        .filter(|r| {
            r.company_id == data.company.id
                && ["draft", "submitted"].contains(&r.status.as_str())
                && (r.employee_id == person || r.reviewer_employee_id == person)
        })
        .count() as u64
}

pub fn apply_demo(
    data: &mut Snapshot,
    action: &PerformanceAction,
    now: DateTime<Utc>,
) -> Result<()> {
    let payload = action.payload(data)?;
    let p = &payload["p_record"];
    let id: Uuid = serde_json::from_value(p["id"].clone())?;
    let birth = match action {
        PerformanceAction::GoalSave(d) if d.original.is_none() => Some("goal"),
        PerformanceAction::CycleSave(d) if d.original.is_none() => Some("cycle"),
        PerformanceAction::Assign(d) if d.original.is_none() => Some("review"),
        PerformanceAction::NoteSave(d) if d.original.is_none() => Some("note"),
        _ => None,
    };
    let birth_key = birth.map(|kind| format!("{kind}:{id}"));
    if let Some(key) = &birth_key {
        if let Some((actor, old)) = data.performance.retries.get(key) {
            ensure!(
                *actor == data.access.user_id && old == &payload,
                "This record ID already exists. Refresh."
            );
            return Ok(());
        }
        ensure!(
            !match birth.unwrap() {
                "goal" => data.performance.goals.iter().any(|r| r.id == id),
                "cycle" => data.performance.cycles.iter().any(|r| r.id == id),
                "review" => data.performance.reviews.iter().any(|r| r.id == id),
                _ => data.performance.notes.iter().any(|r| r.id == id),
            },
            "This record ID already exists."
        );
    }
    let mut next = data.clone();
    let before;
    let after;
    let kind;
    let mut goal_id = None;
    let mut cycle_id = None;
    let mut review_id = None;
    let mut note_id = None;
    match action {
        PerformanceAction::GoalSave(d) => {
            let old = next.performance.goals.iter().find(|g| g.id == id).cloned();
            let e = employee(data, d.employee_id.unwrap())?;
            let g = Goal {
                id,
                company_id: data.company.id,
                employee_id: e.id,
                employee_name: old
                    .as_ref()
                    .map_or(e.full_name.clone(), |g| g.employee_name.clone()),
                employee_code: old
                    .as_ref()
                    .map_or(e.employee_code.clone(), |g| g.employee_code.clone()),
                title: d.title.trim().into(),
                description: d.description.trim().into(),
                measure: d.measure.trim().into(),
                starts_on: parse_date(&d.starts_on)?,
                due_on: parse_date(&d.due_on)?,
                progress: old.as_ref().map_or(0, |g| g.progress),
                progress_note: old
                    .as_ref()
                    .map_or(String::new(), |g| g.progress_note.clone()),
                status: "active".into(),
                revision: old.as_ref().map_or(0, |g| g.revision + 1),
                created_by: old.as_ref().map_or(data.access.user_id, |g| g.created_by),
                created_at: old.as_ref().map_or(now, |g| g.created_at),
                updated_at: now,
            };
            before = old.as_ref().map(serde_json::to_value).transpose()?;
            after = serde_json::to_value(&g)?;
            kind = if old.is_none() {
                "goal_created".into()
            } else {
                "goal_edited".into()
            };
            goal_id = Some(id);
            next.performance.goals.retain(|g| g.id != id);
            next.performance.goals.push(g);
        }
        PerformanceAction::Progress { progress, note, .. } => {
            let g = next
                .performance
                .goals
                .iter_mut()
                .find(|g| g.id == id)
                .context("Goal not found.")?;
            before = Some(serde_json::to_value(&*g)?);
            g.progress = *progress;
            g.progress_note = note.trim().into();
            g.revision += 1;
            g.updated_at = now;
            after = serde_json::to_value(&*g)?;
            goal_id = Some(id);
            kind = "goal_progress".into();
        }
        PerformanceAction::GoalStatus { status, .. } => {
            let g = next
                .performance
                .goals
                .iter_mut()
                .find(|g| g.id == id)
                .context("Goal not found.")?;
            before = Some(serde_json::to_value(&*g)?);
            g.status = status.clone();
            g.revision += 1;
            g.updated_at = now;
            after = serde_json::to_value(&*g)?;
            goal_id = Some(id);
            kind = format!("goal_{status}");
        }
        PerformanceAction::CycleSave(d) => {
            let old = next.performance.cycles.iter().find(|c| c.id == id).cloned();
            let c = Cycle {
                id,
                company_id: data.company.id,
                name: d.name.trim().into(),
                starts_on: parse_date(&d.starts_on)?,
                ends_on: parse_date(&d.ends_on)?,
                due_on: parse_date(&d.due_on)?,
                status: "open".into(),
                revision: old.as_ref().map_or(0, |c| c.revision + 1),
                created_by: old.as_ref().map_or(data.access.user_id, |c| c.created_by),
                created_at: old.as_ref().map_or(now, |c| c.created_at),
                updated_at: now,
            };
            before = old.as_ref().map(serde_json::to_value).transpose()?;
            after = serde_json::to_value(&c)?;
            kind = if old.is_none() {
                "cycle_created".into()
            } else {
                "cycle_edited".into()
            };
            cycle_id = Some(id);
            next.performance.cycles.retain(|c| c.id != id);
            next.performance.cycles.push(c);
        }
        PerformanceAction::CycleStatus { status, .. } => {
            let c = next
                .performance
                .cycles
                .iter_mut()
                .find(|c| c.id == id)
                .context("Cycle not found.")?;
            before = Some(serde_json::to_value(&*c)?);
            c.status = status.clone();
            c.revision += 1;
            c.updated_at = now;
            after = serde_json::to_value(&*c)?;
            cycle_id = Some(id);
            kind = format!("cycle_{status}");
        }
        PerformanceAction::Assign(d) => {
            let old = next
                .performance
                .reviews
                .iter()
                .find(|r| r.id == id)
                .cloned();
            let e = employee(data, d.employee_id.unwrap())?;
            let c = cycle(data, d.cycle_id.unwrap())?;
            let reviewer = d.reviewer_employee_id.unwrap();
            let name = data
                .recruiting
                .interviewers
                .iter()
                .find(|s| s.employee_id == reviewer)
                .context("Reviewer unavailable.")?
                .full_name
                .clone();
            let r = Review {
                id,
                company_id: data.company.id,
                cycle_id: c.id,
                employee_id: e.id,
                employee_name: old
                    .as_ref()
                    .map_or(e.full_name.clone(), |r| r.employee_name.clone()),
                employee_code: old
                    .as_ref()
                    .map_or(e.employee_code.clone(), |r| r.employee_code.clone()),
                cycle_name: old
                    .as_ref()
                    .map_or(c.name.clone(), |r| r.cycle_name.clone()),
                starts_on: old.as_ref().map_or(c.starts_on, |r| r.starts_on),
                ends_on: old.as_ref().map_or(c.ends_on, |r| r.ends_on),
                due_on: old.as_ref().map_or(c.due_on, |r| r.due_on),
                reviewer_employee_id: reviewer,
                reviewer_name: name,
                rating: None,
                assessment: String::new(),
                strengths: String::new(),
                development: String::new(),
                goals_snapshot: Vec::new(),
                status: "draft".into(),
                submitted_at: None,
                published_at: None,
                published_by: None,
                revision: old.as_ref().map_or(0, |r| r.revision + 1),
                created_by: old.as_ref().map_or(data.access.user_id, |r| r.created_by),
                created_at: old.as_ref().map_or(now, |r| r.created_at),
                updated_at: now,
            };
            before = old.as_ref().map(serde_json::to_value).transpose()?;
            after = serde_json::to_value(&r)?;
            kind = if old.is_none() {
                "review_assigned".into()
            } else {
                "review_reassigned".into()
            };
            cycle_id = Some(c.id);
            review_id = Some(id);
            next.performance.reviews.retain(|r| r.id != id);
            next.performance.reviews.push(r);
        }
        PerformanceAction::Assessment(d) => {
            let r = next
                .performance
                .reviews
                .iter_mut()
                .find(|r| r.id == id)
                .context("Review not found.")?;
            before = Some(serde_json::to_value(&*r)?);
            r.rating = d.rating;
            r.assessment = d.assessment.trim().into();
            r.strengths = d.strengths.trim().into();
            r.development = d.development.trim().into();
            r.revision += 1;
            r.updated_at = now;
            after = serde_json::to_value(&*r)?;
            review_id = Some(id);
            cycle_id = Some(r.cycle_id);
            kind = "review_saved".into();
        }
        PerformanceAction::ReviewStatus { status, .. } => {
            let saved = review(data, id)?;
            let goals = if status == "submitted" {
                Some(saved_goals(data, saved))
            } else {
                None
            };
            let r = next
                .performance
                .reviews
                .iter_mut()
                .find(|r| r.id == id)
                .context("Review not found.")?;
            before = Some(serde_json::to_value(&*r)?);
            r.status = status.clone();
            if let Some(goals) = goals {
                r.goals_snapshot = goals;
                r.submitted_at = Some(now);
            }
            if status == "published" {
                r.published_at = Some(now);
                r.published_by = Some(data.access.user_id);
            }
            r.revision += 1;
            r.updated_at = now;
            after = serde_json::to_value(&*r)?;
            review_id = Some(id);
            cycle_id = Some(r.cycle_id);
            kind = if status == "published" {
                "published".into()
            } else {
                format!("review_{status}")
            };
        }
        PerformanceAction::NoteSave(d) => {
            let old = next.performance.notes.iter().find(|n| n.id == id).cloned();
            let n = Note {
                id,
                company_id: data.company.id,
                review_id: d.review_id,
                note: d.note.trim().into(),
                revision: old.as_ref().map_or(0, |n| n.revision + 1),
                created_by: old.as_ref().map_or(data.access.user_id, |n| n.created_by),
                created_at: old.as_ref().map_or(now, |n| n.created_at),
                updated_at: now,
            };
            before = old.as_ref().map(serde_json::to_value).transpose()?;
            after = serde_json::to_value(&n)?;
            kind = if old.is_none() {
                "note_created".into()
            } else {
                "note_edited".into()
            };
            review_id = Some(d.review_id);
            cycle_id = Some(review(data, d.review_id)?.cycle_id);
            note_id = Some(id);
            next.performance.notes.retain(|n| n.id != id);
            next.performance.notes.push(n);
        }
    }
    next.performance.events.push(Event {
        id: Uuid::new_v4(),
        company_id: data.company.id,
        goal_id,
        cycle_id,
        review_id,
        note_id,
        kind,
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
    });
    if let Some(key) = birth_key {
        next.performance
            .retries
            .insert(key, (data.access.user_id, payload));
    }
    *data = next;
    Ok(())
}

/// Mirror protected event projection for demo/paint paths as well as RLS-loaded data.
pub fn visible_events(data: &Snapshot) -> Vec<&Event> {
    data.performance
        .events
        .iter()
        .filter(|e| {
            if e.company_id != data.company.id {
                return false;
            }
            if data.access.allows(crate::staff_roles::Capability::PrivateHrNotes) {
                return true;
            }
            if e.note_id.is_some() {
                return false;
            }
            if data.access.allows(crate::staff_roles::Capability::DevelopmentAdministration) { return true; }
            if let Some(id) = e.goal_id {
                return goal(data, id).is_ok();
            }
            e.review_id
                .and_then(|id| review(data, id).ok())
                .is_some_and(|r| {
                    e.actor_id == data.access.user_id
                        || (e.kind == "published" && Some(r.employee_id) == data.access.employee_id)
                })
        })
        .collect()
}
