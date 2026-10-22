use crate::domain::{EmployeeStatus, Snapshot, parse_date};
use anyhow::{Result, ensure};
use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShiftTemplate {
    #[serde(default)]
    pub break_rules: Vec<crate::breaks::BreakRule>,
    pub id: Uuid,
    pub company_id: Uuid,
    pub name: String,
    pub days: Vec<u32>,
    pub start_minute: u32,
    pub end_minute: u32,
    pub works_on_holidays: bool,
    pub active: bool,
    pub revision: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShiftPlan {
    #[serde(default)]
    pub break_rules: Vec<crate::breaks::BreakRule>,
    pub template_id: Uuid,
    pub template_revision: i64,
    pub name: String,
    pub days: Vec<u32>,
    pub start_minute: u32,
    pub end_minute: u32,
    pub works_on_holidays: bool,
    pub timezone: String,
}

impl ShiftPlan {
    pub fn capture(template: &ShiftTemplate, timezone: &str) -> Self {
        Self {
            break_rules: template.break_rules.clone(),
            template_id: template.id,
            template_revision: template.revision,
            name: template.name.clone(),
            days: template.days.clone(),
            start_minute: template.start_minute,
            end_minute: template.end_minute,
            works_on_holidays: template.works_on_holidays,
            timezone: timezone.into(),
        }
    }
    pub fn overnight(&self) -> bool {
        self.end_minute < self.start_minute
    }
    pub fn hours(&self) -> String {
        hours(self.start_minute, self.end_minute)
    }
    pub fn working(&self, data: &Snapshot, date: NaiveDate) -> bool {
        self.days.contains(&date.weekday().number_from_monday())
            && (self.works_on_holidays
                || !data
                    .leave_setup
                    .policy
                    .holidays
                    .iter()
                    .any(|h| h.date == date))
    }
    pub fn contains_instant(&self, data: &Snapshot, date: NaiveDate, at: DateTime<Utc>) -> bool {
        self.working(data, date) && self.contains_clock(date, at)
    }
    pub fn contains_clock(&self, date: NaiveDate, at: DateTime<Utc>) -> bool {
        let Ok(tz) = self.timezone.parse::<chrono_tz::Tz>() else {
            return false;
        };
        let local = at.with_timezone(&tz).naive_local();
        let start = date.and_hms_opt(self.start_minute / 60, self.start_minute % 60, 0);
        let end_date = if self.overnight() {
            date.succ_opt()
        } else {
            Some(date)
        };
        let end =
            end_date.and_then(|d| d.and_hms_opt(self.end_minute / 60, self.end_minute % 60, 0));
        start.zip(end).is_some_and(|(s, e)| local >= s && local < e)
    }
    fn boundary(&self, date: NaiveDate, end: bool) -> Result<DateTime<Utc>> {
        let tz: chrono_tz::Tz = self.timezone.parse()?;
        let date = if end && self.overnight() {
            date.succ_opt()
                .ok_or_else(|| anyhow::anyhow!("Date is out of range."))?
        } else {
            date
        };
        let minute = if end {
            self.end_minute
        } else {
            self.start_minute
        };
        let local = date.and_hms_opt(minute / 60, minute % 60, 0).unwrap();
        // PostgreSQL AT TIME ZONE chooses the later occurrence on an ambiguous fall-back time.
        tz.from_local_datetime(&local).latest().map(|v|v.with_timezone(&Utc))
            .ok_or_else(||anyhow::anyhow!("A schedule boundary falls in a daylight-saving gap. Choose an unambiguous effective date or time."))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShiftVersion {
    pub id: Uuid,
    pub company_id: Uuid,
    pub template_id: Uuid,
    pub revision: i64,
    pub snapshot: ShiftTemplate,
    pub actor_id: Uuid,
    pub actor_name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShiftAssignment {
    pub id: Uuid,
    pub company_id: Uuid,
    pub employee_id: Uuid,
    pub effective_on: NaiveDate,
    pub sequence: i64,
    pub plan: Option<ShiftPlan>,
    pub reason: String,
    pub actor_id: Uuid,
    pub actor_name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Default)]
pub struct ShiftSetup {
    pub templates: Vec<ShiftTemplate>,
    pub versions: Vec<ShiftVersion>,
    pub assignments: Vec<ShiftAssignment>,
}
impl ShiftSetup {
    pub fn assignment(&self, employee: Uuid, date: NaiveDate) -> Option<&ShiftAssignment> {
        self.assignments
            .iter()
            .filter(|a| a.employee_id == employee && a.effective_on <= date)
            .max_by_key(|a| (a.effective_on, a.sequence))
    }
    pub fn plan(&self, employee: Uuid, date: NaiveDate) -> Option<&ShiftPlan> {
        self.assignment(employee, date)
            .and_then(|a| a.plan.as_ref())
    }
    pub fn sequence(&self, employee: Uuid) -> i64 {
        self.assignments
            .iter()
            .filter(|a| a.employee_id == employee)
            .map(|a| a.sequence)
            .max()
            .unwrap_or(0)
    }
}

pub fn clock(minute: u32) -> String {
    format!("{:02}:{:02}", minute / 60, minute % 60)
}
pub fn hours(start: u32, end: u32) -> String {
    format!(
        "{} – {}{}",
        clock(start),
        clock(end),
        if end < start { " (+1 day)" } else { "" }
    )
}
pub const DAY_NAMES: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
pub fn day_names(days: &[u32]) -> String {
    (1..=7)
        .filter(|d| days.contains(d))
        .map(|d| DAY_NAMES[(d - 1) as usize])
        .collect::<Vec<_>>()
        .join(", ")
}
fn minute(value: &str) -> Result<u32> {
    let value = value.trim();
    ensure!(
        value.len() == 5
            && value.as_bytes()[2] == b':'
            && value
                .bytes()
                .enumerate()
                .all(|(i, b)| i == 2 || b.is_ascii_digit()),
        "Use 24-hour times, for example 08:00 or 22:30."
    );
    let h: u32 = value[..2].parse()?;
    let m: u32 = value[3..].parse()?;
    ensure!(h < 24 && m < 60, "Use a valid time from 00:00 to 23:59.");
    Ok(h * 60 + m)
}

#[derive(Clone, Debug)]
pub struct ShiftTemplateDraft {
    pub break_rules: Vec<crate::breaks::BreakRule>,
    pub id: Option<Uuid>,
    pub expected_revision: Option<i64>,
    pub name: String,
    pub days: Vec<u32>,
    pub start: String,
    pub end: String,
    pub works_on_holidays: bool,
    pub active: bool,
}
impl Default for ShiftTemplateDraft {
    fn default() -> Self {
        Self {
            break_rules: Vec::new(),
            id: None,
            expected_revision: None,
            name: String::new(),
            days: vec![1, 2, 3, 4, 5],
            start: "08:00".into(),
            end: "17:00".into(),
            works_on_holidays: false,
            active: true,
        }
    }
}
impl From<&ShiftTemplate> for ShiftTemplateDraft {
    fn from(t: &ShiftTemplate) -> Self {
        Self {
            break_rules: t.break_rules.clone(),
            id: Some(t.id),
            expected_revision: Some(t.revision),
            name: t.name.clone(),
            days: t.days.clone(),
            start: clock(t.start_minute),
            end: clock(t.end_minute),
            works_on_holidays: t.works_on_holidays,
            active: t.active,
        }
    }
}
impl ShiftTemplateDraft {
    pub fn template(&self, data: &Snapshot) -> Result<ShiftTemplate> {
        ensure!(
            data.access.role.manages_hr(),
            "Only HR and Administrators can manage shifts."
        );
        let name = self.name.trim();
        ensure!(
            !name.is_empty() && name.chars().count() <= 80 && !name.chars().any(char::is_control),
            "Enter a shift name of 1–80 characters without control characters."
        );
        let mut days = self.days.clone();
        days.sort_unstable();
        days.dedup();
        ensure!(
            !days.is_empty()
                && days.len() == self.days.len()
                && days.iter().all(|d| (1..=7).contains(d)),
            "Choose at least one unique workday from Monday to Sunday."
        );
        let start_minute = minute(&self.start)?;
        let end_minute = minute(&self.end)?;
        ensure!(
            start_minute != end_minute,
            "Start and end must differ. Shifts must be shorter than 24 hours."
        );
        if let Some(id) = self.id {
            let old = data
                .shifts
                .templates
                .iter()
                .find(|t| t.id == id && t.company_id == data.company.id)
                .ok_or_else(|| anyhow::anyhow!("Shift unavailable. Refresh and reopen it."))?;
            ensure!(
                Some(old.revision) == self.expected_revision,
                "This shift changed. Refresh and reopen it before saving."
            );
        } else {
            ensure!(
                self.expected_revision.is_none(),
                "Refresh before adding a shift."
            );
        }
        ensure!(
            !data
                .shifts
                .templates
                .iter()
                .any(|t| Some(t.id) != self.id && t.name.to_lowercase() == name.to_lowercase()),
            "This shift name is already used, including inactive shifts."
        );
        crate::breaks::validate_rules(
            &self.break_rules,
            (end_minute + 1440 - start_minute) % 1440,
        )?;
        Ok(ShiftTemplate {
            break_rules: self
                .break_rules
                .iter()
                .cloned()
                .map(|mut r| {
                    r.name = r.name.trim().into();
                    r
                })
                .collect(),
            id: self.id.unwrap_or_else(Uuid::new_v4),
            company_id: data.company.id,
            name: name.into(),
            days,
            start_minute,
            end_minute,
            works_on_holidays: self.works_on_holidays,
            active: self.active,
            revision: self.expected_revision.map(|r| r + 1).unwrap_or(0),
        })
    }
    pub fn payload(&self, data: &Snapshot) -> Result<serde_json::Value> {
        let mut v = serde_json::to_value(self.template(data)?)?;
        v["id"] = serde_json::json!(self.id);
        v["expected_revision"] = serde_json::json!(self.expected_revision);
        Ok(v)
    }
}

#[derive(Clone, Debug)]
pub struct ShiftAssignmentDraft {
    pub employee_id: Uuid,
    pub template_id: Option<Uuid>,
    pub expected_template_revision: Option<i64>,
    pub expected_timezone: String,
    pub effective_on: String,
    pub expected_sequence: i64,
    pub reason: String,
}
impl ShiftAssignmentDraft {
    pub fn new(data: &Snapshot, employee: Uuid, date: NaiveDate) -> Self {
        let template_id = data.shifts.plan(employee, date).map(|p| p.template_id);
        Self {
            employee_id: employee,
            template_id,
            expected_template_revision: template_id.and_then(|id| {
                data.shifts
                    .templates
                    .iter()
                    .find(|t| t.id == id)
                    .map(|t| t.revision)
            }),
            expected_timezone: data.company.timezone.clone(),
            effective_on: date.to_string(),
            expected_sequence: data.shifts.sequence(employee),
            reason: String::new(),
        }
    }
    pub fn assignment(&self, data: &Snapshot) -> Result<ShiftAssignment> {
        ensure!(
            data.access.role.manages_hr(),
            "Only HR and Administrators can assign shifts."
        );
        let person = data
            .employees
            .iter()
            .find(|e| e.id == self.employee_id)
            .ok_or_else(|| anyhow::anyhow!("Choose an employee in this company."))?;
        let date = parse_date(&self.effective_on)?;
        ensure!(
            person.status == EmployeeStatus::Active,
            "Archived employees cannot receive new shift assignments."
        );
        ensure!(
            person.joined_on.is_none_or(|d| date >= d),
            "A shift assignment cannot begin before the hire date."
        );
        ensure!(
            self.expected_sequence == data.shifts.sequence(self.employee_id),
            "This employee's schedule changed. Refresh and reopen the assignment."
        );
        ensure!(
            self.expected_timezone == data.company.timezone,
            "The company time zone changed. Refresh and reopen the assignment."
        );
        ensure!(
            !self.reason.trim().is_empty()
                && self.reason.trim().chars().count() <= 500
                && !self.reason.chars().any(char::is_control),
            "Enter an assignment reason of 1–500 characters without control characters."
        );
        let plan = self
            .template_id
            .map(|id| {
                let t = data
                    .shifts
                    .templates
                    .iter()
                    .find(|t| t.id == id && t.company_id == data.company.id && t.active)
                    .ok_or_else(|| {
                        anyhow::anyhow!("Choose an active shift, or use the company calendar.")
                    })?;
                ensure!(
                    Some(t.revision) == self.expected_template_revision,
                    "The selected shift changed. Refresh and reopen the assignment."
                );
                Ok::<_, anyhow::Error>(ShiftPlan::capture(t, &data.company.timezone))
            })
            .transpose()?;
        let assignment = ShiftAssignment {
            id: Uuid::new_v4(),
            company_id: data.company.id,
            employee_id: self.employee_id,
            effective_on: date,
            sequence: self.expected_sequence + 1,
            plan,
            reason: self.reason.trim().into(),
            actor_id: data.access.user_id,
            actor_name: data
                .own_employee()
                .map(|e| e.full_name.clone())
                .unwrap_or_else(|| "HR staff".into()),
            created_at: Utc::now(),
        };
        let mut candidate = data.clone();
        candidate.shifts.assignments.push(assignment.clone());
        // Only a boundary can overlap: each weekly shift is less than 24 hours.
        for a in candidate
            .shifts
            .assignments
            .iter()
            .filter(|a| a.employee_id == self.employee_id)
        {
            let day = a.effective_on;
            if let Some(prev) = day.pred_opt() {
                let left = candidate.shifts.plan(self.employee_id, prev);
                let right = candidate.shifts.plan(self.employee_id, day);
                if let (Some(l), Some(r)) = (left, right) {
                    if l.days.contains(&prev.weekday().number_from_monday())
                        && r.days.contains(&day.weekday().number_from_monday())
                    {
                        ensure!(
                            l.boundary(prev, true)? <= r.boundary(day, false)?,
                            "This change overlaps the previous shift. Choose a later start or another effective date."
                        );
                    }
                }
            }
        }
        Ok(assignment)
    }
    pub fn payload(&self) -> Result<serde_json::Value> {
        Ok(
            serde_json::json!({"employee_id":self.employee_id,"template_id":self.template_id,"expected_template_revision":self.expected_template_revision,"expected_timezone":self.expected_timezone,"effective_on":parse_date(&self.effective_on)?,"expected_sequence":self.expected_sequence,"reason":self.reason.trim()}),
        )
    }
}

/// Routine night check-ins after midnight belong to their shift's starting date.
pub fn work_date_at(data: &Snapshot, employee: Uuid, at: DateTime<Utc>) -> NaiveDate {
    let today = at.with_timezone(&data.company.tz()).date_naive();
    for offset in [-1, 0, 1] {
        let Some(date) = today.checked_add_signed(Duration::days(offset)) else {
            continue;
        };
        if data
            .shifts
            .plan(employee, date)
            .is_some_and(|p| p.contains_instant(data, date, at))
        {
            return date;
        }
    }
    today
}

pub fn save_demo_template(data: &mut Snapshot, draft: ShiftTemplateDraft) -> Result<()> {
    let t = draft.template(data)?;
    data.shifts.versions.push(ShiftVersion {
        id: Uuid::new_v4(),
        company_id: data.company.id,
        template_id: t.id,
        revision: t.revision,
        snapshot: t.clone(),
        actor_id: data.access.user_id,
        actor_name: data
            .own_employee()
            .map(|e| e.full_name.clone())
            .unwrap_or_else(|| "HR staff".into()),
        created_at: Utc::now(),
    });
    if let Some(old) = data.shifts.templates.iter_mut().find(|v| v.id == t.id) {
        *old = t;
    } else {
        data.shifts.templates.push(t);
    }
    data.shifts.templates.sort_by_key(|t| t.name.to_lowercase());
    Ok(())
}
