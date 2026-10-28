use super::*;
use crate::workspaces::{Context as WorkspaceContext, Receipt, Save};
impl CloudClient {
    pub(super) fn workspace_context(&mut self, access: &Access) -> Result<WorkspaceContext> {
        let response = self
            .request(Method::POST, "rpc/desktop_workspace_context")?
            .json(&json!({}))
            .send()?;
        if matches!(
            response.status(),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
        ) {
            return Err(AccessRevoked.into());
        }
        if response.status() == StatusCode::NOT_FOUND {
            let error: Value = response.json()?;
            if error["code"] == "PGRST202"
                && error["message"]
                    .as_str()
                    .is_some_and(|s| s.contains("desktop_workspace_context"))
            {
                return Ok(WorkspaceContext::legacy(access));
            }
            bail!("Desktop settings could not be read. Refresh or update HRIS.")
        }
        let context: WorkspaceContext = decode(response)?;
        if context.company_id != access.company_id || context.user_id != access.user_id {
            return Err(AccessRevoked.into());
        }
        ensure!(
            context.schema_version == 1,
            "Update HRIS to read this desktop configuration."
        );
        context.validate(access)?;
        Ok(context)
    }
    pub(super) fn save_workspaces(
        &mut self,
        request: &Save,
        current: &Snapshot,
    ) -> Result<Receipt> {
        request.validate(&current.workspaces, &current.access)?;
        let response = self
            .request(Method::POST, "rpc/save_desktop_workspaces")?
            .json(&json!({"p_change":request}))
            .send()?;
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
            let failure: Value = response.json()?;
            if matches!(failure["code"].as_str(), Some("23514" | "23505")) {
                return Err(crate::workspaces::Rejected.into());
            }
            bail!("Workspace save could not be confirmed. Retain the same request for retry.");
        }
        let receipt: Receipt = decode(response)?;
        receipt.validate(request)?;
        Ok(receipt)
    }
}
