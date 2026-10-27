//! Versioned, credential-free native staff provisioning and exact recovery payloads.
use super::{Capability, SCHEMA_VERSION, StaffRoles, admin::Receipt};
use crate::{
    access::{Access, Role},
    contacts::EmployeeContacts,
    domain::{Employee, EmployeeDraft, EmployeeStatus},
};
use anyhow::{Context as _, Result, ensure};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use serde_json::Value;
use std::collections::BTreeSet;
use uuid::Uuid;

pub fn require_admin(actor: &Access) -> Result<()> {
    ensure!(
        actor.role == Role::Administrator
            && actor.allows(Capability::AccountAdministration)
            && actor.staff_contract_version == SCHEMA_VERSION,
        "Current protected Administrator access is required."
    );
    let context = actor
        .staff_context
        .as_ref()
        .context("Refresh protected staff access first.")?;
    context.validate(actor)?;
    ensure!(
        context.runtime_ready,
        "Staff provisioning is not enabled until the permission rollout is verified."
    );
    Ok(())
}
fn valid_hash(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Person {
    pub id: Option<Uuid>,
    pub company_id: Uuid,
    pub employee_code: String,
    pub full_name: String,
    pub department: String,
    pub job_title: String,
    pub joined_on: Option<NaiveDate>,
    pub reports_to: Option<Uuid>,
    pub employment_type_id: Option<Uuid>,
    pub work_location_id: Option<Uuid>,
    pub annual_leave_days: u32,
    pub status: EmployeeStatus,
}
impl From<&Employee> for Person {
    fn from(e: &Employee) -> Self {
        Self {
            id: Some(e.id),
            company_id: e.company_id,
            employee_code: e.employee_code.clone(),
            full_name: e.full_name.clone(),
            department: e.department.clone(),
            job_title: e.job_title.clone(),
            joined_on: e.joined_on,
            reports_to: e.reports_to,
            employment_type_id: e.employment_type_id,
            work_location_id: e.work_location_id,
            annual_leave_days: e.annual_leave_days,
            status: e.status,
        }
    }
}
impl Person {
    pub fn from_draft(draft: &EmployeeDraft, company: Uuid) -> Result<Self> {
        let mut result = Self::from(&draft.employee(company)?);
        result.id = draft.id;
        Ok(result)
    }
    pub fn record(&self) -> Employee {
        Employee {
            id: self.id.unwrap_or_else(Uuid::nil),
            company_id: self.company_id,
            employee_code: self.employee_code.clone(),
            full_name: self.full_name.clone(),
            department: self.department.clone(),
            job_title: self.job_title.clone(),
            joined_on: self.joined_on,
            reports_to: self.reports_to,
            employment_type_id: self.employment_type_id,
            work_location_id: self.work_location_id,
            annual_leave_days: self.annual_leave_days,
            status: self.status,
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub id: Uuid,
    pub email: String,
    pub roles: StaffRoles,
    pub active: bool,
    pub reason: String,
    pub employee: Person,
    pub contacts: EmployeeContacts,
    pub expected_source: Option<String>,
    pub supersedes: Option<Uuid>,
}
impl Input {
    fn validate(&self) -> Result<()> {
        ensure!(
            !self.id.is_nil()
                && !self.employee.company_id.is_nil()
                && self.employee.id.is_none_or(|id| !id.is_nil()),
            "A valid request and employee identity are required."
        );
        crate::account::validate_email(self.email.trim())?;
        ensure!(
            self.reason == self.reason.trim()
                && (1..=500).contains(&self.reason.chars().count())
                && !self.reason.chars().any(char::is_control),
            "Enter a reason of 1–500 characters without control characters."
        );
        ensure!(
            self.supersedes
                .is_none_or(|id| !id.is_nil() && id != self.id),
            "A replacement must refer to a different saved attempt."
        );
        ensure!(
            self.contacts.company_id == self.employee.company_id
                && (self.contacts.employee_id.is_nil()
                    || Some(self.contacts.employee_id) == self.employee.id),
            "Contacts belong to another employee or company."
        );
        self.contacts.normalized()?;
        let record = self.employee.record();
        EmployeeDraft::from(&record).employee(self.employee.company_id)?;
        ensure!(
            self.employee.status == EmployeeStatus::Active
                || (!self.active && self.roles == StaffRoles::default()),
            "Restore the employee before assigning staff access."
        );
        ensure!(
            if self.employee.id.is_some() {
                self.expected_source.as_deref().is_some_and(valid_hash)
            } else {
                self.expected_source.is_none()
            },
            "An existing employee requires its reviewed saved source; a new person has none."
        );
        Ok(())
    }
}
/// Keep the original JSON, including array order and timestamp spelling, for DB
/// actor/full-payload equality. Parsed fields are never reserialized for retries.
#[derive(Clone)]
pub struct Request {
    raw: Value,
    input: Input,
}
impl std::fmt::Debug for Request {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StaffProvisionRequest")
            .field("id", &self.input.id)
            .finish_non_exhaustive()
    }
}
impl Request {
    pub fn new(input: Input) -> Result<Self> {
        Self::from_value(serde_json::to_value(input)?)
    }
    pub fn from_value(raw: Value) -> Result<Self> {
        ensure!(
            serde_json::to_vec(&raw)?.len() <= 24576,
            "The credential-free account request is too large."
        );
        let input: Input =
            serde_json::from_value(raw.clone()).context("Unsupported staff account payload.")?;
        let contacts = raw
            .get("contacts")
            .and_then(Value::as_object)
            .context("Contact details are required.")?;
        ensure!(
            contacts.keys().all(|k| matches!(
                k.as_str(),
                "company_id"
                    | "employee_id"
                    | "phone"
                    | "personal_email"
                    | "address"
                    | "emergency_name"
                    | "emergency_relationship"
                    | "emergency_phone"
                    | "emergency_alternate_phone"
                    | "updated_at"
            )),
            "Unsupported contact fields in the saved account request."
        );
        input.validate()?;
        Ok(Self { raw, input })
    }
    pub fn data(&self) -> &Input {
        &self.input
    }
    pub fn validate(&self, actor: &Access) -> Result<()> {
        require_admin(actor)?;
        self.input.validate()?;
        ensure!(
            self.input.employee.company_id == actor.company_id,
            "This account request belongs to another company."
        );
        Ok(())
    }
}
impl Serialize for Request {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        self.raw.serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for Request {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        Self::from_value(Value::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub schema_version: u16,
    pub company_id: Uuid,
    pub actor_id: Uuid,
    pub runtime_ready: bool,
    pub employee_id: Uuid,
    pub source_hash: String,
    pub employee: Employee,
    pub contacts: Option<EmployeeContacts>,
}
impl Source {
    pub fn validate(&self, actor: &Access) -> Result<()> {
        require_admin(actor)?;
        ensure!(
            self.schema_version == SCHEMA_VERSION
                && self.company_id == actor.company_id
                && self.actor_id == actor.user_id
                && self.runtime_ready
                && !self.employee_id.is_nil()
                && self.employee.id == self.employee_id
                && self.employee.company_id == actor.company_id
                && valid_hash(&self.source_hash),
            "Employee source does not match the protected Administrator and company."
        );
        if let Some(c) = &self.contacts {
            ensure!(
                c.company_id == actor.company_id && c.employee_id == self.employee_id,
                "Employee source contains another person's contacts."
            );
            c.normalized()?;
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Reserved,
    Committed,
    Superseded,
    Cancelled,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Intent {
    pub schema_version: u16,
    pub id: Uuid,
    pub company_id: Uuid,
    pub actor_id: Uuid,
    pub user_id: Uuid,
    pub auth_preexisting: bool,
    pub email: String,
    pub state: Stage,
    pub receipt: Option<Receipt>,
}
impl Intent {
    pub fn validate(&self, actor: &Access, request: &Request) -> Result<()> {
        request.validate(actor)?;
        ensure!(
            self.schema_version == SCHEMA_VERSION
                && self.id == request.data().id
                && self.company_id == actor.company_id
                && self.actor_id == actor.user_id
                && !self.user_id.is_nil()
                && self.user_id != actor.user_id
                && self.email == request.data().email.trim().to_lowercase(),
            "Provisioning identity does not match the current actor and original request."
        );
        match (self.state, self.receipt.as_ref()) {
            (Stage::Committed, Some(receipt)) => {
                validate_receipt(receipt, actor, request, Some(self.user_id))?
            }
            (Stage::Reserved | Stage::Cancelled | Stage::Superseded, None) => {}
            _ => anyhow::bail!("Provisioning state has no matching immutable receipt."),
        };
        Ok(())
    }
}
pub fn validate_receipt(
    receipt: &Receipt,
    actor: &Access,
    request: &Request,
    user: Option<Uuid>,
) -> Result<()> {
    request.validate(actor)?;
    let input = request.data();
    ensure!(
        receipt.schema_version == SCHEMA_VERSION
            && receipt.id == input.id
            && receipt.company_id == actor.company_id
            && receipt.actor_id == actor.user_id
            && !receipt.user_id.is_nil()
            && receipt.user_id != actor.user_id
            && user.is_none_or(|id| id == receipt.user_id)
            && !receipt.employee_id.is_nil()
            && input.employee.id.is_none_or(|id| id == receipt.employee_id),
        "Provisioning receipt belongs to another actor, account or employee."
    );
    ensure!(
        receipt.revision == 1
            && receipt.roles == input.roles
            && receipt.active == input.active
            && receipt.base_role == input.roles.compatible_base_role(),
        "Provisioning receipt does not match the reviewed responsibilities and sign-in status."
    );
    Ok(())
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Completion {
    pub message: String,
    pub receipt: Receipt,
    pub confirmation_sent: bool,
    pub replayed: bool,
}
impl Completion {
    pub fn validate(&self, actor: &Access, request: &Request) -> Result<()> {
        ensure!(
            !self.confirmation_sent,
            "Confirmation delivery must remain an explicit account action."
        );
        validate_receipt(&self.receipt, actor, request, None)
    }
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingItem {
    pub intent: Intent,
    pub request: Request,
    pub created_at: DateTime<Utc>,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pending {
    pub schema_version: u16,
    pub company_id: Uuid,
    pub actor_id: Uuid,
    pub entry_count: usize,
    pub entries: Vec<PendingItem>,
}
impl Pending {
    pub fn validate(&self, actor: &Access) -> Result<()> {
        require_admin(actor)?;
        ensure!(
            self.schema_version == SCHEMA_VERSION
                && self.company_id == actor.company_id
                && self.actor_id == actor.user_id
                && self.entry_count == self.entries.len()
                && self.entry_count <= 100,
            "The pending account view is incomplete, oversized or belongs to another actor."
        );
        let mut ids = BTreeSet::new();
        let mut users = BTreeSet::new();
        for item in &self.entries {
            item.intent.validate(actor, &item.request)?;
            ensure!(
                item.intent.state == Stage::Reserved
                    && ids.insert(item.intent.id)
                    && users.insert(item.intent.user_id),
                "Pending account attempts contain a closed or duplicate identity."
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::staff_roles::{Context, StaffRole};
    use serde_json::json;
    pub(super) fn fixture() -> (Access, Request) {
        let mut actor = Access {
            company_id: Uuid::from_u128(1),
            user_id: Uuid::from_u128(2),
            employee_id: Some(Uuid::from_u128(3)),
            role: Role::Administrator,
            staff_contract_version: 1,
            ..Default::default()
        };
        actor
            .bind_staff_context(Context {
                schema_version: 1,
                company_id: actor.company_id,
                user_id: actor.user_id,
                employee_id: actor.employee_id.unwrap(),
                base_role: Role::Administrator,
                revision: 1,
                runtime_ready: true,
                roles: StaffRoles::legacy(Role::Administrator),
            })
            .unwrap();
        let person = Person::from_draft(
            &EmployeeDraft {
                employee_code: "000004".into(),
                full_name: "Fictional Staff".into(),
                annual_leave_days: 15,
                ..Default::default()
            },
            actor.company_id,
        )
        .unwrap();
        let request = Request::new(Input {
            id: Uuid::from_u128(4),
            email: "person@example.invalid".into(),
            roles: StaffRoles::new(vec![
                StaffRole::Recruiter,
                StaffRole::LearningPerformanceOfficer,
            ])
            .unwrap(),
            active: true,
            reason: "Reviewed responsibilities".into(),
            employee: person,
            contacts: EmployeeContacts::empty(actor.company_id, Uuid::nil()),
            expected_source: None,
            supersedes: None,
        })
        .unwrap();
        (actor, request)
    }
    #[test]
    fn frozen_recovery_preserves_noncanonical_role_order_and_timestamp_spelling() {
        let (actor, request) = fixture();
        let mut raw = serde_json::to_value(&request).unwrap();
        raw["roles"] = json!(["learning_performance_officer", "recruiter"]);
        raw["contacts"]["updated_at"] = json!("2026-10-27T12:00:00+00:00");
        let recovered: Request = serde_json::from_value(raw.clone()).unwrap();
        recovered.validate(&actor).unwrap();
        assert_eq!(serde_json::to_value(recovered).unwrap(), raw);
        assert_eq!(request.data().employee.employee_code, "000004");
    }
    #[test]
    fn credentials_unknown_fields_and_cross_person_contacts_cannot_enter_a_frozen_request() {
        let (_, request) = fixture();
        for patch in [
            json!({"password":"do not retain"}),
            json!({"roles":["employee","recruiter"]}),
            json!({"supersedes":request.data().id}),
            json!({"expected_source":"pretend"}),
        ] {
            let mut raw = serde_json::to_value(&request).unwrap();
            raw.as_object_mut()
                .unwrap()
                .extend(patch.as_object().unwrap().clone());
            assert!(Request::from_value(raw).is_err());
        }
        let mut raw = serde_json::to_value(&request).unwrap();
        raw["contacts"]["password"] = json!("do not retain");
        assert!(Request::from_value(raw).is_err());
        let mut raw = serde_json::to_value(request).unwrap();
        raw["contacts"]["employee_id"] = json!(Uuid::from_u128(99));
        assert!(Request::from_value(raw).is_err());
    }
    #[test]
    fn actor_metadata_without_current_bound_ready_admin_access_never_authorizes_provisioning() {
        let (actor, request) = fixture();
        assert!(request.validate(&actor).is_ok());
        let mut bad = actor.clone();
        bad.staff_context = None;
        assert!(request.validate(&bad).is_err());
        let mut bad = actor.clone();
        bad.staff_context.as_mut().unwrap().runtime_ready = false;
        assert!(request.validate(&bad).is_err());
        let mut bad = actor.clone();
        bad.company_id = Uuid::from_u128(99);
        assert!(request.validate(&bad).is_err());
        let mut bad = actor;
        bad.role = Role::Manager;
        assert!(request.validate(&bad).is_err());
    }
    #[test]
    fn initial_receipt_is_bound_to_the_exact_actor_person_roles_and_status() {
        let (actor, request) = fixture();
        let receipt = Receipt {
            schema_version: 1,
            id: request.data().id,
            company_id: actor.company_id,
            actor_id: actor.user_id,
            user_id: Uuid::from_u128(5),
            employee_id: Uuid::from_u128(6),
            active: true,
            roles: request.data().roles.clone(),
            revision: 1,
            base_role: Role::Employee,
            member_updated_at: Utc::now(),
        };
        assert!(validate_receipt(&receipt, &actor, &request, None).is_ok());
        for wrong in [
            Receipt {
                actor_id: Uuid::from_u128(99),
                ..receipt.clone()
            },
            Receipt {
                user_id: actor.user_id,
                ..receipt.clone()
            },
            Receipt {
                active: false,
                ..receipt.clone()
            },
            Receipt {
                revision: 2,
                ..receipt.clone()
            },
            Receipt {
                base_role: Role::HrGeneralist,
                ..receipt.clone()
            },
        ] {
            assert!(validate_receipt(&wrong, &actor, &request, None).is_err());
        }
    }
    #[test]
    fn pending_view_rejects_foreign_counts_duplicates_and_closed_attempts() {
        let (actor, request) = fixture();
        let intent = Intent {
            schema_version: 1,
            id: request.data().id,
            company_id: actor.company_id,
            actor_id: actor.user_id,
            user_id: Uuid::from_u128(5),
            auth_preexisting: false,
            email: request.data().email.clone(),
            state: Stage::Reserved,
            receipt: None,
        };
        let item = PendingItem {
            intent,
            request,
            created_at: Utc::now(),
        };
        let view = Pending {
            schema_version: 1,
            company_id: actor.company_id,
            actor_id: actor.user_id,
            entry_count: 1,
            entries: vec![item],
        };
        assert!(view.validate(&actor).is_ok());
        let mut bad = view.clone();
        bad.entry_count = 2;
        assert!(bad.validate(&actor).is_err());
        let mut bad = view.clone();
        bad.actor_id = Uuid::from_u128(99);
        assert!(bad.validate(&actor).is_err());
        let mut bad = view.clone();
        bad.entries.push(bad.entries[0].clone());
        bad.entry_count = 2;
        assert!(bad.validate(&actor).is_err());
        let mut bad = view;
        bad.entries[0].intent.state = Stage::Cancelled;
        assert!(bad.validate(&actor).is_err());
    }
    #[test]
    fn debug_output_contains_only_the_request_identity() {
        let (_, request) = fixture();
        let text = format!("{request:?}");
        assert!(!text.contains("example.invalid"));
        assert!(!text.contains("Fictional"));
        assert!(!text.contains("Reviewed"));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RejectionKind {
    NotStarted,
    NeedsReview,
    Closed,
}
#[derive(Clone, Debug)]
pub struct Rejection {
    pub id: Uuid,
    pub kind: RejectionKind,
}
impl std::fmt::Display for Rejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self.kind{RejectionKind::NotStarted=>"No account reservation was saved. Refresh and review the employee details.",RejectionKind::NeedsReview=>"Review current employee details and a linked replacement attempt. The reserved identity is retained.",RejectionKind::Closed=>"This attempt was replaced. Refresh pending accounts to find the current attempt."})
    }
}
impl std::error::Error for Rejection {}

// No Debug/Serialize: Create carries an initial password only to the worker.
pub enum Operation {
    Source(Uuid),
    Pending,
    Create {
        request: Request,
        password: Option<String>,
    },
    Cancel {
        request: Request,
        reason: String,
    },
    Status(Request),
}
pub enum Update {
    Source(Source),
    Pending(Pending),
    Completed {
        receipt: Receipt,
        accounts: super::admin::AdminContext,
    },
    Cancelled(Uuid),
    Status {
        request: Request,
        intent: Option<Intent>,
    },
}
