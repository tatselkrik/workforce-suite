use super::*;
use crate::staff_roles::{
    Capability, RoleChange, SCHEMA_VERSION,
    admin::{AdminContext, Receipt},
};
#[derive(Debug)]
pub(super) struct Rejected;
impl std::fmt::Display for Rejected {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Nothing was saved. Refresh assignments, then cancel this draft and reopen the account before reviewing another change.")
    }
}
impl std::error::Error for Rejected {}

impl CloudClient {
    pub fn execute_staff_roles(
        &mut self,
        operation: crate::staff_roles::admin::Operation,
        current: &Snapshot,
    ) -> Result<(Snapshot, crate::staff_roles::admin::Update)> {
        use crate::staff_roles::admin::{Operation, Update};
        let (access, context, receipt, history) = match operation {
            Operation::Refresh => {
                let (a, c) = self.staff_role_admin_context(true)?;
                (a, c, None, None)
            }
            Operation::Save(request) => {
                if self.current_access()? != current.access {
                    return Err(AccessRevoked.into());
                }
                let (a, r, c) = self.set_staff_roles(&request)?;
                (a, c, Some(r), None)
            }
            Operation::History { user_id } => {
                let (a,c)=self.staff_role_admin_context(true)?;
                let source=c.accounts.iter().find(|v|v.user_id==user_id)
                    .context("Choose an account from current assignments.")?;
                let response=self.request(Method::POST,"rpc/staff_account_history")?
                    .json(&json!({"p_user_id":user_id})).send()
                    .context("Could not load account history. Your drafts are unchanged.")?;
                if matches!(response.status(),StatusCode::UNAUTHORIZED|StatusCode::FORBIDDEN) {return Err(AccessRevoked.into());}
                let history:crate::staff_roles::history::History=decode(response)?;
                history.validate(&a,source).map_err(|_|anyhow::Error::new(AccessRevoked))?;
                if self.current_access()? != a {return Err(AccessRevoked.into());}
                (a,c,None,Some(history))
            }
        };
        if access != current.access {
            return Err(AccessRevoked.into());
        }
        Ok((current.clone(), Update { context, receipt, history }))
    }

    fn staff_role_admin_for(
        &mut self,
        access: &Access,
        include_disabled: bool,
    ) -> Result<AdminContext> {
        ensure!(
            access.role == crate::access::Role::Administrator
                && access.allows(Capability::AccountAdministration),
            "Only a current Administrator reads staff-role assignments."
        );
        ensure!(
            access.staff_contract_version == SCHEMA_VERSION && access.staff_context.is_some(),
            "The protected staff-role contract is not available. Apply the verified role update first."
        );
        let response = self
            .request(Method::POST, "rpc/staff_role_admin_context")?
            .json(&json!({"p_include_disabled":include_disabled}))
            .send()
            .context("Could not refresh staff-role assignments. Your draft has been retained.")?;
        if matches!(
            response.status(),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
        ) {
            return Err(AccessRevoked.into());
        }
        // A temporary HTTP failure is not proof of revoked membership. Keep
        // the exact pending save when its post-write refresh is unavailable.
        if !response.status().is_success() {
            return decode(response);
        }
        let context: AdminContext =
            decode(response).map_err(|_| anyhow::Error::new(AccessRevoked))?;
        context
            .validate(access)
            .map_err(|_| anyhow::Error::new(AccessRevoked))?;
        if access
            .staff_context
            .as_ref()
            .is_none_or(|c| c.runtime_ready != context.runtime_ready)
        {
            return Err(AccessRevoked.into());
        }
        Ok(context)
    }
    pub fn staff_role_admin_context(
        &mut self,
        include_disabled: bool,
    ) -> Result<(Access, AdminContext)> {
        let access = self.current_access()?;
        let context = self.staff_role_admin_for(&access, include_disabled)?;
        if self.current_access()? != access {
            return Err(AccessRevoked.into());
        }
        Ok((access, context))
    }
    pub fn set_staff_roles(
        &mut self,
        request: &RoleChange,
    ) -> Result<(Access, Receipt, AdminContext)> {
        let access = self.current_access()?;
        let source = access
            .staff_context
            .as_ref()
            .context("Refresh the protected staff-role context first.")?;
        request.validate(source)?;
        let before = self.staff_role_admin_for(&access, true)?;
        let subject = before
            .accounts
            .iter()
            .find(|a| a.user_id == request.user_id)
            .context("Choose an employee account from the current company.")?;
        // Do not reject an old expected revision here: an exact retry may already
        // have committed. The server checks its actor/payload receipt first.
        let response=self.request(Method::POST,"rpc/set_staff_roles")?
            .json(&json!({"p_change":request})).send()
            .context("Staff-role delivery is uncertain. Refresh history or retry this exact request; keep its original identity and contents.")?;
        if matches!(
            response.status(),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
        ) {
            return Err(AccessRevoked.into());
        }
        if matches!(
            response.status(),
            StatusCode::BAD_REQUEST | StatusCode::CONFLICT
        ) {
            let error: Value = response
                .json()
                .context("The server response could not be verified. Retain the exact retry.")?;
            if error.get("code").and_then(Value::as_str) == Some("23514") {
                return Err(Rejected.into());
            }
            bail!(
                "The server could not accept this change. Retain the original request and refresh your access before retrying."
            );
        }
        let receipt: Receipt = decode(response)?;
        receipt
            .validate(&access, request, subject)
            .map_err(|_| anyhow::Error::new(AccessRevoked))?;
        let after=self.staff_role_admin_for(&access,true)
            .context("The role assignment was recorded, but refresh failed. Retain this exact request and reload assignments before retrying.")?;
        if self.current_access()? != access {
            return Err(AccessRevoked.into());
        }
        Ok((access, receipt, after))
    }
}
