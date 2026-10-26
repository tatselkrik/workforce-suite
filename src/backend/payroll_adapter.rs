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
    pub fn government_bir_declarations(&mut self,current:&Snapshot,year:i32,source:&Path,expected_hash:Option<&str>,destination:Option<&Path>)
        ->Result<crate::payroll_government::bir_declarations::Report>{
        use crate::payroll_government::{annual,bir_file,bir_declarations,bir_identity::{Basis,Wire}};
        crate::payroll_reporting::authorize(current)?;annual::validate_year(year,current.company.today())?;
        ensure!(destination.is_none()||expected_hash.is_some(),"Review the BIR declarations before exporting their source worksheet.");
        let before=self.current_access()?;if before!=current.access{return Err(AccessRevoked.into());}
        let wire:Wire=decode(self.request(Method::POST,"rpc/government_bir_identities")?.json(&json!({"p_year":year})).send()?)?;
        let basis=Basis::from_wire(wire,before.company_id,year)?;
        let bytes=bir_file::read_source(source)?;
        let report=bir_declarations::review(basis,&bytes,&source.file_name().context("Choose a BIR file.")?.to_string_lossy(),current.company.today(),before.user_id)?;
        ensure!(expected_hash.is_none_or(|h|h==report.identity.source_sha256),"The BIR source changed. Recheck declarations before exporting.");
        let output=destination.map(|p|report.export(p)).transpose()?;
        if self.current_access()?!=before{return Err(AccessRevoked.into());}
        if let (Some(path),Some(output))=(destination,output){crate::reports::save_export(path,&output)?;}
        Ok(report)
    }
    pub fn government_bir_identities(&mut self,current:&Snapshot,year:i32,source:&Path,expected_hash:Option<&str>,destination:Option<&Path>)
        ->Result<crate::payroll_government::bir_identity::Report>{
        use crate::payroll_government::{annual,bir_file,bir_identity::{self,Basis,Wire}};
        crate::payroll_reporting::authorize(current)?;annual::validate_year(year,current.company.today())?;
        ensure!(destination.is_none()||expected_hash.is_some(),"Check file identities before exporting their review.");
        let before=self.current_access()?;if before!=current.access{return Err(AccessRevoked.into());}
        let wire:Wire=decode(self.request(Method::POST,"rpc/government_bir_identities")?.json(&json!({"p_year":year})).send()?)?;
        let basis=Basis::from_wire(wire,before.company_id,year)?;
        let bytes=bir_file::read_source(source)?;
        let report=bir_identity::review(basis,&bytes,&source.file_name().context("Choose a BIR file.")?.to_string_lossy(),current.company.today(),before.user_id)?;
        ensure!(expected_hash.is_none_or(|h|h==report.source_sha256),"The BIR source changed. Recheck its identities before exporting.");
        let output=destination.map(|p|report.export(p)).transpose()?;
        if self.current_access()?!=before{return Err(AccessRevoked.into());}
        if let (Some(path),Some(output))=(destination,output){crate::reports::save_export(path,&output)?;}
        Ok(report)
    }
    pub fn government_annual_ledger(&mut self,current:&Snapshot,year:i32,destination:Option<&Path>)
        ->Result<crate::payroll_government::annual::Ledger>{
        use crate::payroll_government::annual::{self,Ledger,Wire};
        crate::payroll_reporting::authorize(current)?;
        annual::validate_year(year,current.company.today())?;
        let before=self.current_access()?;
        if before!=current.access{return Err(AccessRevoked.into());}
        let wire:Wire=decode(self.request(Method::POST,"rpc/government_annual_ledger")?
            .json(&json!({"p_year":year})).send()?)?;
        let ledger=Ledger::from_wire(wire,before.company_id,year)?;
        let bytes=destination.map(|path|ledger.export(path)).transpose()?;
        if self.current_access()?!=before{return Err(AccessRevoked.into());}
        if let (Some(path),Some(bytes))=(destination,bytes){crate::reports::save_export(path,&bytes)?;}
        Ok(ledger)
    }
    pub fn government_bir_check(&mut self,current:&Snapshot,source:&Path,expected_hash:Option<&str>,destination:Option<&Path>)
        ->Result<crate::payroll_government::bir_file::Report>{
        use crate::payroll_government::bir_file as bir;
        crate::payroll_reporting::authorize(current)?;
        ensure!(destination.is_none()||expected_hash.is_some(),"Check the BIR source before saving its findings.");
        let registration=self.government_registration(current,None,None)?;
        let employer=registration.record.as_ref().and_then(|r|{
            let tin=r.details.get("bir_tin")?;let branch=r.details.get("bir_branch")?;
            (!tin.is_empty()&&!branch.is_empty()).then(||bir::Employer{tin:tin.clone(),branch:branch.clone()})
        });
        let bytes=bir::read_source(source)?;
        let report=bir::inspect(&bytes,&source.file_name().context("Choose a BIR source file.")?.to_string_lossy(),
            current.company.today(),employer.as_ref(),current.company.id,current.access.user_id)?;
        ensure!(expected_hash.is_none_or(|hash|hash==report.sha256),"The source file changed. Check it again before exporting findings.");
        let output=destination.map(|path|report.findings_export(path)).transpose()?;
        if self.current_access()?!=current.access {return Err(AccessRevoked.into());}
        if let (Some(path),Some(output))=(destination,output){crate::reports::save_export(path,&output)?;}
        Ok(report)
    }
    // Callers recheck access after processing and before returning private data/file IO.
    fn load_government_preparation(&mut self,current:&Snapshot,filter:crate::payroll_government::Filter)->Result<crate::payroll_government::preparation::Preparation>{
        use crate::payroll_government::preparation::{Preparation,PreparationWire};
        crate::payroll_reporting::authorize(current)?;filter.validate(current.company.today())?;
        let before=self.current_access()?;if before!=current.access{return Err(AccessRevoked.into());}
        let wire:PreparationWire=decode(self.request(Method::POST,"rpc/government_preparation")?
            .json(&json!({"p_month":filter.month,"p_agency":filter.agency})).send()?)?;
        Preparation::from_wire(wire,before.company_id,filter)
    }
    pub fn government_preparation(&mut self,current:&Snapshot,filter:crate::payroll_government::Filter,destination:&Path)->Result<crate::payroll_government::Reconciliation>{
        let prepared=self.load_government_preparation(current,filter)?;
        let bytes=prepared.export(destination)?;
        if self.current_access()?!=current.access{return Err(AccessRevoked.into());}
        crate::reports::save_export(destination,&bytes)?;
        Ok(prepared.report)
    }
    pub fn sss_collection_prepare(&mut self,current:&Snapshot,filter:crate::payroll_government::Filter)->Result<crate::payroll_government::sss_collection::Basis>{
        let prepared=self.load_government_preparation(current,filter)?;
        let basis=crate::payroll_government::sss_collection::Basis::from_preparation(&prepared)?;
        if self.current_access()?!=current.access{return Err(AccessRevoked.into());}Ok(basis)
    }
    pub fn sss_collection_export(&mut self,current:&Snapshot,filter:crate::payroll_government::Filter,request:&crate::payroll_government::sss_collection::Request,destination:&Path)->Result<crate::payroll_government::Reconciliation>{
        let prepared=self.load_government_preparation(current,filter)?;
        let basis=crate::payroll_government::sss_collection::Basis::from_preparation(&prepared)?;
        let bytes=basis.render(request)?;
        if self.current_access()?!=current.access{return Err(AccessRevoked.into());}
        crate::payroll_government::sss_collection::save_reference(destination,&bytes)?;Ok(prepared.report)
    }
    pub fn government_registration(
        &mut self, current: &Snapshot, employee_id: Option<Uuid>,
        save: Option<&crate::payroll_government::registration::Save>,
    ) -> Result<crate::payroll_government::registration::Snapshot> {
        use crate::payroll_government::registration as reg;
        crate::payroll_reporting::authorize(current)?;
        ensure!(employee_id.is_none_or(|id| current.employees.iter().any(|e|e.id==id)), "Employee outside the current company.");
        let before=self.current_access()?;
        if before!=current.access {return Err(AccessRevoked.into());}
        let mut saved=None;
        if let Some(request)=save {
            let payload=request.validate(current)?;
            ensure!(payload.employee_id==employee_id,"Save the selected registration target only.");
            let response=self.request(Method::POST,"rpc/save_government_registration")?
                .json(&json!({"p_record":payload})).send()
                .map_err(|_|anyhow::anyhow!("The registration write result is uncertain. Refresh stored records before retrying."))?;
            if response.status().is_server_error(){bail!("The registration may have been saved. Refresh before retrying.");}
            let receipt:reg::Receipt=decode(response)?;
            ensure!(receipt.operation_id==payload.operation_id&&receipt.record_id==payload.id&&receipt.saved_revision==payload.expected_revision.map_or(0,|n|n+1),"The registration result is uncertain. Refresh its stored history.");
            saved=Some(receipt);
            if self.current_access()?!=before {return Err(AccessRevoked.into());}
        }
        let result:reg::Snapshot=decode(self.request(Method::POST,"rpc/government_registration")?
            .json(&json!({"p_employee_id":employee_id})).send()?)?;
        result.validate(before.company_id,employee_id,current.company.today())?;
        if let Some(receipt)=saved {
            ensure!(result.record.as_ref().is_some_and(|r|r.id==receipt.record_id&&r.revision>=receipt.saved_revision)
                &&result.history.iter().any(|e|e.id==receipt.operation_id&&e.revision==receipt.saved_revision),
                "Saved registration history changed. Refresh the current record.");
        }
        if self.current_access()?!=before {return Err(AccessRevoked.into());}
        Ok(result)
    }
    pub fn government_reconciliation(&mut self,current:&Snapshot,filter:crate::payroll_government::Filter,action:Option<&crate::payroll_government::Action>,destination:Option<&Path>)->Result<crate::payroll_government::Reconciliation>{
        crate::payroll_reporting::authorize(current)?;filter.validate(current.company.today())?;
        ensure!(action.is_none()||destination.is_none(),"Save government changes separately from exporting a fresh worksheet.");
        let before=self.current_access()?;if before!=current.access{return Err(AccessRevoked.into());}
        if let Some(a)=action{
            a.authorize(current)?;let(path,op)=a.endpoint()?;
            let response=self.request(Method::POST,path)?.json(&json!({"p_operation":op,"p_record":a.record})).send()
                .map_err(|_|anyhow::anyhow!("The government write result is uncertain. Refresh records before retrying."))?;
            if response.status().is_server_error(){bail!("The government change may have been saved. Refresh records before retrying.");}
            let saved:Value=decode(response)?;
            if saved["id"].as_str().and_then(|v|Uuid::parse_str(v).ok())!=Some(a.id()?){bail!("The government change may have been saved. Refresh records before retrying.");}
            if self.current_access()?!=before{return Err(AccessRevoked.into());}
        }
        let wire:crate::payroll_government::ReviewWire=decode(self.request(Method::POST,"rpc/government_reconciliation")?
            .json(&json!({"p_month":filter.month,"p_agency":filter.agency})).send()?)?;
        let report=crate::payroll_government::Reconciliation::from_wire(wire,before.company_id,filter)?;
        let bytes=destination.map(|path|report.export(path)).transpose()?;
        if self.current_access()?!=before{return Err(AccessRevoked.into());}
        if let (Some(path),Some(bytes))=(destination,bytes){crate::reports::save_export(path,&bytes)?;}
        Ok(report)
    }
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
