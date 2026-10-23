use super::*;
impl CloudClient {
    pub(super) fn career_snapshot(&mut self, access: &Access) -> Result<crate::career::Setup> {
        let company = Some(access.company_id);
        Ok(crate::career::Setup {
            plans: self.rows("career_plans", company)?,
            milestones: self.rows("career_milestones", company)?,
            events: self.rows("career_events", company)?,
            ..Default::default()
        })
    }
}
