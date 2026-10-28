use super::*;
use chrono::Datelike;
impl CloudClient {
    pub(super) fn payroll_snapshot(&mut self, a: &Access) -> Result<crate::payroll::Setup> {
        let company = Some(a.company_id);
        let own_only = self.product == Some(crate::desktop::Product::Hris);
        let employee = if own_only {
            Some(
                a.employee_id
                    .context("An employee record is required for own payslips.")?,
            )
        } else {
            None
        };
        let preparing = a.allows(crate::staff_roles::Capability::PayrollPreparation) && !own_only;
        let mut setup = crate::payroll::Setup {
            slips: self.scoped_rows("payroll_slips", company, employee)?,
            ..Default::default()
        };
        setup.cutoffs.slips = self.scoped_rows("payroll_cutoff_slips", company, employee)?;
        setup.thirteenth.slips = self.scoped_rows("payroll_thirteenth_slips", company, employee)?;
        setup.finals.slips = self.scoped_rows("payroll_final_slips", company, employee)?;
        setup.finals.statuses = self.scoped_rows("payroll_final_status", company, employee)?;
        setup.amendments.slips = self.scoped_rows("payroll_amendment_slips", company, employee)?;
        setup.amendments.statuses =
            self.scoped_rows("payroll_amendment_status", company, employee)?;
        setup.tax.slips = self.scoped_rows("payroll_tax_slips", company, employee)?;
        if preparing {
            setup.amendments.records =
                self.scoped_rows("payroll_amendment_cases", company, employee)?;
            setup.wages.classifications =
                self.scoped_rows("payroll_wage_classifications", company, employee)?;
            setup.wages.settings = self.scoped_rows("payroll_wage_settings", company, employee)?;
            setup.wages.checks = self.scoped_rows("payroll_wage_check_index", company, employee)?;
            setup.profiles = self.scoped_rows("payroll_profiles", company, employee)?;
            setup.holidays = self.scoped_rows("payroll_holidays", company, employee)?;
            setup.runs = self.scoped_rows("payroll_runs", company, employee)?;
            setup.items = self.scoped_rows("payroll_items", company, employee)?;
            setup.events = self.scoped_rows("payroll_events", company, employee)?;
            setup.cutoffs.runs = self.scoped_rows("payroll_cutoff_runs", company, employee)?;
            setup.cutoffs.items = self.scoped_rows("payroll_cutoff_items", company, employee)?;
            setup.thirteenth.runs =
                self.scoped_rows("payroll_thirteenth_runs", company, employee)?;
            setup.thirteenth.items =
                self.scoped_rows("payroll_thirteenth_items", company, employee)?;
            setup.finals.records = self.scoped_rows("payroll_final_cases", company, employee)?;
            for r in &mut setup.finals.records {
                r.worksheet.cash_year_reviewed = r.cash_year_reviewed;
                r.worksheet.cash_year_note = r.cash_year_note.clone();
            }
            setup.tax.records =
                self.scoped_rows("payroll_tax_reconciliations", company, employee)?;
            setup.receipts = self.scoped_rows("payroll_payment_receipts", company, employee)?;
        }
        // Approved slips deliberately omit the private attendance/leave source bundle.
        for s in &setup.slips {
            ensure!(
                s.company_id == a.company_id
                    && s.item.company_id == s.company_id
                    && s.item.employee_id == s.employee_id
                    && s.item.run_id == s.run_id
                    && s.item.id == s.id
                    && (preparing || Some(s.employee_id) == a.employee_id),
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
                    && (preparing || Some(p.employee_id) == a.employee_id),
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
                    && (preparing || Some(s.employee_id) == a.employee_id)
                    && s.year == s.as_of.year(),
                "Annual benefit slip is outside your current access."
            );
        }
        for s in &setup.tax.slips {
            ensure!(
                s.company_id == a.company_id
                    && s.year == s.as_of.year()
                    && (preparing || Some(s.employee_id) == a.employee_id),
                "Annual tax advice is outside your current access."
            );
        }
        ensure!(
            setup
                .finals
                .slips
                .iter()
                .all(|s| s.company_id == a.company_id
                    && (preparing || a.employee_id == Some(s.employee_id)))
                && setup
                    .finals
                    .statuses
                    .iter()
                    .all(|s| s.company_id == a.company_id
                        && (preparing || a.employee_id == Some(s.employee_id))),
            "Final pay is outside your current access."
        );
        ensure!(
            setup
                .wages
                .classifications
                .iter()
                .all(|c| c.company_id == a.company_id)
                && setup
                    .wages
                    .settings
                    .iter()
                    .all(|s| s.company_id == a.company_id)
                && setup
                    .wages
                    .checks
                    .iter()
                    .all(|c| c.company_id == a.company_id),
            "Wage reviews are outside this company."
        );
        ensure!(
            setup
                .receipts
                .iter()
                .all(|r| r.company_id == a.company_id && preparing),
            "Payment receipt is outside your current access."
        );
        ensure!(
            setup
                .amendments
                .slips
                .iter()
                .all(|s| s.company_id == a.company_id
                    && (preparing || a.employee_id == Some(s.employee_id)))
                && setup
                    .amendments
                    .statuses
                    .iter()
                    .all(|s| s.company_id == a.company_id
                        && (preparing || a.employee_id == Some(s.employee_id)))
                && setup
                    .amendments
                    .records
                    .iter()
                    .all(|r| r.company_id == a.company_id && preparing),
            "Payroll amendments are outside your current access."
        );
        Ok(setup)
    }
}

