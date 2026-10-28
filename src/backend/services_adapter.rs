use super::*;
use crate::services::{self, Context, Operation, Receipt, Response};
use anyhow::Context as _;
impl CloudClient {
    pub fn service_context(&mut self, current: &Snapshot) -> Result<Context> {
        let access = self.current_access()?;
        if access != current.access || !services::admitted(&access, services::Audience::Staff) {
            return Err(AccessRevoked.into());
        }
        ensure!(
            self.product != Some(crate::desktop::Product::Payroll),
            "Employee services belong in HRIS."
        );
        let response = self
            .request(Method::POST, "rpc/employee_service_context")?
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
        if context.audience != services::Audience::Staff || self.current_access()? != access {
            return Err(AccessRevoked.into());
        }
        Ok(context)
    }
    pub fn execute_service(
        &mut self,
        current: &Snapshot,
        operation: Operation,
    ) -> Result<Response> {
        let mut receipt = None;
        if let Operation::Save(request) = operation {
            let access = self.current_access()?;
            if access != current.access || !services::admitted(&access, services::Audience::Staff) {
                return Err(AccessRevoked.into());
            }
            ensure!(
                self.product != Some(crate::desktop::Product::Payroll),
                "Employee services belong in HRIS."
            );
            request.binding(&access)?;
            let response=self.request(Method::POST,"rpc/save_employee_service")?.json(&json!({"p_change":request})).send().context("Service request delivery is uncertain. Retry the same saved request; do not create a replacement.")?;
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
                return Err(services::Rejected.into());
            }
            let r: Receipt = decode(response)?;
            r.validate(&request)?;
            if self.current_access()? != access {
                return Err(AccessRevoked.into());
            }
            receipt = Some(r);
        }
        Ok(Response {
            context: self.service_context(current)?,
            receipt,
        })
    }
}
