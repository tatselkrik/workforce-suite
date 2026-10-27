//! Approved offers and employee onboarding. Pay terms never enter Manager task records.
use crate::{
    access::Role,
    domain::{EmployeeDraft, EmployeeStatus, Snapshot, parse_date},
    hiring,
};
use anyhow::{Context, Result, ensure};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OfferStatus {
    #[default]
    Draft,
    Approved,
    Sent,
    Accepted,
    Declined,
    Withdrawn,
}
impl OfferStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Draft => "Draft",
            Self::Approved => "Approved",
            Self::Sent => "Sent",
            Self::Accepted => "Accepted",
            Self::Declined => "Declined",
            Self::Withdrawn => "Withdrawn",
        }
    }
    pub fn active(self) -> bool {
        !matches!(self, Self::Declined | Self::Withdrawn)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Offer {
    pub id: Uuid,
    pub company_id: Uuid,
    pub applicant_id: Uuid,
    pub applicant_name: String,
    pub job_title: String,
    pub department: String,
    pub starts_on: NaiveDate,
    pub amount_minor: i64,
    pub currency: String,
    pub pay_period: String,
    pub terms: String,
    pub expires_on: Option<NaiveDate>,
    pub status: OfferStatus,
    pub revision: i64,
    pub approved_by: Option<Uuid>,
    pub approved_at: Option<DateTime<Utc>>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Conversion {
    pub id: Uuid,
    pub company_id: Uuid,
    pub applicant_id: Uuid,
    pub offer_id: Uuid,
    pub employee_id: Uuid,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Case {
    pub id: Uuid,
    pub company_id: Uuid,
    pub employee_id: Uuid,
    pub conversion_id: Uuid,
    pub employee_name: String,
    pub employee_code: String,
    pub starts_on: NaiveDate,
    pub status: String,
    pub revision: i64,
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
    pub starts_on: NaiveDate,
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
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub id: Uuid,
    pub company_id: Uuid,
    #[serde(default)]
    pub offer_id: Option<Uuid>,
    #[serde(default)]
    pub case_id: Option<Uuid>,
    #[serde(default)]
    pub task_id: Option<Uuid>,
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
    pub offers: Vec<Offer>,
    pub offer_events: Vec<Event>,
    pub conversions: Vec<Conversion>,
    pub cases: Vec<Case>,
    pub tasks: Vec<Task>,
    pub events: Vec<Event>,
    pub(crate) retries: std::collections::HashMap<Uuid, Value>,
}
#[derive(Clone, Debug)]
pub struct OfferDraft {
    pub id: Uuid,
    pub applicant_id: Uuid,
    pub original: Option<Offer>,
    pub job_title: String,
    pub department: String,
    pub starts_on: String,
    pub amount: String,
    pub currency: String,
    pub pay_period: String,
    pub terms: String,
    pub expires_on: String,
    pub reason: String,
}
impl OfferDraft {
    pub fn new(data: &Snapshot, app: Uuid) -> Self {
        Self {
            id: Uuid::new_v4(),
            applicant_id: app,
            original: None,
            job_title: data
                .hiring
                .records
                .iter()
                .find(|a| a.id == app)
                .map_or(String::new(), |a| a.position.clone()),
            department: String::new(),
            starts_on: data.company.today().to_string(),
            amount: "0.00".into(),
            currency: "PHP".into(),
            pay_period: "monthly".into(),
            terms: String::new(),
            expires_on: String::new(),
            reason: "Offer prepared".into(),
        }
    }
    pub fn existing(r: &Offer) -> Self {
        Self {
            id: r.id,
            applicant_id: r.applicant_id,
            original: Some(r.clone()),
            job_title: r.job_title.clone(),
            department: r.department.clone(),
            starts_on: r.starts_on.to_string(),
            amount: money(r.amount_minor),
            currency: r.currency.clone(),
            pay_period: r.pay_period.clone(),
            terms: r.terms.clone(),
            expires_on: r.expires_on.map(|d| d.to_string()).unwrap_or_default(),
            reason: String::new(),
        }
    }
}
#[derive(Clone, Debug)]
pub struct HireDraft {
    pub id: Uuid,
    pub offer_id: Uuid,
    pub revision: i64,
    pub employee_code: String,
    pub reports_to: Option<Uuid>,
    pub employment_type_id: Option<Uuid>,
    pub work_location_id: Option<Uuid>,
    pub reason: String,
}
impl HireDraft {
    pub fn new(r: &Offer) -> Self {
        Self {
            id: Uuid::new_v4(),
            offer_id: r.id,
            revision: r.revision,
            employee_code: String::new(),
            reports_to: None,
            employment_type_id: None,
            work_location_id: None,
            reason: "Accepted offer hired; onboarding started".into(),
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
            reason: "Onboarding task added".into(),
        }
    }
    pub fn existing(r: &Task) -> Self {
        Self {
            id: r.id,
            case_id: r.case_id,
            original: Some(r.clone()),
            assigned_employee_id: r.assigned_employee_id,
            title: r.title.clone(),
            instructions: r.instructions.clone(),
            due_on: r.due_on.map(|d| d.to_string()).unwrap_or_default(),
            required: r.required,
            reason: String::new(),
        }
    }
}
#[derive(Clone, Debug)]
pub enum HiringAction {
    OfferSave(OfferDraft),
    OfferStatus {
        id: Uuid,
        revision: i64,
        status: OfferStatus,
        reason: String,
    },
    Hire(HireDraft),
    TaskSave(TaskDraft),
    TaskStatus {
        id: Uuid,
        revision: i64,
        status: String,
        reason: String,
    },
    CaseStatus {
        id: Uuid,
        revision: i64,
        status: String,
        reason: String,
    },
}
pub fn money(value: i64) -> String {
    format!("{}.{:02}", value / 100, value % 100)
}
pub fn amount_minor(text: &str) -> Result<i64> {
    let s = text.trim();
    let parts: Vec<_> = s.split('.').collect();
    ensure!(
        parts.len() <= 2
            && !parts[0].is_empty()
            && parts[0].bytes().all(|b| b.is_ascii_digit())
            && parts.get(1).is_none_or(|v| !v.is_empty()
                && v.len() <= 2
                && v.bytes().all(|b| b.is_ascii_digit())),
        "Enter an amount with up to two decimal places, without separators."
    );
    let whole = parts[0].parse::<i64>().context("Amount is too large.")?;
    let fraction = parts
        .get(1)
        .map(|s| format!("{s:0<2}").parse::<i64>().unwrap())
        .unwrap_or(0);
    let v = whole
        .checked_mul(100)
        .and_then(|v| v.checked_add(fraction))
        .context("Amount is too large.")?;
    ensure!(v <= 999999999999, "Amount is too large.");
    Ok(v)
}
fn text(s: &str, max: usize, required: bool, multiline: bool) -> Result<()> {
    ensure!(
        (!required || !s.trim().is_empty())
            && s.trim().chars().count() <= max
            && (multiline || !s.chars().any(char::is_control)),
        "Check the required text and maximum length ({max} characters)."
    );
    Ok(())
}
pub fn offer(data: &Snapshot, id: Uuid) -> Result<&Offer> {
    ensure!(
        data.access.allows(crate::staff_roles::Capability::Recruiting),
        "Only HR and Administrators can see offer terms."
    );
    data.onboarding
        .offers
        .iter()
        .find(|r| r.id == id && r.company_id == data.company.id)
        .context("Offer not found. Refresh.")
}
fn task(data: &Snapshot, id: Uuid) -> Result<&Task> {
    let r = data
        .onboarding
        .tasks
        .iter()
        .find(|r| r.id == id && r.company_id == data.company.id)
        .context("Task not found. Refresh.")?;
    ensure!(
        data.access.allows(crate::staff_roles::Capability::Recruiting)
            || (data.access.uses_desktop()
                && r.assigned_employee_id == data.access.employee_id),
        "This task is not assigned to you."
    );
    Ok(r)
}
impl HiringAction {
    pub fn payload(&self, data: &Snapshot) -> Result<Value> {
        ensure!(
            data.mode != crate::domain::Mode::SignedOut
                && data.access.company_id == data.company.id
                && data.access.uses_desktop(),
            "Desktop staff access is required."
        );
        let (op, p) = match self {
            Self::OfferSave(d) => {
                hiring::authorize(&data.access, data)?;
                let a = hiring::record(data, d.applicant_id)?;
                let retry =
                    d.original.is_none() && data.onboarding.offers.iter().any(|r| r.id == d.id);
                ensure!(
                    retry
                        || (!a.archived
                            && !a.stage.terminal()
                            && !data
                                .onboarding
                                .conversions
                                .iter()
                                .any(|c| c.applicant_id == a.id)),
                    "Choose an active application not yet hired."
                );
                if let Some(r) = &d.original {
                    ensure!(
                        offer(data, d.id)?.revision == r.revision && r.status == OfferStatus::Draft,
                        "This offer changed or is no longer a draft."
                    );
                } else {
                    ensure!(
                        !data
                            .onboarding
                            .offers
                            .iter()
                            .any(|r| r.applicant_id == a.id && r.id != d.id && r.status.active()),
                        "Finish or withdraw the existing offer first."
                    );
                }
                text(&d.job_title, 100, true, false)?;
                text(&d.department, 80, false, false)?;
                text(&d.terms, 4000, false, true)?;
                ensure!(
                    d.currency.len() == 3
                        && d.currency.bytes().all(|b| b.is_ascii_uppercase())
                        && ["monthly", "annual", "hourly"].contains(&d.pay_period.as_str()),
                    "Use a three-letter uppercase currency and valid pay period."
                );
                (
                    "offer_save",
                    json!({"id":d.id,"expected_revision":d.original.as_ref().map(|r|r.revision),"applicant_id":d.applicant_id,"job_title":d.job_title.trim(),"department":d.department.trim(),"starts_on":parse_date(&d.starts_on)?,"amount_minor":amount_minor(&d.amount)?,"currency":d.currency,"pay_period":d.pay_period,"terms":d.terms.trim(),"expires_on":if d.expires_on.trim().is_empty(){None}else{Some(parse_date(&d.expires_on)?)},"reason":d.reason.trim()}),
                )
            }
            Self::OfferStatus {
                id,
                revision,
                status,
                reason,
            } => {
                let r = offer(data, *id)?;
                ensure!(r.revision == *revision, "This offer changed. Refresh.");
                let allowed = matches!(
                    (r.status, *status),
                    (OfferStatus::Draft, OfferStatus::Approved)
                        | (OfferStatus::Approved, OfferStatus::Sent)
                        | (
                            OfferStatus::Sent,
                            OfferStatus::Accepted | OfferStatus::Declined
                        )
                ) || (r.status.active() && *status == OfferStatus::Withdrawn);
                ensure!(
                    allowed
                        && !data
                            .onboarding
                            .conversions
                            .iter()
                            .any(|c| c.offer_id == *id),
                    "This offer transition is unavailable."
                );
                ensure!(
                    *status != OfferStatus::Approved || data.access.role == Role::Administrator,
                    "An Administrator must approve the offer."
                );
                if matches!(
                    status,
                    OfferStatus::Approved | OfferStatus::Sent | OfferStatus::Accepted
                ) {
                    ensure!(
                        r.expires_on.is_none_or(|d| d >= data.company.today()),
                        "This offer has expired."
                    );
                    let a = hiring::record(data, r.applicant_id)?;
                    ensure!(
                        !a.archived && !a.stage.terminal(),
                        "Restore an active application first."
                    );
                }
                (
                    "offer_status",
                    json!({"id":id,"expected_revision":revision,"status":status,"reason":reason.trim()}),
                )
            }
            Self::Hire(d) => {
                let r = offer(data, d.offer_id)?;
                ensure!(
                    r.revision == d.revision && r.status == OfferStatus::Accepted,
                    "Hiring requires the current accepted offer."
                );
                ensure!(
                    !data
                        .onboarding
                        .conversions
                        .iter()
                        .any(|c| c.applicant_id == r.applicant_id && c.id != d.id),
                    "This applicant has already been hired."
                );
                text(&d.employee_code, 30, true, false)?;
                if let Some(id) = d.reports_to {
                    ensure!(
                        data.employees
                            .iter()
                            .any(|e| e.id == id && e.status == EmployeeStatus::Active),
                        "Choose an active reporting manager."
                    );
                }
                for (id, options) in [
                    (d.employment_type_id, &data.employment_types),
                    (d.work_location_id, &data.work_locations),
                ] {
                    if let Some(id) = id {
                        ensure!(
                            options.iter().any(|o| o.id == id && o.active),
                            "Choose an active employment option."
                        );
                    }
                }
                (
                    "hire",
                    json!({"id":d.id,"offer_id":d.offer_id,"expected_revision":d.revision,"employee_code":d.employee_code.trim().to_uppercase(),"reports_to":d.reports_to,"employment_type_id":d.employment_type_id,"work_location_id":d.work_location_id,"reason":d.reason.trim()}),
                )
            }
            Self::TaskSave(d) => {
                hiring::authorize(&data.access, data)?;
                let retry =
                    d.original.is_none() && data.onboarding.tasks.iter().any(|r| r.id == d.id);
                ensure!(
                    retry
                        || data
                            .onboarding
                            .cases
                            .iter()
                            .any(|c| c.id == d.case_id && c.status == "open"),
                    "Reopen onboarding before editing its checklist."
                );
                if let Some(r) = &d.original {
                    let current = task(data, d.id)?;
                    ensure!(
                        current.revision == r.revision
                            && current.status == "open"
                            && current.case_id == d.case_id,
                        "Reopen a finished task before editing it, or refresh a changed record."
                    );
                }
                text(&d.title, 120, true, false)?;
                text(&d.instructions, 2000, false, true)?;
                if let Some(id) = d.assigned_employee_id {
                    ensure!(
                        data.recruiting
                            .interviewers
                            .iter()
                            .any(|s| s.employee_id == id),
                        "Assign active staff with desktop access."
                    );
                }
                (
                    "task_save",
                    json!({"id":d.id,"expected_revision":d.original.as_ref().map(|r|r.revision),"case_id":d.case_id,"assigned_employee_id":d.assigned_employee_id,"title":d.title.trim(),"instructions":d.instructions.trim(),"due_on":if d.due_on.trim().is_empty(){None}else{Some(parse_date(&d.due_on)?)},"required":d.required,"reason":d.reason.trim()}),
                )
            }
            Self::TaskStatus {
                id,
                revision,
                status,
                reason,
            } => {
                let r = task(data, *id)?;
                ensure!(
                    r.revision == *revision
                        && r.case_status == "open"
                        && status != &r.status
                        && ["open", "done", "waived"].contains(&status.as_str()),
                    "Refresh the task and select an available status."
                );
                ensure!(
                    data.access.allows(crate::staff_roles::Capability::Recruiting) || (status != "waived" && r.status != "waived"),
                    "HR handles task waivers."
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
                hiring::authorize(&data.access, data)?;
                let r = data
                    .onboarding
                    .cases
                    .iter()
                    .find(|r| r.id == *id && r.company_id == data.company.id)
                    .context("Onboarding not found.")?;
                ensure!(
                    r.revision == *revision
                        && status != &r.status
                        && ["open", "completed"].contains(&status.as_str()),
                    "Refresh onboarding and choose an available status."
                );
                ensure!(
                    status != "completed"
                        || !data
                            .onboarding
                            .tasks
                            .iter()
                            .any(|t| t.case_id == *id && t.required && t.status == "open"),
                    "Finish or waive every required task first."
                );
                (
                    "case_status",
                    json!({"id":id,"expected_revision":revision,"status":status,"reason":reason.trim()}),
                )
            }
        };
        hiring::reason(p["reason"].as_str().unwrap_or(""))?;
        Ok(json!({"p_operation":op,"p_record":p}))
    }
    pub fn saved(&self, data: &Snapshot) -> bool {
        match self {
            Self::OfferSave(d) => data.onboarding.offers.iter().any(|r| {
                r.id == d.id
                    && r.company_id == data.company.id
                    && r.job_title == d.job_title.trim()
                    && r.amount_minor == amount_minor(&d.amount).unwrap_or(-1)
                    && r.revision == d.original.as_ref().map_or(0, |r| r.revision + 1)
            }),
            Self::OfferStatus {
                id,
                revision,
                status,
                ..
            } => data
                .onboarding
                .offers
                .iter()
                .any(|r| r.id == *id && r.revision == revision + 1 && r.status == *status),
            Self::Hire(d) => {
                data.onboarding
                    .conversions
                    .iter()
                    .any(|r| r.id == d.id && r.offer_id == d.offer_id)
                    && data.onboarding.cases.iter().any(|r| r.id == d.id)
            }
            Self::TaskSave(d) => data.onboarding.tasks.iter().any(|r| {
                r.id == d.id
                    && r.title == d.title.trim()
                    && r.assigned_employee_id == d.assigned_employee_id
                    && r.revision == d.original.as_ref().map_or(0, |r| r.revision + 1)
            }),
            Self::TaskStatus {
                id,
                revision,
                status,
                ..
            } => data
                .onboarding
                .tasks
                .iter()
                .any(|r| r.id == *id && r.revision == revision + 1 && r.status == *status),
            Self::CaseStatus {
                id,
                revision,
                status,
                ..
            } => data
                .onboarding
                .cases
                .iter()
                .any(|r| r.id == *id && r.revision == revision + 1 && r.status == *status),
        }
    }
}
pub const DEFAULT_TASKS: [&str; 5] = [
    "Verify contact and emergency details",
    "File employment contract and required documents",
    "Complete company orientation",
    "Prepare equipment and access",
    "Complete required introductory training",
];
pub fn apply_demo(data: &mut Snapshot, action: &HiringAction, now: DateTime<Utc>) -> Result<()> {
    let payload = action.payload(data)?;
    let p = &payload["p_record"];
    let id: Uuid = serde_json::from_value(p["id"].clone())?;
    if matches!(action,HiringAction::OfferSave(d)if d.original.is_none())
        || matches!(action,HiringAction::TaskSave(d)if d.original.is_none())
        || matches!(action, HiringAction::Hire(_))
    {
        if let Some(old) = data.onboarding.retries.get(&id) {
            ensure!(old == &payload, "This record identifier already exists.");
            return Ok(());
        }
    }
    let mut next = data.clone();
    let before;
    let after;
    let offer_id;
    let case_id;
    let task_id;
    let kind;
    match action {
        HiringAction::OfferSave(d) => {
            let old = next.onboarding.offers.iter().find(|r| r.id == id).cloned();
            let r = Offer {
                id,
                company_id: next.company.id,
                applicant_id: d.applicant_id,
                applicant_name: old.as_ref().map_or_else(
                    || {
                        hiring::record(data, d.applicant_id)
                            .unwrap()
                            .full_name
                            .clone()
                    },
                    |r| r.applicant_name.clone(),
                ),
                job_title: d.job_title.trim().into(),
                department: d.department.trim().into(),
                starts_on: parse_date(&d.starts_on)?,
                amount_minor: amount_minor(&d.amount)?,
                currency: d.currency.clone(),
                pay_period: d.pay_period.clone(),
                terms: d.terms.trim().into(),
                expires_on: serde_json::from_value(p["expires_on"].clone())?,
                status: OfferStatus::Draft,
                revision: old.as_ref().map_or(0, |r| r.revision + 1),
                approved_by: None,
                approved_at: None,
                created_by: old.as_ref().map_or(data.access.user_id, |r| r.created_by),
                created_at: old.as_ref().map_or(now, |r| r.created_at),
                updated_at: now,
            };
            before = old.as_ref().map(serde_json::to_value).transpose()?;
            after = serde_json::to_value(&r)?;
            kind = if old.is_some() { "edited" } else { "created" };
            next.onboarding.offers.retain(|r| r.id != id);
            next.onboarding.offers.push(r);
            offer_id = Some(id);
            case_id = None;
            task_id = None;
        }
        HiringAction::OfferStatus { status, .. } => {
            let r = next
                .onboarding
                .offers
                .iter_mut()
                .find(|r| r.id == id)
                .unwrap();
            before = Some(serde_json::to_value(&*r)?);
            r.status = *status;
            r.revision += 1;
            r.updated_at = now;
            if *status == OfferStatus::Approved {
                r.approved_by = Some(data.access.user_id);
                r.approved_at = Some(now);
            }
            after = serde_json::to_value(r)?;
            kind = status.label();
            offer_id = Some(id);
            case_id = None;
            task_id = None;
        }
        HiringAction::Hire(d) => {
            let r = offer(data, d.offer_id)?;
            let a = hiring::record(data, r.applicant_id)?;
            let employee = EmployeeDraft {
                id: None,
                employee_code: d.employee_code.clone(),
                full_name: a.full_name.clone(),
                department: r.department.clone(),
                job_title: r.job_title.clone(),
                joined_on: r.starts_on.to_string(),
                reports_to: d.reports_to,
                employment_type_id: d.employment_type_id,
                work_location_id: d.work_location_id,
                annual_leave_days: data.company.annual_leave_days,
                status: EmployeeStatus::Active,
            }
            .employee(data.company.id)?;
            ensure!(
                !data.employees.iter().any(|e| e
                    .employee_code
                    .eq_ignore_ascii_case(&employee.employee_code)),
                "Employee code already exists."
            );
            let mut contacts =
                crate::contacts::EmployeeContacts::empty(data.company.id, employee.id);
            contacts.phone = a.phone.clone();
            contacts.personal_email = a.email.clone();
            let contacts = contacts.normalized()?;
            let conversion = Conversion {
                id,
                company_id: data.company.id,
                applicant_id: a.id,
                offer_id: r.id,
                employee_id: employee.id,
                created_by: data.access.user_id,
                created_at: now,
            };
            let case = Case {
                id,
                company_id: data.company.id,
                employee_id: employee.id,
                conversion_id: id,
                employee_name: employee.full_name.clone(),
                employee_code: employee.employee_code.clone(),
                starts_on: r.starts_on,
                status: "open".into(),
                revision: 0,
                created_at: now,
                updated_at: now,
            };
            for title in DEFAULT_TASKS {
                next.onboarding.tasks.push(Task {
                    id: Uuid::new_v4(),
                    company_id: data.company.id,
                    case_id: id,
                    employee_id: employee.id,
                    employee_name: employee.full_name.clone(),
                    employee_code: employee.employee_code.clone(),
                    starts_on: r.starts_on,
                    case_status: "open".into(),
                    assigned_employee_id: None,
                    title: title.into(),
                    instructions: String::new(),
                    due_on: Some(r.starts_on),
                    required: true,
                    status: "open".into(),
                    revision: 0,
                    created_by: data.access.user_id,
                    created_at: now,
                    updated_at: now,
                });
            }
            next.employees.push(employee);
            next.contacts.push(contacts);
            next.onboarding.conversions.push(conversion.clone());
            next.onboarding.cases.push(case.clone());
            before = None;
            after = serde_json::to_value(&case)?;
            kind = "started";
            offer_id = Some(r.id);
            case_id = Some(id);
            task_id = None;
        }
        HiringAction::TaskSave(d) => {
            let c = next
                .onboarding
                .cases
                .iter()
                .find(|c| c.id == d.case_id)
                .unwrap()
                .clone();
            let old = next.onboarding.tasks.iter().find(|r| r.id == id).cloned();
            let r = Task {
                id,
                company_id: data.company.id,
                case_id: c.id,
                employee_id: c.employee_id,
                employee_name: c.employee_name,
                employee_code: c.employee_code,
                starts_on: c.starts_on,
                case_status: c.status,
                assigned_employee_id: d.assigned_employee_id,
                title: d.title.trim().into(),
                instructions: d.instructions.trim().into(),
                due_on: serde_json::from_value(p["due_on"].clone())?,
                required: d.required,
                status: "open".into(),
                revision: old.as_ref().map_or(0, |r| r.revision + 1),
                created_by: old.as_ref().map_or(data.access.user_id, |r| r.created_by),
                created_at: old.as_ref().map_or(now, |r| r.created_at),
                updated_at: now,
            };
            before = old.as_ref().map(serde_json::to_value).transpose()?;
            after = serde_json::to_value(&r)?;
            kind = if old.is_some() {
                "task_edited"
            } else {
                "task_created"
            };
            next.onboarding.tasks.retain(|t| t.id != id);
            next.onboarding.tasks.push(r);
            offer_id = None;
            case_id = Some(c.id);
            task_id = Some(id);
        }
        HiringAction::TaskStatus { status, .. } => {
            let t = next
                .onboarding
                .tasks
                .iter_mut()
                .find(|r| r.id == id)
                .unwrap();
            before = Some(serde_json::to_value(&*t)?);
            t.status = status.clone();
            t.revision += 1;
            t.updated_at = now;
            after = serde_json::to_value(&*t)?;
            kind = "task_status";
            offer_id = None;
            case_id = Some(t.case_id);
            task_id = Some(id);
        }
        HiringAction::CaseStatus { status, .. } => {
            let c = next
                .onboarding
                .cases
                .iter_mut()
                .find(|r| r.id == id)
                .unwrap();
            before = Some(serde_json::to_value(&*c)?);
            c.status = status.clone();
            c.revision += 1;
            c.updated_at = now;
            after = serde_json::to_value(&*c)?;
            for t in next.onboarding.tasks.iter_mut().filter(|t| t.case_id == id) {
                t.case_status = status.clone();
                t.revision += 1;
                t.updated_at = now;
            }
            kind = "case_status";
            offer_id = None;
            case_id = Some(id);
            task_id = None;
        }
    }
    let event = Event {
        id: Uuid::new_v4(),
        company_id: data.company.id,
        offer_id,
        case_id,
        task_id,
        kind: kind.to_lowercase(),
        reason: p["reason"].as_str().unwrap().into(),
        actor_id: data.access.user_id,
        actor_name: data
            .access
            .employee_id
            .map(|id| data.employee_name(id).into())
            .unwrap_or_else(|| "Staff".into()),
        actor_role: data.access.role,
        before_record: before,
        after_record: after,
        recorded_at: now,
    };
    if offer_id.is_some() {
        next.onboarding.offer_events.push(event.clone());
    }
    if case_id.is_some() {
        next.onboarding.events.push(event);
    }
    if matches!(action,HiringAction::OfferSave(d)if d.original.is_none())
        || matches!(action,HiringAction::TaskSave(d)if d.original.is_none())
        || matches!(action, HiringAction::Hire(_))
    {
        next.onboarding.retries.insert(id, payload);
    }
    *data = next;
    Ok(())
}
