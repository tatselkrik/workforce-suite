use super::*;
impl CloudClient {
    pub(super) fn payroll_snapshot(&mut self, a: &Access) -> Result<crate::payroll::Setup> {
        let company = Some(a.company_id);
        let mut setup = crate::payroll::Setup {
            slips: self.rows("payroll_slips", company)?,
            ..Default::default()
        };
        if a.role.manages_hr() {
            setup.profiles = self.rows("payroll_profiles", company)?;
            setup.holidays = self.rows("payroll_holidays", company)?;
            setup.runs = self.rows("payroll_runs", company)?;
            setup.items = self.rows("payroll_items", company)?;
            setup.events = self.rows("payroll_events", company)?;
        }
        // Approved slips deliberately omit the private attendance/leave source bundle.
        for s in &setup.slips {
            ensure!(
                s.company_id == a.company_id
                    && s.item.company_id == s.company_id
                    && s.item.employee_id == s.employee_id
                    && s.item.run_id == s.run_id
                    && s.item.id == s.id
                    && (a.role.manages_hr() || Some(s.employee_id) == a.employee_id),
                "Payslip is outside your current access."
            );
        }
        Ok(setup)
    }
}
