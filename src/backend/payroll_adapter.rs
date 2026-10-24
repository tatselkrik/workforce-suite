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
        setup.amendments.slips=self.rows("payroll_amendment_slips",company)?;
        setup.amendments.statuses=self.rows("payroll_amendment_status",company)?;
        setup.tax.slips = self.rows("payroll_tax_slips", company)?;
        if a.role.manages_hr() {
            setup.amendments.records=self.rows("payroll_amendment_cases",company)?;
            setup.wages.classifications=self.rows("payroll_wage_classifications",company)?;
            setup.wages.settings=self.rows("payroll_wage_settings",company)?;
            setup.wages.checks=self.rows("payroll_wage_check_index",company)?;
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
            for r in &mut setup.finals.records{r.worksheet.cash_year_reviewed=r.cash_year_reviewed;r.worksheet.cash_year_note=r.cash_year_note.clone();}
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
        ensure!(setup.wages.classifications.iter().all(|c|c.company_id==a.company_id)&&setup.wages.settings.iter().all(|s|s.company_id==a.company_id)&&setup.wages.checks.iter().all(|c|c.company_id==a.company_id),"Wage reviews are outside this company.");
        ensure!(setup.receipts.iter().all(|r|r.company_id==a.company_id && a.role.manages_hr()), "Payment receipt is outside your current access.");
        ensure!(setup.amendments.slips.iter().all(|s|s.company_id==a.company_id&&(a.role.manages_hr()||a.employee_id==Some(s.employee_id)))&&setup.amendments.statuses.iter().all(|s|s.company_id==a.company_id&&(a.role.manages_hr()||a.employee_id==Some(s.employee_id)))&&setup.amendments.records.iter().all(|r|r.company_id==a.company_id&&a.role.manages_hr()),"Payroll amendments are outside your current access.");
        Ok(setup)
    }
}

impl CloudClient {
    pub fn government_payroll_sources(
        &mut self,
        current: &Snapshot,
        month: chrono::NaiveDate,
        agency: crate::payroll_government::Agency,
    ) -> Result<crate::payroll_government::Wire> {
        crate::payroll_reporting::authorize(current)?;
        ensure!(month.day()==1 && month.year()>=2025 && month<=current.company.today(),"Choose an elapsed/current calendar month from2025 onward.");
        let before=self.current_access()?;
        if before!=current.access{return Err(AccessRevoked.into());}
        let wire:crate::payroll_government::Wire=decode(self.request(Method::POST,"rpc/government_payroll_sources")?
            .json(&json!({"p_month":month,"p_agency":agency})).send()?)?;
        wire.validate(before.company_id,agency,month)?;
        if self.current_access()?!=before{return Err(AccessRevoked.into());}
        Ok(wire)
    }
    pub fn consolidated_payroll_report(&mut self,current:&Snapshot,filter:&crate::payroll_reporting::Filter,destination:Option<&Path>)->Result<crate::payroll_reporting::Report>{
        crate::payroll_reporting::authorize(current)?;filter.validate()?;
        let before=self.current_access()?;
        if before!=current.access{return Err(AccessRevoked.into());}
        let wire:crate::payroll_reporting::Wire=decode(self.request(Method::POST,"rpc/payroll_report_v2")?.json(&json!({"p_filter":filter})).send()?)?;
        let report=crate::payroll_reporting::Report::from_wire(wire,before.company_id,filter)?;
        if self.current_access()?!=before{return Err(AccessRevoked.into());}
        if let Some(path)=destination{crate::reports::save_csv(path,&report.csv()?)?;}
        Ok(report)
    }
}
