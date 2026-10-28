//! HR-reviewed benefits records. No monetary or canonical-employment side effects.
use crate::{
    access::Access,
    domain::{Employee, EmployeeStatus, Snapshot},
    staff_roles::Capability,
};
use anyhow::{Result, ensure};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

pub fn eligible(a: &Access) -> bool {
    a.uses_desktop() && a.allows(Capability::Personnel)
}
fn text(s: &str, min: usize, max: usize) -> bool {
    s.trim() == s
        && (min..=max).contains(&s.chars().count())
        && !s
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
}
fn date(d: NaiveDate) -> bool {
    (1900..=2100).contains(&d.year())
}
fn interval(start: NaiveDate, end: Option<NaiveDate>) -> bool {
    date(start) && end.is_none_or(|end| date(end) && end >= start)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    HealthInsurance,
    Wellness,
    Retirement,
    Other,
}
impl Category {
    pub const ALL: [Self; 4] = [
        Self::HealthInsurance,
        Self::Wellness,
        Self::Retirement,
        Self::Other,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::HealthInsurance => "Health / insurance",
            Self::Wellness => "Wellness",
            Self::Retirement => "Retirement",
            Self::Other => "Other",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Eligibility {
    Pending,
    Eligible,
    Ineligible,
}
impl Eligibility {
    pub const ALL: [Self; 3] = [Self::Pending, Self::Eligible, Self::Ineligible];
    pub fn label(self) -> &'static str {
        match self {
            Self::Pending => "Pending review",
            Self::Eligible => "Reviewed eligible",
            Self::Ineligible => "Reviewed ineligible",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Pending,
    Active,
    Declined,
    Ended,
}
impl Status {
    pub const ALL: [Self; 4] = [Self::Pending, Self::Active, Self::Declined, Self::Ended];
    pub fn label(self) -> &'static str {
        match self {
            Self::Pending => "Pending",
            Self::Active => "Active",
            Self::Declined => "Declined",
            Self::Ended => "Ended",
        }
    }
    pub fn permits(self, next: Self) -> bool {
        match self {
            Self::Pending => matches!(next, Self::Pending | Self::Active | Self::Declined),
            Self::Active => matches!(next, Self::Active | Self::Ended),
            Self::Declined => matches!(next, Self::Pending | Self::Declined),
            Self::Ended => false,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Relationship {
    Spouse,
    Child,
    Parent,
    Sibling,
    Other,
}
impl Relationship {
    pub const ALL: [Self; 5] = [
        Self::Spouse,
        Self::Child,
        Self::Parent,
        Self::Sibling,
        Self::Other,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Spouse => "Spouse",
            Self::Child => "Child",
            Self::Parent => "Parent",
            Self::Sibling => "Sibling",
            Self::Other => "Other",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub code: String,
    pub name: String,
    pub category: Category,
    pub description: String,
    pub eligibility_guidance: String,
    pub dependent_limit: u8,
    pub available: bool,
}
impl Plan {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            (2..=32).contains(&self.code.len())
                && self.code.bytes().all(|c| c.is_ascii_uppercase()
                    || c.is_ascii_digit()
                    || c == b'-'
                    || c == b'_'),
            "Use a unique 2â€“32 character uppercase plan code."
        );
        ensure!(
            text(&self.name, 3, 160)
                && text(&self.description, 3, 2000)
                && text(&self.eligibility_guidance, 3, 2000)
                && self.dependent_limit <= 25,
            "Review plan details, eligibility guidance and dependent limit (0â€“25)."
        );
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmployeeSource {
    pub id: Uuid,
    pub company_id: Uuid,
    pub employee_code: String,
    pub full_name: String,
    pub department: String,
    pub job_title: String,
    pub joined_on: Option<NaiveDate>,
    pub employment_type_id: Option<Uuid>,
    pub work_location_id: Option<Uuid>,
    pub active: bool,
}
impl From<&Employee> for EmployeeSource {
    fn from(e: &Employee) -> Self {
        Self {
            id: e.id,
            company_id: e.company_id,
            employee_code: e.employee_code.clone(),
            full_name: e.full_name.clone(),
            department: e.department.clone(),
            job_title: e.job_title.clone(),
            joined_on: e.joined_on,
            employment_type_id: e.employment_type_id,
            work_location_id: e.work_location_id,
            active: e.status == EmployeeStatus::Active,
        }
    }
}
impl EmployeeSource {
    fn validate(&self, company: Uuid) -> Result<()> {
        ensure!(
            !self.id.is_nil()
                && self.company_id == company
                && text(&self.employee_code, 1, 32)
                && text(&self.full_name, 1, 200)
                && text(&self.department, 0, 100)
                && text(&self.job_title, 0, 100)
                && self.joined_on.is_none_or(date)
                && self.employment_type_id.is_none_or(|v| !v.is_nil())
                && self.work_location_id.is_none_or(|v| !v.is_nil()),
            "Invalid reviewed employment source."
        );
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dependent {
    pub id: Uuid,
    pub full_name: String,
    pub relationship: Relationship,
    pub birth_date: Option<NaiveDate>,
    pub coverage_from: NaiveDate,
    pub coverage_to: Option<NaiveDate>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Enrollment {
    pub plan_id: Uuid,
    pub plan_revision: i64,
    pub plan_snapshot: Plan,
    pub employee_source: EmployeeSource,
    pub eligibility: Eligibility,
    pub eligibility_basis: String,
    pub status: Status,
    pub coverage_from: NaiveDate,
    pub coverage_to: Option<NaiveDate>,
    pub dependents: Vec<Dependent>,
}
impl Enrollment {
    fn validate(&self, company: Uuid) -> Result<()> {
        self.plan_snapshot.validate()?;
        self.employee_source.validate(company)?;
        ensure!(
            !self.plan_id.is_nil()
                && (1..=10000).contains(&self.plan_revision)
                && text(&self.eligibility_basis, 3, 2000)
                && interval(self.coverage_from, self.coverage_to),
            "Review enrollment source, eligibility basis and coverage dates."
        );
        ensure!(
            self.status != Status::Active || self.eligibility == Eligibility::Eligible,
            "Active coverage requires a recorded Eligible review."
        );
        ensure!(
            self.status != Status::Declined || self.eligibility != Eligibility::Eligible,
            "Declined coverage must not claim eligibility approval."
        );
        ensure!(
            self.status != Status::Ended || self.coverage_to.is_some(),
            "Enter the final coverage date when ending coverage."
        );
        ensure!(
            self.dependents.len() <= usize::from(self.plan_snapshot.dependent_limit),
            "The captured plan's dependent limit is exceeded."
        );
        let mut ids = BTreeSet::new();
        for d in &self.dependents {
            ensure!(
                !d.id.is_nil()
                    && ids.insert(d.id)
                    && text(&d.full_name, 1, 200)
                    && interval(d.coverage_from, d.coverage_to)
                    && d.coverage_from >= self.coverage_from
                    && self.coverage_to.is_none_or(
                        |end| d.coverage_from <= end && d.coverage_to.is_some_and(|v| v <= end)
                    )
                    && d.birth_date.is_none_or(|b| date(b) && b <= d.coverage_from),
                "Review dependent identity, name, birth date and coverage within the enrollment."
            );
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record<T> {
    pub id: Uuid,
    pub company_id: Uuid,
    pub revision: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub data: T,
}
pub type PlanRecord = Record<Plan>;
pub type EnrollmentRecord = Record<Enrollment>;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "record",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum State {
    Plan(PlanRecord),
    Enrollment(EnrollmentRecord),
}
impl State {
    pub fn id(&self) -> Uuid {
        match self {
            Self::Plan(v) => v.id,
            Self::Enrollment(v) => v.id,
        }
    }
    pub fn revision(&self) -> i64 {
        match self {
            Self::Plan(v) => v.revision,
            Self::Enrollment(v) => v.revision,
        }
    }
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Plan(_) => "plan",
            Self::Enrollment(_) => "enrollment",
        }
    }
    pub fn company(&self) -> Uuid {
        match self {
            Self::Plan(v) => v.company_id,
            Self::Enrollment(v) => v.company_id,
        }
    }
    fn created_at(&self) -> DateTime<Utc> {
        match self {
            Self::Plan(v) => v.created_at,
            Self::Enrollment(v) => v.created_at,
        }
    }
    pub fn updated_at(&self) -> DateTime<Utc> {
        match self {
            Self::Plan(v) => v.updated_at,
            Self::Enrollment(v) => v.updated_at,
        }
    }
    fn validate(&self, company: Uuid) -> Result<()> {
        let created = match self {
            Self::Plan(v) => {
                v.data.validate()?;
                v.created_at
            }
            Self::Enrollment(v) => {
                v.data.validate(company)?;
                v.created_at
            }
        };
        ensure!(
            !self.id().is_nil()
                && self.company() == company
                && (1..=10000).contains(&self.revision())
                && self.updated_at() >= created,
            "Invalid benefit record identity or revision."
        );
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub id: Uuid,
    pub company_id: Uuid,
    pub actor_id: Uuid,
    pub actor_name: String,
    pub body: String,
    pub created_at: DateTime<Utc>,
    pub previous: Option<State>,
    pub current: State,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Context {
    pub schema_version: u16,
    pub company_id: Uuid,
    pub user_id: Uuid,
    pub today: NaiveDate,
    pub employees: Vec<EmployeeSource>,
    pub plans: Vec<PlanRecord>,
    pub enrollments: Vec<EnrollmentRecord>,
    pub events: Vec<Event>,
}
impl Context {
    pub fn validate(&self, a: &Access) -> Result<()> {
        ensure!(
            eligible(a)
                && !a.company_id.is_nil()
                && !a.user_id.is_nil()
                && self.schema_version == 1
                && self.company_id == a.company_id
                && self.user_id == a.user_id
                && date(self.today),
            "Current HR benefits access is required."
        );
        ensure!(
            self.employees.len() <= 10000
                && self.plans.len() <= 500
                && self.enrollments.len() <= 5000
                && self.events.len() <= 20000,
            "Benefits context exceeds its bounded history budget."
        );
        let mut people = BTreeSet::new();
        for e in &self.employees {
            e.validate(a.company_id)?;
            ensure!(people.insert(e.id), "Duplicate benefit employee source.");
        }
        let states = self
            .plans
            .iter()
            .cloned()
            .map(State::Plan)
            .chain(self.enrollments.iter().cloned().map(State::Enrollment))
            .collect::<Vec<_>>();
        let mut identities = BTreeSet::new();
        let mut codes = BTreeSet::new();
        let mut global_ids = BTreeSet::new();
        for s in &states {
            s.validate(a.company_id)?;
            ensure!(
                identities.insert((s.kind(), s.id())) && global_ids.insert(s.id()),
                "Duplicate benefit record identity."
            );
        }
        for p in &self.plans {
            ensure!(codes.insert(&p.data.code), "Duplicate benefit plan code.");
        }
        let mut event_ids = BTreeSet::new();
        let mut chain = BTreeMap::new();
        for e in &self.events {
            e.current.validate(a.company_id)?;
            ensure!(
                !e.id.is_nil()
                    && event_ids.insert(e.id)
                    && e.company_id == a.company_id
                    && !e.actor_id.is_nil()
                    && text(&e.actor_name, 1, 200)
                    && text(&e.body, 3, 2000)
                    && e.created_at == e.current.updated_at()
                    && identities.contains(&(e.current.kind(), e.current.id()))
                    && chain
                        .insert((e.current.kind(), e.current.id(), e.current.revision()), e)
                        .is_none(),
                "Invalid or duplicated benefit audit event."
            );
            if let Some(old) = &e.previous {
                old.validate(a.company_id)?;
                ensure!(
                    old.kind() == e.current.kind()
                        && old.id() == e.current.id()
                        && old.revision() + 1 == e.current.revision()
                        && old.created_at() == e.current.created_at()
                        && old.updated_at() <= e.current.updated_at(),
                    "Discontinuous benefit history."
                );
            } else {
                ensure!(
                    e.current.revision() == 1,
                    "Missing initial benefit history."
                );
            }
        }
        for e in &self.events {
            if let Some(old) = &e.previous {
                ensure!(
                    chain
                        .get(&(old.kind(), old.id(), old.revision()))
                        .is_some_and(|prior| &prior.current == old),
                    "Benefit history is incomplete or rewritten."
                );
            }
        }
        for s in &states {
            ensure!(
                chain
                    .get(&(s.kind(), s.id(), s.revision()))
                    .is_some_and(|last| last.current == *s)
                    && self
                        .events
                        .iter()
                        .filter(|e| e.current.kind() == s.kind() && e.current.id() == s.id())
                        .count() as i64
                        == s.revision(),
                "Benefit record and complete retained history disagree."
            );
        }
        for e in &self.enrollments {
            ensure!(
                people.contains(&e.data.employee_source.id),
                "Benefit employee source is missing."
            );
            ensure!(chain.get(&("plan",e.data.plan_id,e.data.plan_revision)).is_some_and(|event|matches!(&event.current,State::Plan(p)if p.data==e.data.plan_snapshot)),"Captured plan revision is missing or changed.");
        }
        ensure!(
            serde_json::to_vec(self)?.len() <= 8 * 1024 * 1024,
            "Benefits history exceeds its byte budget; narrow company administration before continuing."
        );
        Ok(())
    }
    pub fn plan(&self, id: Uuid) -> Result<&PlanRecord> {
        self.plans
            .iter()
            .find(|p| p.id == id)
            .ok_or_else(|| anyhow::anyhow!("Choose a current benefit plan."))
    }
    pub fn enrollment(&self, id: Uuid) -> Result<&EnrollmentRecord> {
        self.enrollments
            .iter()
            .find(|e| e.id == id)
            .ok_or_else(|| anyhow::anyhow!("Choose a saved enrollment."))
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "data",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Change {
    Plan(Plan),
    Enrollment(Enrollment),
}
impl Change {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Plan(_) => "plan",
            Self::Enrollment(_) => "enrollment",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub id: Uuid,
    pub company_id: Uuid,
    pub actor_id: Uuid,
    pub subject_id: Uuid,
    pub expected_revision: i64,
    pub body: String,
    pub change: Change,
}
impl Request {
    pub fn new(
        a: &Access,
        c: &Context,
        subject_id: Uuid,
        change: Change,
        body: String,
    ) -> Result<Self> {
        c.validate(a)?;
        let expected_revision = match &change {
            Change::Plan(_) => c
                .plans
                .iter()
                .find(|v| v.id == subject_id)
                .map(|v| v.revision),
            Change::Enrollment(_) => c
                .enrollments
                .iter()
                .find(|v| v.id == subject_id)
                .map(|v| v.revision),
        }
        .unwrap_or(0);
        let r = Self {
            id: Uuid::new_v4(),
            company_id: a.company_id,
            actor_id: a.user_id,
            subject_id,
            expected_revision,
            body,
            change,
        };
        r.validate(c, a)?;
        Ok(r)
    }
    pub fn binding(&self, a: &Access) -> Result<()> {
        ensure!(
            eligible(a)
                && !self.id.is_nil()
                && !self.subject_id.is_nil()
                && self.company_id == a.company_id
                && self.actor_id == a.user_id
                && (0..10000).contains(&self.expected_revision)
                && text(&self.body, 3, 2000)
                && serde_json::to_vec(self)?.len() <= 24000,
            "Review current actor, benefits revision and reason."
        );
        Ok(())
    }
    pub fn validate(&self, c: &Context, a: &Access) -> Result<()> {
        self.binding(a)?;
        c.validate(a)?;
        match &self.change {
            Change::Plan(p) => {
                p.validate()?;
                ensure!(
                    !c.enrollments.iter().any(|v| v.id == self.subject_id),
                    "The identity already belongs to an enrollment."
                );
                let old = c.plans.iter().find(|v| v.id == self.subject_id);
                ensure!(
                    old.map(|v| v.revision).unwrap_or(0) == self.expected_revision,
                    "The plan changed; refresh and review."
                );
                ensure!(
                    old.is_none_or(|v| v.data != *p)
                        && !c
                            .plans
                            .iter()
                            .any(|v| v.id != self.subject_id && v.data.code == p.code),
                    "Change the plan or choose an unused code."
                );
            }
            Change::Enrollment(e) => {
                e.validate(c.company_id)?;
                ensure!(
                    !c.plans.iter().any(|v| v.id == self.subject_id),
                    "The identity already belongs to a plan."
                );
                let old = c.enrollments.iter().find(|v| v.id == self.subject_id);
                ensure!(
                    old.map(|v| v.revision).unwrap_or(0) == self.expected_revision,
                    "The enrollment changed; refresh and review."
                );
                if let Some(old) = old {
                    ensure!(
                        old.data.plan_id == e.plan_id
                            && old.data.employee_source.id == e.employee_source.id
                            && old.data.status.permits(e.status)
                            && old.data != *e,
                        "Review the enrollment identity and permitted status change; ended history is terminal."
                    );
                } else {
                    ensure!(
                        matches!(
                            e.status,
                            Status::Pending | Status::Active | Status::Declined
                        ),
                        "New coverage cannot start ended."
                    );
                }
                if e.status == Status::Ended {
                    let old = old.ok_or_else(|| {
                        anyhow::anyhow!("Choose existing active coverage to end.")
                    })?;
                    ensure!(
                        old.data.plan_revision == e.plan_revision
                            && old.data.plan_snapshot == e.plan_snapshot
                            && old.data.employee_source == e.employee_source
                            && old.data.coverage_from == e.coverage_from
                            && old.data.eligibility == e.eligibility
                            && old.data.eligibility_basis == e.eligibility_basis,
                        "End coverage against its original reviewed source; do not rewrite the prior review."
                    );
                    let end = e.coverage_to.expect("Validated ended interval");
                    ensure!(
                        old.data.coverage_to.is_none_or(|v| end <= v)
                            && old.data.dependents.len() == e.dependents.len()
                            && old
                                .data
                                .dependents
                                .iter()
                                .all(|prior| e.dependents.iter().any(|next| next.id == prior.id
                                    && next.full_name == prior.full_name
                                    && next.relationship == prior.relationship
                                    && next.birth_date == prior.birth_date
                                    && next.coverage_from == prior.coverage_from
                                    && next.coverage_to
                                        == Some(prior.coverage_to.unwrap_or(end).min(end)))),
                        "End the existing dependents without rewriting their identity or extending their coverage."
                    );
                } else {
                    let current = c.plan(e.plan_id)?;
                    ensure!(
                        current.revision == e.plan_revision
                            && current.data == e.plan_snapshot
                            && current.data.available,
                        "Choose a currently available plan revision and review it."
                    );
                    ensure!(
                        c.employees.iter().any(|p| p == &e.employee_source),
                        "Employment changed; refresh and review eligibility."
                    );
                    ensure!(
                        e.status != Status::Active || e.employee_source.active,
                        "New activation requires active employment."
                    );
                }
                ensure!(
                    e.dependents
                        .iter()
                        .all(|d| d.birth_date.is_none_or(|v| v <= c.today)),
                    "A recorded birth date cannot be in the future."
                );
                if e.status != Status::Declined {
                    ensure!(
                        !c.enrollments.iter().any(|other| other.id != self.subject_id
                            && other.data.plan_id == e.plan_id
                            && other.data.employee_source.id == e.employee_source.id
                            && other.data.status != Status::Declined
                            && other.data.coverage_from <= e.coverage_to.unwrap_or(NaiveDate::MAX)
                            && e.coverage_from <= other.data.coverage_to.unwrap_or(NaiveDate::MAX)),
                        "This person already has overlapping coverage or a pending enrollment for this plan."
                    );
                }
            }
        }
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
    pub subject_id: Uuid,
    pub kind: String,
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
                && self.subject_id == r.subject_id
                && self.kind == r.change.kind()
                && self.revision == r.expected_revision + 1,
            "The benefits receipt does not match the frozen request."
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
        f.write_str("The benefits save was rejected. Refresh and explicitly review the saved source before trying again.")
    }
}
impl std::error::Error for Rejected {}
#[derive(Default)]
pub struct Demo {
    context: Option<Context>,
    receipts: BTreeMap<Uuid, (Request, Receipt)>,
}
impl Demo {
    pub fn execute(&mut self, op: Operation, data: &Snapshot) -> Result<Response> {
        let a = &data.access;
        ensure!(eligible(a), "Current HR benefits access is required.");
        let mut c = self.context.clone().unwrap_or(Context {
            schema_version: 1,
            company_id: a.company_id,
            user_id: a.user_id,
            today: data.company.today(),
            employees: vec![],
            plans: vec![],
            enrollments: vec![],
            events: vec![],
        });
        c.user_id = a.user_id;
        c.today = data.company.today();
        c.employees = data.employees.iter().map(EmployeeSource::from).collect();
        c.validate(a)?;
        let mut receipt = None;
        if let Operation::Save(r) = op {
            r.binding(a)?;
            if let Some((old, stored)) = self.receipts.get(&r.id) {
                ensure!(
                    old == r.as_ref(),
                    "The retry identity belongs to another payload."
                );
                receipt = Some(stored.clone());
            } else {
                r.validate(&c, a)?;
                let now = Utc::now();
                let revision = r.expected_revision + 1;
                let (previous, current) = match &r.change {
                    Change::Plan(p) => {
                        let prior = c.plans.iter().find(|v| v.id == r.subject_id).cloned();
                        let current = Record {
                            id: r.subject_id,
                            company_id: c.company_id,
                            revision,
                            created_at: prior.as_ref().map(|v| v.created_at).unwrap_or(now),
                            updated_at: now,
                            data: p.clone(),
                        };
                        c.plans.retain(|v| v.id != r.subject_id);
                        c.plans.push(current.clone());
                        (prior.map(State::Plan), State::Plan(current))
                    }
                    Change::Enrollment(e) => {
                        let prior = c.enrollments.iter().find(|v| v.id == r.subject_id).cloned();
                        let current = Record {
                            id: r.subject_id,
                            company_id: c.company_id,
                            revision,
                            created_at: prior.as_ref().map(|v| v.created_at).unwrap_or(now),
                            updated_at: now,
                            data: e.clone(),
                        };
                        c.enrollments.retain(|v| v.id != r.subject_id);
                        c.enrollments.push(current.clone());
                        (prior.map(State::Enrollment), State::Enrollment(current))
                    }
                };
                let actor_name = data
                    .employees
                    .iter()
                    .find(|v| Some(v.id) == a.employee_id)
                    .map(|v| v.full_name.clone())
                    .unwrap_or_else(|| "Fictional HR reviewer".into());
                c.events.push(Event {
                    id: r.id,
                    company_id: c.company_id,
                    actor_id: a.user_id,
                    actor_name,
                    body: r.body.clone(),
                    created_at: now,
                    previous,
                    current,
                });
                c.validate(a)?;
                let saved = Receipt {
                    schema_version: 1,
                    id: r.id,
                    company_id: c.company_id,
                    actor_id: a.user_id,
                    subject_id: r.subject_id,
                    kind: r.change.kind().into(),
                    revision,
                    applied_at: now,
                };
                saved.validate(&r)?;
                self.receipts.insert(r.id, (*r, saved.clone()));
                receipt = Some(saved);
            }
        }
        self.context = Some(c.clone());
        Ok(Response {
            context: c,
            receipt,
        })
    }
}
