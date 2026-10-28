//! Payroll's native loader intentionally does not fetch personnel cases/files or HR configuration.
use super::*;
impl CloudClient {
    pub(super) fn payroll_desktop_snapshot(&mut self) -> Result<Snapshot> {
        let access = self.current_access()?;
        ensure!(
            crate::desktop::Product::Payroll.admits(&access),
            "Payroll responsibility is required."
        );
        let mut companies: Vec<Company> = self.rows("companies", None)?;
        ensure!(
            companies.len() == 1,
            "This Payroll installation requires exactly one company."
        );
        let company = companies.remove(0);
        ensure!(
            company.id == access.company_id,
            "Payroll company scope changed."
        );
        let mut employees: Vec<Employee> = self.rows("employees", Some(company.id))?;
        ensure!(
            employees.iter().all(|e| e.company_id == company.id),
            "Payroll employee scope changed."
        );
        employees.sort_by(|a, b| a.full_name.cmp(&b.full_name));
        let own = access
            .employee_id
            .context("Payroll access requires an employee record.")?;
        let contacts: Vec<crate::contacts::EmployeeContacts> =
            self.scoped_rows("employee_contacts", Some(company.id), Some(own))?;
        ensure!(
            contacts.len() <= 1
                && contacts
                    .iter()
                    .all(|c| c.company_id == company.id && c.employee_id == own),
            "Own contact scope changed."
        );
        let employment_types = self.rows("employment_types", Some(company.id))?;
        let work_locations = self.rows("work_locations", Some(company.id))?;
        let payroll = self.payroll_snapshot(&access)?;
        let (logo_png, logo_warning) = self.company_logo(&company);
        // Shared personal notices retain their original read identities. The Payroll UI filters
        // their validated projection; it never resets unrelated HR read history.
        let lifecycle: crate::notifications::lifecycle::Inbox = decode(
            self.request(Method::POST, "rpc/lifecycle_notifications")?
                .json(&json!({"p_surface":"desktop"}))
                .send()?,
        )?;
        if lifecycle.company_id != access.company_id
            || lifecycle.user_id != access.user_id
            || lifecycle.employee_id != access.employee_id
            || lifecycle.role != access.role
        {
            return Err(AccessRevoked.into());
        }
        lifecycle.validate(&access, company.today())?;
        ensure!(
            lifecycle
                .items
                .iter()
                .all(|i| employees.iter().any(|e| e.id == i.employee_id)),
            "Payroll notice subject is outside the employee directory."
        );
        let mut snapshot = Snapshot::signed_out();
        snapshot.workspaces = crate::workspaces::Context::legacy(&access);
        snapshot.notification_reads = crate::notifications::ReadState {
            company_id: company.id,
            user_id: access.user_id,
            ..Default::default()
        };
        snapshot.lifecycle_notifications = Some(lifecycle);
        snapshot.company = company;
        snapshot.access = access;
        snapshot.employees = employees;
        snapshot.contacts = contacts;
        snapshot.employment_types = employment_types;
        snapshot.work_locations = work_locations;
        snapshot.payroll = payroll;
        snapshot.logo_png = logo_png;
        snapshot.logo_warning = logo_warning;
        snapshot.mode = Mode::Supabase;
        self.verify_snapshot_staff_scope(&snapshot.access)?;
        Ok(snapshot)
    }
}
