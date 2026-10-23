use anyhow::{Result, bail, ensure};
use chrono::{DateTime, Datelike, Days, NaiveDate, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Company {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub name: String,
    pub timezone: String,
    pub annual_leave_days: u32,
    #[serde(default)]
    pub logo_version: Option<Uuid>,
}

impl Default for Company {
    fn default() -> Self {
        Self {
            id: Uuid::from_u128(1),
            owner_id: Uuid::nil(),
            name: "Northstar Company".into(),
            timezone: "Asia/Manila".into(),
            annual_leave_days: 15,
            logo_version: None,
        }
    }
}

impl Company {
    pub fn tz(&self) -> Tz {
        self.timezone.parse().unwrap_or(chrono_tz::UTC)
    }

    pub fn today(&self) -> NaiveDate {
        Utc::now().with_timezone(&self.tz()).date_naive()
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(!self.name.trim().is_empty(), "Enter a company name.");
        ensure!(
            self.name.chars().count() <= 100,
            "Company name is too long."
        );
        ensure!(
            self.timezone.parse::<Tz>().is_ok(),
            "Use a valid IANA time zone, such as Asia/Manila."
        );
        ensure!(
            self.annual_leave_days <= 366,
            "Annual leave must be between 0 and 366 days."
        );
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EmployeeStatus {
    #[default]
    Active,
    Archived,
}

impl EmployeeStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Active => "Active",
            Self::Archived => "Archived",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Employee {
    pub id: Uuid,
    pub company_id: Uuid,
    pub employee_code: String,
    pub full_name: String,
    pub department: String,
    pub job_title: String,
    pub joined_on: Option<NaiveDate>,
    #[serde(default)]
    pub reports_to: Option<Uuid>,
    #[serde(default)]
    pub employment_type_id: Option<Uuid>,
    #[serde(default)]
    pub work_location_id: Option<Uuid>,
    pub annual_leave_days: u32,
    pub status: EmployeeStatus,
}

#[derive(Clone, Debug, Default)]
pub struct EmployeeDraft {
    pub id: Option<Uuid>,
    pub employee_code: String,
    pub full_name: String,
    pub department: String,
    pub job_title: String,
    pub joined_on: String,
    pub reports_to: Option<Uuid>,
    pub employment_type_id: Option<Uuid>,
    pub work_location_id: Option<Uuid>,
    pub annual_leave_days: u32,
    pub status: EmployeeStatus,
}

impl From<&Employee> for EmployeeDraft {
    fn from(e: &Employee) -> Self {
        Self {
            id: Some(e.id),
            employee_code: e.employee_code.clone(),
            full_name: e.full_name.clone(),
            department: e.department.clone(),
            job_title: e.job_title.clone(),
            joined_on: e.joined_on.map(|d| d.to_string()).unwrap_or_default(),
            reports_to: e.reports_to,
            employment_type_id: e.employment_type_id,
            work_location_id: e.work_location_id,
            annual_leave_days: e.annual_leave_days,
            status: e.status,
        }
    }
}

impl EmployeeDraft {
    pub fn employee(&self, company_id: Uuid) -> Result<Employee> {
        ensure!(
            !self.employee_code.trim().is_empty(),
            "Enter an employee code."
        );
        ensure!(
            self.employee_code.trim().chars().count() <= 30,
            "Employee code is too long."
        );
        ensure!(
            !self.full_name.trim().is_empty(),
            "Enter the employee's name."
        );
        ensure!(
            self.full_name.trim().chars().count() <= 120,
            "Employee name is too long."
        );
        ensure!(
            self.department.chars().count() <= 80 && self.job_title.chars().count() <= 100,
            "Department or job title is too long."
        );
        ensure!(
            self.annual_leave_days <= 366,
            "Annual leave must be between 0 and 366 days."
        );
        Ok(Employee {
            id: self.id.unwrap_or_else(Uuid::new_v4),
            company_id,
            employee_code: self.employee_code.trim().to_uppercase(),
            full_name: self.full_name.trim().into(),
            department: self.department.trim().into(),
            job_title: self.job_title.trim().into(),
            joined_on: if self.joined_on.trim().is_empty() {
                None
            } else {
                Some(parse_date(&self.joined_on)?)
            },
            reports_to: self.reports_to,
            employment_type_id: self.employment_type_id,
            work_location_id: self.work_location_id,
            annual_leave_days: self.annual_leave_days,
            status: self.status,
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Attendance {
    #[serde(default)]
    pub calculation_snapshot: Option<crate::calculations::CalculationSnapshot>,
    #[serde(default)]
    pub planned_shift: Option<crate::shifts::ShiftPlan>,
    pub id: Uuid,
    pub company_id: Uuid,
    pub employee_id: Uuid,
    pub work_date: NaiveDate,
    pub checked_in_at: DateTime<Utc>,
    pub checked_out_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub revision: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LeaveKind {
    #[default]
    Annual,
    Sick,
    Unpaid,
    Custom,
}

impl LeaveKind {
    pub const ALL: [Self; 3] = [Self::Annual, Self::Sick, Self::Unpaid];
    pub fn label(self) -> &'static str {
        match self {
            Self::Annual => "Annual leave",
            Self::Sick => "Sick leave",
            Self::Unpaid => "Unpaid leave",
            Self::Custom => "Custom leave",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeaveStatus {
    Pending,
    Approved,
    Rejected,
    Cancelled,
}

impl LeaveStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pending => "Pending",
            Self::Approved => "Approved",
            Self::Rejected => "Rejected",
            Self::Cancelled => "Cancelled",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct LeaveRequest {
    #[serde(flatten)]
    pub policy: crate::leave::RequestPolicy,
    pub id: Uuid,
    pub company_id: Uuid,
    pub employee_id: Uuid,
    pub kind: LeaveKind,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    pub reason: String,
    pub status: LeaveStatus,
}

#[derive(Clone, Debug, Default)]
pub struct LeaveDraft {
    pub options: crate::leave::LeaveOptions,
    pub employee_id: Option<Uuid>,
    pub kind: LeaveKind,
    pub starts_on: String,
    pub ends_on: String,
    pub reason: String,
}

impl LeaveDraft {
    pub fn request(&self, company_id: Uuid) -> Result<LeaveRequest> {
        let starts_on = parse_date(&self.starts_on)?;
        let ends_on = parse_date(&self.ends_on)?;
        ensure!(
            ends_on >= starts_on,
            "End date must be on or after the start date."
        );
        ensure!(
            starts_on.year() == ends_on.year(),
            "Split requests that cross a calendar year into two requests."
        );
        ensure!(
            self.reason.chars().count() <= 500,
            "Keep the note to 500 characters."
        );
        Ok(LeaveRequest {
            policy: Default::default(),
            id: Uuid::new_v4(),
            company_id,
            employee_id: self
                .employee_id
                .ok_or_else(|| anyhow::anyhow!("Choose an employee."))?,
            kind: self.kind,
            starts_on,
            ends_on,
            reason: self.reason.trim().into(),
            status: LeaveStatus::Pending,
        })
    }
}

pub fn parse_date(value: &str) -> Result<NaiveDate> {
    let value = value.trim();
    let date = NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| anyhow::anyhow!("Enter a valid date as YYYY-MM-DD."))?;
    ensure!(
        date.format("%Y-%m-%d").to_string() == value && (1900..=2200).contains(&date.year()),
        "Use YYYY-MM-DD and a year between 1900 and 2200."
    );
    Ok(date)
}

pub fn weekdays(start: NaiveDate, end: NaiveDate) -> u32 {
    if end < start {
        return 0;
    }
    let count = end.signed_duration_since(start).num_days() as u64 + 1;
    let full_weeks = count / 7;
    let remainder = count % 7;
    (full_weeks * 5
        + (0..remainder)
            .filter(|i| {
                start
                    .checked_add_days(Days::new(*i))
                    .is_some_and(|d| d.weekday().number_from_monday() <= 5)
            })
            .count() as u64) as u32
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    SignedOut,
    Demo,
    Supabase,
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub payroll: crate::payroll::Setup,
    pub learning: crate::learning::Setup,
    pub career: crate::career::Setup,
    pub performance: crate::performance::Setup,
    pub exits: crate::exits::Setup,
    pub onboarding: crate::onboarding::Setup,
    pub recruiting: crate::recruiting::Setup,
    pub hiring: crate::hiring::Setup,
    pub notification_reads: crate::notifications::ReadState,
    pub imports: crate::imports::Setup,
    pub overtime: crate::overtime::Setup,
    pub requests: crate::requests::Setup,
    pub calculations: crate::calculations::CalculationSetup,
    pub breaks: crate::breaks::BreakSetup,
    pub shifts: crate::shifts::ShiftSetup,
    pub leave_setup: crate::leave::LeaveSetup,
    pub company: Company,
    pub logo_png: Option<Vec<u8>>,
    pub logo_warning: Option<String>,
    pub employees: Vec<Employee>,
    pub contacts: Vec<crate::contacts::EmployeeContacts>,
    pub attendance: Vec<Attendance>,
    pub leave: Vec<LeaveRequest>,
    pub mode: Mode,
    pub access: crate::access::Access,
    pub accounts: Vec<crate::access::ManagedAccount>,
    pub access_events: Vec<crate::access::AccessEvent>,
    pub corrections: Vec<crate::corrections::CorrectionEvent>,
    pub employment_types: Vec<crate::employment::EmploymentOption>,
    pub work_locations: Vec<crate::employment::EmploymentOption>,
    pub employment_history: Vec<crate::history::EmploymentEvent>,
    pub documents: Vec<crate::documents::EmployeeDocument>,
    pub document_events: Vec<crate::documents::DocumentEvent>,
}

impl Snapshot {
    pub fn signed_out() -> Self {
        Self {
            recruiting: Default::default(),
            onboarding: Default::default(),
            exits: Default::default(),
            learning: Default::default(),
            payroll: Default::default(),
            career: Default::default(),
            performance: Default::default(),
            hiring: Default::default(),
            notification_reads: Default::default(),
            imports: Default::default(),
            overtime: Default::default(),
            requests: Default::default(),
            calculations: Default::default(),
            breaks: Default::default(),
            shifts: Default::default(),
            leave_setup: Default::default(),
            company: Company {
                id: Uuid::nil(),
                name: "HRIS".into(),
                ..Company::default()
            },
            employees: Vec::new(),
            logo_png: None,
            logo_warning: None,
            contacts: Vec::new(),
            attendance: Vec::new(),
            leave: Vec::new(),
            mode: Mode::SignedOut,
            access: crate::access::Access::default(),
            accounts: Vec::new(),
            access_events: Vec::new(),
            corrections: Vec::new(),
            employment_types: Vec::new(),
            work_locations: Vec::new(),
            employment_history: Vec::new(),
            documents: Vec::new(),
            document_events: Vec::new(),
        }
    }

    pub fn employee_name(&self, id: Uuid) -> &str {
        self.employees
            .iter()
            .find(|e| e.id == id)
            .map(|e| e.full_name.as_str())
            .or_else(|| {
                self.own_employee()
                    .filter(|e| e.reports_to == Some(id))
                    .and(self.access.reports_to_name.as_deref())
            })
            .unwrap_or("Unknown employee")
    }

    pub fn own_employee(&self) -> Option<&Employee> {
        self.access
            .employee_id
            .and_then(|id| self.employees.iter().find(|e| e.id == id))
    }

    pub fn own_contacts(&self) -> Option<&crate::contacts::EmployeeContacts> {
        self.access
            .employee_id
            .and_then(|id| self.contacts.iter().find(|c| c.employee_id == id))
    }

    pub fn annual_used(&self, id: Uuid, year: i32) -> u32 {
        self.leave
            .iter()
            .filter(|r| {
                r.employee_id == id
                    && r.kind == LeaveKind::Annual
                    && r.status == LeaveStatus::Approved
                    && r.starts_on.year() == year
            })
            .map(|r| r.units() as u32)
            .sum::<u32>()
            .div_ceil(2)
    }

    pub fn active(&self) -> impl Iterator<Item = &Employee> {
        self.employees
            .iter()
            .filter(|e| e.status == EmployeeStatus::Active)
    }
}

#[derive(Clone, Debug)]
pub enum Action {
    Payroll(crate::payroll::PayrollAction),
    Learning(crate::learning::LearningAction),
    Career(crate::career::CareerAction),
    Performance(crate::performance::PerformanceAction),
    Exit(crate::exits::ExitAction),
    Onboarding(crate::onboarding::HiringAction),
    Interview(crate::recruiting::InterviewAction),
    Hiring(crate::hiring::HiringAction),
    NotificationReads(crate::notifications::ReadAction),
    ImportEmployees(crate::imports::Preview, String),
    Overtime(crate::overtime::OvertimeAction),
    AttendanceRequest(crate::requests::RequestAction),
    SaveCalculationPolicy(crate::calculations::PolicyDraft),
    ApplyCalculationPolicy(crate::calculations::ApplyPolicy),
    AttendanceBreak(crate::breaks::BreakAction),
    SaveShiftTemplate(crate::shifts::ShiftTemplateDraft),
    AssignShift(crate::shifts::ShiftAssignmentDraft),
    SaveEmployee(EmployeeDraft),
    SaveEmployeeRecord(EmployeeDraft, crate::contacts::EmployeeContacts),
    SaveEmployeeContacts(crate::contacts::EmployeeContacts),
    SaveOwnContacts(crate::contacts::EmployeeContacts),
    UploadCompanyLogo(std::path::PathBuf),
    RemoveCompanyLogo,
    CheckIn(Uuid),
    CheckOut(Uuid),
    CorrectAttendance(crate::corrections::CorrectionChange),
    RequestLeave(LeaveDraft),
    DecideLeave(Uuid, LeaveStatus),
    ReviewLeave(Uuid, i64, bool, String),
    CancelLeave(Uuid, i64, String),
    SaveLeavePolicy(crate::leave::LeavePolicy),
    SaveCompany(Company),
    SaveEmploymentOption(crate::employment::EmploymentOptionDraft),
}

pub struct DemoStore {
    pub data: Snapshot,
}

impl Default for DemoStore {
    fn default() -> Self {
        Self::new()
    }
}

impl DemoStore {
    fn validate_reporting(&self, employee: &Employee) -> Result<()> {
        crate::employment::validate_assignments(&self.data, employee)?;
        if let Some(manager) = employee.reports_to {
            ensure!(
                manager != employee.id,
                "An employee cannot report to themselves."
            );
            ensure!(
                self.data.active().any(|e| e.id == manager),
                "Choose an active employee as the reporting manager."
            );
            let mut chain = Some(manager);
            let mut visited = std::collections::HashSet::new();
            while let Some(id) = chain {
                ensure!(
                    id != employee.id && visited.insert(id),
                    "Reporting relationships cannot contain a cycle."
                );
                chain = self
                    .data
                    .employees
                    .iter()
                    .find(|e| e.id == id)
                    .and_then(|e| e.reports_to);
            }
        }
        ensure!(
            employee.status != EmployeeStatus::Archived
                || !self
                    .data
                    .active()
                    .any(|e| e.reports_to == Some(employee.id)),
            "Reassign direct reports before archiving this employee."
        );
        Ok(())
    }

    pub fn new() -> Self {
        let company = Company::default();
        let today = company.today();
        let roster = [
            ("Alex Rivera", "People Operations", "HR lead"),
            ("Jamie Santos", "Engineering", "Software engineer"),
            ("Taylor Chen", "Operations", "Operations associate"),
            ("Morgan Lee", "Finance", "Finance officer"),
            ("Sam Reyes", "Engineering", "Product designer"),
            ("Jordan Cruz", "Operations", "Team coordinator"),
            ("Casey Torres", "People Operations", "HR associate"),
            ("Riley Tan", "Finance", "Accounts assistant"),
        ];
        let employees: Vec<_> = roster
            .iter()
            .enumerate()
            .map(|(i, (name, department, title))| Employee {
                id: Uuid::from_u128(100 + i as u128),
                company_id: company.id,
                employee_code: format!("NS-{:03}", i + 1),
                full_name: (*name).into(),
                department: (*department).into(),
                job_title: (*title).into(),
                joined_on: Some(today - chrono::Duration::days(180 + 23 * i as i64)),
                reports_to: (i == 1).then_some(Uuid::from_u128(103)),
                employment_type_id: (i != 6).then_some(Uuid::from_u128(1000 + (i % 3) as u128)),
                work_location_id: (i != 6).then_some(Uuid::from_u128(2000 + (i % 2) as u128)),
                annual_leave_days: 15,
                status: if i == 7 {
                    EmployeeStatus::Archived
                } else {
                    EmployeeStatus::Active
                },
            })
            .collect();
        let now = Utc::now();
        let attendance = employees
            .iter()
            .take(5)
            .enumerate()
            .map(|(i, e)| Attendance {
                calculation_snapshot: None,
                planned_shift: None,
                revision: 0,
                id: Uuid::from_u128(200 + i as u128),
                company_id: company.id,
                employee_id: e.id,
                work_date: today,
                checked_in_at: now - chrono::Duration::minutes(150 + i as i64 * 5),
                checked_out_at: if i == 4 {
                    Some(now - chrono::Duration::minutes(10))
                } else {
                    None
                },
            })
            .collect();
        let next_workday = (0..7)
            .map(|i| today + chrono::Duration::days(i))
            .find(|d| d.weekday().number_from_monday() <= 5)
            .unwrap_or(today);
        let leave = [
            (1, LeaveKind::Annual, LeaveStatus::Pending),
            (3, LeaveKind::Sick, LeaveStatus::Pending),
            (5, LeaveKind::Annual, LeaveStatus::Approved),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, (employee, kind, status))| LeaveRequest {
            policy: Default::default(),
            id: Uuid::from_u128(300 + i as u128),
            company_id: company.id,
            employee_id: employees[employee].id,
            kind,
            starts_on: next_workday,
            ends_on: next_workday,
            reason: "Sample request".into(),
            status,
        })
        .collect();
        let mut store = Self {
            data: Snapshot {
                recruiting: Default::default(),
                onboarding: Default::default(),
            exits: Default::default(),
            learning: Default::default(),
            payroll: Default::default(),
            career: Default::default(),
            performance: Default::default(),
                hiring: Default::default(),
                notification_reads: crate::notifications::ReadState {
                    company_id: company.id,
                    ..Default::default()
                },
                imports: Default::default(),
                overtime: Default::default(),
                requests: Default::default(),
                calculations: crate::calculations::CalculationSetup {
                    policy: crate::calculations::CalculationPolicy {
                        company_id: company.id,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                breaks: Default::default(),
                shifts: Default::default(),
                leave_setup: crate::leave::LeaveSetup {
                    policy: crate::leave::LeavePolicy {
                        company_id: company.id,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                company,
                logo_png: None,
                logo_warning: None,
                employees,
                contacts: vec![crate::contacts::EmployeeContacts {
                    phone: "000 000 0000".into(),
                    personal_email: "synthetic.employee@example.invalid".into(),
                    address: "Fictional sample address — demo only".into(),
                    emergency_name: "Sample Emergency Contact".into(),
                    emergency_relationship: "Sibling".into(),
                    emergency_phone: "000 000 0001".into(),
                    ..crate::contacts::EmployeeContacts::empty(
                        Uuid::from_u128(1),
                        Uuid::from_u128(103),
                    )
                }],
                attendance,
                leave,
                mode: Mode::Demo,
                access: crate::access::Access::demo_admin(),
                accounts: vec![
                    crate::access::ManagedAccount {
                        user_id: Uuid::nil(),
                        email: "administrator@example.invalid".into(),
                        role: crate::access::Role::Administrator,
                        active: true,
                        employee_id: Some(Uuid::from_u128(100)),
                        manager_user_id: None,
                        email_confirmed: true,
                        is_owner: true,
                    },
                    crate::access::ManagedAccount {
                        user_id: Uuid::from_u128(11),
                        email: "hr@example.invalid".into(),
                        role: crate::access::Role::HrGeneralist,
                        active: true,
                        employee_id: Some(Uuid::from_u128(106)),
                        manager_user_id: None,
                        email_confirmed: true,
                        is_owner: false,
                    },
                    crate::access::ManagedAccount {
                        user_id: Uuid::from_u128(12),
                        email: "manager@example.invalid".into(),
                        role: crate::access::Role::Manager,
                        active: true,
                        employee_id: Some(Uuid::from_u128(103)),
                        manager_user_id: None,
                        email_confirmed: true,
                        is_owner: false,
                    },
                    crate::access::ManagedAccount {
                        user_id: Uuid::from_u128(13),
                        email: "employee@example.invalid".into(),
                        role: crate::access::Role::Employee,
                        active: true,
                        employee_id: Some(Uuid::from_u128(101)),
                        manager_user_id: Some(Uuid::from_u128(12)),
                        email_confirmed: true,
                        is_owner: false,
                    },
                ],
                access_events: Vec::new(),
                corrections: Vec::new(),
                employment_history: Vec::new(),
                documents: Vec::new(),
                document_events: Vec::new(),
                employment_types: crate::employment::demo_choices(
                    crate::employment::EmploymentKind::EmploymentType,
                    Uuid::from_u128(1),
                ),
                work_locations: crate::employment::demo_choices(
                    crate::employment::EmploymentKind::WorkLocation,
                    Uuid::from_u128(1),
                ),
            },
        };
        store.data.employment_history = crate::history::demo_initial(&store.data);
        store
    }

    pub fn execute(&mut self, action: Action) -> Result<()> {
        if let Action::ImportEmployees(preview, reason) = action {
            return crate::imports::apply_demo(self, preview, reason);
        }
        let before = self
            .data
            .employees
            .iter()
            .map(|e| crate::history::EmploymentState::capture(&self.data, e))
            .collect();
        self.apply(action)?;
        crate::history::record_demo_changes(&mut self.data, before);
        Ok(())
    }

    fn apply(&mut self, action: Action) -> Result<()> {
        self.data.access.authorize(&action, &self.data)?;
        match action {
            Action::Payroll(action) => {crate::payroll::apply_demo(&mut self.data, &action)?;}
            Action::Career(action) => {crate::career::apply_demo(&mut self.data,&action,chrono::Utc::now())?;}
            Action::Learning(action) => {crate::learning::apply_demo(&mut self.data,&action,chrono::Utc::now())?;}
            Action::Performance(action) => {crate::performance::apply_demo(&mut self.data,&action,chrono::Utc::now())?;}
            Action::Exit(action) => { crate::exits::apply_demo(&mut self.data, &action, chrono::Utc::now())?; }
            Action::Onboarding(action) => {
                crate::onboarding::apply_demo(&mut self.data, &action, chrono::Utc::now())?
            }
            Action::Interview(action) => {
                crate::recruiting::apply_demo(&mut self.data, &action, chrono::Utc::now())?
            }
            Action::Hiring(action) => {
                crate::hiring::apply_demo(&mut self.data, &action, chrono::Utc::now())?
            }
            Action::NotificationReads(action) => {
                self.data.notification_reads.user_id = self.data.access.user_id;
                self.data.notification_reads.company_id = self.data.company.id;
                self.data.notification_reads.apply(&action)?;
            }
            Action::ImportEmployees(preview, reason) => {
                crate::imports::apply_demo(self, preview, reason)?
            }
            Action::SaveCalculationPolicy(draft) => {
                crate::calculations::save_demo_policy(&mut self.data, draft)?
            }
            Action::ApplyCalculationPolicy(change) => {
                crate::calculations::apply_demo_policy(&mut self.data, change)?
            }
            Action::AttendanceBreak(action) => crate::breaks::apply_demo(&mut self.data, action)?,
            Action::SaveShiftTemplate(draft) => {
                crate::shifts::save_demo_template(&mut self.data, draft)?
            }
            Action::AssignShift(draft) => {
                let assignment = draft.assignment(&self.data)?;
                self.data.shifts.assignments.push(assignment);
            }
            Action::SaveEmploymentOption(draft) => {
                let option = draft.option(&self.data)?;
                let choices = match draft.kind {
                    crate::employment::EmploymentKind::EmploymentType => {
                        &mut self.data.employment_types
                    }
                    crate::employment::EmploymentKind::WorkLocation => {
                        &mut self.data.work_locations
                    }
                };
                if let Some(existing) = choices.iter_mut().find(|c| c.id == option.id) {
                    *existing = option;
                } else {
                    choices.push(option);
                }
                choices.sort_by(|a, b| {
                    a.name
                        .to_lowercase()
                        .cmp(&b.name.to_lowercase())
                        .then(a.id.cmp(&b.id))
                });
            }
            Action::Overtime(action) => crate::overtime::apply_demo(&mut self.data, action)?,
            Action::AttendanceRequest(action) => {
                crate::requests::apply_demo(&mut self.data, action)?
            }
            Action::CorrectAttendance(mut change) => {
                change.record.calculation_snapshot = if let Some(old) = self
                    .data
                    .attendance
                    .iter()
                    .find(|a| a.id == change.record.id)
                {
                    old.calculation_snapshot.clone()
                } else {
                    Some(crate::calculations::CalculationSnapshot::capture(
                        &self.data,
                        &change.record,
                    ))
                };
                change.validate(&self.data, Utc::now())?;
                crate::breaks::validate_attendance(&self.data, &change.record)?;
                let before_record = self
                    .data
                    .attendance
                    .iter()
                    .find(|a| a.id == change.record.id)
                    .cloned();
                let actor_name = self
                    .data
                    .own_employee()
                    .map(|e| e.full_name.clone())
                    .unwrap_or_else(|| "HR staff".into());
                self.data
                    .corrections
                    .push(crate::corrections::CorrectionEvent {
                        id: Uuid::new_v4(),
                        company_id: self.data.company.id,
                        attendance_id: change.record.id,
                        employee_id: change.record.employee_id,
                        work_date: change.record.work_date,
                        actor_id: self.data.access.user_id,
                        actor_name,
                        actor_role: self.data.access.role,
                        reason: change.reason.trim().into(),
                        before_record,
                        after_record: change.record.clone(),
                        created_at: Utc::now(),
                    });
                if let Some(row) = self
                    .data
                    .attendance
                    .iter_mut()
                    .find(|a| a.id == change.record.id)
                {
                    *row = change.record;
                } else {
                    self.data.attendance.push(change.record);
                }
            }
            Action::SaveEmployeeRecord(draft, contacts) => {
                let contacts = contacts.normalized()?;
                ensure!(
                    contacts.company_id == self.data.company.id,
                    "Choose an employee in this company."
                );
                let before = self.data.clone();
                let id = draft.id.unwrap_or_else(Uuid::new_v4);
                let employee_draft = draft;
                if employee_draft.id.is_none() {
                    let mut employee = employee_draft.employee(self.data.company.id)?;
                    employee.id = id;
                    self.validate_reporting(&employee)?;
                    ensure!(
                        !self.data.employees.iter().any(|e| e
                            .employee_code
                            .eq_ignore_ascii_case(&employee.employee_code)),
                        "That employee code is already in use."
                    );
                    self.data.employees.push(employee);
                } else if let Err(error) = self.apply(Action::SaveEmployee(employee_draft)) {
                    return Err(error);
                }
                let mut contacts = contacts;
                contacts.employee_id = id;
                if let Err(error) = self.apply(Action::SaveEmployeeContacts(contacts)) {
                    self.data = before;
                    return Err(error);
                }
            }
            Action::SaveEmployeeContacts(contacts) | Action::SaveOwnContacts(contacts) => {
                let mut contacts = contacts.normalized()?;
                ensure!(
                    contacts.company_id == self.data.company.id
                        && self
                            .data
                            .employees
                            .iter()
                            .any(|e| e.id == contacts.employee_id),
                    "Choose an employee in this company."
                );
                contacts.updated_at = Some(Utc::now());
                if let Some(existing) = self
                    .data
                    .contacts
                    .iter_mut()
                    .find(|c| c.employee_id == contacts.employee_id)
                {
                    *existing = contacts;
                } else {
                    self.data.contacts.push(contacts);
                }
            }
            Action::SaveEmployee(draft) => {
                let employee = draft.employee(self.data.company.id)?;
                self.validate_reporting(&employee)?;
                ensure!(
                    employee.status != EmployeeStatus::Archived
                        || !self
                            .data
                            .accounts
                            .iter()
                            .any(|a| a.active && a.employee_id == Some(employee.id)),
                    "Disable this employee's sign-in access before archiving their record."
                );
                ensure!(
                    !self.data.employees.iter().any(|e| e.id != employee.id
                        && e.employee_code
                            .eq_ignore_ascii_case(&employee.employee_code)),
                    "That employee code is already in use."
                );
                ensure!(
                    employee.annual_leave_days
                        >= crate::leave::minimum_annual_allowance(
                            &self.data,
                            &employee,
                            self.data.company.today().year()
                        ),
                    "Allowance cannot be less than approved annual leave this year."
                );
                if let Some(id) = draft.id {
                    let existing = self
                        .data
                        .employees
                        .iter_mut()
                        .find(|e| e.id == id)
                        .ok_or_else(|| anyhow::anyhow!("Employee was not found."))?;
                    *existing = employee;
                } else {
                    self.data.employees.push(employee);
                }
            }
            Action::CheckIn(id) => {
                self.require_active(id)?;
                let today = crate::shifts::work_date_at(&self.data, id, Utc::now());
                ensure!(
                    !self
                        .data
                        .attendance
                        .iter()
                        .any(|a| a.employee_id == id && a.work_date == today),
                    "A check-in already exists for this employee today."
                );
                ensure!(
                    !self
                        .data
                        .attendance
                        .iter()
                        .any(|a| a.employee_id == id && a.checked_out_at.is_none()),
                    "Close the employee's previous open attendance record first."
                );
                let mut row = Attendance {
                    calculation_snapshot: None,
                    planned_shift: self.data.shifts.plan(id, today).cloned(),
                    revision: 0,
                    id: Uuid::new_v4(),
                    company_id: self.data.company.id,
                    employee_id: id,
                    work_date: today,
                    checked_in_at: Utc::now(),
                    checked_out_at: None,
                };
                row.calculation_snapshot = Some(crate::calculations::CalculationSnapshot::capture(
                    &self.data, &row,
                ));
                self.data.attendance.push(row);
            }
            Action::CheckOut(id) => {
                ensure!(
                    self.data.breaks.open(id).is_none(),
                    "End the open break before checking out."
                );
                let row = self
                    .data
                    .attendance
                    .iter_mut()
                    .find(|a| a.id == id)
                    .ok_or_else(|| anyhow::anyhow!("Attendance record was not found."))?;
                ensure!(
                    row.checked_out_at.is_none(),
                    "This attendance record is already closed."
                );
                ensure!(
                    Utc::now() >= row.checked_in_at,
                    "Check-out cannot be before check-in."
                );
                row.checked_out_at = Some(Utc::now());
                row.revision += 1;
            }
            action @ (Action::RequestLeave(_)
            | Action::DecideLeave(_, _)
            | Action::ReviewLeave(_, _, _, _)
            | Action::CancelLeave(_, _, _)
            | Action::SaveLeavePolicy(_)) => crate::leave::apply_demo(&mut self.data, action)?,
            Action::UploadCompanyLogo(path) => {
                self.data.logo_png = Some(crate::branding::logo_from_file(&path)?);
                self.data.company.logo_version = Some(Uuid::new_v4());
                self.data.logo_warning = None;
            }
            Action::RemoveCompanyLogo => {
                self.data.logo_png = None;
                self.data.company.logo_version = None;
                self.data.logo_warning = None;
            }
            Action::SaveCompany(mut company) => {
                company.validate()?;
                company.name = company.name.trim().into();
                company.id = self.data.company.id;
                company.owner_id = self.data.company.owner_id;
                self.data.company = company;
            }
        }
        Ok(())
    }

    fn require_active(&self, id: Uuid) -> Result<()> {
        match self.data.employees.iter().find(|e| e.id == id) {
            Some(e) if e.status == EmployeeStatus::Active => Ok(()),
            Some(_) => bail!("Archived employees cannot receive new records."),
            None => bail!("Employee was not found."),
        }
    }
}
