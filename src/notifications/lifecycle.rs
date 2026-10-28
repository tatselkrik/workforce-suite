//! Minimal protected lifecycle notices; no salary amounts, HR notes or references.
use crate::{
    access::{Access, Role},
    notifications::{Alert, Category, Target},
};
use anyhow::{Result, ensure};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

pub const MAX_ITEMS: usize = 5000;
pub const KINDS: &[&str] = &[
    "payroll-monthly-issued",
    "payroll-cutoff-issued",
    "payroll-thirteenth-issued",
    "payroll-tax-issued",
    "payroll-final-issued",
    "payroll-amendment-issued",
    "payroll-receipt",
    "payroll-final-settled",
    "payroll-amendment-paid",
    "appraisal-review",
    "appraisal-publish",
    "appraisal-published",
    "training-start",
    "training-due",
    "certificate-expiry",
];
pub fn valid_key(key: &str) -> bool {
    let p: Vec<_> = key.split(':').collect();
    let staged = matches!(
        p[0],
        "appraisal-review" | "training-start" | "training-due" | "certificate-expiry"
    );
    (if staged { p.len() == 4 } else { p.len() == 3 })
        && KINDS.contains(&p[0])
        && Uuid::parse_str(p[1]).is_ok_and(|v| !v.is_nil() && v.to_string() == p[1])
        && p[2]
            .parse::<i64>()
            .is_ok_and(|v| v >= 0 && v.to_string() == p[2])
        && (p.len() == 3 || matches!(p[3], "soon" | "today" | "expired"))
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Item {
    pub key: String,
    pub kind: String,
    pub title: String,
    pub detail: String,
    pub date: NaiveDate,
    pub urgent: bool,
    pub target_id: Uuid,
    pub target_page: String,
    pub employee_id: Uuid,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Inbox {
    pub version: String,
    pub company_id: Uuid,
    pub user_id: Uuid,
    pub employee_id: Option<Uuid>,
    pub role: Role,
    pub surface: String,
    pub as_of: NaiveDate,
    pub generated_at: DateTime<Utc>,
    pub read_keys: Vec<String>,
    pub total_items: usize,
    pub truncated: bool,
    pub items: Vec<Item>,
}
impl Inbox {
    pub fn validate(&self, access: &Access, today: NaiveDate) -> Result<()> {
        ensure!(
            self.version == "lifecycle-notifications-v1"
                && self.surface == "desktop"
                && self.company_id == access.company_id
                && self.user_id == access.user_id
                && self.employee_id == access.employee_id
                && self.role == access.role
                && access.uses_desktop()
                && self.as_of == today,
            "Lifecycle notices do not match current staff access/date."
        );
        ensure!(
            self.items.len() <= MAX_ITEMS
                && self.total_items >= self.items.len()
                && self.truncated == (self.total_items > self.items.len())
                && (!self.truncated || self.items.len() == MAX_ITEMS),
            "Lifecycle notice count is invalid."
        );
        ensure!(
            self.read_keys.len() <= MAX_ITEMS
                && self.read_keys.iter().all(|k| valid_key(k))
                && self.read_keys.iter().collect::<HashSet<_>>().len() == self.read_keys.len(),
            "Lifecycle read history is invalid."
        );
        let mut seen = HashSet::new();
        for i in &self.items {
            ensure!(
                valid_key(&i.key)
                    && i.key.split(':').next() == Some(i.kind.as_str())
                    && seen.insert(&i.key)
                    && !i.target_id.is_nil()
                    && !i.employee_id.is_nil()
                    && !i.title.trim().is_empty()
                    && i.title.chars().count() <= 180
                    && i.detail.chars().count() <= 600
                    && !i
                        .title
                        .chars()
                        .chain(i.detail.chars())
                        .any(char::is_control),
                "A lifecycle notice is malformed."
            );
            let page = i.target_page.as_str();
            let own = self.employee_id == Some(i.employee_id);
            ensure!(
                matches!(
                    page,
                    "payroll_monthly"
                        | "payroll_cutoff"
                        | "payroll_thirteenth"
                        | "payroll_tax"
                        | "payroll_final"
                        | "payroll_amendment"
                        | "appraisal"
                        | "training"
                        | "certificate"
                ),
                "Unknown lifecycle destination."
            );
            ensure!(
                !i.kind.starts_with("payroll-") || (own && page.starts_with("payroll_")),
                "Payroll notice is not your own advice."
            );
            ensure!(
                i.kind != "appraisal-published" || (own && page == "appraisal"),
                "Published appraisal is not your own."
            );
            ensure!(
                i.kind != "appraisal-publish"
                    || (access.allows(crate::staff_roles::Capability::DevelopmentAdministration)
                        && !own
                        && page == "appraisal"),
                "Publication task is outside HR scope."
            );
            ensure!(
                !i.kind.starts_with("appraisal-") || page == "appraisal",
                "Appraisal destination is invalid."
            );
            ensure!(
                !i.kind.starts_with("training-") || page == "training",
                "Training destination is invalid."
            );
            ensure!(
                i.kind != "certificate-expiry" || page == "certificate",
                "Certificate destination is invalid."
            );
            ensure!(
                own || access.allows(crate::staff_roles::Capability::DevelopmentAdministration)
                    || (access.allows(crate::staff_roles::Capability::TeamReview)
                        && access.direct_reports.contains(&i.employee_id)),
                "Notice is outside your current reporting scope."
            );
        }
        Ok(())
    }
    pub fn alerts(&self) -> Vec<Alert> {
        self.items
            .iter()
            .map(|i| Alert {
                key: i.key.clone(),
                category: if i.kind.starts_with("payroll-") {
                    Category::Payroll
                } else if i.kind.starts_with("appraisal-") {
                    Category::Appraisals
                } else {
                    Category::Learning
                },
                title: i.title.clone(),
                detail: i.detail.clone(),
                date: i.date,
                urgent: i.urgent,
                target: Target::Lifecycle {
                    page: i.target_page.clone(),
                    record: i.target_id,
                    employee: i.employee_id,
                },
            })
            .collect()
    }
}
#[cfg(feature = "qa-screenshot")]
pub(crate) fn preview(data: &crate::domain::Snapshot, now: DateTime<Utc>) -> Inbox {
    let own = data.access.employee_id.unwrap();
    let today = now.with_timezone(&data.company.tz()).date_naive();
    let peer = data
        .employees
        .iter()
        .find(|e| e.id != own)
        .map(|e| e.id)
        .unwrap_or(own);
    let templates = [
        (
            "payroll-cutoff-issued",
            "Your cutoff payslip is available",
            "Approved advice is separate from payment",
            "payroll_cutoff",
            own,
            0,
            false,
            None,
        ),
        (
            "payroll-receipt",
            "Your payslip was settled through final pay",
            "Source offsets may apply; this is not a second full-net cash payment",
            "payroll_monthly",
            own,
            0,
            false,
            None,
        ),
        (
            "appraisal-review",
            "Your assigned appraisal is due today",
            "Fictional direct report · October review",
            "appraisal",
            peer,
            0,
            true,
            Some("today"),
        ),
        (
            "appraisal-published",
            "Your published appraisal is available",
            "Fictional review period · No rating or private HR notes copied",
            "appraisal",
            own,
            0,
            false,
            None,
        ),
        (
            "training-due",
            "Training is overdue",
            "Fictional required safety training",
            "training",
            own,
            -2,
            true,
            Some("expired"),
        ),
        (
            "certificate-expiry",
            "Recorded certificate expires within 30 days",
            "Fictional recorded certificate · Expiry reminder",
            "certificate",
            own,
            7,
            false,
            Some("soon"),
        ),
    ];
    let items = templates
        .into_iter()
        .enumerate()
        .filter(|(_, v)| {
            v.4 == own
                || data
                    .access
                    .allows(crate::staff_roles::Capability::DevelopmentAdministration)
                || (data
                    .access
                    .allows(crate::staff_roles::Capability::TeamReview)
                    && data.access.direct_reports.contains(&v.4))
        })
        .map(
            |(i, (kind, title, detail, page, employee, days, urgent, phase))| {
                let id = Uuid::from_u128(946000 + i as u128);
                Item {
                    key: format!(
                        "{kind}:{id}:0{}",
                        phase.map(|s| format!(":{s}")).unwrap_or_default()
                    ),
                    kind: kind.into(),
                    title: title.into(),
                    detail: detail.into(),
                    date: today + chrono::Duration::days(days),
                    urgent,
                    target_id: id,
                    target_page: page.into(),
                    employee_id: employee,
                }
            },
        )
        .collect::<Vec<_>>();
    Inbox {
        version: "lifecycle-notifications-v1".into(),
        company_id: data.company.id,
        user_id: data.access.user_id,
        employee_id: Some(own),
        role: data.access.role,
        surface: "desktop".into(),
        as_of: today,
        generated_at: now,
        read_keys: vec![],
        total_items: items.len(),
        truncated: false,
        items,
    }
}
