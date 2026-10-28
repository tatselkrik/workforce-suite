//! Confidential employee-relations context; deliberately outside the shared Snapshot.
use crate::{access::Access, staff_roles::Capability};
use anyhow::{Result, ensure};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Open,
    InReview,
    AwaitingFollowUp,
    Resolved,
    Closed,
}
impl Status {
    pub const ALL: [Self; 5] = [
        Self::Open,
        Self::InReview,
        Self::AwaitingFollowUp,
        Self::Resolved,
        Self::Closed,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Open => "Open",
            Self::InReview => "In review",
            Self::AwaitingFollowUp => "Awaiting follow-up",
            Self::Resolved => "Resolved",
            Self::Closed => "Closed",
        }
    }
    pub fn permits(self, next: Self, admin: bool) -> bool {
        match self {
            Self::Open => matches!(
                next,
                Self::InReview | Self::AwaitingFollowUp | Self::Resolved
            ),
            Self::InReview => matches!(next, Self::AwaitingFollowUp | Self::Resolved),
            Self::AwaitingFollowUp => matches!(next, Self::InReview | Self::Resolved),
            Self::Resolved => matches!(next, Self::InReview | Self::Closed),
            Self::Closed => next == Self::InReview && admin,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    WorkplaceConcern,
    Grievance,
    Conduct,
    Other,
}
impl Category {
    pub const ALL: [Self; 4] = [
        Self::WorkplaceConcern,
        Self::Grievance,
        Self::Conduct,
        Self::Other,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::WorkplaceConcern => "Workplace concern",
            Self::Grievance => "Grievance",
            Self::Conduct => "Conduct",
            Self::Other => "Other",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    Normal,
    High,
}
impl Priority {
    pub fn label(self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::High => "High",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub id: Uuid,
    pub company_id: Uuid,
    pub employee_id: Uuid,
    pub owner_id: Uuid,
    pub code: String,
    pub title: String,
    pub category: Category,
    pub priority: Priority,
    pub status: Status,
    pub revision: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FollowUp {
    pub id: Uuid,
    pub case_id: Uuid,
    pub company_id: Uuid,
    pub task: String,
    pub due_on: NaiveDate,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub completed_by: Option<Uuid>,
    pub completed_at: Option<DateTime<Utc>>,
    pub completion_note: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub id: Uuid,
    pub company_id: Uuid,
    pub case_id: Uuid,
    pub actor_id: Uuid,
    pub actor_name: String,
    pub revision: i64,
    pub kind: String,
    pub body: String,
    pub previous_state: Option<Case>,
    pub current_state: Case,
    pub created_at: DateTime<Utc>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Handler {
    pub user_id: Uuid,
    pub employee_id: Uuid,
    pub full_name: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Context {
    pub schema_version: u16,
    pub company_id: Uuid,
    pub user_id: Uuid,
    pub can_assign: bool,
    pub cases: Vec<Case>,
    pub follow_ups: Vec<FollowUp>,
    pub events: Vec<Event>,
    pub handlers: Vec<Handler>,
}
fn text(v: &str, min: usize, max: usize) -> bool {
    let n = v.chars().count();
    v.trim() == v
        && n >= min
        && n <= max
        && !v
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
}
pub fn eligible(access: &Access) -> bool {
    access.uses_desktop() && access.allows(Capability::Personnel)
}
impl Context {
    pub fn validate(&self, a: &Access) -> Result<()> {
        ensure!(
            eligible(a)
                && self.schema_version == 1
                && self.company_id == a.company_id
                && self.user_id == a.user_id
                && self.can_assign == a.allows(Capability::AccountAdministration),
            "The confidential case context no longer matches current access."
        );
        ensure!(
            self.cases.len() <= 1000
                && self.events.len() <= 10000
                && self.follow_ups.len() <= 5000
                && self.handlers.len() <= 2000,
            "Case context exceeds its supported budget."
        );
        ensure!(
            serde_json::to_vec(self)?.len() <= 8 * 1024 * 1024,
            "Confidential context exceeds its byte budget."
        );
        let ids: BTreeSet<_> = self.cases.iter().map(|c| c.id).collect();
        ensure!(ids.len() == self.cases.len(), "Duplicate case identities.");
        let handlers: BTreeSet<_> = self.handlers.iter().map(|h| h.user_id).collect();
        ensure!(
            handlers.len() == self.handlers.len()
                && self.handlers.iter().all(|h| text(&h.full_name, 1, 200)),
            "Invalid case handlers."
        );
        for c in &self.cases {
            ensure!(
                !c.id.is_nil()
                    && c.company_id == a.company_id
                    && c.revision > 0
                    && text(&c.title, 3, 160)
                    && c.code == format!("ER-{}", c.id.simple().to_string().to_uppercase())
                    && c.updated_at >= c.created_at
                    && (self.can_assign || c.owner_id == a.user_id),
                "Foreign, unassigned or invalid confidential case."
            );
        }
        let mut seen = BTreeSet::new();
        let mut revisions = BTreeMap::<Uuid, BTreeSet<i64>>::new();
        for e in &self.events {
            ensure!(
                seen.insert(e.id)
                    && ids.contains(&e.case_id)
                    && e.company_id == a.company_id
                    && e.revision > 0
                    && e.current_state.id == e.case_id
                    && e.current_state.company_id == a.company_id
                    && e.current_state.revision == e.revision
                    && text(&e.body, 3, 12000)
                    && text(&e.actor_name, 1, 200)
                    && matches!(
                        e.kind.as_str(),
                        "create" | "note" | "status" | "assign" | "follow_up" | "complete"
                    ),
                "Invalid confidential case event."
            );
            ensure!(
                revisions.entry(e.case_id).or_default().insert(e.revision),
                "Duplicate case revision."
            );
            ensure!(
                match &e.previous_state {
                    None => e.revision == 1 && e.kind == "create",
                    Some(old) =>
                        old.id == e.case_id
                            && old.company_id == a.company_id
                            && old.revision + 1 == e.revision,
                },
                "Case event history is incomplete."
            );
        }
        let chain: BTreeMap<_, _> = self
            .events
            .iter()
            .map(|e| ((e.case_id, e.revision), e))
            .collect();
        for e in &self.events {
            if e.revision > 1 {
                ensure!(
                    chain
                        .get(&(e.case_id, e.revision - 1))
                        .is_some_and(
                            |prior| e.previous_state.as_ref() == Some(&prior.current_state)
                        ),
                    "Case event history is discontinuous."
                );
            }
        }
        for c in &self.cases {
            let rs = revisions.get(&c.id);
            ensure!(
                rs.is_some_and(|rs| rs.len() as i64 == c.revision
                    && rs.first() == Some(&1)
                    && rs.last() == Some(&c.revision)),
                "Case history must not be truncated."
            );
            let last = self
                .events
                .iter()
                .find(|e| e.case_id == c.id && e.revision == c.revision);
            ensure!(
                last.is_some_and(|e| e.current_state == *c),
                "Case source and history disagree."
            );
        }
        let mut follow = BTreeSet::new();
        for f in &self.follow_ups {
            ensure!(
                follow.insert(f.id)
                    && !f.id.is_nil()
                    && ids.contains(&f.case_id)
                    && f.company_id == a.company_id
                    && text(&f.task, 3, 2000)
                    && f.completed_at.is_some() == f.completed_by.is_some()
                    && f.completed_at.is_some() == f.completion_note.is_some()
                    && f.completion_note.as_ref().is_none_or(|s| text(s, 3, 12000)),
                "Invalid case follow-up."
            );
        }
        Ok(())
    }
    pub fn case(&self, id: Uuid) -> Result<&Case> {
        self.cases
            .iter()
            .find(|c| c.id == id)
            .ok_or_else(|| anyhow::anyhow!("This case is no longer in your assigned scope."))
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Change {
    Create {
        employee_id: Uuid,
        owner_id: Uuid,
        title: String,
        category: Category,
        priority: Priority,
        body: String,
    },
    Note {
        body: String,
    },
    Status {
        status: Status,
        body: String,
    },
    Assign {
        owner_id: Uuid,
        body: String,
    },
    FollowUp {
        follow_up_id: Uuid,
        due_on: NaiveDate,
        body: String,
    },
    Complete {
        follow_up_id: Uuid,
        body: String,
    },
}
impl Change {
    pub fn body(&self) -> &str {
        match self {
            Self::Create { body, .. }
            | Self::Note { body }
            | Self::Status { body, .. }
            | Self::Assign { body, .. }
            | Self::FollowUp { body, .. }
            | Self::Complete { body, .. } => body,
        }
    }
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Create { .. } => "create",
            Self::Note { .. } => "note",
            Self::Status { .. } => "status",
            Self::Assign { .. } => "assign",
            Self::FollowUp { .. } => "follow_up",
            Self::Complete { .. } => "complete",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub id: Uuid,
    pub company_id: Uuid,
    pub actor_id: Uuid,
    pub case_id: Uuid,
    pub expected_revision: i64,
    pub change: Change,
}
impl Request {
    pub fn new(a: &Access, c: &Context, case_id: Uuid, change: Change) -> Result<Self> {
        c.validate(a)?;
        let expected_revision = if matches!(change, Change::Create { .. }) {
            0
        } else {
            c.case(case_id)?.revision
        };
        let r = Self {
            id: Uuid::new_v4(),
            company_id: a.company_id,
            actor_id: a.user_id,
            case_id,
            expected_revision,
            change,
        };
        r.validate(c, a)?;
        Ok(r)
    }
    pub fn binding(&self, a: &Access) -> Result<()> {
        ensure!(
            serde_json::to_vec(self)?.len() <= 24000,
            "Case note exceeds this request byte budget."
        );
        ensure!(
            eligible(a)
                && self.company_id == a.company_id
                && self.actor_id == a.user_id
                && !self.id.is_nil()
                && !self.case_id.is_nil()
                && self.expected_revision >= 0
                && text(self.change.body(), 3, 12000),
            "Review the case identity, access and confidential note."
        );
        Ok(())
    }
    pub fn validate(&self, c: &Context, a: &Access) -> Result<()> {
        self.binding(a)?;
        c.validate(a)?;
        if let Change::Create {
            employee_id,
            owner_id,
            title,
            ..
        } = &self.change
        {
            ensure!(
                !employee_id.is_nil()
                    && text(title, 3, 160)
                    && self.expected_revision == 0
                    && !c.cases.iter().any(|x| x.id == self.case_id)
                    && c.handlers.iter().any(|h| h.user_id == *owner_id)
                    && (c.can_assign || *owner_id == a.user_id),
                "Review the employee, case title and current HR handler."
            );
            return Ok(());
        }
        let source = c.case(self.case_id)?;
        ensure!(
            source.revision == self.expected_revision,
            "The case changed. Refresh and review before saving."
        );
        ensure!(
            source.status != Status::Closed
                || matches!(
                    self.change,
                    Change::Status {
                        status: Status::InReview,
                        ..
                    }
                ) && c.can_assign,
            "Only an Administrator may reopen a closed case."
        );
        match &self.change {
            Change::Status { status, .. } => {
                ensure!(
                    source.status.permits(*status, c.can_assign),
                    "Choose a supported case transition."
                );
                ensure!(
                    !matches!(status, Status::Resolved | Status::Closed)
                        || !c
                            .follow_ups
                            .iter()
                            .any(|f| f.case_id == source.id && f.completed_at.is_none()),
                    "Complete the pending follow-ups first."
                );
            }
            Change::Assign { owner_id, .. } => ensure!(
                c.can_assign
                    && *owner_id != source.owner_id
                    && c.handlers.iter().any(|h| h.user_id == *owner_id),
                "Only an Administrator may select another active HR handler."
            ),
            Change::FollowUp {
                follow_up_id,
                due_on,
                body,
            } => ensure!(
                source.status != Status::Resolved
                    && !follow_up_id.is_nil()
                    && !c.follow_ups.iter().any(|f| f.id == *follow_up_id)
                    && text(body, 3, 2000)
                    && due_on.year_ce().1 >= 2000
                    && due_on.year_ce().1 <= 2100,
                "Review the task and due date, reopening a resolved case first."
            ),
            Change::Complete { follow_up_id, .. } => ensure!(
                c.follow_ups.iter().any(|f| f.id == *follow_up_id
                    && f.case_id == source.id
                    && f.completed_at.is_none()),
                "Choose a pending follow-up on this case."
            ),
            _ => {}
        };
        Ok(())
    }
}
use chrono::Datelike;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub schema_version: u16,
    pub id: Uuid,
    pub company_id: Uuid,
    pub actor_id: Uuid,
    pub case_id: Uuid,
    pub revision: i64,
    pub applied_at: DateTime<Utc>,
}
impl Receipt {
    pub fn validate(&self, r: &Request) -> Result<()> {
        ensure!(
            self.schema_version == 1
                && self.id == r.id
                && self.company_id == r.company_id
                && self.actor_id == r.actor_id
                && self.case_id == r.case_id
                && self.revision == r.expected_revision.checked_add(1).unwrap_or(-1),
            "Case receipt does not match this exact request."
        );
        Ok(())
    }
}
#[derive(Clone, Debug)]
pub enum Operation {
    Refresh,
    Save(Box<Request>),
}
#[derive(Clone, Debug)]
pub struct Response {
    pub context: Context,
    pub receipt: Option<Receipt>,
}
#[derive(Debug)]
pub struct Rejected;
impl std::fmt::Display for Rejected {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Nothing was saved. Refresh the case, then review or cancel the draft before a new change.")
    }
}
impl std::error::Error for Rejected {}

#[derive(Default)]
pub struct Demo {
    context: Option<Context>,
    receipts: BTreeMap<Uuid, (Request, Receipt)>,
}
impl Demo {
    pub fn execute(&mut self, op: Operation, data: &crate::domain::Snapshot) -> Result<Response> {
        let a = &data.access;
        ensure!(eligible(a), "Current HR case access is required.");
        let mut c = self.context.clone().unwrap_or(Context {
            schema_version: 1,
            company_id: a.company_id,
            user_id: a.user_id,
            can_assign: a.allows(Capability::AccountAdministration),
            cases: vec![],
            events: vec![],
            follow_ups: vec![],
            handlers: data
                .accounts
                .iter()
                .filter(|m| {
                    m.active
                        && ((m.user_id == a.user_id && eligible(a))
                            || (m.user_id != a.user_id && m.role.manages_hr()))
                })
                .filter_map(|m| {
                    data.employees
                        .iter()
                        .find(|e| Some(e.id) == m.employee_id)
                        .map(|e| Handler {
                            user_id: m.user_id,
                            employee_id: e.id,
                            full_name: e.full_name.clone(),
                        })
                })
                .collect(),
        });
        ensure!(
            c.user_id == a.user_id && c.company_id == a.company_id,
            "Demo case actor changed."
        );
        let mut receipt = None;
        if let Operation::Save(r) = op {
            r.binding(a)?;
            if let Some((prior, old)) = self.receipts.get(&r.id) {
                ensure!(
                    *prior == *r,
                    "Request identity already has another payload."
                );
                receipt = Some(old.clone());
            } else {
                r.validate(&c, a)?;
                let now = Utc::now();
                let before = c.cases.iter().find(|v| v.id == r.case_id).cloned();
                let mut case = if let Change::Create {
                    employee_id,
                    owner_id,
                    title,
                    category,
                    priority,
                    ..
                } = &r.change
                {
                    ensure!(
                        data.employees.iter().any(|e| e.id == *employee_id),
                        "Choose a company employee."
                    );
                    Case {
                        id: r.case_id,
                        company_id: a.company_id,
                        employee_id: *employee_id,
                        owner_id: *owner_id,
                        code: format!("ER-{}", r.case_id.simple().to_string().to_uppercase()),
                        title: title.clone(),
                        category: *category,
                        priority: *priority,
                        status: Status::Open,
                        revision: 0,
                        created_at: now,
                        updated_at: now,
                    }
                } else {
                    c.case(r.case_id)?.clone()
                };
                match &r.change {
                    Change::Status { status, .. } => case.status = *status,
                    Change::Assign { owner_id, .. } => case.owner_id = *owner_id,
                    Change::FollowUp {
                        follow_up_id,
                        due_on,
                        body,
                    } => c.follow_ups.push(FollowUp {
                        id: *follow_up_id,
                        case_id: case.id,
                        company_id: a.company_id,
                        task: body.clone(),
                        due_on: *due_on,
                        created_by: a.user_id,
                        created_at: now,
                        completed_by: None,
                        completed_at: None,
                        completion_note: None,
                    }),
                    Change::Complete { follow_up_id, body } => {
                        let f = c
                            .follow_ups
                            .iter_mut()
                            .find(|f| f.id == *follow_up_id)
                            .unwrap();
                        f.completed_by = Some(a.user_id);
                        f.completed_at = Some(now);
                        f.completion_note = Some(body.clone());
                    }
                    _ => {}
                }
                case.revision += 1;
                case.updated_at = now;
                c.cases.retain(|v| v.id != case.id);
                c.cases.push(case.clone());
                let rr = Receipt {
                    schema_version: 1,
                    id: r.id,
                    company_id: a.company_id,
                    actor_id: a.user_id,
                    case_id: case.id,
                    revision: case.revision,
                    applied_at: now,
                };
                c.events.push(Event {
                    id: r.id,
                    company_id: a.company_id,
                    case_id: case.id,
                    actor_id: a.user_id,
                    actor_name: data
                        .own_employee()
                        .map(|e| e.full_name.clone())
                        .unwrap_or_else(|| "Fictional HR".into()),
                    revision: case.revision,
                    kind: r.change.kind().into(),
                    body: r.change.body().into(),
                    previous_state: before,
                    current_state: case,
                    created_at: now,
                });
                self.receipts.insert(r.id, ((*r).clone(), rr.clone()));
                receipt = Some(rr);
            }
        }
        c.validate(a)?;
        self.context = Some(c.clone());
        Ok(Response {
            context: c,
            receipt,
        })
    }
}
