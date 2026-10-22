//! Applicant files and scoped interviews. Employee accounts are not created here.
use crate::{
    access::Role,
    documents::{self, DocumentDraft, DocumentState, EmployeeDocument},
    domain::{Mode, Snapshot},
    hiring,
};
use anyhow::{Context, Result, ensure};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::HashMap, path::PathBuf};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Attachment {
    pub id: Uuid,
    pub company_id: Uuid,
    pub applicant_id: Uuid,
    #[serde(flatten)]
    pub details: documents::DocumentDetails,
    pub original_filename: String,
    pub mime_type: String,
    pub byte_size: usize,
    pub sha256: String,
    pub object_name: String,
    pub state: DocumentState,
    pub archived: bool,
    pub revision: i64,
    pub created_by: Uuid,
    pub created_by_name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
impl Attachment {
    /// Adapt only the shared file receipt for size/path/hash verification; authorization is separate.
    pub fn file_record(&self) -> Result<EmployeeDocument> {
        let mut value = serde_json::to_value(self)?;
        value["employee_id"] = value
            .as_object_mut()
            .unwrap()
            .remove("applicant_id")
            .unwrap();
        Ok(serde_json::from_value(value)?)
    }
    pub fn validate(&self) -> Result<()> {
        self.file_record()?.validate_file_record()
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AttachmentEvent {
    pub id: Uuid,
    pub sequence: i64,
    pub company_id: Uuid,
    pub applicant_id: Uuid,
    pub document_id: Uuid,
    pub kind: String,
    pub actor_id: Uuid,
    pub actor_name: String,
    pub actor_role: Role,
    pub before_record: Option<Attachment>,
    pub after_record: Attachment,
    pub recorded_at: DateTime<Utc>,
}
#[derive(Clone, Debug)]
pub struct ResumeDraft {
    pub document: DocumentDraft,
}
impl ResumeDraft {
    pub fn new(applicant: Uuid) -> Self {
        let mut document = DocumentDraft::new(applicant);
        document.title = "Résumé".into();
        Self { document }
    }
    pub fn existing(r: &Attachment) -> Result<Self> {
        Ok(Self {
            document: DocumentDraft::existing(&r.file_record()?),
        })
    }
    pub fn payload(&self) -> Result<Value> {
        let mut p = self.document.payload()?;
        p["applicant_id"] = p.as_object_mut().unwrap().remove("employee_id").unwrap();
        Ok(p)
    }
    pub fn prepared(&self, data: &Snapshot) -> Result<documents::PreparedUpload> {
        authorize_attachment(data, self.document.employee_id)?;
        ensure!(
            !hiring::record(data, self.document.employee_id)?.archived,
            "Restore the application before adding a file."
        );
        ensure!(
            self.document
                .original
                .as_ref()
                .is_none_or(|d| d.state == DocumentState::Pending),
            "Upload another file to replace a filed résumé."
        );
        let mut file = documents::prepare_file(
            self.document
                .file
                .as_ref()
                .context("Choose a résumé or application attachment.")?,
        )?;
        let mut p = self.payload()?;
        p.as_object_mut()
            .unwrap()
            .extend(file.payload.as_object().unwrap().clone());
        file.payload = p;
        Ok(file)
    }
}
#[derive(Clone, Debug)]
pub enum ResumeAction {
    Upload(ResumeDraft),
    Edit(ResumeDraft),
    Archive {
        id: Uuid,
        revision: i64,
        archived: bool,
    },
    Discard {
        id: Uuid,
    },
    Download {
        id: Uuid,
        destination: PathBuf,
    },
}
pub fn authorize_attachment(data: &Snapshot, applicant: Uuid) -> Result<()> {
    hiring::authorize(&data.access, data)?;
    hiring::record(data, applicant)?;
    Ok(())
}
pub fn attachment(data: &Snapshot, id: Uuid) -> Result<&Attachment> {
    let r = data
        .recruiting
        .documents
        .iter()
        .find(|r| r.id == id && r.company_id == data.company.id)
        .context("Attachment not found. Refresh this application.")?;
    authorize_attachment(data, r.applicant_id)?;
    Ok(r)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterviewStatus {
    #[default]
    Scheduled,
    Completed,
    Cancelled,
}
impl InterviewStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Scheduled => "Scheduled",
            Self::Completed => "Completed",
            Self::Cancelled => "Cancelled",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Interview {
    pub id: Uuid,
    pub company_id: Uuid,
    pub applicant_id: Uuid,
    pub interviewer_employee_id: Uuid,
    pub applicant_name: String,
    pub position: String,
    pub title: String,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub timezone: String,
    pub location: String,
    pub brief: String,
    pub status: InterviewStatus,
    pub revision: i64,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Recommendation {
    #[default]
    Proceed,
    Hold,
    NotRecommended,
}
impl Recommendation {
    pub const ALL: [Self; 3] = [Self::Proceed, Self::Hold, Self::NotRecommended];
    pub fn label(self) -> &'static str {
        match self {
            Self::Proceed => "Proceed",
            Self::Hold => "Hold",
            Self::NotRecommended => "Not recommended",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Feedback {
    pub id: Uuid,
    pub company_id: Uuid,
    pub interview_id: Uuid,
    pub author_id: Uuid,
    pub author_name: String,
    pub author_role: Role,
    pub rating: u32,
    pub recommendation: Recommendation,
    pub assessment: String,
    pub revision: i64,
    pub submitted_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InterviewEvent {
    pub id: Uuid,
    pub company_id: Uuid,
    pub interview_id: Uuid,
    pub kind: String,
    pub actor_id: Uuid,
    pub actor_name: String,
    pub actor_role: Role,
    pub reason: String,
    pub before_record: Option<Value>,
    pub after_record: Value,
    pub recorded_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Interviewer {
    pub employee_id: Uuid,
    pub full_name: String,
    pub role: Role,
}
#[derive(Clone, Debug, Default)]
pub struct Setup {
    pub documents: Vec<Attachment>,
    pub document_events: Vec<AttachmentEvent>,
    pub interviews: Vec<Interview>,
    pub feedback: Vec<Feedback>,
    pub events: Vec<InterviewEvent>,
    pub interviewers: Vec<Interviewer>,
}
#[derive(Clone, Debug)]
pub struct InterviewDraft {
    pub id: Uuid,
    pub applicant_id: Uuid,
    pub original: Option<Interview>,
    pub interviewer: Option<Uuid>,
    pub title: String,
    pub date: String,
    pub start: String,
    pub end: String,
    pub location: String,
    pub brief: String,
    pub reason: String,
}
impl InterviewDraft {
    pub fn new(data: &Snapshot, applicant: Uuid) -> Self {
        Self {
            id: Uuid::new_v4(),
            applicant_id: applicant,
            original: None,
            interviewer: None,
            title: "Interview".into(),
            date: data.company.today().to_string(),
            start: "09:00".into(),
            end: "10:00".into(),
            location: String::new(),
            brief: String::new(),
            reason: "Interview scheduled".into(),
        }
    }
    pub fn existing(data: &Snapshot, r: &Interview) -> Self {
        let start = r.starts_at.with_timezone(&data.company.tz());
        Self {
            id: r.id,
            applicant_id: r.applicant_id,
            original: Some(r.clone()),
            interviewer: Some(r.interviewer_employee_id),
            title: r.title.clone(),
            date: start.date_naive().to_string(),
            start: start.format("%H:%M").to_string(),
            end: r
                .ends_at
                .with_timezone(&data.company.tz())
                .format("%H:%M")
                .to_string(),
            location: r.location.clone(),
            brief: r.brief.clone(),
            reason: String::new(),
        }
    }
    pub fn payload(&self, data: &Snapshot) -> Result<Value> {
        hiring::authorize(&data.access, data)?;
        let app = hiring::record(data, self.applicant_id)?;
        ensure!(!app.archived, "Restore this application before scheduling.");
        hiring::reason(&self.reason)?;
        let interviewer = self
            .interviewer
            .context("Choose an active staff interviewer.")?;
        ensure!(
            data.recruiting
                .interviewers
                .iter()
                .any(|s| s.employee_id == interviewer && s.role.uses_desktop()),
            "Choose an active staff interviewer with desktop access."
        );
        text(&self.title, 120, true, false, "Interview title")?;
        text(&self.location, 240, false, false, "Location")?;
        text(&self.brief, 1000, false, true, "Interview brief")?;
        let date = crate::domain::parse_date(&self.date)?;
        let start = crate::corrections::local_time(date, &self.start, data.company.tz())?;
        let end = crate::corrections::local_time(date, &self.end, data.company.tz())?;
        ensure!(
            end > start && end <= start + Duration::hours(8),
            "Set an end time after the start, within eight hours on the selected date."
        );
        if let Some(old) = &self.original {
            let latest = interview(data, self.id)?;
            ensure!(
                latest.revision == old.revision && old.status == InterviewStatus::Scheduled,
                "This interview changed or is already closed. Refresh and reopen it."
            );
        }
        ensure!(
            (self.original.is_none() && data.recruiting.interviews.iter().any(|i| i.id == self.id))
                || !data.recruiting.interviews.iter().any(|i| i.id != self.id
                    && i.interviewer_employee_id == interviewer
                    && i.status == InterviewStatus::Scheduled
                    && i.starts_at < end
                    && start < i.ends_at),
            "The interviewer has another scheduled interview during that time."
        );
        let mut record = json!({"id":self.id,"applicant_id":self.applicant_id,"interviewer_employee_id":interviewer,"title":self.title.trim(),"starts_at":start,"ends_at":end,"timezone":data.company.timezone,"location":self.location.trim(),"brief":self.brief.trim(),"reason":self.reason.trim()});
        if let Some(old) = &self.original {
            record["expected_revision"] = json!(old.revision);
        }
        Ok(json!({"p_operation":"save","p_record":record}))
    }
}
#[derive(Clone, Debug)]
pub struct FeedbackDraft {
    pub interview_id: Uuid,
    pub expected_revision: i64,
    pub rating: u32,
    pub recommendation: Recommendation,
    pub assessment: String,
    pub reason: String,
}
impl FeedbackDraft {
    pub fn new(data: &Snapshot, id: Uuid) -> Self {
        let original = data
            .recruiting
            .feedback
            .iter()
            .find(|f| f.interview_id == id && f.author_id == data.access.user_id);
        Self {
            interview_id: id,
            expected_revision: original.map_or(0, |f| f.revision),
            rating: original.map_or(3, |f| f.rating),
            recommendation: original.map_or(Recommendation::Hold, |f| f.recommendation),
            assessment: original.map_or_else(String::new, |f| f.assessment.clone()),
            reason: if original.is_some() {
                String::new()
            } else {
                "Interview assessment recorded".into()
            },
        }
    }
}
#[derive(Clone, Debug)]
pub enum InterviewAction {
    Save(InterviewDraft),
    Status {
        id: Uuid,
        expected_revision: i64,
        status: InterviewStatus,
        reason: String,
    },
    Feedback(FeedbackDraft),
}
fn text(value: &str, max: usize, required: bool, multiline: bool, label: &str) -> Result<()> {
    ensure!(
        (!required || !value.trim().is_empty())
            && value.trim().chars().count() <= max
            && !value
                .chars()
                .any(|c| c.is_control() && !(multiline && c == '\n')),
        "{label} has missing text or an unsupported length/format."
    );
    Ok(())
}
pub fn interview(data: &Snapshot, id: Uuid) -> Result<&Interview> {
    let i = data
        .recruiting
        .interviews
        .iter()
        .find(|i| i.id == id && i.company_id == data.company.id)
        .context("Interview not found. Refresh your interviews.")?;
    ensure!(
        data.mode != Mode::SignedOut
            && data.access.company_id == data.company.id
            && (data.access.role.manages_hr()
                || (data.access.role == Role::Manager
                    && data.access.employee_id == Some(i.interviewer_employee_id))),
        "This interview is outside your access."
    );
    Ok(i)
}
impl InterviewAction {
    pub fn payload(&self, data: &Snapshot, now: DateTime<Utc>) -> Result<Value> {
        match self {
            Self::Save(d) => d.payload(data),
            Self::Status {
                id,
                expected_revision,
                status,
                reason,
            } => {
                hiring::authorize(&data.access, data)?;
                hiring::reason(reason)?;
                let r = interview(data, *id)?;
                ensure!(
                    r.status == InterviewStatus::Scheduled
                        && r.revision == *expected_revision
                        && *status != InterviewStatus::Scheduled,
                    "Refresh and choose a scheduled interview to complete or cancel."
                );
                ensure!(
                    *status != InterviewStatus::Completed || r.starts_at <= now,
                    "The interview has not started yet."
                );
                Ok(
                    json!({"p_operation":"status","p_record":{"id":id,"expected_revision":expected_revision,"status":status,"reason":reason.trim()}}),
                )
            }
            Self::Feedback(d) => {
                let r = interview(data, d.interview_id)?;
                ensure!(
                    data.access.employee_id == Some(r.interviewer_employee_id)
                        && r.status != InterviewStatus::Cancelled
                        && r.starts_at <= now,
                    "Only the assigned interviewer can submit feedback after it starts."
                );
                let old = data
                    .recruiting
                    .feedback
                    .iter()
                    .find(|f| f.interview_id == r.id && f.author_id == data.access.user_id);
                ensure!(
                    d.expected_revision >= 0
                        && d.expected_revision == old.map_or(0, |f| f.revision),
                    "Feedback changed. Refresh and reopen it."
                );
                ensure!((1..=5).contains(&d.rating), "Choose a rating from 1 to 5.");
                text(&d.assessment, 2000, true, true, "Assessment")?;
                hiring::reason(&d.reason)?;
                Ok(
                    json!({"p_operation":"feedback","p_record":{"id":r.id,"expected_revision":d.expected_revision,"rating":d.rating,"recommendation":d.recommendation,"assessment":d.assessment.trim(),"reason":d.reason.trim()}}),
                )
            }
        }
    }
}
pub fn apply_demo(data: &mut Snapshot, action: &InterviewAction, now: DateTime<Utc>) -> Result<()> {
    let payload = action.payload(data, now)?;
    let p = &payload["p_record"];
    let id: Uuid = serde_json::from_value(p["id"].clone())?;
    let before;
    let after;
    let kind;
    let name = data
        .own_employee()
        .map(|e| e.full_name.clone())
        .unwrap_or_else(|| "Staff".into());
    match action {
        InterviewAction::Save(d) => {
            let old = data
                .recruiting
                .interviews
                .iter()
                .find(|i| i.id == id)
                .cloned();
            let app = hiring::record(data, d.applicant_id)?;
            let saved = Interview {
                id,
                company_id: data.company.id,
                applicant_id: d.applicant_id,
                interviewer_employee_id: d.interviewer.unwrap(),
                applicant_name: old
                    .as_ref()
                    .map_or_else(|| app.full_name.clone(), |r| r.applicant_name.clone()),
                position: old
                    .as_ref()
                    .map_or_else(|| app.position.clone(), |r| r.position.clone()),
                title: d.title.trim().into(),
                starts_at: serde_json::from_value(p["starts_at"].clone())?,
                ends_at: serde_json::from_value(p["ends_at"].clone())?,
                timezone: data.company.timezone.clone(),
                location: d.location.trim().into(),
                brief: d.brief.trim().into(),
                status: InterviewStatus::Scheduled,
                revision: old.as_ref().map_or(0, |r| r.revision + 1),
                created_by: old.as_ref().map_or(data.access.user_id, |r| r.created_by),
                created_at: old.as_ref().map_or(now, |r| r.created_at),
                updated_at: now,
            };
            if old.is_some() && d.original.is_none() {
                let prior = data.recruiting.events.iter().find(|e| {
                    e.interview_id == id
                        && e.kind == "scheduled"
                        && e.actor_id == data.access.user_id
                        && e.reason == d.reason.trim()
                });
                ensure!(
                    prior.is_some_and(|e| {
                        [
                            "applicant_id",
                            "interviewer_employee_id",
                            "title",
                            "starts_at",
                            "ends_at",
                            "timezone",
                            "location",
                            "brief",
                        ]
                        .iter()
                        .all(|key| {
                            e.after_record[*key] == serde_json::to_value(&saved).unwrap()[*key]
                        })
                    }),
                    "This interview identifier already exists."
                );
                return Ok(());
            }
            before = old.as_ref().map(serde_json::to_value).transpose()?;
            after = serde_json::to_value(&saved)?;
            kind = if old.is_some() {
                "rescheduled"
            } else {
                "scheduled"
            };
            if let Some(r) = data.recruiting.interviews.iter_mut().find(|r| r.id == id) {
                *r = saved;
            } else {
                data.recruiting.interviews.push(saved);
            }
        }
        InterviewAction::Status { status, .. } => {
            let old = interview(data, id)?.clone();
            let mut saved = old.clone();
            saved.status = *status;
            saved.revision += 1;
            saved.updated_at = now;
            before = Some(serde_json::to_value(old)?);
            after = serde_json::to_value(&saved)?;
            kind = if *status == InterviewStatus::Completed {
                "completed"
            } else {
                "cancelled"
            };
            *data
                .recruiting
                .interviews
                .iter_mut()
                .find(|r| r.id == id)
                .unwrap() = saved;
        }
        InterviewAction::Feedback(d) => {
            let old = data
                .recruiting
                .feedback
                .iter()
                .find(|f| f.interview_id == id && f.author_id == data.access.user_id)
                .cloned();
            let saved = Feedback {
                id: old.as_ref().map_or_else(Uuid::new_v4, |f| f.id),
                company_id: data.company.id,
                interview_id: id,
                author_id: data.access.user_id,
                author_name: old
                    .as_ref()
                    .map_or_else(|| name.clone(), |r| r.author_name.clone()),
                author_role: old.as_ref().map_or(data.access.role, |r| r.author_role),
                rating: d.rating,
                recommendation: d.recommendation,
                assessment: d.assessment.trim().into(),
                revision: d.expected_revision + 1,
                submitted_at: old.as_ref().map_or(now, |f| f.submitted_at),
                updated_at: now,
            };
            before = old.as_ref().map(serde_json::to_value).transpose()?;
            after = serde_json::to_value(&saved)?;
            kind = "feedback_saved";
            if let Some(f) = data
                .recruiting
                .feedback
                .iter_mut()
                .find(|f| f.id == saved.id)
            {
                *f = saved;
            } else {
                data.recruiting.feedback.push(saved);
            }
        }
    }
    data.recruiting.events.push(InterviewEvent {
        id: Uuid::new_v4(),
        company_id: data.company.id,
        interview_id: id,
        kind: kind.into(),
        actor_id: data.access.user_id,
        actor_name: name,
        actor_role: data.access.role,
        reason: p["reason"].as_str().unwrap().into(),
        before_record: before,
        after_record: after,
        recorded_at: now,
    });
    Ok(())
}
pub fn resume_demo(
    data: &mut Snapshot,
    files: &mut HashMap<Uuid, Vec<u8>>,
    action: ResumeAction,
) -> Result<String> {
    // The existing document engine supplies byte validation, immutable files and audit semantics.
    let mut scoped = data.clone();
    scoped.documents = data
        .recruiting
        .documents
        .iter()
        .map(Attachment::file_record)
        .collect::<Result<_>>()?;
    scoped.document_events = Vec::new();
    let applicant = match &action {
        ResumeAction::Upload(d) | ResumeAction::Edit(d) => d.document.employee_id,
        ResumeAction::Archive { id, .. }
        | ResumeAction::Discard { id }
        | ResumeAction::Download { id, .. } => attachment(data, *id)?.applicant_id,
    };
    authorize_attachment(data, applicant)?;
    let app = hiring::record(data, applicant)?;
    if matches!(&action, ResumeAction::Upload(_)) {
        ensure!(
            !app.archived,
            "Restore the application before adding a file."
        );
    }
    if let ResumeAction::Edit(d) = &action {
        ensure!(
            attachment(data, d.document.id)?.applicant_id == applicant,
            "Reopen this application's attachment."
        );
    }
    let mut owner = scoped
        .employees
        .first()
        .cloned()
        .context("Demo employee fixture unavailable.")?;
    owner.id = app.id;
    owner.company_id = app.company_id;
    owner.full_name = app.full_name.clone();
    scoped.employees.push(owner);
    let action = match action {
        ResumeAction::Upload(d) => documents::DocumentAction::Upload(d.document),
        ResumeAction::Edit(d) => documents::DocumentAction::Edit(d.document),
        ResumeAction::Archive {
            id,
            revision,
            archived,
        } => documents::DocumentAction::Archive {
            id,
            revision,
            archived,
        },
        ResumeAction::Discard { id } => documents::DocumentAction::Discard { id },
        ResumeAction::Download { id, destination } => {
            documents::DocumentAction::Download { id, destination }
        }
    };
    let message = documents::execute_demo(&mut scoped, files, action)?;
    for r in scoped.documents {
        let mut v = serde_json::to_value(r)?;
        v["applicant_id"] = v.as_object_mut().unwrap().remove("employee_id").unwrap();
        let doc: Attachment = serde_json::from_value(v)?;
        if let Some(old) = data
            .recruiting
            .documents
            .iter_mut()
            .find(|old| old.id == doc.id)
        {
            *old = doc;
        } else {
            data.recruiting.documents.push(doc);
        }
    }
    for event in scoped.document_events {
        let mut v = serde_json::to_value(event)?;
        v["applicant_id"] = v.as_object_mut().unwrap().remove("employee_id").unwrap();
        for key in ["before_record", "after_record"] {
            if v[key].is_object() {
                v[key]["applicant_id"] = v[key]
                    .as_object_mut()
                    .unwrap()
                    .remove("employee_id")
                    .unwrap();
            }
        }
        data.recruiting
            .document_events
            .push(serde_json::from_value(v)?);
    }
    Ok(message.replace("employee document", "application attachment"))
}
