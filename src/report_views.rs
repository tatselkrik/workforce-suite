use crate::{
    domain::{Mode, Snapshot},
    reports::{self, Filter},
};
use anyhow::{Result, ensure};
use chrono::{Datelike, Duration, NaiveDate};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
pub const MAX_VIEWS: usize = 20;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Period {
    #[default]
    Fixed,
    Last7,
    Last30,
    ThisMonth,
}
impl Period {
    pub const ALL: [Self; 4] = [Self::Fixed, Self::Last7, Self::Last30, Self::ThisMonth];
    pub fn label(self) -> &'static str {
        match self {
            Self::Fixed => "Saved dates",
            Self::Last7 => "Last 7 days",
            Self::Last30 => "Last 30 days",
            Self::ThisMonth => "This month",
        }
    }
}
// These are user preferences, never authority to read employee records.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct View {
    pub id: Uuid,
    pub company_id: Uuid,
    pub name: String,
    pub period: Period,
    pub filter: Filter,
}
impl View {
    pub fn new(company_id: Uuid, name: String, filter: Filter, period: Period) -> Result<Self> {
        let view = Self {
            id: Uuid::new_v4(),
            company_id,
            name: name.trim().into(),
            period,
            filter,
        };
        view.validate()?;
        Ok(view)
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.id.is_nil() && !self.company_id.is_nil(),
            "Saved report identity is invalid."
        );
        ensure!(
            !self.name.trim().is_empty()
                && self.name.chars().count() <= 80
                && !self.name.chars().any(char::is_control),
            "Name the saved view using 1–80 characters without control characters."
        );
        ensure!(
            self.filter.department.chars().count() <= 80 && !self.filter.department.contains('\0'),
            "The saved department filter is invalid."
        );
        self.filter.validate()
    }
    pub fn resolve(&self, data: &Snapshot, today: NaiveDate) -> Result<Filter> {
        self.validate()?;
        ensure!(
            data.mode != Mode::SignedOut
                && data.access.role.uses_desktop()
                && data.company.id == self.company_id
                && data.access.company_id == self.company_id,
            "This saved view is unavailable in your current company/access."
        );
        let mut f = self.filter.clone();
        if self.period != Period::Fixed {
            f.ends_on = today;
            f.starts_on = match self.period {
                Period::Last7 => today.checked_sub_signed(Duration::days(6)),
                Period::Last30 => today.checked_sub_signed(Duration::days(29)),
                Period::ThisMonth => today.with_day(1),
                Period::Fixed => Some(f.starts_on),
            }
            .ok_or_else(|| anyhow::anyhow!("Saved report dates are out of range."))?;
        }
        if let Some(id) = f.employee_id {
            ensure!(
                data.employees.iter().any(|e| e.id == id
                    && e.company_id == self.company_id
                    && reports::permitted(data, id)),
                "This saved employee filter is outside your current access. Choose another employee."
            );
        }
        f.validate()?;
        Ok(f)
    }
}
pub fn validate_collection(views: &[View]) -> Result<()> {
    ensure!(views.len() <= MAX_VIEWS, "Save at most 20 report views.");
    let mut ids = std::collections::BTreeSet::new();
    let mut names = std::collections::BTreeSet::new();
    for view in views {
        view.validate()?;
        ensure!(ids.insert(view.id), "Saved report IDs must be unique.");
        ensure!(
            names.insert((view.company_id, view.name.trim().to_lowercase())),
            "Use distinct saved report names within a company."
        );
    }
    Ok(())
}

// Auth metadata is user-editable. Ignore malformed preferences without blocking
// sign-in; permissions still come only from protected memberships and RLS.
pub fn from_metadata(value: Option<&serde_json::Value>) -> Vec<View> {
    value
        .and_then(|value| value.as_array())
        .filter(|values| values.len() <= MAX_VIEWS)
        .and_then(|values| serde_json::from_value::<Vec<View>>(values.clone().into()).ok())
        .filter(|views| validate_collection(views).is_ok())
        .unwrap_or_default()
}
