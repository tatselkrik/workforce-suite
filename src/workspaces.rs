//! Native navigation visibility. Protected data authority remains in Access.
use crate::{access::Access, domain::Snapshot, staff_roles::Capability};
use anyhow::{Result, ensure};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Module {
    Organization,
    Development,
    Hiring,
    ExitClearance,
    Payroll,
    Reports,
}
impl Module {
    pub const ALL: [Self; 6] = [
        Self::Organization,
        Self::Development,
        Self::Hiring,
        Self::ExitClearance,
        Self::Payroll,
        Self::Reports,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Organization => "Organization chart",
            Self::Development => "Development",
            Self::Hiring => "Hiring",
            Self::ExitClearance => "Exit & clearance",
            Self::Payroll => "My payslips",
            Self::Reports => "Reports",
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            Self::Organization => "Reporting relationships and organization charts.",
            Self::Development => "Goals, appraisals, training, certifications and career plans.",
            Self::Hiring => "Applicants, interviews, offers and onboarding tasks.",
            Self::ExitClearance => "Separation and assigned clearance tasks.",
            Self::Payroll => {
                "Personal published advice in HRIS. Payroll Desktop is a separate application."
            }
            Self::Reports => "Attendance, leave and workforce reports.",
        }
    }
}
fn canonical(modules: &[Module]) -> Result<Vec<Module>> {
    let set: BTreeSet<_> = modules.iter().copied().collect();
    ensure!(
        set.len() == modules.len() && modules.len() <= 6,
        "Optional sections must be distinct and supported."
    );
    Ok(set.into_iter().collect())
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Context {
    pub schema_version: u16,
    pub company_id: Uuid,
    pub user_id: Uuid,
    pub revision: i64,
    pub enabled: Vec<Module>,
    pub updated_at: Option<DateTime<Utc>>,
}
impl Default for Context {
    fn default() -> Self {
        Self {
            schema_version: 0,
            company_id: Uuid::nil(),
            user_id: Uuid::nil(),
            revision: 0,
            enabled: Module::ALL.to_vec(),
            updated_at: None,
        }
    }
}
impl Context {
    pub fn legacy(access: &Access) -> Self {
        Self {
            company_id: access.company_id,
            user_id: access.user_id,
            ..Self::default()
        }
    }
    pub fn available(&self, module: Module) -> bool {
        self.enabled.contains(&module)
    }
    pub fn validate(&self, access: &Access) -> Result<()> {
        ensure!(
            access.uses_desktop()
                && self.company_id == access.company_id
                && self.user_id == access.user_id
                && !self.company_id.is_nil(),
            "Desktop sections do not match the current company/person."
        );
        ensure!(
            self.enabled == canonical(&self.enabled)? && self.revision >= 0,
            "Desktop visibility is incomplete or invalid."
        );
        ensure!(
            self.revision != 0 || self.enabled == Module::ALL,
            "An unset desktop configuration must preserve all sections."
        );
        ensure!(
            matches!(self.schema_version, 0 | 1),
            "Upgrade HRIS to read this desktop configuration."
        );
        ensure!(
            (self.revision == 0) == self.updated_at.is_none(),
            "Desktop revision and date disagree."
        );
        ensure!(
            self.schema_version != 0 || (self.revision == 0 && self.enabled == Module::ALL),
            "Unavailable settings must preserve prior visibility."
        );
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Save {
    pub id: Uuid,
    pub company_id: Uuid,
    pub actor_id: Uuid,
    pub expected_revision: i64,
    pub enabled: Vec<Module>,
}
impl Save {
    pub fn new(context: &Context, access: &Access, enabled: &[Module]) -> Result<Self> {
        context.validate(access)?;
        ensure!(
            access.allows(Capability::CompanyConfiguration) && context.schema_version == 1,
            "Only a current Administrator configures supported desktop sections."
        );
        Ok(Self {
            id: Uuid::new_v4(),
            company_id: access.company_id,
            actor_id: access.user_id,
            expected_revision: context.revision,
            enabled: canonical(enabled)?,
        })
    }
    pub fn validate(&self, context: &Context, access: &Access) -> Result<()> {
        context.validate(access)?;
        ensure!(
            !self.id.is_nil()
                && self.actor_id == access.user_id
                && self.company_id == access.company_id
                && self.expected_revision >= 0,
            "Visibility request identity or scope changed."
        );
        ensure!(
            access.allows(Capability::CompanyConfiguration) && context.schema_version == 1,
            "Current Administrator access is required."
        );
        ensure!(
            self.enabled == canonical(&self.enabled)?,
            "Visibility request is not canonical."
        );
        // Exact retries may carry an older revision; server receipts bind the entire original payload.
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub schema_version: u16,
    pub id: Uuid,
    pub company_id: Uuid,
    pub actor_id: Uuid,
    pub revision: i64,
    pub enabled: Vec<Module>,
    pub applied_at: DateTime<Utc>,
}
impl Receipt {
    pub fn validate(&self, request: &Save) -> Result<()> {
        ensure!(
            self.schema_version == 1
                && self.id == request.id
                && self.company_id == request.company_id
                && self.actor_id == request.actor_id
                && self.revision == request.expected_revision.checked_add(1).unwrap_or(-1)
                && self.enabled == request.enabled,
            "Desktop visibility receipt does not match this exact change."
        );
        Ok(())
    }
}
#[derive(Clone, Debug)]
pub struct Draft {
    pub source: Context,
    pub enabled: Vec<Module>,
    pub pending: Option<Save>,
}
impl Draft {
    pub fn changed(&self) -> bool {
        self.pending.is_some() || self.enabled != self.source.enabled
    }
    pub fn stale(&self, current: &Context) -> bool {
        self.source != *current
    }
    pub fn request(&mut self, current: &Context, access: &Access) -> Result<Save> {
        if let Some(request) = &self.pending {
            request.validate(current, access)?;
            return Ok(request.clone());
        }
        ensure!(
            !self.stale(current),
            "Desktop settings changed. Cancel and review their current values before saving."
        );
        let request = Save::new(&self.source, access, &self.enabled)?;
        self.pending = Some(request.clone());
        Ok(request)
    }
    pub fn accept(&self, receipt: &Receipt) -> Result<()> {
        receipt.validate(
            self.pending
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("No visibility save is pending."))?,
        )
    }
}
impl From<&Context> for Draft {
    fn from(source: &Context) -> Self {
        Self {
            source: source.clone(),
            enabled: source.enabled.clone(),
            pending: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Payroll,
    Overview,
    Employees,
    Attendance,
    Leave,
    Reports,
    Hiring,
    Exits,
    Accounts,
    Settings,
    Profile,
    Notifications,
}
impl Page {
    pub const ALL: [Self; 10] = [
        Self::Overview,
        Self::Employees,
        Self::Attendance,
        Self::Leave,
        Self::Payroll,
        Self::Reports,
        Self::Hiring,
        Self::Exits,
        Self::Accounts,
        Self::Settings,
    ];
    pub fn title(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Employees => "Employees",
            Self::Attendance => "Attendance",
            Self::Leave => "Leave",
            Self::Payroll => "Payroll",
            Self::Reports => "Reports",
            Self::Hiring => "Hiring",
            Self::Exits => "Exit & clearance",
            Self::Accounts => "Account Manager",
            Self::Settings => "Settings",
            Self::Profile => "Profile",
            Self::Notifications => "Notifications",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Work,
    Personal,
    Administration,
}
impl Group {
    pub fn label(self) -> &'static str {
        match self {
            Self::Work => "WORKSPACES",
            Self::Personal => "MY WORK",
            Self::Administration => "ADMINISTRATION",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub page: Page,
    pub label: &'static str,
    pub group: Option<Group>,
}
pub fn people_directory(access: &Access) -> bool {
    access.allows(Capability::Personnel) || access.allows(Capability::TeamReview)
}
pub fn development(data: &Snapshot) -> bool {
    data.workspaces.available(Module::Development) && data.access.uses_desktop()
}
pub fn organization(data: &Snapshot) -> bool {
    people_directory(&data.access) && data.workspaces.available(Module::Organization)
}
pub fn hiring(data: &Snapshot) -> bool {
    data.workspaces.available(Module::Hiring)
        && (data.access.allows(Capability::Recruiting)
            || !data.recruiting.interviews.is_empty()
            || !data.onboarding.tasks.is_empty())
}
pub fn exits(data: &Snapshot) -> bool {
    data.workspaces.available(Module::ExitClearance)
        && (data.access.allows(Capability::Personnel) || !data.exits.tasks.is_empty())
}
pub fn visible(page: Page, data: &Snapshot) -> bool {
    if !data.access.uses_desktop() {
        return false;
    }
    match page {
        Page::Overview
        | Page::Attendance
        | Page::Leave
        | Page::Settings
        | Page::Profile
        | Page::Notifications => true,
        Page::Employees => people_directory(&data.access) || development(data),
        Page::Accounts => data.access.allows(Capability::AccountAdministration),
        Page::Hiring => hiring(data),
        Page::Exits => exits(data),
        Page::Payroll => data.workspaces.available(Module::Payroll),
        Page::Reports => {
            data.workspaces.available(Module::Reports)
                && (data.access.allows(Capability::GeneralHrReports)
                    || data.access.allows(Capability::TeamReview))
        }
    }
}
pub fn entries(data: &Snapshot) -> Vec<Entry> {
    if !data.access.uses_desktop() {
        return Vec::new();
    }
    let a = &data.access;
    let mut result = vec![Entry {
        page: Page::Overview,
        label: "Overview",
        group: None,
    }];
    let mut add = |page, label, group| {
        if visible(page, data) {
            result.push(Entry {
                page,
                label,
                group: Some(group),
            })
        }
    };
    if people_directory(a) {
        add(Page::Employees, "Employees", Group::Work)
    }
    if a.allows(Capability::Recruiting) {
        add(Page::Hiring, "Hiring", Group::Work)
    }
    if !people_directory(a) && development(data) {
        add(
            Page::Employees,
            "Development",
            if a.allows(Capability::DevelopmentAdministration) {
                Group::Work
            } else {
                Group::Personal
            },
        )
    }
    if a.allows(Capability::PayrollPreparation) {
        add(Page::Payroll, "Payroll", Group::Work)
    }
    if a.allows(Capability::AttendanceAdministration) {
        add(Page::Attendance, "Attendance", Group::Work);
        add(Page::Leave, "Leave", Group::Work)
    }
    add(Page::Reports, "Reports", Group::Work);
    if a.allows(Capability::Personnel) {
        add(Page::Exits, "Exit & clearance", Group::Work)
    }
    if !a.allows(Capability::AttendanceAdministration) {
        add(
            Page::Attendance,
            if a.allows(Capability::TeamReview) {
                "Team attendance"
            } else {
                "My attendance"
            },
            Group::Personal,
        );
        add(
            Page::Leave,
            if a.allows(Capability::TeamReview) {
                "Team leave"
            } else {
                "My leave"
            },
            Group::Personal,
        )
    }
    if !a.allows(Capability::Recruiting) {
        add(Page::Hiring, "Hiring tasks", Group::Personal)
    }
    if !a.allows(Capability::Personnel) {
        add(Page::Exits, "Clearance tasks", Group::Personal)
    }
    if !a.allows(Capability::PayrollPreparation) {
        add(Page::Payroll, "My payslips", Group::Personal)
    }
    add(Page::Accounts, "Account Manager", Group::Administration);
    add(Page::Settings, "Settings", Group::Administration);
    result.sort_by_key(|e| match e.group {
        None => 0,
        Some(Group::Work) => 1,
        Some(Group::Personal) => 2,
        Some(Group::Administration) => 3,
    });
    result
}
pub fn landing(data: &Snapshot) -> Page {
    let a = &data.access;
    if people_directory(a) {
        return Page::Overview;
    }
    for (cap, page) in [
        (Capability::Recruiting, Page::Hiring),
        (Capability::DevelopmentAdministration, Page::Employees),
        (Capability::PayrollPreparation, Page::Payroll),
    ] {
        if a.allows(cap) && visible(page, data) {
            return page;
        }
    }
    Page::Overview
}
pub fn label(page: Page, data: &Snapshot) -> &'static str {
    entries(data)
        .into_iter()
        .find(|e| e.page == page)
        .map(|e| e.label)
        .unwrap_or(page.title())
}
pub fn prefers_development(data: &Snapshot) -> bool {
    !people_directory(&data.access) && development(data)
}

#[derive(Debug)]
pub struct Rejected;
impl std::fmt::Display for Rejected {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Nothing was saved. Refresh and review the current desktop sections before another change."
        )
    }
}
impl std::error::Error for Rejected {}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Section {
    #[default]
    Directory,
    Organization,
    Development,
    Relations,
}
pub fn hidden(page: Page, section: Section, data: &Snapshot) -> bool {
    !visible(page, data)
        || (page == Page::Employees
            && match section {
                Section::Organization => !organization(data),
                Section::Development => !development(data),
                Section::Directory => !people_directory(&data.access),
                Section::Relations => !crate::relations::eligible(&data.access),
            })
}
pub fn reconcile(
    page: Page,
    section: Section,
    data: &Snapshot,
    has_draft: bool,
) -> (Page, Section) {
    if has_draft {
        return (page, section);
    }
    if !visible(page, data) {
        let target = landing(data);
        return (
            target,
            if target == Page::Employees && prefers_development(data) {
                Section::Development
            } else {
                section
            },
        );
    }
    if page == Page::Employees {
        if prefers_development(data) {
            return (page, Section::Development);
        }
        if hidden(page, section, data) {
            return (page, Section::Directory);
        }
    }
    (page, section)
}
