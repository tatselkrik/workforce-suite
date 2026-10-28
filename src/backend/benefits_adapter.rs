use super::*;
use crate::benefits::{self, Context, Operation, Receipt, Response};
use anyhow::Context as _;
impl CloudClient {
    pub fn benefit_context(&mut self, current: &Snapshot) -> Result<Context> {
        let access = self.current_access()?;
        if access != current.access || !benefits::eligible(&access) {
            return Err(AccessRevoked.into());
        }
        ensure!(
            self.product != Some(crate::desktop::Product::Payroll),
            "Benefits administration belongs in HRIS."
        );
        let response = self
            .request(Method::POST, "rpc/employee_benefit_context")?
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
    pub fn execute_benefit(
        &mut self,
        current: &Snapshot,
        operation: Operation,
    ) -> Result<Response> {
        let mut receipt = None;
        if let Operation::Save(request) = operation {
            let access = self.current_access()?;
            if access != current.access || !benefits::eligible(&access) {
                return Err(AccessRevoked.into());
            }
            ensure!(
                self.product != Some(crate::desktop::Product::Payroll),
                "Benefits administration belongs in HRIS."
            );
            request.binding(&access)?;
            let response=self.request(Method::POST,"rpc/save_employee_benefit")?.json(&json!({"p_change":request})).send().context("Benefits delivery is uncertain. Retry the same saved request; do not create a replacement.")?;
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
                return Err(benefits::Rejected.into());
            }
            let r: Receipt = decode(response)?;
            r.validate(&request)?;
            if self.current_access()? != access {
                return Err(AccessRevoked.into());
            }
            receipt = Some(r);
        }
        Ok(Response {
            context: self.benefit_context(current)?,
            receipt,
        })
    }
}
