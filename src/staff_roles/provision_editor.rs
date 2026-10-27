//! In-memory account forms; secrets never enter the frozen request or history.
use super::{
    StaffRole, StaffRoles,
    admin::Receipt,
    provision::{
        self, Input, Pending, PendingItem, Person, Rejection, RejectionKind, Request, Source, Stage,
    },
};
use crate::{
    access::Access,
    contacts::EmployeeContacts,
    domain::{Employee, EmployeeDraft, Snapshot},
};
use anyhow::{Context as _, Result, ensure};
use uuid::Uuid;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Section {
    #[default]
    Employment,
    Contact,
    Emergency,
    Access,
}
// Intentionally no Debug/Serialize/Clone: initial credentials remain in UI memory.
pub struct Form {
    pub employee: EmployeeDraft,
    pub contacts: EmployeeContacts,
    pub email: String,
    pub roles: Vec<StaffRole>,
    pub active: bool,
    pub reason: String,
    pub password: String,
    pub confirmation: String,
    pub section: Section,
}
impl Form {
    fn new(employee: EmployeeDraft, contacts: EmployeeContacts) -> Self {
        Self {
            employee,
            contacts,
            email: String::new(),
            roles: vec![StaffRole::HrGeneralist],
            active: true,
            reason: String::new(),
            password: String::new(),
            confirmation: String::new(),
            section: Section::Employment,
        }
    }
    fn from_request(request: &Request) -> Self {
        let input = request.data();
        let mut employee = EmployeeDraft::from(&input.employee.record());
        employee.id = input.employee.id;
        Self {
            employee,
            contacts: input.contacts.clone(),
            email: input.email.clone(),
            roles: input
                .roles
                .as_slice()
                .iter()
                .copied()
                .filter(|r| *r != StaffRole::Employee)
                .collect(),
            active: input.active,
            reason: input.reason.clone(),
            password: String::new(),
            confirmation: String::new(),
            section: Section::Access,
        }
    }
    fn take_password(&mut self, required: bool) -> Result<Option<String>> {
        if self.password.is_empty() && self.confirmation.is_empty() && !required {
            return Ok(None);
        }
        ensure!(
            (12..=128).contains(&self.password.chars().count()),
            "Use an initial password of 12–128 characters."
        );
        ensure!(
            self.password == self.confirmation,
            "The initial passwords do not match."
        );
        self.confirmation.clear();
        Ok(Some(std::mem::take(&mut self.password)))
    }
}
#[derive(Default)]
pub struct State {
    pub form: Option<Form>,
    pub source: Option<Source>,
    pub offered_source: Option<Source>,
    baseline: Option<(Employee, Option<EmployeeContacts>)>,
    pub pending: Option<Request>,
    pub expected_user: Option<Uuid>,
    pub supersedes: Option<Uuid>,
    pub review: Option<RejectionKind>,
    pub history: Option<Pending>,
    pub history_requested: bool,
    pub cancel_reason: Option<String>,
    pub pending_cancel: Option<(Request, String)>,
    pub recent_closed: Vec<PendingItem>,
}
impl State {
    pub fn has_draft(&self) -> bool {
        self.form.is_some()
            || self.pending.is_some()
            || self.cancel_reason.is_some()
            || self.pending_cancel.is_some()
    }
    pub fn start_new(&mut self, data: &Snapshot) -> Result<()> {
        provision::require_admin(&data.access)?;
        ensure!(
            !self.has_draft(),
            "Finish or close the current staff form first."
        );
        let employee = EmployeeDraft {
            employee_code: format!("EMP-{:06}", data.employees.len() + 1),
            annual_leave_days: data.company.annual_leave_days,
            ..Default::default()
        };
        self.form = Some(Form::new(
            employee,
            EmployeeContacts::empty(data.company.id, Uuid::nil()),
        ));
        self.source = None;
        self.offered_source = None;
        self.baseline = None;
        self.supersedes = None;
        self.review = None;
        self.expected_user = None;
        Ok(())
    }
    pub fn start_existing(
        &mut self,
        data: &Snapshot,
        employee: EmployeeDraft,
        contacts: EmployeeContacts,
    ) -> Result<Uuid> {
        provision::require_admin(&data.access)?;
        ensure!(
            !self.has_draft(),
            "Finish or close the current staff form first."
        );
        let id = employee.id.context("Choose a saved employee.")?;
        ensure!(
            !data.accounts.iter().any(|a| a.employee_id == Some(id)),
            "This employee already has an account. Edit its responsibilities instead."
        );
        let saved = data
            .employees
            .iter()
            .find(|e| e.id == id)
            .cloned()
            .context("Choose a current company employee.")?;
        self.baseline = Some((
            saved,
            data.contacts.iter().find(|c| c.employee_id == id).cloned(),
        ));
        self.form = Some(Form::new(employee, contacts));
        self.source = None;
        self.offered_source = None;
        self.supersedes = None;
        self.review = None;
        self.expected_user = None;
        Ok(id)
    }
    pub fn accept_source(&mut self, actor: &Access, source: Source) -> Result<()> {
        source.validate(actor)?;
        let form = self
            .form
            .as_ref()
            .context("No staff form is open for this source.")?;
        ensure!(
            form.employee.id == Some(source.employee_id),
            "This saved source belongs to another employee form."
        );
        let changed = self
            .baseline
            .as_ref()
            .is_some_and(|(e, c)| e != &source.employee || c != &source.contacts)
            || self
                .source
                .as_ref()
                .is_some_and(|s| s.source_hash != source.source_hash);
        if self.pending.is_some() || self.review.is_some() || changed {
            self.offered_source = Some(source);
            if self.review.is_none() {
                self.review = Some(RejectionKind::NotStarted);
            }
        } else {
            self.source = Some(source);
        }
        Ok(())
    }
    pub fn submit(&mut self, data: &Snapshot) -> Result<(Request, Option<String>)> {
        provision::require_admin(&data.access)?;
        ensure!(
            self.pending_cancel.is_none(),
            "Check the cancellation outcome before resuming this account."
        );
        ensure!(
            self.review.is_none(),
            "Review the saved account attempt before another submission."
        );
        if let Some(pending) = &self.pending {
            pending.validate(&data.access)?;
            let request = pending.clone();
            let password = self
                .form
                .as_mut()
                .context("The original staff form is missing.")?
                .take_password(false)?;
            return Ok((request, password));
        }
        let form = self.form.as_ref().context("Open a staff form first.")?;
        let person = Person::from_draft(&form.employee, data.company.id)?;
        crate::employment::validate_assignments(data, &form.employee.employee(data.company.id)?)?;
        let contacts = form.contacts.normalized()?;
        let expected_source = if person.id.is_some() {
            let source = self.source.as_ref().context(
                "Refresh the employee's saved employment and contact details before saving access.",
            )?;
            source.validate(&data.access)?;
            ensure!(
                person.id == Some(source.employee_id),
                "The saved source belongs to another employee."
            );
            Some(source.source_hash.clone())
        } else {
            None
        };
        let roles = if form.roles.is_empty() {
            StaffRoles::default()
        } else {
            StaffRoles::new(form.roles.clone())?
        };
        let request = Request::new(Input {
            id: Uuid::new_v4(),
            email: form.email.trim().to_lowercase(),
            roles,
            active: form.active,
            reason: form.reason.trim().into(),
            employee: person,
            contacts,
            expected_source,
            supersedes: self.supersedes,
        })?;
        request.validate(&data.access)?;
        let password = self
            .form
            .as_mut()
            .unwrap()
            .take_password(self.supersedes.is_none())?;
        self.pending = Some(request.clone());
        Ok((request, password))
    }
    pub fn reject(&mut self, rejection: &Rejection) -> Result<()> {
        ensure!(
            self.pending
                .as_ref()
                .is_some_and(|p| p.data().id == rejection.id),
            "The account rejection does not match the pending attempt."
        );
        self.review = Some(rejection.kind);
        Ok(())
    }
    /// Explicit UI review may replace employment/contact edits with the freshly shown saved source.
    pub fn review_current(&mut self, actor: &Access) -> Result<()> {
        provision::require_admin(actor)?;
        let kind = self.review.context("No reviewed replacement is needed.")?;
        ensure!(
            kind != RejectionKind::Closed,
            "Choose the newest attempt from pending accounts."
        );
        let form = self.form.as_mut().context("Open the staff form first.")?;
        if form.employee.id.is_some() {
            let source = self
                .offered_source
                .take()
                .context("Load and inspect the current saved employee details first.")?;
            source.validate(actor)?;
            ensure!(
                form.employee.id == Some(source.employee_id),
                "The review belongs to another employee."
            );
            form.employee = EmployeeDraft::from(&source.employee);
            form.contacts = source
                .contacts
                .clone()
                .unwrap_or_else(|| EmployeeContacts::empty(actor.company_id, source.employee_id));
            self.baseline = Some((source.employee.clone(), source.contacts.clone()));
            self.source = Some(source);
        }
        if let Some(previous) = self.pending.take() {
            self.supersedes = match kind {
                RejectionKind::NeedsReview => Some(previous.data().id),
                RejectionKind::NotStarted => previous.data().supersedes,
                RejectionKind::Closed => unreachable!(),
            };
        }
        form.password.clear();
        form.confirmation.clear();
        self.review = None;
        self.cancel_reason = None;
        self.pending_cancel = None;
        Ok(())
    }
    pub fn accept_pending(&mut self, actor: &Access, pending: Pending) -> Result<()> {
        pending.validate(actor)?;
        self.history = Some(pending);
        Ok(())
    }
    pub fn resume(&mut self, actor: &Access, item: &PendingItem) -> Result<()> {
        provision::require_admin(actor)?;
        ensure!(
            !self.has_draft(),
            "Finish or close the open staff form first."
        );
        item.intent.validate(actor, &item.request)?;
        ensure!(
            matches!(item.intent.state, Stage::Reserved | Stage::Cancelled),
            "Choose a recoverable account attempt."
        );
        self.form = Some(Form::from_request(&item.request));
        self.pending = Some(item.request.clone());
        self.expected_user = Some(item.intent.user_id);
        self.supersedes = item.request.data().supersedes;
        self.source = None;
        self.offered_source = None;
        self.baseline = None;
        self.review = (item.intent.state == Stage::Cancelled).then_some(RejectionKind::NeedsReview);
        Ok(())
    }
    pub fn accept_receipt(&mut self, actor: &Access, receipt: &Receipt) -> Result<()> {
        let request = self
            .pending
            .as_ref()
            .context("The original account request is missing.")?;
        provision::validate_receipt(receipt, actor, request, self.expected_user)?;
        let id = request.data().id;
        if let Some(h) = self.history.as_mut() {
            h.entries.retain(|e| e.intent.id != id);
            h.entry_count = h.entries.len();
        }
        self.clear_form();
        Ok(())
    }
    pub fn cancel_request(&mut self) -> Result<(Request, String)> {
        if let Some(pending) = &self.pending_cancel {
            return Ok(pending.clone());
        }
        let request = self
            .pending
            .as_ref()
            .context("There is no saved account attempt to cancel.")?
            .clone();
        let reason = self
            .cancel_reason
            .as_ref()
            .context("Enter a reason to close this attempt.")?
            .trim()
            .to_owned();
        ensure!(
            (1..=500).contains(&reason.chars().count()) && !reason.chars().any(char::is_control),
            "Enter a cancellation reason of 1–500 characters."
        );
        self.cancel_reason = Some(reason.clone());
        self.pending_cancel = Some((request.clone(), reason.clone()));
        Ok((request, reason))
    }
    pub fn accept_cancelled(&mut self, actor: &Access, id: Uuid) -> Result<()> {
        provision::require_admin(actor)?;
        ensure!(
            self.pending.as_ref().is_some_and(|p| p.data().id == id),
            "Cancellation belongs to another account attempt."
        );
        if let Some(history) = self.history.as_mut() {
            if let Some(index) = history.entries.iter().position(|e| e.intent.id == id) {
                let mut item = history.entries.remove(index);
                item.intent.state = Stage::Cancelled;
                self.recent_closed.push(item);
            }
            history.entry_count = history.entries.len();
        }
        self.clear_form();
        Ok(())
    }
    pub fn discard(&mut self) -> Result<()> {
        ensure!(
            self.pending.is_none() || self.review == Some(RejectionKind::NotStarted),
            "Keep this pending attempt until its result is checked or its cancellation is confirmed."
        );
        self.clear_form();
        Ok(())
    }
    fn clear_form(&mut self) {
        self.form = None;
        self.source = None;
        self.offered_source = None;
        self.baseline = None;
        self.pending = None;
        self.expected_user = None;
        self.supersedes = None;
        self.review = None;
        self.cancel_reason = None;
        self.pending_cancel = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{access::Role, domain::DemoStore, staff_roles::Context};
    pub(super) fn data() -> Snapshot {
        let mut d = DemoStore::new().data;
        d.access.role = Role::Administrator;
        d.access.company_id = d.company.id;
        d.access.user_id = Uuid::from_u128(900);
        d.access.employee_id = Some(d.employees[0].id);
        d.access.staff_contract_version = 1;
        d.access
            .bind_staff_context(Context {
                schema_version: 1,
                company_id: d.company.id,
                user_id: d.access.user_id,
                employee_id: d.access.employee_id.unwrap(),
                base_role: Role::Administrator,
                revision: 1,
                runtime_ready: true,
                roles: StaffRoles::legacy(Role::Administrator),
            })
            .unwrap();
        d
    }
    pub(super) fn prepared(d: &Snapshot) -> State {
        let mut s = State::default();
        s.start_new(d).unwrap();
        let f = s.form.as_mut().unwrap();
        f.employee.full_name = "Fictional new staff".into();
        f.email = "person@example.invalid".into();
        f.reason = "Reviewed responsibilities".into();
        f.password = "fictional initial password".into();
        f.confirmation = f.password.clone();
        s
    }
    #[test]
    fn submit_freezes_exact_business_request_but_moves_credentials_only_once() {
        let d = data();
        let mut s = prepared(&d);
        let (first, password) = s.submit(&d).unwrap();
        assert!(password.is_some());
        assert!(s.form.as_ref().unwrap().password.is_empty());
        assert!(s.form.as_ref().unwrap().confirmation.is_empty());
        s.form.as_mut().unwrap().reason = "Later accidental edit".into();
        let (retry, password) = s.submit(&d).unwrap();
        assert_eq!(
            serde_json::to_value(first).unwrap(),
            serde_json::to_value(retry).unwrap()
        );
        assert!(password.is_none());
        assert!(s.discard().is_err());
    }
    #[test]
    fn not_started_rejection_allows_explicit_review_without_inventing_a_parent_reservation() {
        let d = data();
        let mut s = prepared(&d);
        let (request, _) = s.submit(&d).unwrap();
        s.reject(&Rejection {
            id: request.data().id,
            kind: RejectionKind::NotStarted,
        })
        .unwrap();
        assert!(s.submit(&d).is_err());
        s.review_current(&d.access).unwrap();
        assert!(s.supersedes.is_none());
        assert!(s.pending.is_none());
    }
    #[test]
    fn confirmed_review_keeps_the_prior_identity_link_and_never_resets_credentials() {
        let d = data();
        let mut s = prepared(&d);
        let (request, _) = s.submit(&d).unwrap();
        s.reject(&Rejection {
            id: request.data().id,
            kind: RejectionKind::NeedsReview,
        })
        .unwrap();
        s.review_current(&d.access).unwrap();
        assert_eq!(s.supersedes, Some(request.data().id));
        let (revised, password) = s.submit(&d).unwrap();
        assert_eq!(revised.data().supersedes, Some(request.data().id));
        assert!(password.is_none());
        assert_ne!(revised.data().id, request.data().id);
    }
    #[test]
    fn unrelated_rejection_cannot_release_the_pending_draft() {
        let d = data();
        let mut s = prepared(&d);
        s.submit(&d).unwrap();
        assert!(
            s.reject(&Rejection {
                id: Uuid::new_v4(),
                kind: RejectionKind::NotStarted
            })
            .is_err()
        );
        assert!(s.pending.is_some());
        assert!(s.review.is_none());
    }
    #[test]
    fn invalid_password_confirmation_keeps_credentials_and_creates_no_pending_attempt() {
        let d = data();
        let mut s = prepared(&d);
        s.form.as_mut().unwrap().confirmation = "different".into();
        assert!(s.submit(&d).is_err());
        assert!(s.pending.is_none());
        assert!(!s.form.as_ref().unwrap().password.is_empty());
    }
}

#[cfg(test)]
mod source_and_cancel_tests {
    use super::*;
    #[test]
    fn cancellation_retry_keeps_the_original_reason_and_blocks_a_competing_commit() {
        let d = super::tests::data();
        let mut s = super::tests::prepared(&d);
        s.submit(&d).unwrap();
        s.cancel_reason = Some("Reviewed cancellation".into());
        let (first, reason) = s.cancel_request().unwrap();
        s.cancel_reason = Some("Later accidental edit".into());
        let (retry, again) = s.cancel_request().unwrap();
        assert_eq!(
            serde_json::to_value(first).unwrap(),
            serde_json::to_value(retry).unwrap()
        );
        assert_eq!(reason, again);
        assert!(s.submit(&d).is_err());
    }
    #[test]
    fn changed_saved_contact_source_requires_explicit_review_without_overwriting_a_draft() {
        let mut d = super::tests::data();
        let employee = d.employees.last().unwrap().clone();
        d.accounts.retain(|a| a.employee_id != Some(employee.id));
        let contact = d
            .contacts
            .iter()
            .find(|c| c.employee_id == employee.id)
            .cloned();
        let mut draft = EmployeeDraft::from(&employee);
        draft.full_name = "Unsaved proposed name".into();
        let mut state = State::default();
        state
            .start_existing(
                &d,
                draft,
                contact
                    .clone()
                    .unwrap_or_else(|| EmployeeContacts::empty(d.company.id, employee.id)),
            )
            .unwrap();
        let mut changed = employee.clone();
        changed.department = "Changed saved department".into();
        let source = Source {
            schema_version: 1,
            company_id: d.company.id,
            actor_id: d.access.user_id,
            runtime_ready: true,
            employee_id: employee.id,
            source_hash: "a".repeat(64),
            employee: changed.clone(),
            contacts: contact,
        };
        state.accept_source(&d.access, source).unwrap();
        assert_eq!(state.review, Some(RejectionKind::NotStarted));
        assert_eq!(
            state.form.as_ref().unwrap().employee.full_name,
            "Unsaved proposed name"
        );
        state.review_current(&d.access).unwrap();
        assert_eq!(
            state.form.as_ref().unwrap().employee.department,
            changed.department
        );
        assert!(state.source.is_some());
        assert!(state.pending.is_none());
    }
}
