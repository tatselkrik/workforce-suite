//! Attendance arrangements and work reporting remain independent from recorded punches and pay.
use crate::{domain::Snapshot, staff_roles::Capability};
use anyhow::{Result, ensure};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Policy {
    pub id: Uuid,
    pub company_id: Uuid,
    pub employee_id: Uuid,
    pub starts_on: NaiveDate,
    pub ends_on: Option<NaiveDate>,
    pub arrangement: String,
    pub office_days: Vec<u8>,
    pub enabled: bool,
    pub reason: String,
    pub sequence: i64,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Task {
    pub id: Uuid,
    pub company_id: Uuid,
    pub employee_id: Uuid,
    pub title: String,
    pub outcome: String,
    pub due_on: NaiveDate,
    pub priority: String,
    pub status: String,
    pub progress: String,
    pub review_note: String,
    pub revision: i64,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReportBody {
    pub summary: String,
    pub completed: String,
    pub unfinished: String,
    pub blockers: String,
    pub next_steps: String,
}
impl ReportBody {
    pub fn validate(&self, submitting: bool) -> Result<()> {
        for field in [
            &self.summary,
            &self.completed,
            &self.unfinished,
            &self.blockers,
            &self.next_steps,
        ] {
            ensure!(
                field.chars().count() <= 4000,
                "Keep each report field to 4,000 characters."
            );
        }
        ensure!(
            !submitting || !self.summary.trim().is_empty(),
            "Enter a work summary before submitting."
        );
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DailyReport {
    pub id: Uuid,
    pub company_id: Uuid,
    pub employee_id: Uuid,
    pub work_date: NaiveDate,
    pub status: String,
    pub revision: i64,
    pub submission: i32,
    pub body: ReportBody,
    #[serde(default)]
    pub task_ids: Vec<Uuid>,
    pub review_note: String,
    pub updated_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Submission {
    pub report_id: Uuid,
    pub version: i32,
    pub company_id: Uuid,
    pub employee_id: Uuid,
    pub body: ReportBody,
    pub task_snapshot: Vec<Task>,
    pub submitted_by: Uuid,
    pub submitted_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Context {
    pub version: String,
    pub company_id: Uuid,
    pub user_id: Uuid,
    pub employee_id: Uuid,
    pub own_only: bool,
    pub can_administer: bool,
    pub policies: Vec<Policy>,
    pub tasks: Vec<Task>,
    pub reports: Vec<DailyReport>,
    pub submissions: Vec<Submission>,
    pub events: Vec<Value>,
    #[serde(default)]
    pub stations: Vec<Value>,
    #[serde(skip)]
    pub receipt: Option<Value>,
    #[serde(skip)]
    pub images: Vec<EvidenceImage>,
}
#[derive(Clone)]
pub struct EvidenceImage {
    pub id: Uuid,
    pub employee_id: Uuid,
    pub attendance_id: Uuid,
    pub operation: String,
    pub recorded_at: String,
    pub station: String,
    pub bytes: Vec<u8>,
}
impl std::fmt::Debug for EvidenceImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EvidenceImage")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}
impl Context {
    pub fn empty(data: &Snapshot) -> Self {
        Self {
            version: "workforce-attendance-v1".into(),
            company_id: data.company.id,
            user_id: data.access.user_id,
            employee_id: data.access.employee_id.unwrap_or_default(),
            own_only: false,
            can_administer: data.access.allows(Capability::AttendanceAdministration),
            policies: vec![],
            tasks: vec![],
            reports: vec![],
            submissions: vec![],
            events: vec![],
            stations: vec![],
            receipt: None,
            images: vec![],
        }
    }
    pub fn validate(&self, data: &Snapshot) -> Result<()> {
        ensure!(
            self.version == "workforce-attendance-v1"
                && self.company_id == data.company.id
                && self.user_id == data.access.user_id
                && Some(self.employee_id) == data.access.employee_id
                && !self.own_only,
            "Work records do not match your current account."
        );
        ensure!(
            !self.can_administer || data.access.allows(Capability::AttendanceAdministration),
            "Attendance permissions changed."
        );
        ensure!(
            self.policies.len() <= 5000
                && self.tasks.len() <= 5000
                && self.reports.len() <= 5000
                && self.submissions.len() <= 10000
                && self.events.len() <= 1000,
            "Narrow the work record history."
        );
        let scope = |company: Uuid, employee: Uuid| {
            company == self.company_id
                && data.employees.iter().any(|e| {
                    e.id == employee
                        && e.company_id == company
                        && (Some(e.id) == data.access.employee_id
                            || data.access.allows(Capability::AttendanceAdministration)
                            || (data.access.allows(Capability::TeamReview)
                                && e.reports_to == data.access.employee_id
                                && e.status == crate::domain::EmployeeStatus::Active))
                })
        };
        for p in &self.policies {
            ensure!(
                scope(p.company_id, p.employee_id)
                    && !p.id.is_nil()
                    && matches!(p.arrangement.as_str(), "onsite" | "remote" | "hybrid")
                    && p.office_days.len() <= 7
                    && p.office_days.iter().all(|d| (1..=7).contains(d))
                    && p.ends_on.is_none_or(|end| end >= p.starts_on),
                "Invalid work-arrangement projection."
            );
        }
        for t in &self.tasks {
            ensure!(
                scope(t.company_id, t.employee_id)
                    && !t.id.is_nil()
                    && t.revision >= 0
                    && matches!(
                        t.status.as_str(),
                        "assigned" | "in_progress" | "submitted" | "accepted" | "returned"
                    )
                    && t.title.chars().count() <= 200
                    && t.outcome.chars().count() <= 4000,
                "Invalid task projection."
            );
        }
        for r in &self.reports {
            ensure!(
                scope(r.company_id, r.employee_id)
                    && !r.id.is_nil()
                    && r.revision >= 0
                    && matches!(
                        r.status.as_str(),
                        "draft" | "submitted" | "returned" | "approved"
                    ),
                "Invalid daily report projection."
            );
            r.body.validate(false)?;
        }
        for s in &self.submissions {
            ensure!(
                scope(s.company_id, s.employee_id)
                    && s.version > 0
                    && s.task_snapshot
                        .iter()
                        .all(|t| t.company_id == s.company_id && t.employee_id == s.employee_id),
                "Invalid submitted work evidence."
            );
            s.body.validate(true)?;
        }
        for e in &self.events {
            ensure!(
                e.get("company_id")
                    .and_then(Value::as_str)
                    .and_then(|v| Uuid::parse_str(v).ok())
                    == Some(self.company_id)
                    && e.get("employee_id")
                        .and_then(Value::as_str)
                        .and_then(|v| Uuid::parse_str(v).ok())
                        .is_some_and(|id| scope(self.company_id, id)),
                "Work history is outside your current scope."
            );
        }
        ensure!(
            self.stations.len() <= 100
                && (self.stations.is_empty()
                    || data.access.allows(Capability::AccountAdministration)),
            "Station scope changed."
        );
        for station in &self.stations {
            ensure!(
                station["company_id"] == self.company_id.to_string()
                    && station["id"]
                        .as_str()
                        .and_then(|v| Uuid::parse_str(v).ok())
                        .is_some(),
                "Invalid station scope."
            );
        }
        ensure!(
            self.images.len() <= 32
                && (self.images.is_empty()
                    || data.access.allows(Capability::AttendanceAdministration)),
            "Photo review permissions changed."
        );
        for image in &self.images {
            ensure!(
                scope(self.company_id, image.employee_id)
                    && image.bytes.len() <= 65536
                    && data
                        .attendance
                        .iter()
                        .any(|a| a.id == image.attendance_id && a.employee_id == image.employee_id),
                "Photo does not match the current attendance record."
            );
        }
        Ok(())
    }
    pub fn policy(&self, employee: Uuid, day: NaiveDate) -> Option<&Policy> {
        self.policies
            .iter()
            .filter(|p| {
                p.employee_id == employee
                    && p.enabled
                    && p.starts_on <= day
                    && p.ends_on.is_none_or(|end| end >= day)
            })
            .max_by_key(|p| (p.starts_on, p.sequence))
    }
}

#[derive(Debug)]
pub struct Rejected(pub String);
impl std::fmt::Display for Rejected {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl std::error::Error for Rejected {}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn daily_report_requires_a_summary_only_on_submission() {
        let mut body = ReportBody::default();
        assert!(body.validate(false).is_ok());
        assert!(body.validate(true).is_err());
        body.summary = "Completed work and recorded an unresolved blocker".into();
        body.unfinished = "Carry remaining work to tomorrow".into();
        assert!(body.validate(true).is_ok());
        body.blockers = "a".repeat(4001);
        assert!(body.validate(false).is_err());
    }
    #[test]
    fn report_body_rejects_hidden_identity_fields() {
        assert!(serde_json::from_value::<ReportBody>(serde_json::json!({"summary":"done","completed":"","unfinished":"","blockers":"","next_steps":"","employee_id":"forged"})).is_err());
    }
}
