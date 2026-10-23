use super::*;
impl CloudClient {
    pub(super) fn learning_snapshot(&mut self, access: &Access) -> Result<crate::learning::Setup> {
        let company = Some(access.company_id);
        Ok(crate::learning::Setup {
            training: self.rows("employee_training", company)?,
            certificates: self.rows("employee_certifications", company)?,
            evidence: if access.role.manages_hr() {
                self.rows("certification_evidence", company)?
            } else {
                Vec::new()
            },
            events: self.rows("learning_events", company)?,
            ..Default::default()
        })
    }
}