impl CloudClient {
    pub fn government_bir_declarations(
        &mut self,
        current: &Snapshot,
        year: i32,
        source: &Path,
        expected_hash: Option<&str>,
        destination: Option<&Path>,
    ) -> Result<crate::payroll_government::bir_declarations::Report> {
        use crate::payroll_government::{
            annual, bir_declarations, bir_file,
            bir_identity::{Basis, Wire},
        };
        crate::payroll_reporting::authorize(current)?;
        annual::validate_year(year, current.company.today())?;
        ensure!(
            destination.is_none() || expected_hash.is_some(),
            "Review the BIR declarations before exporting their source worksheet."
        );
        let before = self.current_access()?;
        if before != current.access {
            return Err(AccessRevoked.into());
        }
        let wire: Wire = decode(
            self.request(Method::POST, "rpc/government_bir_identities")?
                .json(&json!({"p_year":year}))
                .send()?,
        )?;
        let basis = Basis::from_wire(wire, before.company_id, year)?;
        let bytes = bir_file::read_source(source)?;
        let report = bir_declarations::review(
            basis,
            &bytes,
            &source
                .file_name()
                .context("Choose a BIR file.")?
                .to_string_lossy(),
            current.company.today(),
            before.user_id,
        )?;
        ensure!(
            expected_hash.is_none_or(|h| h == report.identity.source_sha256),
            "The BIR source changed. Recheck declarations before exporting."
        );
        let output = destination.map(|p| report.export(p)).transpose()?;
        if self.current_access()? != before {
            return Err(AccessRevoked.into());
        }
        if let (Some(path), Some(output)) = (destination, output) {
            crate::reports::save_export(path, &output)?;
        }
        Ok(report)
    }
    pub fn government_bir_identities(
        &mut self,
        current: &Snapshot,
        year: i32,
        source: &Path,
        expected_hash: Option<&str>,
        destination: Option<&Path>,
    ) -> Result<crate::payroll_government::bir_identity::Report> {
        use crate::payroll_government::{
            annual, bir_file,
            bir_identity::{self, Basis, Wire},
        };
        crate::payroll_reporting::authorize(current)?;
        annual::validate_year(year, current.company.today())?;
        ensure!(
            destination.is_none() || expected_hash.is_some(),
            "Check file identities before exporting their review."
        );
        let before = self.current_access()?;
        if before != current.access {
            return Err(AccessRevoked.into());
        }
        let wire: Wire = decode(
            self.request(Method::POST, "rpc/government_bir_identities")?
                .json(&json!({"p_year":year}))
                .send()?,
        )?;
        let basis = Basis::from_wire(wire, before.company_id, year)?;
        let bytes = bir_file::read_source(source)?;
        let report = bir_identity::review(
            basis,
            &bytes,
            &source
                .file_name()
                .context("Choose a BIR file.")?
                .to_string_lossy(),
            current.company.today(),
            before.user_id,
        )?;
        ensure!(
            expected_hash.is_none_or(|h| h == report.source_sha256),
            "The BIR source changed. Recheck its identities before exporting."
        );
        let output = destination.map(|p| report.export(p)).transpose()?;
        if self.current_access()? != before {
            return Err(AccessRevoked.into());
        }
        if let (Some(path), Some(output)) = (destination, output) {
            crate::reports::save_export(path, &output)?;
        }
        Ok(report)
    }
    pub fn government_annual_pay_details(
        &mut self,
        current: &Snapshot,
        year: i32,
        destination: Option<&Path>,
    ) -> Result<crate::payroll_government::annual_pay::Report> {
        use crate::payroll_government::{
            annual,
            annual_pay::{Report, Wire},
        };
        crate::payroll_reporting::authorize(current)?;
        annual::validate_year(year, current.company.today())?;
        let before = self.current_access()?;
        if before != current.access {
            return Err(AccessRevoked.into());
        }
        let wire: Wire = decode(
            self.request(Method::POST, "rpc/government_annual_pay_details")?
                .json(&json!({"p_year":year}))
                .send()?,
        )?;
        let report = Report::from_wire(wire, before.company_id, year)?;
        let bytes = destination.map(|path| report.export(path)).transpose()?;
        if self.current_access()? != before {
            return Err(AccessRevoked.into());
        }
        if let (Some(path), Some(bytes)) = (destination, bytes) {
            crate::reports::save_export(path, &bytes)?;
        }
        Ok(report)
    }
    pub fn government_annual_ledger(
        &mut self,
        current: &Snapshot,
        year: i32,
        destination: Option<&Path>,
    ) -> Result<crate::payroll_government::annual::Ledger> {
        use crate::payroll_government::annual::{self, Ledger, Wire};
        crate::payroll_reporting::authorize(current)?;
        annual::validate_year(year, current.company.today())?;
        let before = self.current_access()?;
        if before != current.access {
            return Err(AccessRevoked.into());
        }
        let wire: Wire = decode(
            self.request(Method::POST, "rpc/government_annual_ledger")?
                .json(&json!({"p_year":year}))
                .send()?,
        )?;
        let ledger = Ledger::from_wire(wire, before.company_id, year)?;
        let bytes = destination.map(|path| ledger.export(path)).transpose()?;
        if self.current_access()? != before {
            return Err(AccessRevoked.into());
        }
        if let (Some(path), Some(bytes)) = (destination, bytes) {
            crate::reports::save_export(path, &bytes)?;
        }
        Ok(ledger)
    }
    pub fn government_bir_build(
        &mut self,
        current: &Snapshot,
        action: &crate::payroll_government::bir_file::builder::Action,
    ) -> Result<Option<crate::payroll_government::bir_file::builder::Preview>> {
        use crate::payroll_government::bir_file::{self as bir, builder};
        crate::payroll_reporting::authorize(current)?;
        if self.current_access()? != current.access {
            return Err(AccessRevoked.into());
        }
        match action {
            builder::Action::Template {
                schedule,
                destination,
            } => {
                let bytes = builder::template(*schedule, crate::tabular::is_xlsx(destination))?;
                if self.current_access()? != current.access {
                    return Err(AccessRevoked.into());
                }
                crate::reports::save_export(destination, &bytes)?;
                Ok(None)
            }
            builder::Action::Prepare {
                selection,
                source,
                expected_basis,
                destination,
                reviewed,
            } => {
                ensure!(
                    destination.is_none() || (*reviewed && expected_basis.is_some()),
                    "Preview and explicitly review the declarations before saving a BIR draft."
                );
                let registration = self.government_registration(current, None, None)?;
                let record = registration
                    .record
                    .as_ref()
                    .context("Complete company BIR registration before building a draft.")?;
                let employer = bir::Employer {
                    tin: record.details.get("bir_tin").cloned().unwrap_or_default(),
                    branch: record
                        .details
                        .get("bir_branch")
                        .cloned()
                        .unwrap_or_default(),
                };
                let (bytes, excel) = builder::read_template(source)?;
                let draft = builder::assemble(
                    &bytes,
                    excel,
                    &builder::Basis {
                        selection: *selection,
                        today: current.company.today(),
                        employer: &employer,
                        company_id: current.company.id,
                        user_id: current.access.user_id,
                        registration_id: record.id,
                        registration_revision: record.revision,
                    },
                )?;
                ensure!(
                    expected_basis
                        .as_ref()
                        .is_none_or(|hash| hash == &draft.preview.basis_sha256),
                    "The source, selection or company registration changed. Preview and review the declarations again."
                );
                let fresh = self.government_registration(current, None, None)?;
                ensure!(
                    fresh.record.as_ref().is_some_and(|r| r.id == record.id
                        && r.revision == record.revision
                        && r.details.get("bir_tin") == Some(&employer.tin)
                        && r.details.get("bir_branch") == Some(&employer.branch)),
                    "Company registration changed while preparing the draft. Preview again."
                );
                if self.current_access()? != current.access {
                    return Err(AccessRevoked.into());
                }
                if let Some(path) = destination {
                    draft.save_new(path)?;
                }
                Ok(Some(draft.preview))
            }
        }
    }
    pub fn government_bir_check(
        &mut self,
        current: &Snapshot,
        source: &Path,
        expected_hash: Option<&str>,
        destination: Option<&Path>,
    ) -> Result<crate::payroll_government::bir_file::Report> {
        use crate::payroll_government::bir_file as bir;
        crate::payroll_reporting::authorize(current)?;
        ensure!(
            destination.is_none() || expected_hash.is_some(),
            "Check the BIR source before saving its findings."
        );
        let registration = self.government_registration(current, None, None)?;
        let employer = registration.record.as_ref().and_then(|r| {
            let tin = r.details.get("bir_tin")?;
            let branch = r.details.get("bir_branch")?;
            (!tin.is_empty() && !branch.is_empty()).then(|| bir::Employer {
                tin: tin.clone(),
                branch: branch.clone(),
            })
        });
        let bytes = bir::read_source(source)?;
        let report = bir::inspect(
            &bytes,
            &source
                .file_name()
                .context("Choose a BIR source file.")?
                .to_string_lossy(),
            current.company.today(),
            employer.as_ref(),
            current.company.id,
            current.access.user_id,
        )?;
        ensure!(
            expected_hash.is_none_or(|hash| hash == report.sha256),
            "The source file changed. Check it again before exporting findings."
        );
        let output = destination
            .map(|path| report.findings_export(path))
            .transpose()?;
        if self.current_access()? != current.access {
            return Err(AccessRevoked.into());
        }
        if let (Some(path), Some(output)) = (destination, output) {
            crate::reports::save_export(path, &output)?;
        }
        Ok(report)
    }
    // Callers recheck access after processing and before returning private data/file IO.
    fn load_government_preparation(
        &mut self,
        current: &Snapshot,
        filter: crate::payroll_government::Filter,
    ) -> Result<crate::payroll_government::preparation::Preparation> {
        use crate::payroll_government::preparation::{Preparation, PreparationWire};
        crate::payroll_reporting::authorize(current)?;
        filter.validate(current.company.today())?;
        let before = self.current_access()?;
        if before != current.access {
            return Err(AccessRevoked.into());
        }
        let wire: PreparationWire = decode(
            self.request(Method::POST, "rpc/government_preparation")?
                .json(&json!({"p_month":filter.month,"p_agency":filter.agency}))
                .send()?,
        )?;
        Preparation::from_wire(wire, before.company_id, filter)
    }
    pub fn government_preparation(
        &mut self,
        current: &Snapshot,
        filter: crate::payroll_government::Filter,
        destination: &Path,
    ) -> Result<crate::payroll_government::Reconciliation> {
        let prepared = self.load_government_preparation(current, filter)?;
        let bytes = prepared.export(destination)?;
        if self.current_access()? != current.access {
            return Err(AccessRevoked.into());
        }
        crate::reports::save_export(destination, &bytes)?;
        Ok(prepared.report)
    }
    pub fn sss_collection_prepare(
        &mut self,
        current: &Snapshot,
        filter: crate::payroll_government::Filter,
    ) -> Result<crate::payroll_government::sss_collection::Basis> {
        let prepared = self.load_government_preparation(current, filter)?;
        let basis = crate::payroll_government::sss_collection::Basis::from_preparation(&prepared)?;
        if self.current_access()? != current.access {
            return Err(AccessRevoked.into());
        }
        Ok(basis)
    }
    pub fn sss_collection_export(
        &mut self,
        current: &Snapshot,
        filter: crate::payroll_government::Filter,
        request: &crate::payroll_government::sss_collection::Request,
        destination: &Path,
    ) -> Result<crate::payroll_government::Reconciliation> {
        let prepared = self.load_government_preparation(current, filter)?;
        let basis = crate::payroll_government::sss_collection::Basis::from_preparation(&prepared)?;
        let bytes = basis.render(request)?;
        if self.current_access()? != current.access {
            return Err(AccessRevoked.into());
        }
        crate::payroll_government::sss_collection::save_reference(destination, &bytes)?;
        Ok(prepared.report)
    }
    pub fn government_registration(
        &mut self,
        current: &Snapshot,
        employee_id: Option<Uuid>,
        save: Option<&crate::payroll_government::registration::Save>,
    ) -> Result<crate::payroll_government::registration::Snapshot> {
        use crate::payroll_government::registration as reg;
        crate::payroll_reporting::authorize(current)?;
        ensure!(
            employee_id.is_none_or(|id| current.employees.iter().any(|e| e.id == id)),
            "Employee outside the current company."
        );
        let before = self.current_access()?;
        if before != current.access {
            return Err(AccessRevoked.into());
        }
        let mut saved = None;
        if let Some(request) = save {
            let payload = request.validate(current)?;
            ensure!(
                payload.employee_id == employee_id,
                "Save the selected registration target only."
            );
            let response=self.request(Method::POST,"rpc/save_government_registration")?
                .json(&json!({"p_record":payload})).send()
                .map_err(|_|anyhow::anyhow!("The registration write result is uncertain. Refresh stored records before retrying."))?;
            if response.status().is_server_error() {
                bail!("The registration may have been saved. Refresh before retrying.");
            }
            let receipt: reg::Receipt = decode(response)?;
            ensure!(
                receipt.operation_id == payload.operation_id
                    && receipt.record_id == payload.id
                    && receipt.saved_revision == payload.expected_revision.map_or(0, |n| n + 1),
                "The registration result is uncertain. Refresh its stored history."
            );
            saved = Some(receipt);
            if self.current_access()? != before {
                return Err(AccessRevoked.into());
            }
        }
        let result: reg::Snapshot = decode(
            self.request(Method::POST, "rpc/government_registration")?
                .json(&json!({"p_employee_id":employee_id}))
                .send()?,
        )?;
        result.validate(before.company_id, employee_id, current.company.today())?;
        if let Some(receipt) = saved {
            ensure!(
                result.record.as_ref().is_some_and(
                    |r| r.id == receipt.record_id && r.revision >= receipt.saved_revision
                ) && result
                    .history
                    .iter()
                    .any(|e| e.id == receipt.operation_id && e.revision == receipt.saved_revision),
                "Saved registration history changed. Refresh the current record."
            );
        }
        if self.current_access()? != before {
            return Err(AccessRevoked.into());
        }
        Ok(result)
    }
    pub fn government_reconciliation(
        &mut self,
        current: &Snapshot,
        filter: crate::payroll_government::Filter,
        action: Option<&crate::payroll_government::Action>,
        destination: Option<&Path>,
    ) -> Result<crate::payroll_government::Reconciliation> {
        crate::payroll_reporting::authorize(current)?;
        filter.validate(current.company.today())?;
        ensure!(
            action.is_none() || destination.is_none(),
            "Save government changes separately from exporting a fresh worksheet."
        );
        let before = self.current_access()?;
        if before != current.access {
            return Err(AccessRevoked.into());
        }
        if let Some(a) = action {
            a.authorize(current)?;
            let (path, op) = a.endpoint()?;
            let response = self
                .request(Method::POST, path)?
                .json(&json!({"p_operation":op,"p_record":a.record}))
                .send()
                .map_err(|_| {
                    anyhow::anyhow!(
                        "The government write result is uncertain. Refresh records before retrying."
                    )
                })?;
            if response.status().is_server_error() {
                bail!(
                    "The government change may have been saved. Refresh records before retrying."
                );
            }
            let saved: Value = decode(response)?;
            if saved["id"].as_str().and_then(|v| Uuid::parse_str(v).ok()) != Some(a.id()?) {
                bail!(
                    "The government change may have been saved. Refresh records before retrying."
                );
            }
            if self.current_access()? != before {
                return Err(AccessRevoked.into());
            }
        }
        let wire: crate::payroll_government::ReviewWire = decode(
            self.request(Method::POST, "rpc/government_reconciliation")?
                .json(&json!({"p_month":filter.month,"p_agency":filter.agency}))
                .send()?,
        )?;
        let report =
            crate::payroll_government::Reconciliation::from_wire(wire, before.company_id, filter)?;
        let bytes = destination.map(|path| report.export(path)).transpose()?;
        if self.current_access()? != before {
            return Err(AccessRevoked.into());
        }
        if let (Some(path), Some(bytes)) = (destination, bytes) {
            crate::reports::save_export(path, &bytes)?;
        }
        Ok(report)
    }
    pub fn government_payroll_sources(
        &mut self,
        current: &Snapshot,
        month: chrono::NaiveDate,
        agency: crate::payroll_government::Agency,
    ) -> Result<crate::payroll_government::Wire> {
        crate::payroll_reporting::authorize(current)?;
        ensure!(
            month.day() == 1 && month.year() >= 2025 && month <= current.company.today(),
            "Choose an elapsed/current calendar month from2025 onward."
        );
        let before = self.current_access()?;
        if before != current.access {
            return Err(AccessRevoked.into());
        }
        let wire: crate::payroll_government::Wire = decode(
            self.request(Method::POST, "rpc/government_payroll_sources")?
                .json(&json!({"p_month":month,"p_agency":agency}))
                .send()?,
        )?;
        wire.validate(before.company_id, agency, month)?;
        if self.current_access()? != before {
            return Err(AccessRevoked.into());
        }
        Ok(wire)
    }
    pub fn consolidated_payroll_report(
        &mut self,
        current: &Snapshot,
        filter: &crate::payroll_reporting::Filter,
        destination: Option<&Path>,
    ) -> Result<crate::payroll_reporting::Report> {
        crate::payroll_reporting::authorize(current)?;
        filter.validate()?;
        let before = self.current_access()?;
        if before != current.access {
            return Err(AccessRevoked.into());
        }
        let wire: crate::payroll_reporting::Wire = decode(
            self.request(Method::POST, "rpc/payroll_report_v2")?
                .json(&json!({"p_filter":filter}))
                .send()?,
        )?;
        let report = crate::payroll_reporting::Report::from_wire(wire, before.company_id, filter)?;
        if self.current_access()? != before {
            return Err(AccessRevoked.into());
        }
        if let Some(path) = destination {
            crate::reports::save_csv(path, &report.csv()?)?;
        }
        Ok(report)
    }
}
