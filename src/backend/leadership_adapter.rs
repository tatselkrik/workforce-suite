use super::*;
impl CloudClient {
    pub fn leadership_report(
        &mut self,
        current: &Snapshot,
        filter: &crate::leadership::Filter,
        destination: Option<&Path>,
    ) -> Result<crate::leadership::Report> {
        ensure!(
            self.product != Some(crate::desktop::Product::Payroll),
            "Leadership reporting belongs in HRIS."
        );
        let access = self.current_access()?;
        if access != current.access || !crate::leadership::eligible(&access) {
            return Err(AccessRevoked.into());
        }
        filter.validate(current.company.today())?;
        let response = self
            .request(Method::POST, "rpc/hr_leadership_report")?
            .json(&json!({"p_start":filter.starts_on,"p_end":filter.ends_on}))
            .send()
            .context("Could not refresh protected leadership totals.")?;
        if matches!(
            response.status(),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
        ) {
            return Err(AccessRevoked.into());
        }
        let report: crate::leadership::Report = decode(response)?;
        report
            .validate(&access, filter)
            .context("Leadership totals did not pass the protected report checks.")?;
        if self.current_access()? != access {
            return Err(AccessRevoked.into());
        }
        if let Some(path) = destination {
            let bytes = report.export_bytes(&access, path)?;
            if self.current_access()? != access {
                return Err(AccessRevoked.into());
            }
            crate::reports::save_export(path, &bytes)?;
        }
        Ok(report)
    }
}
