use crate::domain::{Employee, Snapshot};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmploymentKind {
    EmploymentType,
    WorkLocation,
}

impl EmploymentKind {
    pub const ALL: [Self; 2] = [Self::EmploymentType, Self::WorkLocation];
    pub fn label(self) -> &'static str {
        match self {
            Self::EmploymentType => "Employment type",
            Self::WorkLocation => "Work location",
        }
    }
    pub fn plural(self) -> &'static str {
        match self {
            Self::EmploymentType => "Employment types",
            Self::WorkLocation => "Work locations",
        }
    }
    pub fn table(self) -> &'static str {
        match self {
            Self::EmploymentType => "employment_types",
            Self::WorkLocation => "work_locations",
        }
    }
    pub fn choices(self, data: &Snapshot) -> &[EmploymentOption] {
        match self {
            Self::EmploymentType => &data.employment_types,
            Self::WorkLocation => &data.work_locations,
        }
    }
    pub fn assignment(self, employee: &Employee) -> Option<Uuid> {
        match self {
            Self::EmploymentType => employee.employment_type_id,
            Self::WorkLocation => employee.work_location_id,
        }
    }
    pub fn display(self, data: &Snapshot, id: Option<Uuid>) -> String {
        id.and_then(|id| self.choices(data).iter().find(|c| c.id == id))
            .map(|c| format!("{}{}", c.name, if c.active { "" } else { " (inactive)" }))
            .unwrap_or_else(|| "Not set".into())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmploymentOption {
    pub id: Uuid,
    pub company_id: Uuid,
    pub name: String,
    pub active: bool,
    pub revision: i64,
}

#[derive(Clone, Debug)]
pub struct EmploymentOptionDraft {
    pub kind: EmploymentKind,
    pub id: Option<Uuid>,
    pub name: String,
    pub active: bool,
    pub expected_revision: Option<i64>,
}

impl EmploymentOptionDraft {
    pub fn new(kind: EmploymentKind) -> Self {
        Self {
            kind,
            id: None,
            name: String::new(),
            active: true,
            expected_revision: None,
        }
    }
    pub fn existing(kind: EmploymentKind, value: &EmploymentOption) -> Self {
        Self {
            kind,
            id: Some(value.id),
            name: value.name.clone(),
            active: value.active,
            expected_revision: Some(value.revision),
        }
    }
    pub fn option(&self, data: &Snapshot) -> Result<EmploymentOption> {
        ensure!(
            data.access.role.manages_accounts(),
            "Only Administrators can manage employment choices."
        );
        let name = self.name.trim();
        ensure!(
            !name.is_empty() && name.chars().count() <= 80 && !name.chars().any(char::is_control),
            "Enter a name of 1–80 characters without control characters."
        );
        if let Some(id) = self.id {
            let value = self
                .kind
                .choices(data)
                .iter()
                .find(|c| c.id == id && c.company_id == data.company.id)
                .ok_or_else(|| {
                    anyhow::anyhow!("This choice is unavailable. Cancel, refresh and reopen it.")
                })?;
            ensure!(
                Some(value.revision) == self.expected_revision,
                "This choice changed. Cancel, refresh and reopen it before saving."
            );
        } else {
            ensure!(
                self.expected_revision.is_none(),
                "Refresh before adding a choice."
            );
        }
        ensure!(
            !self
                .kind
                .choices(data)
                .iter()
                .any(|c| Some(c.id) != self.id && c.name.to_lowercase() == name.to_lowercase()),
            "That name already exists, including inactive choices. Edit or reactivate the existing choice."
        );
        Ok(EmploymentOption {
            id: self.id.unwrap_or_else(Uuid::new_v4),
            company_id: data.company.id,
            name: name.into(),
            active: self.active,
            revision: self.expected_revision.map(|r| r + 1).unwrap_or(0),
        })
    }
    pub fn payload(&self) -> serde_json::Value {
        serde_json::json!({"id":self.id,"name":self.name.trim(),"active":self.active,"expected_revision":self.expected_revision})
    }
}

pub fn validate_assignments(data: &Snapshot, employee: &Employee) -> Result<()> {
    ensure!(
        employee.company_id == data.company.id,
        "Choose an employee in this company."
    );
    let previous = data.employees.iter().find(|e| e.id == employee.id);
    for kind in EmploymentKind::ALL {
        if let Some(id) = kind.assignment(employee) {
            let choice = kind
                .choices(data)
                .iter()
                .find(|c| c.id == id && c.company_id == employee.company_id)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Choose a valid {} in this company.",
                        kind.label().to_lowercase()
                    )
                })?;
            ensure!(
                choice.active || previous.and_then(|e| kind.assignment(e)) == Some(id),
                "Choose an active {}. Existing inactive assignments can be retained.",
                kind.label().to_lowercase()
            );
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EmploymentFilter {
    #[default]
    All,
    NotSet,
    Choice(Uuid),
}
impl EmploymentFilter {
    pub fn matches(self, assignment: Option<Uuid>) -> bool {
        match self {
            Self::All => true,
            Self::NotSet => assignment.is_none(),
            Self::Choice(id) => assignment == Some(id),
        }
    }
}

pub fn demo_choices(kind: EmploymentKind, company: Uuid) -> Vec<EmploymentOption> {
    let (base, names): (u128, &[&str]) = match kind {
        EmploymentKind::EmploymentType => {
            (1000, &["Full-time", "Part-time", "Contract", "Internship"])
        }
        EmploymentKind::WorkLocation => (2000, &["Main office", "Remote"]),
    };
    names
        .iter()
        .enumerate()
        .map(|(i, name)| EmploymentOption {
            id: Uuid::from_u128(base + i as u128),
            company_id: company,
            name: (*name).into(),
            active: true,
            revision: 0,
        })
        .collect()
}
