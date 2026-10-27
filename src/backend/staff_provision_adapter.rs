use super::*;
use crate::staff_roles::provision::{
    self, Completion, Intent, Pending, Rejection, RejectionKind, Request, Source,
};
fn checked<T: DeserializeOwned>(response: Response) -> Result<T> {
    if matches!(
        response.status(),
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
    ) {
        return Err(AccessRevoked.into());
    }
    if !response.status().is_success() {
        return decode(response);
    }
    decode(response).map_err(|_| anyhow::Error::new(AccessRevoked))
}
impl CloudClient {
    pub fn execute_staff_provision(
        &mut self,
        operation: provision::Operation,
        current: &Snapshot,
    ) -> Result<(Snapshot, provision::Update)> {
        use provision::{Operation, Stage, Update};
        match operation {
            Operation::Source(id) => Ok((
                current.clone(),
                Update::Source(self.staff_employee_source(&current.access, id)?),
            )),
            Operation::Pending => Ok((
                current.clone(),
                Update::Pending(self.pending_staff_provisions(&current.access)?),
            )),
            Operation::Create { request, password } => {
                let completed =
                    self.create_staff_provision(&current.access, &request, password.as_deref())?;
                self.provision_completed_snapshot(current, completed.receipt)
            }
            Operation::Cancel { request, reason } => {
                self.cancel_staff_provision(&current.access, &request, &reason)?;
                Ok((current.clone(), Update::Cancelled(request.data().id)))
            }
            Operation::Status(request) => {
                let intent = self.staff_provision_status(&current.access, &request)?;
                if let Some(i) = &intent {
                    match i.state {
                        Stage::Committed => {
                            return self.provision_completed_snapshot(
                                current,
                                i.receipt
                                    .clone()
                                    .context("The saved account receipt is missing.")?,
                            );
                        }
                        Stage::Cancelled => {
                            return Ok((current.clone(), Update::Cancelled(request.data().id)));
                        }
                        _ => {}
                    }
                }
                Ok((current.clone(), Update::Status { request, intent }))
            }
        }
    }
    fn provision_completed_snapshot(
        &mut self,
        current: &Snapshot,
        receipt: crate::staff_roles::admin::Receipt,
    ) -> Result<(Snapshot, provision::Update)> {
        let data=self.snapshot().context("The account is saved, but refresh failed. Keep and retry its original attempt to recover the recorded result.")?;
        let (actor, accounts) = self.staff_role_admin_context(true)?;
        if actor != current.access || data.access != current.access {
            return Err(AccessRevoked.into());
        }
        Ok((data, provision::Update::Completed { receipt, accounts }))
    }

