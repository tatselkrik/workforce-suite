use super::*;
use crate::relations::{self, Context, Operation, Receipt, Response};
use anyhow::Context as _;
impl CloudClient {
    pub fn relation_context(&mut self, current: &Snapshot) -> Result<Context> {
        let access = self.current_access()?;
        if access != current.access || !relations::eligible(&access) {
            return Err(AccessRevoked.into());
        }
        ensure!(
            self.product != Some(crate::desktop::Product::Payroll),
            "Confidential cases belong in HRIS."
        );
        let response = self
            .request(Method::POST, "rpc/employee_relation_context")?
            .json(&json!({}))
            .send()?;
        if matches!(
            response.status(),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
        ) {
            return Err(AccessRevoked.into());
        }
        let context: Context = decode(response)?;
        context
            .validate(&access)
            .map_err(|_| anyhow::Error::new(AccessRevoked))?;
        if self.current_access()? != access {
            return Err(AccessRevoked.into());
        }
        Ok(context)
    }
    pub fn execute_relation(
        &mut self,
        current: &Snapshot,
        operation: Operation,
    ) -> Result<Response> {
        let mut receipt = None;
        if let Operation::Save(request) = operation {
            let access = self.current_access()?;
            if access != current.access || !relations::eligible(&access) {
                return Err(AccessRevoked.into());
            }
            ensure!(
                self.product != Some(crate::desktop::Product::Payroll),
                "Confidential cases belong in HRIS."
            );
            request.binding(&access)?;
            let response=self.request(Method::POST,"rpc/save_employee_relation")?.json(&json!({"p_change":request})).send().context("Case delivery is uncertain. Retry the same saved request; do not create a replacement.")?;
            if matches!(
                response.status(),
                StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
            ) {
                return Err(AccessRevoked.into());
            }
            if matches!(
                response.status(),
                StatusCode::BAD_REQUEST | StatusCode::CONFLICT | StatusCode::UNPROCESSABLE_ENTITY
            ) {
                return Err(relations::Rejected.into());
            }
            let r: Receipt = decode(response)?;
            r.validate(&request)?;
            if self.current_access()? != access {
                return Err(AccessRevoked.into());
            }
            receipt = Some(r);
        }
        Ok(Response {
            context: self.relation_context(current)?,
            receipt,
        })
    }
}
