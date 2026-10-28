//! Half-day integer accounting shared by the demo, previews and policy UI.
//! Hosted mutations revalidate the same rules under a database lock.
use crate::{access::Role, domain::*};
use anyhow::{Result, ensure};
use chrono::{Datelike, Days, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
pub mod credits;
pub mod dated;
pub mod entitlements;
pub mod opening_import;
pub mod openings;
pub mod request_import;
pub mod versions;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DayPart {
    #[default]
    Full,
    Am,
    Pm,
}
impl DayPart {
    pub fn label(self) -> &'static str {
        match self {
            Self::Full => "Full day(s)",
            Self::Am => "Morning half-day",
            Self::Pm => "Afternoon half-day",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Accrual {
    #[default]
    Upfront,
    Monthly,
    Unlimited,
}
impl Accrual {
    pub fn label(self) -> &'static str {
        match self {
            Self::Upfront => "Annual grant",
            Self::Monthly => "Monthly accrual",
            Self::Unlimited => "No balance limit",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApproverKind {
    #[default]
    Reviewer,
    Manager,
    Hr,
    Administrator,
    Person,
}
impl ApproverKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Reviewer => "HR or direct manager",
            Self::Manager => "Direct manager",
            Self::Hr => "HR / Administrator",
            Self::Administrator => "Administrator",
            Self::Person => "Specific staff member",
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalRule {
    pub kind: ApproverKind,
    pub employee_id: Option<Uuid>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalStep {
    pub kind: ApproverKind,
    pub employee_id: Option<Uuid>,
    pub approved_by: Option<Uuid>,
    pub approved_at: Option<chrono::DateTime<Utc>>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Holiday {
    pub date: NaiveDate,
    pub name: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeaveType {
    pub code: String,
    pub name: String,
    pub active: bool,
    pub paid: bool,
    pub use_employee_allowance: bool,
    pub annual_units: i32,
    pub accrual: Accrual,
    pub carryover_units: i32,
    pub carryover_expiry_month: u32,
    pub approval_chain: Vec<ApprovalRule>,
}
impl Default for LeaveType {
    fn default() -> Self {
        Self {
            code: String::new(),
            name: String::new(),
            active: true,
            paid: true,
            use_employee_allowance: false,
            annual_units: 30,
            accrual: Accrual::Upfront,
            carryover_units: 0,
            carryover_expiry_month: 3,
            approval_chain: vec![ApprovalRule::default()],
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeavePolicy {
    pub tracking_starts_year: i32,
    pub company_id: Uuid,
    pub revision: i64,
    pub workweek: Vec<u32>,
    pub holidays: Vec<Holiday>,
    pub types: Vec<LeaveType>,
}
impl Default for LeavePolicy {
    fn default() -> Self {
        Self {
            tracking_starts_year: Utc::now().year(),
            company_id: Uuid::nil(),
            revision: 0,
            workweek: vec![1, 2, 3, 4, 5],
            holidays: vec![],
            types: vec![
                LeaveType {
                    code: "annual".into(),
                    name: "Annual leave".into(),
                    use_employee_allowance: true,
                    ..Default::default()
                },
                LeaveType {
                    code: "sick".into(),
                    name: "Sick leave".into(),
                    accrual: Accrual::Unlimited,
                    ..Default::default()
                },
                LeaveType {
                    code: "unpaid".into(),
                    name: "Unpaid leave".into(),
                    paid: false,
                    accrual: Accrual::Unlimited,
                    ..Default::default()
                },
            ],
        }
    }
}
impl LeavePolicy {
    pub fn validate(&self, previous: Option<&Self>) -> Result<()> {
        ensure!(
            (1900..=2200).contains(&self.tracking_starts_year),
            "Invalid leave tracking start year."
        );
        ensure!(
            !self.workweek.is_empty() && self.workweek.len() <= 7,
            "Choose at least one workday."
        );
        let mut seen = std::collections::HashSet::new();
        ensure!(
            self.workweek
                .iter()
                .all(|d| (1..=7).contains(d) && seen.insert(*d)),
            "Workdays must be unique Monday–Sunday values."
        );
        ensure!(
            self.holidays.len() <= 1000,
            "Use at most 1000 dated holidays."
        );
        seen.clear();
        let mut dates = std::collections::HashSet::new();
        for h in &self.holidays {
            ensure!(
                (1900..=2200).contains(&h.date.year())
                    && dates.insert(h.date)
                    && !h.name.trim().is_empty()
                    && h.name.chars().count() <= 100,
                "Give each holiday a unique valid date and a name up to 100 characters."
            );
        }
        ensure!(
            !self.types.is_empty() && self.types.len() <= 30,
            "Configure between 1 and 30 leave types."
        );
        let mut codes = std::collections::HashSet::new();
        for t in &self.types {
            ensure!(
                !t.code.is_empty()
                    && t.code.len() <= 30
                    && t.code
                        .bytes()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
                    && codes.insert(&t.code),
                "Leave type codes must be unique lowercase letters, digits or underscores, up to 30 characters."
            );
            ensure!(
                !t.name.trim().is_empty() && t.name.chars().count() <= 80,
                "Enter a leave type name up to 80 characters."
            );
            ensure!(
                (0..=732).contains(&t.annual_units)
                    && (0..=732).contains(&t.carryover_units)
                    && (1..=12).contains(&t.carryover_expiry_month),
                "Allowances and carryover must be 0–366 days, with an expiry month from 1–12."
            );
            ensure!(
                !t.use_employee_allowance || t.code == "annual",
                "Only annual leave uses the employee allowance."
            );
            ensure!(
                !t.approval_chain.is_empty() && t.approval_chain.len() <= 5,
                "Each leave type needs 1–5 approval steps."
            );
            for s in &t.approval_chain {
                ensure!(
                    s.kind != ApproverKind::Person || s.employee_id.is_some(),
                    "Choose a staff member for every specific-person step."
                );
            }
        }
        for code in ["annual", "sick", "unpaid"] {
            ensure!(
                self.types.iter().any(|t| t.code == code),
                "Keep the original leave types; deactivate a type instead of removing it."
            );
        }
        if let Some(old) = previous {
            for t in &old.types {
                ensure!(
                    self.types.iter().any(|n| n.code == t.code),
                    "Retain existing leave type codes; deactivate types instead of deleting them."
                );
            }
        }
        Ok(())
    }
    pub fn leave_type(&self, code: &str) -> Option<&LeaveType> {
        self.types.iter().find(|t| t.code == code)
    }
    pub fn working(&self, date: NaiveDate) -> bool {
        self.workweek.contains(&date.weekday().number_from_monday())
            && !self.holidays.iter().any(|h| h.date == date)
    }
    pub fn charges(
        &self,
        start: NaiveDate,
        end: NaiveDate,
        part: DayPart,
    ) -> Result<Vec<DayCharge>> {
        ensure!(
            end >= start && start.year() == end.year(),
            "Use dates in the same calendar year, with end on or after start."
        );
        ensure!(
            part == DayPart::Full || start == end,
            "A half-day request must use one date."
        );
        let dates = (0..=end.signed_duration_since(start).num_days() as u64)
            .filter_map(|i| start.checked_add_days(Days::new(i)))
            .filter(|d| self.working(*d))
            .map(|date| DayCharge {
                date,
                units: if part == DayPart::Full { 2 } else { 1 },
            })
            .collect::<Vec<_>>();
        ensure!(
            !dates.is_empty(),
            "Choose at least one scheduled workday outside the holiday calendar."
        );
        Ok(dates)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DayCharge {
    pub date: NaiveDate,
    pub units: i32,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LeaveOptions {
    pub type_code: String,
    pub day_part: DayPart,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestPolicy {
    #[serde(default)]
    pub type_code: String,
    #[serde(default)]
    pub type_name: String,
    #[serde(default)]
    pub day_part: DayPart,
    #[serde(default)]
    pub charged_units: i32,
    #[serde(default)]
    pub charged_dates: Vec<DayCharge>,
    #[serde(default)]
    pub policy_revision: i64,
    #[serde(default)]
    pub revision: i64,
    #[serde(default)]
    pub approval_steps: Vec<ApprovalStep>,
    #[serde(default)]
    pub requester_name: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LeaveEvent {
    pub id: Uuid,
    pub company_id: Uuid,
    pub request_id: Uuid,
    pub employee_id: Uuid,
    pub actor_id: Option<Uuid>,
    pub actor_name: String,
    pub action: String,
    pub note: String,
    pub created_at: chrono::DateTime<Utc>,
    pub revision: i64,
}
#[derive(Clone, Debug, Default)]
pub struct LeaveSetup {
    pub dated: Option<dated::Context>,
    pub credits: Option<credits::Context>,
    pub policy: LeavePolicy,
    pub events: Vec<LeaveEvent>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Balance {
    pub earned_units: i32,
    pub carryover_units: i32,
    pub used_units: i32,
    pub reserved_units: i32,
    pub available_units: i32,
}
pub fn days(units: i32) -> String {
    if units % 2 == 0 {
        format!("{}", units / 2)
    } else {
        format!("{:.1}", units as f32 / 2.0)
    }
}
impl LeaveRequest {
    pub fn type_code(&self) -> &str {
        if self.policy.type_code.is_empty() {
            match self.kind {
                LeaveKind::Annual => "annual",
                LeaveKind::Sick => "sick",
                LeaveKind::Unpaid => "unpaid",
                LeaveKind::Custom => "custom",
            }
        } else {
            &self.policy.type_code
        }
    }
    pub fn type_name(&self) -> &str {
        if self.policy.type_name.is_empty() {
            self.kind.label()
        } else {
            &self.policy.type_name
        }
    }
    pub fn units(&self) -> i32 {
        if self.policy.charged_dates.is_empty() {
            weekdays(self.starts_on, self.ends_on) as i32 * 2
        } else {
            self.policy.charged_units
        }
    }
    pub fn live(&self) -> bool {
        matches!(self.status, LeaveStatus::Pending | LeaveStatus::Approved)
    }
    pub fn charges(&self) -> Vec<DayCharge> {
        if !self.policy.charged_dates.is_empty() {
            self.policy.charged_dates.clone()
        } else {
            (0..=self
                .ends_on
                .signed_duration_since(self.starts_on)
                .num_days()
                .max(0) as u64)
                .filter_map(|i| self.starts_on.checked_add_days(Days::new(i)))
                .filter(|d| d.weekday().number_from_monday() <= 5)
                .map(|date| DayCharge { date, units: 2 })
                .collect()
        }
    }
}
fn expiry(year: i32, month: u32) -> NaiveDate {
    let next = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    };
    next.unwrap().pred_opt().unwrap()
}
fn earned(e: &Employee, t: &LeaveType, year: i32, as_of: NaiveDate) -> i32 {
    if e.joined_on.is_some_and(|j| j.year() > year) {
        return 0;
    }
    let units = if t.use_employee_allowance {
        e.annual_leave_days as i32 * 2
    } else {
        t.annual_units
    };
    if t.accrual != Accrual::Monthly {
        return units;
    }
    if e.joined_on.is_none() {
        return 0;
    }
    let months = (1..=12)
        .filter(|m| {
            let first = NaiveDate::from_ymd_opt(year, *m, 1).unwrap();
            expiry(year, *m) <= as_of && e.joined_on.is_none_or(|j| j <= first)
        })
        .count() as i32;
    units * months / 12
}
/// Calculate with integer half-days. Carry comes only from unused base grant;
/// prior carry is consumed first before expiry and never carried a second time.
pub fn balance(
    data: &Snapshot,
    e: &Employee,
    t: &LeaveType,
    year: i32,
    as_of: NaiveDate,
    exclude: Option<Uuid>,
) -> Balance {
    // Callers showing an editable preview use balance_checked to surface the
    // reason. A failed finite calculation must never allow a positive charge.
    balance_checked(data, e, t, year, as_of, exclude).unwrap_or_default()
}
pub fn balance_checked(
    data: &Snapshot,
    e: &Employee,
    t: &LeaveType,
    year: i32,
    as_of: NaiveDate,
    exclude: Option<Uuid>,
) -> Result<Balance> {
    if data.leave_setup.dated.as_ref().is_some_and(|c| c.enabled) {
        return dated::balance(data, e, &t.code, year, as_of, exclude);
    }
    Ok(legacy_balance(data, e, t, year, as_of, exclude))
}
fn legacy_balance(
    data: &Snapshot,
    e: &Employee,
    t: &LeaveType,
    year: i32,
    as_of: NaiveDate,
    exclude: Option<Uuid>,
) -> Balance {
    let first = e
        .joined_on
        .map(|j| j.year())
        .unwrap_or(year)
        .min(year)
        .max(data.leave_setup.policy.tracking_starts_year.min(year));
    let mut carry = 0;
    for y in first..year {
        let end = NaiveDate::from_ymd_opt(y, 12, 31).unwrap();
        let (used, before) = usage(
            data,
            e.id,
            &t.code,
            y,
            expiry(y, t.carryover_expiry_month),
            exclude,
        );
        let adjustment = data
            .leave_setup
            .credits
            .as_ref()
            .map_or(0, |c| c.units(e.id, &t.code, y, end));
        let unused = (earned(e, t, y, end) + adjustment - (used - before.min(carry))).max(0);
        carry = unused.min(t.carryover_units);
    }
    let boundary = expiry(year, t.carryover_expiry_month);
    let (total, before) = usage(data, e.id, &t.code, year, boundary, exclude);
    let used = data
        .leave
        .iter()
        .filter(|r| {
            Some(r.id) != exclude
                && r.employee_id == e.id
                && r.type_code() == t.code
                && r.starts_on.year() == year
                && r.status == LeaveStatus::Approved
        })
        .map(|r| r.units())
        .sum();
    let earned = earned(e, t, year, as_of)
        + data
            .leave_setup
            .credits
            .as_ref()
            .map_or(0, |c| c.units(e.id, &t.code, year, as_of));
    let applicable = if as_of <= boundary {
        carry
    } else {
        carry.min(before)
    };
    Balance {
        earned_units: earned,
        carryover_units: if as_of <= boundary { carry } else { 0 },
        used_units: used,
        reserved_units: total - used,
        available_units: earned + applicable - total,
    }
}
fn usage(
    data: &Snapshot,
    e: Uuid,
    code: &str,
    year: i32,
    boundary: NaiveDate,
    exclude: Option<Uuid>,
) -> (i32, i32) {
    let mut total = 0;
    let mut before = 0;
    for r in data.leave.iter().filter(|r| {
        Some(r.id) != exclude
            && r.employee_id == e
            && r.type_code() == code
            && r.starts_on.year() == year
            && r.live()
    }) {
        total += r.units();
        before += r
            .charges()
            .iter()
            .filter(|d| d.date <= boundary)
            .map(|d| d.units)
            .sum::<i32>();
    }
    (total, before)
}
pub fn can_review(data: &Snapshot, row: &LeaveRequest) -> bool {
    can_review_access(&data.access, row)
        || (row.status == LeaveStatus::Pending
            && data.access.employee_id != Some(row.employee_id)
            && crate::approvals::permitted(
                data,
                crate::approvals::Scope::Leave,
                row.id,
                row.policy.revision,
            ))
}
pub fn can_review_access(access: &crate::access::Access, row: &LeaveRequest) -> bool {
    if row.status != LeaveStatus::Pending
        || access.employee_id == Some(row.employee_id)
        || !access.uses_desktop()
    {
        return false;
    }
    let Some(step) = row
        .policy
        .approval_steps
        .iter()
        .find(|s| s.approved_by.is_none())
    else {
        return row.policy.approval_steps.is_empty() && access.can_review(row.employee_id);
    };
    match step.kind {
        ApproverKind::Reviewer => access.can_review(row.employee_id),
        ApproverKind::Manager | ApproverKind::Person => step.employee_id == access.employee_id,
        ApproverKind::Hr => access.allows(crate::staff_roles::Capability::LeaveAdministration),
        ApproverKind::Administrator => access.role == Role::Administrator,
    }
}
pub fn minimum_annual_allowance(data: &Snapshot, employee: &Employee, year: i32) -> u32 {
    let Some(t) = data.leave_setup.policy.leave_type("annual") else {
        return data.annual_used(employee.id, year);
    };
    let carry = balance(
        data,
        employee,
        t,
        year,
        NaiveDate::from_ymd_opt(year, 1, 1).unwrap(),
        None,
    )
    .carryover_units;
    let rows = data
        .leave
        .iter()
        .filter(|r| {
            r.employee_id == employee.id
                && r.type_code() == "annual"
                && r.status == LeaveStatus::Approved
                && r.starts_on.year() == year
        })
        .collect::<Vec<_>>();
    let total = rows.iter().map(|r| r.units()).sum::<i32>();
    let early = rows
        .iter()
        .flat_map(|r| r.charges())
        .filter(|d| d.date <= expiry(year, t.carryover_expiry_month))
        .map(|d| d.units)
        .sum::<i32>();
    ((total - carry.min(early)).max(0) as u32).div_ceil(2)
}
pub fn prepare(data: &Snapshot, draft: &LeaveDraft) -> Result<LeaveRequest> {
    let mut row = draft.request(data.company.id)?;
    let e = data
        .active()
        .find(|e| e.id == row.employee_id)
        .ok_or_else(|| anyhow::anyhow!("Choose an active employee."))?;
    ensure!(
        e.joined_on.is_none_or(|d| d <= row.starts_on),
        "Leave cannot start before the employee joined."
    );
    let code = if draft.options.type_code.is_empty() {
        row.type_code()
    } else {
        &draft.options.type_code
    };
    let selected_policy = dated::policy_on(data, row.starts_on);
    let t = selected_policy
        .leave_type(code)
        .filter(|t| t.active)
        .ok_or_else(|| anyhow::anyhow!("Choose an active leave type."))?;
    ensure!(
        t.accrual != Accrual::Monthly || e.joined_on.is_some(),
        "Set the employee Date joined before requesting accrued leave."
    );
    let charges = dated::charges(
        data,
        code,
        row.starts_on,
        row.ends_on,
        draft.options.day_part,
    )?;
    row.policy = RequestPolicy {
        type_code: t.code.clone(),
        type_name: t.name.clone(),
        day_part: draft.options.day_part,
        charged_units: charges.iter().map(|d| d.units).sum(),
        charged_dates: charges,
        policy_revision: selected_policy.revision,
        revision: 0,
        approval_steps: t
            .approval_chain
            .iter()
            .map(|s| ApprovalStep {
                kind: s.kind,
                employee_id: if s.kind == ApproverKind::Manager {
                    e.reports_to
                } else {
                    s.employee_id
                },
                ..Default::default()
            })
            .collect(),
        requester_name: e.full_name.clone(),
    };
    row.kind = match t.code.as_str() {
        "annual" => LeaveKind::Annual,
        "sick" => LeaveKind::Sick,
        "unpaid" => LeaveKind::Unpaid,
        _ => LeaveKind::Custom,
    };
    for step in &row.policy.approval_steps {
        ensure!(
            !matches!(step.kind, ApproverKind::Manager | ApproverKind::Person)
                || step
                    .employee_id
                    .is_some_and(|id| id != e.id && data.active().any(|p| p.id == id)),
            "An approval step needs another active staff member. Set Reports to or change the chain."
        );
    }
    for other in data
        .leave
        .iter()
        .filter(|r| r.employee_id == row.employee_id && r.live())
    {
        ensure!(
            !other
                .charges()
                .iter()
                .any(|d| row.policy.charged_dates.iter().any(|n| n.date == d.date
                    && (row.policy.day_part == DayPart::Full
                        || other.policy.day_part == DayPart::Full
                        || row.policy.day_part == other.policy.day_part))),
            "These dates overlap an existing pending or approved request."
        );
    }
    if t.accrual != Accrual::Unlimited {
        ensure!(
            balance_checked(data, e, t, row.starts_on.year(), row.starts_on, None)?.available_units
                >= row.units(),
            "This request exceeds the available leave balance, including pending reservations."
        );
    }
    Ok(row)
}
pub fn demo_event(data: &mut Snapshot, id: Uuid, action: &str, note: &str) {
    if let Some(row) = data.leave.iter().find(|r| r.id == id) {
        data.leave_setup.events.push(LeaveEvent {
            id: Uuid::new_v4(),
            company_id: data.company.id,
            request_id: id,
            employee_id: row.employee_id,
            actor_id: Some(data.access.user_id),
            actor_name: data
                .own_employee()
                .map(|e| e.full_name.clone())
                .unwrap_or_else(|| "Demo staff".into()),
            action: action.into(),
            note: note.into(),
            created_at: Utc::now(),
            revision: row.policy.revision,
        });
    }
}
pub fn apply_demo(data: &mut Snapshot, action: Action) -> Result<()> {
    match action {
        Action::SaveLeavePolicy(mut p) => {
            ensure!(
                p.revision == data.leave_setup.policy.revision,
                "Leave policies changed. Refresh before saving."
            );
            p.validate(Some(&data.leave_setup.policy))?;
            p.company_id = data.company.id;
            p.revision += 1;
            data.leave_setup.policy = p;
        }
        Action::RequestLeave(draft) => {
            let row = prepare(data, &draft)?;
            let id = row.id;
            data.leave.push(row);
            demo_event(data, id, "submitted", "");
        }
        Action::DecideLeave(id, status) => {
            ensure!(
                matches!(status, LeaveStatus::Approved | LeaveStatus::Rejected),
                "Choose approve or reject."
            );
            let rev = data
                .leave
                .iter()
                .find(|r| r.id == id)
                .ok_or_else(|| anyhow::anyhow!("Request not found."))?
                .policy
                .revision;
            apply_demo(
                data,
                Action::ReviewLeave(id, rev, status == LeaveStatus::Approved, String::new()),
            )?;
        }
        Action::ReviewLeave(id, revision, approve, note) => {
            ensure!(
                note.chars().count() <= 500,
                "Keep the review note to 500 characters."
            );
            let row = data
                .leave
                .iter()
                .find(|r| r.id == id)
                .ok_or_else(|| anyhow::anyhow!("Request not found."))?;
            ensure!(
                revision == row.policy.revision,
                "This request changed. Refresh before reviewing."
            );
            ensure!(
                can_review(data, row),
                "This request is awaiting another reviewer, or is your own request."
            );
            if approve {
                let e = data
                    .active()
                    .find(|e| e.id == row.employee_id)
                    .ok_or_else(|| {
                        anyhow::anyhow!("Archived employees cannot have new leave approved.")
                    })?;
                let t = data
                    .leave_setup
                    .policy
                    .leave_type(row.type_code())
                    .ok_or_else(|| anyhow::anyhow!("Leave type not found."))?;
                if t.accrual != Accrual::Unlimited {
                    ensure!(
                        if data.leave_setup.dated.as_ref().is_some_and(|c| c.enabled) {
                            let end =
                                NaiveDate::from_ymd_opt(row.starts_on.year(), 12, 31).unwrap();
                            balance_checked(
                                data,
                                e,
                                t,
                                row.starts_on.year(),
                                row.starts_on.max(data.company.today().min(end)),
                                None,
                            )?
                            .available_units
                                >= 0
                        } else {
                            balance_checked(
                                data,
                                e,
                                t,
                                row.starts_on.year(),
                                row.starts_on,
                                Some(id),
                            )?
                            .available_units
                                >= row.units()
                        },
                        "This request exceeds the available leave balance."
                    );
                }
            }
            let row = data.leave.iter_mut().find(|r| r.id == id).unwrap();
            if approve {
                if let Some(step) = row
                    .policy
                    .approval_steps
                    .iter_mut()
                    .find(|s| s.approved_by.is_none())
                {
                    step.approved_by = Some(data.access.user_id);
                    step.approved_at = Some(Utc::now());
                }
                if row
                    .policy
                    .approval_steps
                    .iter()
                    .all(|s| s.approved_by.is_some())
                {
                    row.status = LeaveStatus::Approved;
                }
            } else {
                row.status = LeaveStatus::Rejected;
            }
            row.policy.revision += 1;
            demo_event(
                data,
                id,
                if approve { "approved_step" } else { "rejected" },
                &note,
            );
        }
        Action::CancelLeave(id, revision, reason) => {
            ensure!(
                !reason.trim().is_empty() && reason.chars().count() <= 500,
                "Enter a cancellation reason up to 500 characters."
            );
            let today = data.company.today();
            let hr = data
                .access
                .allows(crate::staff_roles::Capability::LeaveAdministration);
            let own = data.access.employee_id;
            let row = data
                .leave
                .iter_mut()
                .find(|r| r.id == id)
                .ok_or_else(|| anyhow::anyhow!("Request not found."))?;
            ensure!(
                row.policy.revision == revision,
                "This request changed. Refresh before cancelling."
            );
            ensure!(
                row.live()
                    && (hr
                        || (own == Some(row.employee_id)
                            && (row.status == LeaveStatus::Pending || row.starts_on >= today))),
                "Only HR or the requester before approved leave begins may cancel active leave."
            );
            row.status = LeaveStatus::Cancelled;
            row.policy.revision += 1;
            demo_event(data, id, "cancelled", reason.trim());
        }
        _ => anyhow::bail!("Unsupported leave action."),
    }
    Ok(())
}
