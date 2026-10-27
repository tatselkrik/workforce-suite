//! Learning facts and protected evidence links. Certifications are recorded metadata, not compliance verification.
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
pub struct Training {
    pub id: Uuid,
    pub company_id: Uuid,
    pub employee_id: Uuid,
    pub employee_name: String,
    pub employee_code: String,
    pub title: String,
    pub provider: String,
    pub starts_on: Option<NaiveDate>,
    pub ends_on: Option<NaiveDate>,
    pub due_on: Option<NaiveDate>,
    pub required: bool,
    pub status: String,
    pub progress_note: String,
    pub completed_on: Option<NaiveDate>,
    pub learning_minutes: Option<u32>,
    pub outcome: String,
    pub revision: i64,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Certificate {
    pub id: Uuid,
    pub company_id: Uuid,
    pub employee_id: Uuid,
    pub employee_name: String,
    pub employee_code: String,
    pub title: String,
    pub issuer: String,
    pub reference_number: String,
    pub issued_on: NaiveDate,
    pub expires_on: Option<NaiveDate>,
    pub status: String,
    pub archived: bool,
    pub revision: i64,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
impl Certificate {
    pub fn label(&self, today: NaiveDate) -> &str {
        if self.archived {
            "Archived"
        } else if self.status == "revoked" {
            "Revoked"
        } else if self.expires_on.is_some_and(|d| d < today) {
            "Expired"
        } else if self.expires_on.is_some_and(|d| {
            d <= today
                .checked_add_days(chrono::Days::new(30))
                .unwrap_or(today)
        }) {
            "Expires within 30 days"
        } else {
            "Valid (recorded)"
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Evidence {
    pub id: Uuid,
    pub company_id: Uuid,
    pub employee_id: Uuid,
    pub certificate_id: Uuid,
    pub document_id: Option<Uuid>,
    pub revision: i64,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub id: Uuid,
    pub company_id: Uuid,
    pub training_id: Option<Uuid>,
    pub certificate_id: Option<Uuid>,
    pub evidence_id: Option<Uuid>,
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
    pub training: Vec<Training>,
    pub certificates: Vec<Certificate>,
    pub evidence: Vec<Evidence>,
    pub events: Vec<Event>,
    pub(crate) retries: std::collections::HashMap<String, (Uuid, Value)>,
}
#[derive(Clone, Debug)]
pub struct TrainingDraft {
    pub id: Uuid,
    pub original: Option<Training>,
    pub employee_id: Option<Uuid>,
    pub title: String,
    pub provider: String,
    pub starts_on: String,
    pub ends_on: String,
    pub due_on: String,
    pub required: bool,
    pub reason: String,
}
impl TrainingDraft {
    pub fn new(employee_id: Option<Uuid>) -> Self {
        Self {
            id: Uuid::new_v4(),
            original: None,
            employee_id,
            title: String::new(),
            provider: String::new(),
            starts_on: String::new(),
            ends_on: String::new(),
            due_on: String::new(),
            required: false,
            reason: String::new(),
        }
    }
    pub fn existing(t: &Training) -> Self {
        Self {
            id: t.id,
            original: Some(t.clone()),
            employee_id: Some(t.employee_id),
            title: t.title.clone(),
            provider: t.provider.clone(),
            starts_on: date_text(t.starts_on),
            ends_on: date_text(t.ends_on),
            due_on: date_text(t.due_on),
            required: t.required,
            reason: String::new(),
        }
    }
}
#[derive(Clone, Debug)]
pub struct CertificateDraft {
    pub id: Uuid,
    pub original: Option<Certificate>,
    pub employee_id: Option<Uuid>,
    pub title: String,
    pub issuer: String,
    pub reference_number: String,
    pub issued_on: String,
    pub expires_on: String,
    pub reason: String,
}
impl CertificateDraft {
    pub fn new(employee_id: Option<Uuid>, today: NaiveDate) -> Self {
        Self {
            id: Uuid::new_v4(),
            original: None,
            employee_id,
            title: String::new(),
            issuer: String::new(),
            reference_number: String::new(),
            issued_on: today.to_string(),
            expires_on: String::new(),
            reason: String::new(),
        }
    }
    pub fn existing(c: &Certificate) -> Self {
        Self {
            id: c.id,
            original: Some(c.clone()),
            employee_id: Some(c.employee_id),
            title: c.title.clone(),
            issuer: c.issuer.clone(),
            reference_number: c.reference_number.clone(),
            issued_on: c.issued_on.to_string(),
            expires_on: date_text(c.expires_on),
            reason: String::new(),
        }
    }
}
#[derive(Clone, Debug)]
pub struct EvidenceDraft {
    pub id: Uuid,
    pub original: Option<Evidence>,
    pub certificate_id: Uuid,
    pub document_id: Option<Uuid>,
    pub reason: String,
}
impl EvidenceDraft {
    pub fn new(certificate_id: Uuid) -> Self {
        Self {
            id: Uuid::new_v4(),
            original: None,
            certificate_id,
            document_id: None,
            reason: String::new(),
        }
    }
    pub fn existing(e: &Evidence) -> Self {
        Self {
            id: e.id,
            original: Some(e.clone()),
            certificate_id: e.certificate_id,
            document_id: e.document_id,
            reason: String::new(),
        }
    }
}
#[derive(Clone, Debug)]
pub enum LearningAction {
    TrainingSave(TrainingDraft),
    Progress {
        id: Uuid,
        revision: i64,
        note: String,
        reason: String,
    },
    TrainingStatus {
        id: Uuid,
        revision: i64,
        status: String,
        completed_on: String,
        learning_minutes: Option<u32>,
        outcome: String,
        reason: String,
    },
    CertificateSave(CertificateDraft),
    CertificateStatus {
        id: Uuid,
        revision: i64,
        status: String,
        reason: String,
    },
    CertificateArchive {
        id: Uuid,
        revision: i64,
        archived: bool,
        reason: String,
    },
    EvidenceSave(EvidenceDraft),
}
pub fn date_text(d: Option<NaiveDate>) -> String {
    d.map(|d| d.to_string()).unwrap_or_default()
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
fn hr(data: &Snapshot) -> Result<()> {
    ensure!(
        data.access.allows(crate::staff_roles::Capability::DevelopmentAdministration),
        "Only HR and Administrators manage training completion and certifications."
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
        (data.access.allows(crate::staff_roles::Capability::DevelopmentAdministration) || data.access.allows(crate::staff_roles::Capability::PersonnelDocuments))
            || Some(e.id) == data.access.employee_id
            || (data.access.allows(crate::staff_roles::Capability::TeamReview)
                && data.access.employee_id.is_some()
                && e.reports_to == data.access.employee_id)
    })
}
pub fn training(data: &Snapshot, id: Uuid) -> Result<&Training> {
    let t = data
        .learning
        .training
        .iter()
        .find(|t| t.id == id && t.company_id == data.company.id)
        .context("Training not found. Refresh.")?;
    ensure!(
        in_scope(data, t.employee_id),
        "Training is outside your current access."
    );
    Ok(t)
}
pub fn certificate(data: &Snapshot, id: Uuid) -> Result<&Certificate> {
    let c = data
        .learning
        .certificates
        .iter()
        .find(|c| c.id == id && c.company_id == data.company.id)
        .context("Certification not found. Refresh.")?;
    ensure!(
        in_scope(data, c.employee_id),
        "Certification is outside your current access."
    );
    Ok(c)
}
impl LearningAction {
    pub fn reason_mut(&mut self) -> &mut String {
        match self {
            Self::TrainingSave(d) => &mut d.reason,
            Self::CertificateSave(d) => &mut d.reason,
            Self::EvidenceSave(d) => &mut d.reason,
            Self::Progress { reason, .. }
            | Self::TrainingStatus { reason, .. }
            | Self::CertificateStatus { reason, .. }
            | Self::CertificateArchive { reason, .. } => reason,
        }
    }
    fn record(&self) -> Result<(&'static str, Value)> {
        Ok(match self {
            Self::TrainingSave(d) => {
                text(&d.title, 120, true, false)?;
                text(&d.provider, 120, false, false)?;
                let start = optional(&d.starts_on)?;
                let end = optional(&d.ends_on)?;
                let due = optional(&d.due_on)?;
                ensure!(
                    start.zip(end).is_none_or(|(a, b)| b >= a)
                        && start.zip(due).is_none_or(|(a, b)| b >= a),
                    "End/due dates cannot precede the start."
                );
                (
                    "training_save",
                    json!({"id":d.id,"expected_revision":d.original.as_ref().map(|t|t.revision),"employee_id":d.employee_id.context("Choose an employee.")?,"title":d.title.trim(),"provider":d.provider.trim(),"starts_on":start,"ends_on":end,"due_on":due,"required":d.required,"reason":d.reason.trim()}),
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
                    "training_progress",
                    json!({"id":id,"expected_revision":revision,"progress_note":note.trim(),"reason":reason.trim()}),
                )
            }
            Self::TrainingStatus {
                id,
                revision,
                status,
                completed_on,
                learning_minutes,
                outcome,
                reason,
            } => {
                text(outcome, 2000, false, true)?;
                ensure!(
                    learning_minutes.is_none_or(|n| n <= 600000),
                    "Use learning minutes between 0 and 600000, or leave blank."
                );
                (
                    "training_status",
                    json!({"id":id,"expected_revision":revision,"status":status,"completed_on":optional(completed_on)?,"learning_minutes":learning_minutes,"outcome":outcome.trim(),"reason":reason.trim()}),
                )
            }
            Self::CertificateSave(d) => {
                text(&d.title, 120, true, false)?;
                text(&d.issuer, 120, false, false)?;
                text(&d.reference_number, 120, false, false)?;
                let issue = parse_date(&d.issued_on)?;
                let expiry = optional(&d.expires_on)?;
                ensure!(
                    expiry.is_none_or(|end| end >= issue),
                    "Expiry cannot precede issue date."
                );
                (
                    "certificate_save",
                    json!({"id":d.id,"expected_revision":d.original.as_ref().map(|c|c.revision),"employee_id":d.employee_id.context("Choose an employee.")?,"title":d.title.trim(),"issuer":d.issuer.trim(),"reference_number":d.reference_number.trim(),"issued_on":issue,"expires_on":expiry,"reason":d.reason.trim()}),
                )
            }
            Self::CertificateStatus {
                id,
                revision,
                status,
                reason,
            } => (
                "certificate_status",
                json!({"id":id,"expected_revision":revision,"status":status,"reason":reason.trim()}),
            ),
            Self::CertificateArchive {
                id,
                revision,
                archived,
                reason,
            } => (
                "certificate_archive",
                json!({"id":id,"expected_revision":revision,"archived":archived,"reason":reason.trim()}),
            ),
            Self::EvidenceSave(d) => (
                "evidence_save",
                json!({"id":d.id,"expected_revision":d.original.as_ref().map(|e|e.revision),"certificate_id":d.certificate_id,"document_id":d.document_id,"reason":d.reason.trim()}),
            ),
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
            Self::TrainingSave(d) => {
                hr(data)?;
                let e = person(data, d.employee_id.unwrap())?;
                if let Some(old) = &d.original {
                    let t = training(data, d.id)?;
                    ensure!(
                        old.revision == t.revision && t.employee_id == e.id,
                        "Training changed. Refresh and reopen it."
                    );
                    ensure!(
                        t.completed_on
                            .zip(optional(&d.starts_on)?)
                            .is_none_or(|(c, s)| c >= s),
                        "Completion cannot precede the start."
                    );
                } else if !data.learning.training.iter().any(|t| t.id == d.id) {
                    ensure!(
                        e.status == EmployeeStatus::Active,
                        "New training requires an active employee."
                    );
                }
            }
            Self::Progress { id, revision, .. } => {
                let t = training(data, *id)?;
                ensure!(data.access.allows(crate::staff_roles::Capability::DevelopmentAdministration) || data.access.employee_id==Some(t.employee_id) || (data.access.allows(crate::staff_roles::Capability::TeamReview) && person(data,t.employee_id)?.reports_to==data.access.employee_id), "Only the employee, current Manager or development staff record progress.");
                ensure!(
                    t.revision == *revision
                        && ["planned", "in_progress"].contains(&t.status.as_str())
                        && person(data, t.employee_id)?.status == EmployeeStatus::Active,
                    "Training changed, is closed or the employee is archived. Refresh."
                );
            }
            Self::TrainingStatus {
                id,
                revision,
                status,
                completed_on,
                outcome,
                ..
            } => {
                hr(data)?;
                let t = training(data, *id)?;
                ensure!(
                    t.revision == *revision
                        && ["planned", "in_progress", "completed", "cancelled"]
                            .contains(&status.as_str())
                        && (status != &t.status || status == "completed"),
                    "Choose an available training status after Refresh."
                );
                if ["planned", "in_progress"].contains(&status.as_str()) {
                    ensure!(
                        person(data, t.employee_id)?.status == EmployeeStatus::Active,
                        "Archived employees cannot restart training."
                    );
                }
                let complete = optional(completed_on)?;
                if status == "completed" {
                    ensure!(
                        complete.is_some_and(
                            |c| c <= data.company.today() && t.starts_on.is_none_or(|s| c >= s)
                        ) && !outcome.trim().is_empty(),
                        "Completion needs a factual date on/before today, on/after the start, and an outcome."
                    );
                } else {
                    ensure!(
                        complete.is_none() && outcome.is_empty(),
                        "Completion details belong only to completed training."
                    );
                }
            }
            Self::CertificateSave(d) => {
                hr(data)?;
                let e = person(data, d.employee_id.unwrap())?;
                ensure!(
                    parse_date(&d.issued_on)? <= data.company.today(),
                    "Issue date cannot be in the future."
                );
                if let Some(old) = &d.original {
                    let c = certificate(data, d.id)?;
                    ensure!(
                        old.revision == c.revision && c.employee_id == e.id && !c.archived,
                        "Certification changed or is archived. Refresh/restore before editing."
                    );
                } else if !data.learning.certificates.iter().any(|c| c.id == d.id) {
                    ensure!(
                        e.status == EmployeeStatus::Active,
                        "New certification requires an active employee."
                    );
                }
            }
            Self::CertificateStatus {
                id,
                revision,
                status,
                ..
            } => {
                hr(data)?;
                let c = certificate(data, *id)?;
                ensure!(
                    c.revision == *revision
                        && !c.archived
                        && status != &c.status
                        && ["valid", "revoked"].contains(&status.as_str()),
                    "Choose a different status on an unarchived certification after Refresh."
                );
            }
            Self::CertificateArchive {
                id,
                revision,
                archived,
                ..
            } => {
                hr(data)?;
                let c = certificate(data, *id)?;
                ensure!(
                    c.revision == *revision && c.archived != *archived,
                    "Certification changed. Refresh and reopen it."
                );
            }
            Self::EvidenceSave(d) => {
                ensure!(data.access.allows(crate::staff_roles::Capability::PersonnelDocuments), "Certificate files are private to HR and Administrators.");
                let c = certificate(data, d.certificate_id)?;
                let retry =
                    d.original.is_none() && data.learning.evidence.iter().any(|e| e.id == d.id);
                if !retry {
                    ensure!(
                        !c.archived,
                        "Restore the certification before changing evidence."
                    );
                    if let Some(doc) = d.document_id {
                        ensure!(
                            data.documents.iter().any(|doc_row| doc_row.id == doc
                                && doc_row.company_id == data.company.id
                                && doc_row.employee_id == c.employee_id
                                && doc_row.state == crate::documents::DocumentState::Ready
                                && !doc_row.archived),
                            "Choose a ready, unarchived 201 document for the same employee."
                        );
                    }
                    if let Some(old) = &d.original {
                        let e = data
                            .learning
                            .evidence
                            .iter()
                            .find(|e| e.id == d.id && e.company_id == data.company.id)
                            .context("Evidence not found. Refresh.")?;
                        ensure!(
                            e.revision == old.revision && e.certificate_id == c.id,
                            "Evidence changed. Refresh."
                        );
                    } else {
                        ensure!(
                            !data.learning.evidence.iter().any(
                                |e| e.certificate_id == c.id && e.company_id == data.company.id
                            ),
                            "Evidence exists. Refresh and reopen it."
                        );
                    }
                }
            }
        }
        Ok(json!({"p_operation":op,"p_record":p}))
    }
    pub fn saved(&self, data: &Snapshot) -> bool {
        let Ok((op, p)) = self.record() else {
            return false;
        };
        let id = &p["id"];
        let row = match self {
            Self::TrainingSave(_) | Self::Progress { .. } | Self::TrainingStatus { .. } => data
                .learning
                .training
                .iter()
                .find(|r| json!(r.id) == *id && r.company_id == data.company.id)
                .and_then(|r| serde_json::to_value(r).ok()),
            Self::EvidenceSave(_) => data
                .learning
                .evidence
                .iter()
                .find(|r| json!(r.id) == *id && r.company_id == data.company.id)
                .and_then(|r| serde_json::to_value(r).ok()),
            _ => data
                .learning
                .certificates
                .iter()
                .find(|r| json!(r.id) == *id && r.company_id == data.company.id)
                .and_then(|r| serde_json::to_value(r).ok()),
        };
        let Some(row) = row else {
            return false;
        };
        let expected = p["expected_revision"].as_i64();
        if expected.is_some_and(|n| row["revision"].as_i64() != n.checked_add(1)) {
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
        match op {
            "training_save" => matches!(
                row["status"].as_str(),
                Some("planned" | "in_progress" | "completed" | "cancelled")
            ),
            "certificate_save" => row["status"].as_str().is_some(),
            _ => true,
        }
    }
}
pub fn visible_events(
    data: &Snapshot,
    training_id: Option<Uuid>,
    certificate_id: Option<Uuid>,
) -> Vec<&Event> {
    data.learning
        .events
        .iter()
        .filter(|e| {
            e.company_id == data.company.id
                && training_id.is_none_or(|id| e.training_id == Some(id))
                && certificate_id.is_none_or(|id| e.certificate_id == Some(id))
                && (data.access.allows(crate::staff_roles::Capability::PersonnelDocuments)
                    || (e.evidence_id.is_none()
                        && (e.training_id.is_some_and(|id| training(data, id).is_ok())
                            || e.certificate_id
                                .is_some_and(|id| certificate(data, id).is_ok()))))
        })
        .collect()
}
pub fn apply_demo(data: &mut Snapshot, action: &LearningAction, now: DateTime<Utc>) -> Result<()> {
    let payload = action.payload(data)?;
    let p = &payload["p_record"];
    let id: Uuid = serde_json::from_value(p["id"].clone())?;
    let birth = match action {
        LearningAction::TrainingSave(d) if d.original.is_none() => Some("training"),
        LearningAction::CertificateSave(d) if d.original.is_none() => Some("certificate"),
        LearningAction::EvidenceSave(d) if d.original.is_none() => Some("evidence"),
        _ => None,
    };
    let birth_key = birth.map(|k| format!("{k}:{id}"));
    if let Some(key) = &birth_key {
        if let Some((actor, old)) = data.learning.retries.get(key) {
            ensure!(
                *actor == data.access.user_id && old == &payload,
                "This record ID exists. Refresh."
            );
            return Ok(());
        }
    }
    let mut next = data.clone();
    let mut training_id = None;
    let mut certificate_id = None;
    let mut evidence_id = None;
    let before: Option<Value> = match action {
        LearningAction::TrainingSave(_)
        | LearningAction::Progress { .. }
        | LearningAction::TrainingStatus { .. } => data
            .learning
            .training
            .iter()
            .find(|r| r.id == id)
            .map(serde_json::to_value)
            .transpose()?,
        LearningAction::EvidenceSave(_) => data
            .learning
            .evidence
            .iter()
            .find(|r| r.id == id)
            .map(serde_json::to_value)
            .transpose()?,
        _ => data
            .learning
            .certificates
            .iter()
            .find(|r| r.id == id)
            .map(serde_json::to_value)
            .transpose()?,
    };
    ensure!(
        birth.is_none() || before.is_none(),
        "This record ID exists. Refresh."
    );
    let mut after=before.clone().unwrap_or_else(||json!({"id":id,"company_id":data.company.id,"revision":0,"created_by":data.access.user_id,"created_at":now,"updated_at":now}));
    if before.is_some() {
        after["revision"] = json!(before.as_ref().unwrap()["revision"].as_i64().unwrap() + 1);
        after["updated_at"] = json!(now);
    }
    let kind: String = match action {
        LearningAction::TrainingSave(d) => {
            let e = person(data, d.employee_id.unwrap())?;
            if before.is_none() {
                for(k,v)in json!({"employee_id":e.id,"employee_name":e.full_name,"employee_code":e.employee_code,"status":"planned","progress_note":"","completed_on":null,"learning_minutes":null,"outcome":""}).as_object().unwrap(){after[k]=v.clone();}
            }
            for k in [
                "title",
                "provider",
                "starts_on",
                "ends_on",
                "due_on",
                "required",
            ] {
                after[k] = p[k].clone();
            }
            training_id = Some(id);
            if before.is_none() {
                "training_created"
            } else {
                "training_edited"
            }
            .into()
        }
        LearningAction::Progress { .. } => {
            training_id = Some(id);
            after["progress_note"] = p["progress_note"].clone();
            "training_progress".into()
        }
        LearningAction::TrainingStatus { status, .. } => {
            training_id = Some(id);
            for k in ["status", "completed_on", "learning_minutes", "outcome"] {
                after[k] = p[k].clone();
            }
            format!("training_{status}")
        }
        LearningAction::CertificateSave(d) => {
            let e = person(data, d.employee_id.unwrap())?;
            if before.is_none() {
                for(k,v)in json!({"employee_id":e.id,"employee_name":e.full_name,"employee_code":e.employee_code,"status":"valid","archived":false}).as_object().unwrap(){after[k]=v.clone();}
            }
            for k in [
                "title",
                "issuer",
                "reference_number",
                "issued_on",
                "expires_on",
            ] {
                after[k] = p[k].clone();
            }
            certificate_id = Some(id);
            if before.is_none() {
                "certificate_created"
            } else {
                "certificate_edited"
            }
            .into()
        }
        LearningAction::CertificateStatus { status, .. } => {
            certificate_id = Some(id);
            after["status"] = p["status"].clone();
            format!("certificate_{status}")
        }
        LearningAction::CertificateArchive { archived, .. } => {
            certificate_id = Some(id);
            after["archived"] = p["archived"].clone();
            if *archived {
                "certificate_archived"
            } else {
                "certificate_restored"
            }
            .into()
        }
        LearningAction::EvidenceSave(d) => {
            certificate_id = Some(d.certificate_id);
            evidence_id = Some(id);
            if before.is_none() {
                after["employee_id"] = json!(certificate(data, d.certificate_id)?.employee_id);
                after["certificate_id"] = json!(d.certificate_id);
            }
            after["document_id"] = json!(d.document_id);
            if before.is_none() {
                "evidence_linked"
            } else if d.document_id.is_none() {
                "evidence_unlinked"
            } else {
                "evidence_replaced"
            }
            .into()
        }
    };
    if training_id.is_some() {
        let row: Training = serde_json::from_value(after.clone())?;
        if let Some(pos) = next.learning.training.iter().position(|r| r.id == id) {
            next.learning.training[pos] = row;
        } else {
            next.learning.training.push(row);
        }
    } else if evidence_id.is_some() {
        let row: Evidence = serde_json::from_value(after.clone())?;
        if let Some(pos) = next.learning.evidence.iter().position(|r| r.id == id) {
            next.learning.evidence[pos] = row;
        } else {
            next.learning.evidence.push(row);
        }
    } else {
        let row: Certificate = serde_json::from_value(after.clone())?;
        if let Some(pos) = next.learning.certificates.iter().position(|r| r.id == id) {
            next.learning.certificates[pos] = row;
        } else {
            next.learning.certificates.push(row);
        }
    }
    next.learning.events.push(Event {
        id: Uuid::new_v4(),
        company_id: data.company.id,
        training_id,
        certificate_id,
        evidence_id,
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
    if let Some(key) = birth_key {
        next.learning
            .retries
            .insert(key, (data.access.user_id, payload));
    }
    *data = next;
    Ok(())
}
