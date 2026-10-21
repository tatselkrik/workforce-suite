use crate::{
    access::Role,
    domain::{Employee, Snapshot},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoryKind {
    StartingSnapshot,
    Created,
    Updated,
}
impl HistoryKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::StartingSnapshot => "Starting snapshot",
            Self::Created => "Employee created",
            Self::Updated => "Employment updated",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmploymentState {
    pub employee: Employee,
    pub reports_to_name: Option<String>,
    pub employment_type_name: Option<String>,
    pub work_location_name: Option<String>,
}
impl EmploymentState {
    pub fn capture(data: &Snapshot, employee: &Employee) -> Self {
        Self {
            employee: employee.clone(),
            reports_to_name: employee.reports_to.and_then(|id| {
                data.employees
                    .iter()
                    .find(|e| e.id == id)
                    .map(|e| e.full_name.clone())
            }),
            employment_type_name: employee.employment_type_id.and_then(|id| {
                data.employment_types
                    .iter()
                    .find(|o| o.id == id)
                    .map(|o| o.name.clone())
            }),
            work_location_name: employee.work_location_id.and_then(|id| {
                data.work_locations
                    .iter()
                    .find(|o| o.id == id)
                    .map(|o| o.name.clone())
            }),
        }
    }
    pub fn fields(&self) -> Vec<(&'static str, String)> {
        let e = &self.employee;
        let text = |value: &str| {
            if value.is_empty() {
                "Not set".into()
            } else {
                value.to_owned()
            }
        };
        let reference = |id: Option<Uuid>, name: &Option<String>| {
            if id.is_none() {
                "Not set".into()
            } else {
                name.clone().unwrap_or_else(|| "Name unavailable".into())
            }
        };
        vec![
            ("Employee code", text(&e.employee_code)),
            ("Full name", text(&e.full_name)),
            ("Department", text(&e.department)),
            ("Job title", text(&e.job_title)),
            (
                "Date joined",
                e.joined_on
                    .map(|d| d.to_string())
                    .unwrap_or_else(|| "Not set".into()),
            ),
            (
                "Employment type",
                reference(e.employment_type_id, &self.employment_type_name),
            ),
            (
                "Work location",
                reference(e.work_location_id, &self.work_location_name),
            ),
            ("Reports to", reference(e.reports_to, &self.reports_to_name)),
            (
                "Annual leave allowance",
                format!("{} days", e.annual_leave_days),
            ),
            ("Status", e.status.label().into()),
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmploymentEvent {
    pub id: Uuid,
    pub sequence: i64,
    pub company_id: Uuid,
    pub employee_id: Uuid,
    pub kind: HistoryKind,
    pub actor_id: Option<Uuid>,
    pub actor_name: String,
    pub actor_role: Option<Role>,
    pub before_record: Option<EmploymentState>,
    pub after_record: EmploymentState,
    pub recorded_at: DateTime<Utc>,
}
#[derive(Debug, PartialEq, Eq)]
pub struct FieldChange {
    pub label: &'static str,
    pub before: Option<String>,
    pub after: String,
}
impl EmploymentEvent {
    pub fn changes(&self) -> Vec<FieldChange> {
        let before = self.before_record.as_ref().map(EmploymentState::fields);
        self.after_record
            .fields()
            .into_iter()
            .enumerate()
            .filter_map(|(i, (label, after))| {
                let previous = before.as_ref().map(|fields| fields[i].1.clone());
                // A same-label manager/type/location change must still be shown by identity.
                let identity_changed = self.before_record.as_ref().is_some_and(|old| match label {
                    "Reports to" => {
                        old.employee.reports_to != self.after_record.employee.reports_to
                    }
                    "Employment type" => {
                        old.employee.employment_type_id
                            != self.after_record.employee.employment_type_id
                    }
                    "Work location" => {
                        old.employee.work_location_id != self.after_record.employee.work_location_id
                    }
                    _ => false,
                });
                (previous.as_ref() != Some(&after) || identity_changed).then_some(FieldChange {
                    label,
                    before: previous,
                    after,
                })
            })
            .collect()
    }
}

pub fn timeline(data: &Snapshot, employee: Uuid) -> Vec<&EmploymentEvent> {
    if !data.access.role.manages_hr() {
        return Vec::new();
    }
    let mut events: Vec<_> = data
        .employment_history
        .iter()
        .filter(|event| event.company_id == data.company.id && event.employee_id == employee)
        .collect();
    events.sort_by(|a, b| b.sequence.cmp(&a.sequence).then(b.id.cmp(&a.id)));
    events
}

pub fn demo_initial(data: &Snapshot) -> Vec<EmploymentEvent> {
    data.employees
        .iter()
        .enumerate()
        .map(|(index, employee)| EmploymentEvent {
            id: Uuid::new_v4(),
            sequence: index as i64 + 1,
            company_id: data.company.id,
            employee_id: employee.id,
            kind: HistoryKind::StartingSnapshot,
            actor_id: None,
            actor_name: "System".into(),
            actor_role: None,
            before_record: None,
            after_record: EmploymentState::capture(data, employee),
            recorded_at: Utc::now(),
        })
        .collect()
}

pub fn record_demo_changes(data: &mut Snapshot, before: Vec<EmploymentState>) {
    let mut sequence = data
        .employment_history
        .iter()
        .map(|e| e.sequence)
        .max()
        .unwrap_or(0);
    let actor_name = data
        .own_employee()
        .map(|e| e.full_name.clone())
        .unwrap_or_else(|| "Demo HR".into());
    let mut events = Vec::new();
    for employee in &data.employees {
        let previous = before.iter().find(|state| state.employee.id == employee.id);
        if previous.is_some_and(|state| state.employee == *employee) {
            continue;
        }
        sequence += 1;
        events.push(EmploymentEvent {
            id: Uuid::new_v4(),
            sequence,
            company_id: data.company.id,
            employee_id: employee.id,
            kind: if previous.is_some() {
                HistoryKind::Updated
            } else {
                HistoryKind::Created
            },
            actor_id: Some(data.access.user_id),
            actor_name: actor_name.clone(),
            actor_role: Some(data.access.role),
            before_record: previous.cloned(),
            after_record: EmploymentState::capture(data, employee),
            recorded_at: Utc::now(),
        });
    }
    data.employment_history.extend(events);
}
