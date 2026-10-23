use super::*;
impl CloudClient {
    pub(super) fn performance_snapshot(
        &mut self,
        access: &Access,
    ) -> Result<crate::performance::Setup> {
        let company = Some(access.company_id);
        Ok(crate::performance::Setup {
            goals: self.rows("performance_goals", company)?,
            cycles: self.rows("performance_cycles", company)?,
            reviews: self.rows("performance_reviews", company)?,
            notes: if access.role.manages_hr() {
                self.rows("performance_hr_notes", company)?
            } else {
                Vec::new()
            },
            events: self.rows("performance_events", company)?,
            ..Default::default()
        })
    }
}
