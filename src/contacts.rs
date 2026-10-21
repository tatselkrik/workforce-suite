use anyhow::{Result, ensure};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Company-managed HR details, separate from account Profile and directory access.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmployeeContacts {
    pub company_id: Uuid,
    pub employee_id: Uuid,
    pub phone: String,
    pub personal_email: String,
    pub address: String,
    pub emergency_name: String,
    pub emergency_relationship: String,
    pub emergency_phone: String,
    pub emergency_alternate_phone: String,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

impl EmployeeContacts {
    pub fn empty(company_id: Uuid, employee_id: Uuid) -> Self {
        Self {
            company_id,
            employee_id,
            ..Self::default()
        }
    }

    pub fn normalized(&self) -> Result<Self> {
        let mut result = self.clone();
        for field in [
            &mut result.phone,
            &mut result.personal_email,
            &mut result.address,
            &mut result.emergency_name,
            &mut result.emergency_relationship,
            &mut result.emergency_phone,
            &mut result.emergency_alternate_phone,
        ] {
            *field = field.trim().to_owned();
        }
        ensure!(
            result.address.chars().count() <= 500,
            "Keep the address to 500 characters."
        );
        ensure!(
            result.emergency_name.chars().count() <= 120,
            "Keep the emergency contact name to 120 characters."
        );
        ensure!(
            result.emergency_relationship.chars().count() <= 80,
            "Keep the relationship to 80 characters."
        );
        if !result.personal_email.is_empty() {
            ensure!(
                crate::account::validate_email(&result.personal_email).is_ok(),
                "Enter a valid personal email address."
            );
        }
        for phone in [
            &result.phone,
            &result.emergency_phone,
            &result.emergency_alternate_phone,
        ] {
            ensure!(
                phone.is_empty()
                    || (phone.chars().count() <= 40
                        && phone.chars().filter(char::is_ascii_digit).count() >= 3
                        && phone
                            .chars()
                            .all(|c| c.is_ascii_digit() || " +()-./xX#".contains(c))),
                "Use phone numbers up to 40 characters with at least 3 digits. Spaces, +, (), -, ., /, x and # are allowed."
            );
        }
        let emergency_empty = result.emergency_name.is_empty()
            && result.emergency_relationship.is_empty()
            && result.emergency_phone.is_empty()
            && result.emergency_alternate_phone.is_empty();
        ensure!(
            emergency_empty
                || (!result.emergency_name.is_empty()
                    && !result.emergency_relationship.is_empty()
                    && !result.emergency_phone.is_empty()),
            "For an emergency contact, enter a name, relationship and primary phone. Otherwise leave all emergency fields blank."
        );
        Ok(result)
    }
}