    fn provision_actor(&mut self, expected: &Access) -> Result<Access> {
        let actor = self.current_access()?;
        if &actor != expected {
            return Err(AccessRevoked.into());
        }
        provision::require_admin(&actor)?;
        Ok(actor)
    }
    fn finish_provision_read(&mut self, actor: &Access) -> Result<()> {
        if self.current_access()? != *actor {
            return Err(AccessRevoked.into());
        }
        Ok(())
    }
    pub fn staff_employee_source(
        &mut self,
        expected: &Access,
        employee_id: Uuid,
    ) -> Result<Source> {
        let actor = self.provision_actor(expected)?;
        ensure!(!employee_id.is_nil(), "Choose a saved employee.");
        let source: Source = checked(
            self.request(Method::POST, "rpc/staff_employee_source")?
                .json(&json!({"p_employee":employee_id}))
                .send()
                .context("Could not refresh the saved employee source. Your form is retained.")?,
        )?;
        source
            .validate(&actor)
            .map_err(|_| anyhow::Error::new(AccessRevoked))?;
        if source.employee_id != employee_id {
            return Err(AccessRevoked.into());
        }
        self.finish_provision_read(&actor)?;
        Ok(source)
    }
    pub fn pending_staff_provisions(&mut self, expected: &Access) -> Result<Pending> {
        let actor = self.provision_actor(expected)?;
        let pending: Pending = checked(
            self.request(Method::POST, "rpc/pending_staff_provisions")?
                .json(&json!({}))
                .send()
                .context("Could not refresh pending staff accounts.")?,
        )?;
        pending
            .validate(&actor)
            .map_err(|_| anyhow::Error::new(AccessRevoked))?;
        self.finish_provision_read(&actor)?;
        Ok(pending)
    }
    pub fn staff_provision_status(
        &mut self,
        expected: &Access,
        request: &Request,
    ) -> Result<Option<Intent>> {
        let actor = self.provision_actor(expected)?;
        request.validate(&actor)?;
        let intent: Option<Intent> = checked(
            self.request(Method::POST, "rpc/staff_provision_status")?
                .json(&json!({"p_id":request.data().id}))
                .send()
                .context("Could not verify the pending account outcome. Keep its exact retry.")?,
        )?;
        if let Some(i) = &intent {
            i.validate(&actor, request)
                .map_err(|_| anyhow::Error::new(AccessRevoked))?;
        }
        self.finish_provision_read(&actor)?;
        Ok(intent)
    }
    pub fn create_staff_provision(
        &mut self,
        expected: &Access,
        request: &Request,
        password: Option<&str>,
    ) -> Result<Completion> {
        let actor = self.provision_actor(expected)?;
        request.validate(&actor)?;
        let mut body = json!({"operation":"create_staff_v2","request":request});
        if let Some(password) = password {
            ensure!(
                (12..=128).contains(&password.chars().count()),
                "Use a 12–128 character initial password."
            );
            body["password"] = json!(password);
        }
        ensure!(
            serde_json::to_vec(&body)?.len() <= 8192,
            "These account details are too large to save. Shorten the entered details and try again."
        );
        let response=self.account_request(Method::POST,"/functions/v1/account-admin")?.json(&body).send()
   .context("Account delivery is uncertain. Retain and retry this exact attempt; do not create another identity.")?;
        if matches!(
            response.status(),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
        ) {
            return Err(AccessRevoked.into());
        }
        if !response.status().is_success() {
            let status = response.status();
            let problem: Value = response
                .json()
                .context("The account response could not be verified. Keep the exact retry.")?;
            if status == StatusCode::CONFLICT {
                if let Some(kind) = problem.get("provision_state").and_then(Value::as_str) {
                    if problem.get("intent_id") != Some(&json!(request.data().id)) {
                        return Err(AccessRevoked.into());
                    }
                    let kind = match kind {
                        "not_started" => RejectionKind::NotStarted,
                        "needs_review" => RejectionKind::NeedsReview,
                        "closed" => RejectionKind::Closed,
                        _ => return Err(AccessRevoked.into()),
                    };
                    return Err(Rejection {
                        id: request.data().id,
                        kind,
                    }
                    .into());
                }
            }
            bail!(
                "The account outcome is unresolved. Keep this original attempt, refresh pending accounts and retry it."
            );
        }
        let completed: Completion = checked(response)?;
        completed
            .validate(&actor, request)
            .map_err(|_| anyhow::Error::new(AccessRevoked))?;
        self.finish_provision_read(&actor)?;
        Ok(completed)
    }
    pub fn cancel_staff_provision(
        &mut self,
        expected: &Access,
        request: &Request,
        reason: &str,
    ) -> Result<()> {
        let actor = self.provision_actor(expected)?;
        request.validate(&actor)?;
        ensure!(
            reason == reason.trim()
                && (1..=500).contains(&reason.chars().count())
                && !reason.chars().any(char::is_control),
            "Enter a cancellation reason of 1–500 characters."
        );
        let result:Value=checked(self.request(Method::POST,"rpc/cancel_staff_provision")?.json(&json!({"p_id":request.data().id,"p_reason":reason})).send().context("Cancellation delivery is uncertain. Retain its original reason and verify status before retrying.")?)?;
        if result != json!({"id":request.data().id,"state":"cancelled"}) {
            return Err(AccessRevoked.into());
        }
        self.finish_provision_read(&actor)?;
        Ok(())
    }
}
