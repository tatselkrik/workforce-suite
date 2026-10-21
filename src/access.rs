use crate::{
    account,
    domain::{Action, EmployeeStatus, LeaveStatus, Snapshot},
};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const EMPLOYEE_PORTAL_NOTICE: &str = "This employee account is for the employee portal, which is planned for a later update. Desktop access requires an assigned staff role. Contact your administrator.";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Administrator,
    HrGeneralist,
    Manager,
    #[default]
    Employee,
}

impl Role {
    pub const ALL: [Self; 4] = [
        Self::Administrator,
        Self::HrGeneralist,
        Self::Manager,
        Self::Employee,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Administrator => "Administrator",
            Self::HrGeneralist => "HR Generalist",
            Self::Manager => "Manager",
            Self::Employee => "Employee",
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            Self::Administrator => {
                "Accounts, company settings and all HR records. Cannot review own leave."
            }
            Self::HrGeneralist => {
                "Manage employee and attendance records; review leave. Cannot manage accounts or company settings."
            }
            Self::Manager => {
                "View own and direct reports' records; review direct reports' leave. Record own attendance and leave."
            }
            Self::Employee => {
                "Employee portal (planned): own record, attendance and leave. No desktop access."
            }
        }
    }
    pub fn manages_hr(self) -> bool {
        matches!(self, Self::Administrator | Self::HrGeneralist)
    }
    pub fn uses_desktop(self) -> bool {
        self != Self::Employee
    }
    pub fn manages_accounts(self) -> bool {
        self == Self::Administrator
    }
    pub fn can_manage_reports(self) -> bool {
        self != Self::Employee
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Membership {
    pub company_id: Uuid,
    pub user_id: Uuid,
    pub role: Role,
    pub active: bool,
    pub employee_id: Option<Uuid>,
    pub manager_user_id: Option<Uuid>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct Access {
    pub company_id: Uuid,
    pub user_id: Uuid,
    pub role: Role,
    pub employee_id: Option<Uuid>,
    pub direct_reports: Vec<Uuid>,
}

impl Access {
    pub fn can_read_contacts(&self, employee: Uuid) -> bool {
        self.role.manages_hr() || self.employee_id == Some(employee)
    }

    pub fn demo_admin() -> Self {
        Self {
            company_id: Uuid::from_u128(1),
            role: Role::Administrator,
            ..Self::default()
        }
    }
    pub fn can_record(&self, employee: Uuid) -> bool {
        self.role.manages_hr() || self.employee_id == Some(employee)
    }
    pub fn can_review(&self, employee: Uuid) -> bool {
        self.employee_id != Some(employee)
            && (self.role.manages_hr()
                || (self.role == Role::Manager && self.direct_reports.contains(&employee)))
    }
    pub fn authorize(&self, action: &Action, data: &Snapshot) -> Result<()> {
        let allowed = match action {
            Action::SaveEmployee(_) | Action::SaveEmployeeContacts(_) => self.role.manages_hr(),
            Action::SaveCompany(_) => self.role.manages_accounts(),
            Action::CheckIn(id) => self.can_record(*id),
            Action::CheckOut(id) => data
                .attendance
                .iter()
                .find(|r| r.id == *id)
                .is_some_and(|r| self.can_record(r.employee_id)),
            Action::RequestLeave(draft) => draft.employee_id.is_some_and(|id| self.can_record(id)),
            Action::DecideLeave(id, status) => {
                *status != LeaveStatus::Pending
                    && data
                        .leave
                        .iter()
                        .find(|r| r.id == *id)
                        .is_some_and(|r| self.can_review(r.employee_id))
            }
        };
        ensure!(
            allowed,
            "Your role does not permit this action. Refresh to check your current access."
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagedAccount {
    pub user_id: Uuid,
    pub email: String,
    pub role: Role,
    pub active: bool,
    pub employee_id: Option<Uuid>,
    pub manager_user_id: Option<Uuid>,
    pub email_confirmed: bool,
    pub is_owner: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccessEvent {
    pub id: Uuid,
    pub actor_id: Uuid,
    pub user_id: Uuid,
    pub action: String,
    pub created_at: String,
    pub details: serde_json::Value,
}

// Initial credentials belong only in memory; do not derive Debug or Serialize.
#[derive(Default)]
pub struct AccountDraft {
    pub user_id: Option<Uuid>,
    pub email: String,
    pub role: Role,
    pub active: bool,
    pub employee_id: Option<Uuid>,
    pub manager_user_id: Option<Uuid>,
    pub create_new: bool,
    pub password: String,
    pub confirmation: String,
}

impl AccountDraft {
    pub fn new() -> Self {
        Self {
            active: true,
            create_new: true,
            ..Self::default()
        }
    }
    pub fn from_account(a: &ManagedAccount) -> Self {
        Self {
            user_id: Some(a.user_id),
            email: a.email.clone(),
            role: a.role,
            active: a.active,
            employee_id: a.employee_id,
            manager_user_id: a.manager_user_id,
            ..Self::default()
        }
    }
    pub fn validate(&self, data: &Snapshot) -> Result<()> {
        account::validate_email(&self.email)?;
        if self.active && matches!(self.role, Role::Employee | Role::Manager) {
            ensure!(
                self.employee_id.is_some(),
                "Link an employee record for this role."
            );
        }
        if let Some(id) = self.employee_id {
            ensure!(
                data.employees
                    .iter()
                    .any(|e| e.id == id && (e.status == EmployeeStatus::Active || !self.active)),
                "Choose an active employee in this company."
            );
            ensure!(
                !self.active
                    || !data.accounts.iter().any(|a| a.active
                        && a.employee_id == Some(id)
                        && Some(a.user_id) != self.user_id),
                "This employee already has an active account. Edit or deactivate that account first."
            );
        }
        if let Some(id) = self.manager_user_id {
            ensure!(
                Some(id) != self.user_id,
                "An account cannot report to itself."
            );
            ensure!(
                data.accounts
                    .iter()
                    .any(|a| a.user_id == id && a.active && a.role.can_manage_reports()),
                "Choose an active Administrator, HR Generalist or Manager."
            );
        }
        if self.user_id.is_none() && self.create_new {
            ensure!(
                (12..=128).contains(&self.password.chars().count()),
                "Use an initial password between 12 and 128 characters."
            );
            ensure!(
                self.password == self.confirmation,
                "The initial passwords do not match."
            );
        }
        Ok(())
    }
}

pub enum AdminAction {
    Save(AccountDraft),
    ResendConfirmation(Uuid),
}
