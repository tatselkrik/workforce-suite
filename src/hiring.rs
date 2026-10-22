//! Private applicant records and pipeline. No employee/Auth conversion.
use crate::{
    access::Access,
    domain::{Mode, Snapshot, parse_date},
};
use anyhow::{Context, Result, ensure};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    #[default]
    Applied,
    Screening,
    Interview,
    Offer,
    Rejected,
    Withdrawn,
}
impl Stage {
    pub const ALL: [Self; 6] = [
        Self::Applied,
        Self::Screening,
        Self::Interview,
        Self::Offer,
        Self::Rejected,
        Self::Withdrawn,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Applied => "Applied",
            Self::Screening => "Screening",
            Self::Interview => "Interview",
            Self::Offer => "Offer",
            Self::Rejected => "Rejected",
            Self::Withdrawn => "Withdrawn",
        }
    }
    pub fn terminal(self) -> bool {
        matches!(self, Self::Rejected | Self::Withdrawn)
    }
    pub fn change(self, next: Self, reason: &str) -> Result<()> {
        ensure!(self != next, "Choose a different application stage.");
        ensure!(
            !reason.trim().is_empty()
                && reason.trim().chars().count() <= 500
                && !reason.chars().any(char::is_control),
            "Enter a stage-change reason of 1–500 characters."
        );
        let allowed = if self.terminal() {
            next == Self::Applied
        } else {
            next.terminal()
                || matches!(
                    (self, next),
                    (Self::Applied, Self::Screening | Self::Interview)
                        | (Self::Screening, Self::Interview | Self::Offer)
                        | (Self::Interview, Self::Offer)
                )
        };
        ensure!(
            allowed,
            "This stage change is not available. Reopen a closed application at Applied."
        );
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Applicant {
    pub id: Uuid,
    pub company_id: Uuid,
    pub full_name: String,
    pub email: String,
    pub phone: String,
    pub position: String,
    pub source: String,
    pub applied_on: NaiveDate,
    pub notes: String,
    pub stage: Stage,
    pub archived: bool,
    pub revision: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub id: Uuid,
    pub company_id: Uuid,
    pub applicant_id: Uuid,
    pub revision: i64,
    pub kind: String,
    pub reason: String,
    pub actor_id: Uuid,
    pub actor_name: String,
    pub actor_role: crate::access::Role,
    pub before_record: Option<Applicant>,
    pub after_record: Applicant,
    pub recorded_at: chrono::DateTime<chrono::Utc>,
}
#[derive(Clone, Debug, Default)]
pub struct Setup {
    pub records: Vec<Applicant>,
    pub events: Vec<Event>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ApplicantDraft {
    pub id: Option<Uuid>,
    pub full_name: String,
    pub email: String,
    pub phone: String,
    pub position: String,
    pub source: String,
    pub applied_on: String,
    pub notes: String,
}
impl ApplicantDraft {
    pub fn existing(r: &Applicant) -> Self {
        Self {
            id: Some(r.id),
            full_name: r.full_name.clone(),
            email: r.email.clone(),
            phone: r.phone.clone(),
            position: r.position.clone(),
            source: r.source.clone(),
            applied_on: r.applied_on.to_string(),
            notes: r.notes.clone(),
        }
    }
    pub fn new(today: NaiveDate) -> Self {
        Self {
            id: Some(Uuid::new_v4()),
            applied_on: today.to_string(),
            ..Default::default()
        }
    }
    pub fn fields(&self, today: NaiveDate) -> Result<serde_json::Value> {
        self.validate(today)?;
        let id = self
            .id
            .context("Start an application draft before saving.")?;
        Ok(
            serde_json::json!({"id":id,"full_name":self.full_name.trim(),"email":self.email.trim(),"phone":self.phone.trim(),"position":self.position.trim(),"source":self.source.trim(),"applied_on":self.applied_on.trim(),"notes":self.notes.trim()}),
        )
    }
    pub fn validate(&self, today: NaiveDate) -> Result<()> {
        for (label, value, max, required) in [
            ("Full name", &self.full_name, 120, true),
            ("Email", &self.email, 254, false),
            ("Phone", &self.phone, 40, false),
            ("Position", &self.position, 100, true),
            ("Source", &self.source, 80, false),
        ] {
            ensure!(
                (!required || !value.trim().is_empty())
                    && value.trim().chars().count() <= max
                    && !value.chars().any(char::is_control),
                "{label} is required or exceeds its permitted length/format."
            );
        }
        if !self.email.trim().is_empty() {
            crate::account::validate_email(&self.email)?;
        }
        ensure!(
            self.notes.trim().chars().count() <= 2000
                && !self.notes.chars().any(|c| c.is_control() && c != '\n'),
            "Notes must be at most 2000 characters without unsupported control characters."
        );
        ensure!(
            parse_date(self.applied_on.trim())? <= today,
            "Application date cannot be in the future."
        );
        ensure!(
            self.id.is_none_or(|id| !id.is_nil()),
            "The applicant identifier is invalid."
        );
        Ok(())
    }
}
pub fn reason(value: &str) -> Result<()> {
    ensure!(
        !value.trim().is_empty()
            && value.trim().chars().count() <= 500
            && !value.chars().any(char::is_control),
        "Enter a change reason of 1–500 characters."
    );
    Ok(())
}
#[derive(Clone, Debug)]
pub enum HiringAction {
    Create {
        draft: ApplicantDraft,
        reason: String,
    },
    Edit {
        draft: ApplicantDraft,
        expected_revision: i64,
        reason: String,
    },
    Stage {
        id: Uuid,
        expected_revision: i64,
        next: Stage,
        reason: String,
    },
    Archive {
        id: Uuid,
        expected_revision: i64,
        archived: bool,
        reason: String,
    },
}
impl HiringAction {
    pub fn payload(&self, data: &Snapshot) -> Result<serde_json::Value> {
        authorize(&data.access, data)?;
        let (operation, record) = match self {
            Self::Create { draft, reason: r } => {
                reason(r)?;
                let mut f = draft.fields(data.company.today())?;
                f["reason"] = serde_json::json!(r.trim());
                ("create", f)
            }
            Self::Edit {
                draft,
                expected_revision,
                reason: r,
            } => {
                reason(r)?;
                let mut f = draft.fields(data.company.today())?;
                let id = draft.id.unwrap();
                let old = record(data, id)?;
                check_revision(old, *expected_revision)?;
                ensure!(!old.archived, "Restore this application before editing it.");
                f["expected_revision"] = serde_json::json!(expected_revision);
                f["reason"] = serde_json::json!(r.trim());
                ("edit", f)
            }
            Self::Stage {
                id,
                expected_revision,
                next,
                reason: r,
            } => (
                "stage",
                record(data, *id)?.stage_change(*expected_revision, *next, r)?,
            ),
            Self::Archive {
                id,
                expected_revision,
                archived,
                reason: r,
            } => {
                reason(r)?;
                let old = record(data, *id)?;
                check_revision(old, *expected_revision)?;
                ensure!(
                    old.archived != *archived,
                    "The application is already in that archive state."
                );
                (
                    if *archived { "archive" } else { "restore" },
                    serde_json::json!({"id":id,"expected_revision":expected_revision,"reason":r.trim()}),
                )
            }
        };
        Ok(
            serde_json::json!({"p_company_id":data.company.id,"p_operation":operation,"p_record":record}),
        )
    }
}
pub fn record(data: &Snapshot, id: Uuid) -> Result<&Applicant> {
    data.hiring
        .records
        .iter()
        .find(|r| r.id == id && r.company_id == data.company.id)
        .context("Application not found in your company. Refresh and reopen it.")
}
fn check_revision(r: &Applicant, expected: i64) -> Result<()> {
    ensure!(
        expected >= 0 && r.revision == expected,
        "The application changed. Refresh and reopen it before saving."
    );
    Ok(())
}
pub fn apply_demo(
    data: &mut Snapshot,
    action: &HiringAction,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<()> {
    let p = action.payload(data)?;
    let f = &p["p_record"];
    let id: Uuid = serde_json::from_value(f["id"].clone())?;
    let operation = p["p_operation"].as_str().unwrap();
    let old = data
        .hiring
        .records
        .iter()
        .find(|r| r.id == id && r.company_id == data.company.id)
        .cloned();
    let r = f["reason"].as_str().unwrap().to_owned();
    let mut saved = if operation == "create" {
        if let Some(existing) = old {
            let prior = data.hiring.events.iter().find(|e| {
                e.applicant_id == id
                    && e.company_id == data.company.id
                    && e.revision == 0
                    && e.actor_id == data.access.user_id
                    && e.reason == r
            });
            let mut expected = f.clone();
            expected.as_object_mut().unwrap().remove("reason");
            ensure!(
                prior.is_some_and(|e| ApplicantDraft::existing(&e.after_record)
                    .fields(data.company.today())
                    .is_ok_and(|p| p == expected)),
                "This application identifier already exists. Refresh before retrying."
            );
            let _ = existing;
            return Ok(());
        }
        Applicant {
            id,
            company_id: data.company.id,
            full_name: f["full_name"].as_str().unwrap().into(),
            email: f["email"].as_str().unwrap().into(),
            phone: f["phone"].as_str().unwrap().into(),
            position: f["position"].as_str().unwrap().into(),
            source: f["source"].as_str().unwrap().into(),
            applied_on: parse_date(f["applied_on"].as_str().unwrap())?,
            notes: f["notes"].as_str().unwrap().into(),
            stage: Stage::Applied,
            archived: false,
            revision: 0,
        }
    } else {
        old.clone().unwrap()
    };
    let kind = match operation {
        "create" => "created",
        "edit" => {
            saved.full_name = f["full_name"].as_str().unwrap().into();
            saved.email = f["email"].as_str().unwrap().into();
            saved.phone = f["phone"].as_str().unwrap().into();
            saved.position = f["position"].as_str().unwrap().into();
            saved.source = f["source"].as_str().unwrap().into();
            saved.applied_on = parse_date(f["applied_on"].as_str().unwrap())?;
            saved.notes = f["notes"].as_str().unwrap().into();
            "saved"
        }
        "stage" => {
            saved.stage = serde_json::from_value(f["stage"].clone())?;
            "stage_changed"
        }
        "archive" => {
            saved.archived = true;
            "archived"
        }
        "restore" => {
            saved.archived = false;
            "restored"
        }
        _ => unreachable!(),
    };
    if operation != "create" {
        saved.revision = saved
            .revision
            .checked_add(1)
            .context("Application revision limit reached.")?;
    }
    let event = Event {
        id: Uuid::new_v4(),
        company_id: data.company.id,
        applicant_id: id,
        revision: saved.revision,
        kind: kind.into(),
        reason: r,
        actor_id: data.access.user_id,
        actor_name: data
            .access
            .employee_id
            .map(|id| data.employee_name(id))
            .unwrap_or("HR staff")
            .into(),
        actor_role: data.access.role,
        before_record: old,
        after_record: saved.clone(),
        recorded_at: now,
    };
    if let Some(row) = data.hiring.records.iter_mut().find(|r| r.id == id) {
        *row = saved;
    } else {
        data.hiring.records.push(saved);
    }
    data.hiring.events.push(event);
    Ok(())
}
impl Applicant {
    pub fn stage_change(
        &self,
        expected_revision: i64,
        next: Stage,
        reason: &str,
    ) -> Result<serde_json::Value> {
        ensure!(
            !self.id.is_nil() && !self.company_id.is_nil(),
            "The application identity is invalid."
        );
        ensure!(
            expected_revision >= 0 && expected_revision == self.revision,
            "The application changed. Refresh and reopen it before saving."
        );
        ensure!(
            !self.archived,
            "Restore this application before changing its stage."
        );
        self.stage.change(next, reason)?;
        Ok(
            serde_json::json!({"id":self.id,"expected_revision":expected_revision,"stage":next,"reason":reason.trim()}),
        )
    }
}
pub fn authorize(access: &Access, data: &Snapshot) -> Result<()> {
    ensure!(
        data.mode != Mode::SignedOut
            && access.company_id == data.company.id
            && access.role.manages_hr(),
        "Only active HR staff and Administrators can access applicant records."
    );
    Ok(())
}
