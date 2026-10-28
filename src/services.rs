//! Service requests are shared with their requester/authorized handlers, never case data.
use crate::{access::Access, staff_roles::Capability};
use anyhow::{Result, ensure};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Audience {
    Staff,
    Own,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Submitted,
    InProgress,
    WaitingOnEmployee,
    Completed,
    Cancelled,
}
impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Self::Submitted => "Submitted",
            Self::InProgress => "In progress",
            Self::WaitingOnEmployee => "Waiting on employee",
            Self::Completed => "Completed",
            Self::Cancelled => "Cancelled",
        }
    }
    pub fn active(self) -> bool {
        !matches!(self, Self::Completed | Self::Cancelled)
    }
    pub fn permits(self, next: Self, hr: bool) -> bool {
        match self {
            Self::Submitted => matches!(
                next,
                Self::InProgress | Self::WaitingOnEmployee | Self::Completed
            ),
            Self::InProgress => matches!(next, Self::WaitingOnEmployee | Self::Completed),
            Self::WaitingOnEmployee => matches!(next, Self::InProgress | Self::Completed),
            Self::Completed | Self::Cancelled => hr && next == Self::InProgress,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    EmploymentDocument,
    InformationUpdate,
    BenefitsQuestion,
    GeneralSupport,
}
impl Category {
    pub const ALL: [Self; 4] = [
        Self::EmploymentDocument,
        Self::InformationUpdate,
        Self::BenefitsQuestion,
        Self::GeneralSupport,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::EmploymentDocument => "Employment document",
            Self::InformationUpdate => "Employee information update",
            Self::BenefitsQuestion => "Benefits question",
            Self::GeneralSupport => "General HR support",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    Normal,
    High,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ticket {
    pub id: Uuid,
    pub company_id: Uuid,
    pub employee_id: Uuid,
    pub requester_code: String,
    pub requester_name: String,
    pub created_by: Uuid,
    pub assignee_user_id: Option<Uuid>,
    pub assignee_name: Option<String>,
    pub code: String,
    pub title: String,
    pub category: Category,
    pub priority: Priority,
    pub status: Status,
    pub due_on: Option<NaiveDate>,
    pub revision: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
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
pub struct Event {
    pub id: Uuid,
    pub company_id: Uuid,
    pub request_id: Uuid,
    pub actor_id: Uuid,
    pub actor_name: String,
    pub revision: i64,
    pub kind: String,
    pub body: String,
    pub previous_state: Option<Ticket>,
    pub current_state: Ticket,
    pub created_at: DateTime<Utc>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Context {
    pub schema_version: u16,
    pub company_id: Uuid,
    pub user_id: Uuid,
    pub employee_id: Uuid,
    pub audience: Audience,
    pub can_route: bool,
    pub requests: Vec<Ticket>,
    pub events: Vec<Event>,
    pub handlers: Vec<Handler>,
}
fn text(v: &str, min: usize, max: usize) -> bool {
    let n = v.chars().count();
    v == v.trim()
        && n >= min
        && n <= max
        && !v
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
}
pub fn admitted(a: &Access, scope: Audience) -> bool {
    !a.company_id.is_nil()
        && !a.user_id.is_nil()
        && a.employee_id.is_some_and(|id| !id.is_nil())
        && a.allows(Capability::OwnRecords)
        && (scope == Audience::Own || a.uses_desktop())
}
impl Context {
    pub fn hr(&self, a: &Access) -> bool {
        self.audience == Audience::Staff && a.allows(Capability::Personnel)
    }
    pub fn owns(&self, t: &Ticket) -> bool {
        t.employee_id == self.employee_id
    }
    pub fn handles(&self, t: &Ticket, a: &Access) -> bool {
        self.audience == Audience::Staff
            && (self.hr(a) || (a.uses_desktop() && t.assignee_user_id == Some(a.user_id)))
    }
    pub fn visible(&self, t: &Ticket, a: &Access) -> bool {
        t.company_id == self.company_id && (self.owns(t) || self.handles(t, a))
    }
    pub fn ticket(&self, id: Uuid) -> Result<&Ticket> {
        self.requests
            .iter()
            .find(|t| t.id == id)
            .ok_or_else(|| anyhow::anyhow!("This service request is outside the current scope."))
    }
    pub fn validate(&self, a: &Access) -> Result<()> {
        ensure!(
            self.schema_version == 1
                && admitted(a, self.audience)
                && self.company_id == a.company_id
                && self.user_id == a.user_id
                && Some(self.employee_id) == a.employee_id
                && self.can_route == self.hr(a),
            "The service context does not match current access."
        );
        ensure!(
            self.requests.len() <= 1000
                && self.events.len() <= 10000
                && self.handlers.len() <= 2000
                && serde_json::to_vec(self)?.len() <= 8 * 1024 * 1024,
            "Service history exceeds its supported budget."
        );
        ensure!(
            self.can_route || self.handlers.is_empty(),
            "A self/assigned view must not expose the staff routing directory."
        );
        let handlers: BTreeSet<_> = self.handlers.iter().map(|h| h.user_id).collect();
        ensure!(
            handlers.len() == self.handlers.len()
                && self.handlers.iter().all(|h| !h.user_id.is_nil()
                    && !h.employee_id.is_nil()
                    && text(&h.full_name, 1, 200)),
            "Invalid service handlers."
        );
        let ids: BTreeSet<_> = self.requests.iter().map(|t| t.id).collect();
        ensure!(
            ids.len() == self.requests.len(),
            "Duplicate service request identities."
        );
        for t in &self.requests {
            ensure!(
                !t.id.is_nil()
                    && !t.employee_id.is_nil()
                    && !t.created_by.is_nil()
                    && t.assignee_user_id.is_none_or(|id| !id.is_nil())
                    && t.assignee_user_id.is_some() == t.assignee_name.is_some()
                    && t.assignee_name
                        .as_deref()
                        .is_none_or(|name| text(name, 1, 200))
                    && self.visible(t, a)
                    && t.code == format!("SR-{}", t.id.simple().to_string().to_uppercase())
                    && text(&t.title, 3, 160)
                    && text(&t.requester_name, 1, 200)
                    && text(&t.requester_code, 1, 40)
                    && t.revision > 0
                    && t.updated_at >= t.created_at,
                "Foreign, unassigned or invalid service request."
            );
        }
        let mut events = BTreeMap::new();
        let mut event_ids = BTreeSet::new();
        for e in &self.events {
            ensure!(
                !e.id.is_nil()
                    && !e.actor_id.is_nil()
                    && ids.contains(&e.request_id)
                    && e.company_id == a.company_id
                    && event_ids.insert(e.id)
                    && e.revision > 0
                    && text(&e.body, 3, 12000)
                    && text(&e.actor_name, 1, 200)
                    && e.current_state.id == e.request_id
                    && e.current_state.company_id == a.company_id
                    && e.current_state.revision == e.revision
                    && matches!(
                        e.kind.as_str(),
                        "submit" | "reply" | "route" | "progress" | "cancel"
                    ),
                "Invalid shared service event."
            );
            ensure!(
                events.insert((e.request_id, e.revision), e).is_none(),
                "Duplicate service revision."
            );
        }
        for t in &self.requests {
            ensure!(
                events.range((t.id, 1)..=(t.id, i64::MAX)).count() as i64 == t.revision,
                "Service history is incomplete."
            );
            let mut previous = None;
            for rev in 1..=t.revision {
                let e = events
                    .get(&(t.id, rev))
                    .ok_or_else(|| anyhow::anyhow!("Service history was truncated."))?;
                ensure!(
                    e.previous_state.as_ref() == previous
                        && ((rev == 1 && e.kind == "submit") || (rev > 1 && e.kind != "submit"))
                        && e.current_state.employee_id == t.employee_id
                        && e.current_state.created_by == t.created_by
                        && e.current_state.requester_code == t.requester_code
                        && e.current_state.requester_name == t.requester_name
                        && e.current_state.code == t.code
                        && e.current_state.title == t.title
                        && e.current_state.category == t.category
                        && e.current_state.priority == t.priority
                        && e.current_state.created_at == t.created_at
                        && e.current_state.updated_at == e.created_at
                        && e.current_state
                            .assignee_user_id
                            .is_none_or(|id| !id.is_nil())
                        && e.current_state.assignee_user_id.is_some()
                            == e.current_state.assignee_name.is_some()
                        && e.current_state
                            .assignee_name
                            .as_deref()
                            .is_none_or(|name| text(name, 1, 200)),
                    "Service history is discontinuous."
                );
                previous = Some(&e.current_state);
            }
            ensure!(
                previous == Some(t),
                "Service source disagrees with history."
            );
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Change {
    Submit {
        employee_id: Uuid,
        title: String,
        category: Category,
        priority: Priority,
        body: String,
    },
    Reply {
        body: String,
    },
    Route {
        assignee_user_id: Option<Uuid>,
        due_on: Option<NaiveDate>,
        body: String,
    },
    Progress {
        status: Status,
        body: String,
    },
    Cancel {
        body: String,
    },
}
impl Change {
    pub fn body(&self) -> &str {
        match self {
            Self::Submit { body, .. }
            | Self::Reply { body }
            | Self::Route { body, .. }
            | Self::Progress { body, .. }
            | Self::Cancel { body } => body,
        }
    }
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Submit { .. } => "submit",
            Self::Reply { .. } => "reply",
            Self::Route { .. } => "route",
            Self::Progress { .. } => "progress",
            Self::Cancel { .. } => "cancel",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub id: Uuid,
    pub company_id: Uuid,
    pub actor_id: Uuid,
    pub request_id: Uuid,
    pub expected_revision: i64,
    pub change: Change,
}
impl Request {
    pub fn new(a: &Access, c: &Context, request_id: Uuid, change: Change) -> Result<Self> {
        c.validate(a)?;
        let expected_revision = if matches!(change, Change::Submit { .. }) {
            0
        } else {
            c.ticket(request_id)?.revision
        };
        let r = Self {
            id: Uuid::new_v4(),
            company_id: a.company_id,
            actor_id: a.user_id,
            request_id,
            expected_revision,
            change,
        };
        r.validate(c, a)?;
        Ok(r)
    }
    pub fn binding(&self, a: &Access) -> Result<()> {
        ensure!(
            !self.id.is_nil()
                && !self.request_id.is_nil()
                && self.company_id == a.company_id
                && self.actor_id == a.user_id
                && self.expected_revision >= 0
                && text(self.change.body(), 3, 12000)
                && serde_json::to_vec(self)?.len() <= 24000,
            "Review the service identity, actor and shared message."
        );
        Ok(())
    }
    pub fn validate(&self, c: &Context, a: &Access) -> Result<()> {
        self.binding(a)?;
        c.validate(a)?;
        if let Change::Submit {
            employee_id, title, ..
        } = &self.change
        {
            ensure!(
                !employee_id.is_nil()
                    && (c.hr(a) || *employee_id == c.employee_id)
                    && self.expected_revision == 0
                    && !c.requests.iter().any(|t| t.id == self.request_id)
                    && text(title, 3, 160),
                "Choose your own employee, or use authorized HR assistance."
            );
            return Ok(());
        }
        let t = c.ticket(self.request_id)?;
        ensure!(
            t.revision == self.expected_revision && c.visible(t, a),
            "The service request changed. Refresh and review it."
        );
        match &self.change {
            Change::Reply { .. } => ensure!(
                t.status.active() && (c.owns(t) || c.handles(t, a)),
                "Only the requester or current handling staff can reply to active work."
            ),
            Change::Route {
                assignee_user_id,
                due_on,
                ..
            } => {
                ensure!(
                    c.can_route
                        && t.status.active()
                        && assignee_user_id
                            .is_none_or(|id| c.handlers.iter().any(|h| h.user_id == id)),
                    "Only HR can route active work to a current staff handler."
                );
                ensure!(
                    *assignee_user_id != t.assignee_user_id || *due_on != t.due_on,
                    "Routing values did not change."
                );
                if let Some(day) = due_on {
                    ensure!(
                        *day >= NaiveDate::from_ymd_opt(2000, 1, 1).unwrap()
                            && *day <= NaiveDate::from_ymd_opt(2100, 12, 31).unwrap(),
                        "Use a supported due date."
                    );
                }
            }
            Change::Progress { status, .. } => ensure!(
                c.handles(t, a) && t.status.permits(*status, c.hr(a)),
                "The current handler may progress work; only HR can reopen terminal requests."
            ),
            Change::Cancel { .. } => ensure!(
                t.status.active() && (c.owns(t) || c.hr(a)),
                "Only the requester or HR can cancel active work."
            ),
            _ => {}
        };
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub schema_version: u16,
    pub id: Uuid,
    pub company_id: Uuid,
    pub actor_id: Uuid,
    pub request_id: Uuid,
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
                && self.request_id == r.request_id
                && self.revision == r.expected_revision.checked_add(1).unwrap_or(-1),
            "Service receipt does not match this exact request."
        );
        Ok(())
    }
}
/// Fictional source-build simulation only; never used for configured installations.
#[derive(Default)]
pub struct Demo {
    records: Vec<Ticket>,
    events: Vec<Event>,
    receipts: BTreeMap<Uuid, (Request, Receipt)>,
}
#[derive(Clone, Debug)]
pub struct Response {
    pub context: Context,
    pub receipt: Option<Receipt>,
}
impl Demo {
    pub fn context(&self, data: &crate::domain::Snapshot, audience: Audience) -> Result<Context> {
        let a = &data.access;
        ensure!(
            data.mode == crate::domain::Mode::Demo && admitted(a, audience),
            "The fictional service context needs current employee access."
        );
        let mut c = Context {
            schema_version: 1,
            company_id: a.company_id,
            user_id: a.user_id,
            employee_id: a.employee_id.unwrap(),
            audience,
            can_route: audience == Audience::Staff && a.allows(Capability::Personnel),
            requests: vec![],
            events: vec![],
            handlers: vec![],
        };
        c.requests = self
            .records
            .iter()
            .filter(|t| c.visible(t, a))
            .cloned()
            .collect();
        let ids: BTreeSet<_> = c.requests.iter().map(|t| t.id).collect();
        c.events = self
            .events
            .iter()
            .filter(|e| ids.contains(&e.request_id))
            .cloned()
            .collect();
        if c.can_route {
            c.handlers = data
                .accounts
                .iter()
                .filter(|m| {
                    m.active
                        && !m.user_id.is_nil()
                        && ((m.user_id == a.user_id && a.uses_desktop())
                            || (m.user_id != a.user_id && m.role.uses_desktop()))
                })
                .filter_map(|m| {
                    data.employees
                        .iter()
                        .find(|e| {
                            Some(e.id) == m.employee_id
                                && e.status == crate::domain::EmployeeStatus::Active
                        })
                        .map(|e| Handler {
                            user_id: m.user_id,
                            employee_id: e.id,
                            full_name: e.full_name.clone(),
                        })
                })
                .collect();
        }
        c.validate(a)?;
        Ok(c)
    }
    pub fn save(
        &mut self,
        data: &crate::domain::Snapshot,
        audience: Audience,
        r: &Request,
    ) -> Result<Response> {
        let a = &data.access;
        let c = self.context(data, audience)?;
        r.binding(a)?;
        if let Some((old, receipt)) = self.receipts.get(&r.id) {
            ensure!(
                old == r,
                "A service request identity already has another actor or payload."
            );
            c.ticket(r.request_id)?;
            receipt.validate(r)?;
            return Ok(Response {
                context: c,
                receipt: Some(receipt.clone()),
            });
        }
        r.validate(&c, a)?;
        let now = Utc::now();
        let before = self.records.iter().find(|t| t.id == r.request_id).cloned();
        let mut ticket = if let Change::Submit {
            employee_id,
            title,
            category,
            priority,
            ..
        } = &r.change
        {
            let e = data
                .employees
                .iter()
                .find(|e| e.id == *employee_id && e.company_id == a.company_id)
                .ok_or_else(|| anyhow::anyhow!("Choose a company employee."))?;
            Ticket {
                id: r.request_id,
                company_id: a.company_id,
                employee_id: *employee_id,
                requester_code: e.employee_code.clone(),
                requester_name: e.full_name.clone(),
                created_by: a.user_id,
                assignee_user_id: None,
                assignee_name: None,
                code: format!("SR-{}", r.request_id.simple().to_string().to_uppercase()),
                title: title.clone(),
                category: *category,
                priority: *priority,
                status: Status::Submitted,
                due_on: None,
                revision: 0,
                created_at: now,
                updated_at: now,
            }
        } else {
            c.ticket(r.request_id)?.clone()
        };
        match &r.change {
            Change::Route {
                assignee_user_id,
                due_on,
                ..
            } => {
                ticket.assignee_user_id = *assignee_user_id;
                ticket.assignee_name = assignee_user_id.and_then(|id| {
                    c.handlers
                        .iter()
                        .find(|h| h.user_id == id)
                        .map(|h| h.full_name.clone())
                });
                ticket.due_on = *due_on;
            }
            Change::Progress { status, .. } => ticket.status = *status,
            Change::Cancel { .. } => ticket.status = Status::Cancelled,
            _ => {}
        }
        ticket.revision += 1;
        ticket.updated_at = now;
        let event = Event {
            id: r.id,
            company_id: a.company_id,
            request_id: ticket.id,
            actor_id: a.user_id,
            actor_name: data
                .own_employee()
                .map(|e| e.full_name.clone())
                .unwrap_or_else(|| "Fictional staff".into()),
            revision: ticket.revision,
            kind: r.change.kind().into(),
            body: r.change.body().into(),
            previous_state: before,
            current_state: ticket.clone(),
            created_at: now,
        };
        let receipt = Receipt {
            schema_version: 1,
            id: r.id,
            company_id: a.company_id,
            actor_id: a.user_id,
            request_id: ticket.id,
            revision: ticket.revision,
            applied_at: now,
        };
        // Prepare/validate first so simulation errors cannot leave a partial state.
        let mut records = self.records.clone();
        records.retain(|t| t.id != ticket.id);
        records.push(ticket);
        let mut events = self.events.clone();
        events.push(event);
        let mut next = Self {
            records,
            events,
            receipts: self.receipts.clone(),
        };
        next.receipts.insert(r.id, (r.clone(), receipt.clone()));
        let context = next.context(data, audience)?;
        *self = next;
        Ok(Response {
            context,
            receipt: Some(receipt),
        })
    }
}

/// Browser DTO: only the requester-visible thread, never raw staff context or receipts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OwnView {
    pub schema_version: u16,
    pub requests: Vec<OwnTicket>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OwnTicket {
    pub id: Uuid,
    pub code: String,
    pub title: String,
    pub category: Category,
    pub priority: Priority,
    pub status: Status,
    pub handler_name: Option<String>,
    pub due_on: Option<NaiveDate>,
    pub revision: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub can_reply: bool,
    pub can_cancel: bool,
    pub messages: Vec<OwnMessage>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OwnMessage {
    pub id: Uuid,
    pub revision: i64,
    pub kind: String,
    pub actor_name: String,
    pub body: String,
    pub status: Status,
    pub created_at: DateTime<Utc>,
}
impl OwnView {
    pub fn from_context(c: &Context, a: &Access) -> Result<Self> {
        c.validate(a)?;
        ensure!(
            c.audience == Audience::Own,
            "The portal requires an own-only service context."
        );
        let mut requests = Vec::with_capacity(c.requests.len());
        for t in &c.requests {
            ensure!(
                c.owns(t),
                "A foreign service request cannot enter the portal."
            );
            let mut messages: Vec<_> = c
                .events
                .iter()
                .filter(|e| e.request_id == t.id)
                .map(|e| OwnMessage {
                    id: e.id,
                    revision: e.revision,
                    kind: e.kind.clone(),
                    actor_name: e.actor_name.clone(),
                    body: e.body.clone(),
                    status: e.current_state.status,
                    created_at: e.created_at,
                })
                .collect();
            messages.sort_by_key(|e| e.revision);
            requests.push(OwnTicket {
                id: t.id,
                code: t.code.clone(),
                title: t.title.clone(),
                category: t.category,
                priority: t.priority,
                status: t.status,
                handler_name: t.assignee_name.clone(),
                due_on: t.due_on,
                revision: t.revision,
                created_at: t.created_at,
                updated_at: t.updated_at,
                can_reply: t.status.active(),
                can_cancel: t.status.active(),
                messages,
            });
        }
        requests.sort_by_key(|t| (std::cmp::Reverse(t.updated_at), t.id));
        Ok(Self {
            schema_version: 1,
            requests,
        })
    }
}

#[derive(Clone, Debug)]
pub enum Operation {
    Refresh,
    Save(Box<Request>),
}
#[derive(Debug)]
pub struct Rejected;
impl std::fmt::Display for Rejected {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Nothing was saved. Refresh the service request, then review or cancel the draft before preparing a new change.")
    }
}
impl std::error::Error for Rejected {}
