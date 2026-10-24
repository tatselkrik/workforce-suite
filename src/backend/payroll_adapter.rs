use super::*;
use chrono::Datelike;
impl CloudClient {
    pub(super) fn payroll_snapshot(&mut self, a: &Access) -> Result<crate::payroll::Setup> {
        let company = Some(a.company_id);
        let mut setup = crate::payroll::Setup {
            slips: self.rows("payroll_slips", company)?,
            ..Default::default()
        };
        setup.cutoffs.slips = self.rows("payroll_cutoff_slips", company)?;
        setup.thirteenth.slips = self.rows("payroll_thirteenth_slips", company)?;
        setup.finals.slips = self.rows("payroll_final_slips",company)?;
        setup.finals.statuses = self.rows("payroll_final_status",company)?;
        setup.tax.slips = self.rows("payroll_tax_slips", company)?;
        if a.role.manages_hr() {
            setup.profiles = self.rows("payroll_profiles", company)?;
            setup.holidays = self.rows("payroll_holidays", company)?;
            setup.runs = self.rows("payroll_runs", company)?;
            setup.items = self.rows("payroll_items", company)?;
            setup.events = self.rows("payroll_events", company)?;
            setup.cutoffs.runs = self.rows("payroll_cutoff_runs", company)?;
            setup.cutoffs.items = self.rows("payroll_cutoff_items", company)?;
            setup.thirteenth.runs = self.rows("payroll_thirteenth_runs", company)?;
            setup.thirteenth.items = self.rows("payroll_thirteenth_items", company)?;
            setup.finals.records = self.rows("payroll_final_cases",company)?;
            setup.tax.records = self.rows("payroll_tax_reconciliations", company)?;
            setup.receipts = self.rows("payroll_payment_receipts", company)?;
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
        for s in &setup.cutoffs.slips {
            let p = &s.base;
            ensure!(
                p.company_id == a.company_id
                    && p.item.company_id == p.company_id
                    && p.item.employee_id == p.employee_id
                    && p.item.run_id == p.run_id
                    && p.item.id == p.id
                    && (a.role.manages_hr() || Some(p.employee_id) == a.employee_id),
                "Cutoff payslip is outside your current access."
            );
            ensure!(
                crate::payroll_cutoffs::bounds(p.month, s.part)? == (s.starts_on, s.ends_on)
                    && s.calculation.month == p.month
                    && s.calculation.part == s.part
                    && s.calculation.totals == p.item.totals,
                "Cutoff payslip facts do not reconcile."
            );
        }
        for s in &setup.thirteenth.slips {
            ensure!(
                s.company_id == a.company_id
                    && s.item.company_id == s.company_id
                    && s.item.id == s.id
                    && s.item.employee_id == s.employee_id
                    && s.item.run_id == s.run_id
                    && (a.role.manages_hr() || Some(s.employee_id) == a.employee_id)
                    && s.year == s.as_of.year(),
                "Annual benefit slip is outside your current access."
            );
        }
        for s in &setup.tax.slips {
            ensure!(s.company_id==a.company_id && s.year==s.as_of.year() && (a.role.manages_hr() || Some(s.employee_id)==a.employee_id), "Annual tax advice is outside your current access.");
        }
        ensure!(setup.finals.slips.iter().all(|s|s.company_id==a.company_id&&(a.role.manages_hr()||a.employee_id==Some(s.employee_id)))&&setup.finals.statuses.iter().all(|s|s.company_id==a.company_id&&(a.role.manages_hr()||a.employee_id==Some(s.employee_id))),"Final pay is outside your current access.");
        ensure!(setup.receipts.iter().all(|r|r.company_id==a.company_id && a.role.manages_hr()), "Payment receipt is outside your current access.");
        Ok(setup)
    }
}
