//! HR-private registration facts. These neither rename employees nor certify filings.
use super::*;
use anyhow::Context;
use std::collections::BTreeMap;
pub type Details = BTreeMap<String, String>;
const COMMON: &[&str] = &[
    "address_line1",
    "address_line2",
    "city",
    "province",
    "postal_code",
    "country",
    "bir_tin",
    "bir_branch",
    "bir_rdo",
    "sss_number",
    "philhealth_number",
    "pagibig_number",
];
const PERSON: &[&str] = &[
    "last_name",
    "first_name",
    "middle_name",
    "name_suffix",
    "birth_date",
    "sex",
    "nationality",
    "bir_region",
];
const EMPLOYER: &[&str] = &["registered_name", "telephone", "email", "employer_type"];
pub fn fields(person: bool) -> Vec<&'static str> {
    (if person { PERSON } else { EMPLOYER })
        .iter()
        .chain(COMMON.iter())
        .copied()
        .collect()
}
pub fn label(key: &str, person: bool) -> &'static str {
    match key {
        "registered_name" => "Registered employer name",
        "last_name" => "Legal last name",
        "first_name" => "Legal first name",
        "middle_name" => "Legal middle name",
        "name_suffix" => "Name suffix",
        "birth_date" => "Date of birth (YYYY-MM-DD)",
        "sex" => "Sex recorded with agency",
        "nationality" => "Nationality recorded with agency",
        "bir_region" => "BIR assigned region code",
        "address_line1" => "Registered address — line 1",
        "address_line2" => "Registered address — line 2",
        "city" => "City / municipality",
        "province" => "Province / state",
        "postal_code" => "Postal code",
        "country" => "Country code",
        "bir_tin" => "BIR TIN",
        "bir_branch" => "BIR branch code",
        "bir_rdo" => "BIR RDO code",
        "sss_number" => {
            if person {
                "SSS member number"
            } else {
                "SSS employer number"
            }
        }
        "philhealth_number" => {
            if person {
                "PhilHealth PIN"
            } else {
                "PhilHealth PEN"
            }
        }
        "pagibig_number" => {
            if person {
                "Pag-IBIG MID"
            } else {
                "Pag-IBIG employer number"
            }
        }
        "telephone" => "Registered telephone",
        "email" => "Registered contact email",
        "employer_type" => "Agency employer category",
        _ => "Registration field",
    }
}
pub fn normalize(values: &Details, person: bool, today: NaiveDate) -> Result<Details> {
    let keys = fields(person);
    ensure!(
        values.keys().all(|k| keys.contains(&k.as_str())),
        "A registration field is outside the selected scope."
    );
    let mut out = Details::new();
    for k in keys {
        let mut v = values
            .get(k)
            .map(|v| v.trim_matches(' '))
            .unwrap_or("")
            .to_owned();
        let max = match k {
            "address_line1" | "address_line2" => 300,
            "registered_name" | "email" => 254,
            _ => 100,
        };
        ensure!(
            v.chars().count() <= max && !v.chars().any(char::is_control),
            "{} is too long or contains control characters.",
            label(k, person)
        );
        let length = match k {
            "bir_tin" => Some(9),
            "bir_branch" => Some(4),
            "bir_rdo" => Some(3),
            "bir_region" => Some(4),
            "sss_number" => Some(10),
            "philhealth_number" => Some(12),
            "pagibig_number" if person => Some(12),
            _ => None,
        };
        if !v.is_empty() && (length.is_some() || k == "pagibig_number") {
            ensure!(
                v.bytes()
                    .all(|c| c.is_ascii_digit() || c == b' ' || c == b'-'),
                "{} accepts digits, spaces and dashes.",
                label(k, person)
            );
            v.retain(|c| c != ' ' && c != '-');
            ensure!(
                length.map_or((1..=20).contains(&v.len()), |n| v.len() == n),
                "Check {} against the registration document.",
                label(k, person)
            );
            ensure!(
                ![
                    "bir_tin",
                    "sss_number",
                    "philhealth_number",
                    "pagibig_number"
                ]
                .contains(&k)
                    || v.bytes().any(|c| c != b'0'),
                "Leave an unassigned identifier blank; zero placeholders are not assigned IDs."
            );
        } else if k == "birth_date" && !v.is_empty() {
            ensure!(
                v.len() == 10
                    && v.as_bytes()
                        .iter()
                        .enumerate()
                        .all(|(i, c)| if i == 4 || i == 7 {
                            *c == b'-'
                        } else {
                            c.is_ascii_digit()
                        }),
                "Use YYYY-MM-DD for the recorded birth date."
            );
            let d: NaiveDate = v.parse().context("Use a valid recorded birth date.")?;
            ensure!(
                d >= NaiveDate::from_ymd_opt(1900, 1, 1).unwrap() && d <= today,
                "Check the recorded birth date."
            );
        } else if k == "sex" {
            ensure!(
                ["", "M", "F"].contains(&v.as_str()),
                "Choose the recorded agency sex code or unknown."
            );
        } else if k == "country" && !v.is_empty() {
            v.make_ascii_uppercase();
            ensure!(
                v.len() == 2 && v.bytes().all(|c| c.is_ascii_uppercase()),
                "Use a two-letter country code."
            );
        } else if k == "employer_type" {
            ensure!(
                ["", "private", "government", "household"].contains(&v.as_str()),
                "Choose the recorded employer category."
            );
        } else if k == "email" && !v.is_empty() {
            let valid = v.split_once('@').is_some_and(|(name, domain)| {
                !name.is_empty()
                    && !domain.contains('@')
                    && !v.contains(' ')
                    && domain
                        .split_once('.')
                        .is_some_and(|(a, b)| !a.is_empty() && !b.is_empty())
            });
            ensure!(valid, "Check the registered contact email.");
        }
        out.insert(k.into(), v);
    }
    Ok(out)
}
fn text(value: &str, max: usize, required: bool) -> Result<()> {
    ensure!(
        (!required || !value.trim().is_empty())
            && value.chars().count() <= max
            && !value
                .chars()
                .any(|c| c.is_control() && c != '\n' && c != '\t'),
        "Check the registration reference/reason."
    );
    Ok(())
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub id: Uuid,
    pub company_id: Uuid,
    pub employee_id: Option<Uuid>,
    pub revision: i64,
    pub details: Details,
    pub reference: String,
    pub note: String,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
impl Record {
    fn validate(&self, cid: Uuid, eid: Option<Uuid>, today: NaiveDate) -> Result<()> {
        ensure!(
            self.company_id == cid
                && self.employee_id == eid
                && !self.id.is_nil()
                && !self.created_by.is_nil()
                && !self.updated_by.is_nil()
                && self.revision >= 0,
            "Registration identity/revision is inconsistent."
        );
        ensure!(
            self.details == normalize(&self.details, eid.is_some(), today)?,
            "Registration fields are not canonical."
        );
        text(&self.reference, 2000, true)?;
        text(&self.note, 2000, false)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub id: Uuid,
    pub company_id: Uuid,
    pub record_id: Uuid,
    pub revision: i64,
    pub actor_id: Uuid,
    pub actor_name: String,
    pub reason: String,
    pub before_record: Option<Record>,
    pub after_record: Record,
    pub recorded_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub version: String,
    pub company_id: Uuid,
    pub employee_id: Option<Uuid>,
    pub record: Option<Record>,
    pub history: Vec<Event>,
}
impl Snapshot {
    pub fn validate(&self, cid: Uuid, eid: Option<Uuid>, today: NaiveDate) -> Result<()> {
        ensure!(
            self.version == "government-registration-v1"
                && self.company_id == cid
                && self.employee_id == eid
                && !cid.is_nil()
                && eid.is_none_or(|id| !id.is_nil())
                && self.history.len() <= 1000,
            "Registration response is outside the protected scope/version."
        );
        let Some(record) = &self.record else {
            ensure!(
                self.history.is_empty(),
                "History has no registration record."
            );
            return Ok(());
        };
        record.validate(cid, eid, today)?;
        ensure!(
            Some(self.history.len() as i64) == record.revision.checked_add(1),
            "Registration revision history is incomplete."
        );
        let mut seen = HashSet::new();
        let mut prior: Option<&Record> = None;
        for (i, e) in self.history.iter().enumerate() {
            ensure!(
                !e.id.is_nil()
                    && seen.insert(e.id)
                    && e.company_id == cid
                    && e.record_id == record.id
                    && e.revision == i as i64
                    && !e.actor_id.is_nil()
                    && e.after_record.id == record.id
                    && e.after_record.revision == e.revision
                    && e.after_record.created_by == record.created_by
                    && e.after_record.created_at == record.created_at
                    && e.after_record.updated_by == e.actor_id
                    && e.before_record.as_ref() == prior,
                "Registration audit history is inconsistent."
            );
            e.after_record.validate(cid, eid, today)?;
            text(&e.actor_name, 200, true)?;
            text(&e.reason, 2000, true)?;
            prior = Some(&e.after_record);
        }
        ensure!(
            prior == Some(record),
            "Current registration does not match its retained history."
        );
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct Save {
    pub id: Uuid,
    pub operation_id: Uuid,
    pub employee_id: Option<Uuid>,
    pub expected_revision: Option<i64>,
    pub details: Details,
    pub reference: String,
    pub note: String,
    pub reason: String,
    pub reviewed: bool,
}
impl Save {
    pub fn validate(&self, current: &crate::domain::Snapshot) -> Result<Self> {
        crate::payroll_reporting::authorize(current)?;
        ensure!(
            !self.id.is_nil()
                && !self.operation_id.is_nil()
                && self
                    .expected_revision
                    .is_none_or(|n| (0..=100_000_000_000_000).contains(&n))
                && self.reviewed,
            "Review the registration evidence and current revision."
        );
        ensure!(
            self.employee_id
                .is_none_or(|id| current.employees.iter().any(|e| e.id == id)),
            "Employee outside the current company."
        );
        text(&self.reference, 2000, true)?;
        text(&self.note, 2000, false)?;
        text(&self.reason, 2000, true)?;
        let mut result = self.clone();
        result.details = normalize(
            &self.details,
            self.employee_id.is_some(),
            current.company.today(),
        )?;
        Ok(result)
    }
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub operation_id: Uuid,
    pub record_id: Uuid,
    pub saved_revision: i64,
}

#[cfg(any(test, feature = "qa-screenshot"))]
pub(crate) fn preview(cid: Uuid, eid: Option<Uuid>) -> Snapshot {
    let now = Utc::now();
    let actor = Uuid::from_u128(160010);
    let mut values = Details::new();
    for (k, v) in [
        ("bir_tin", "012345678"),
        ("bir_branch", "0000"),
        ("sss_number", "0123456789"),
        ("philhealth_number", "012345678901"),
        ("pagibig_number", "012345678901"),
        ("country", "PH"),
        (
            "address_line1",
            "Fictional Building, Sample Street and Barangay",
        ),
        ("city", "Fictional City"),
    ] {
        values.insert(k.into(), v.into());
    }
    if eid.is_some() {
        for (k, v) in [
            ("last_name", "Dela Cruz"),
            ("first_name", "Fictional Taylor"),
            ("middle_name", "Ángel"),
            ("birth_date", "1990-02-28"),
            ("sex", "M"),
        ] {
            values.insert(k.into(), v.into());
        }
    } else {
        values.insert(
            "registered_name".into(),
            "Northstar Registered Industries (fictional)".into(),
        );
        values.insert("employer_type".into(), "private".into());
    }
    let r = Record {
        id: Uuid::from_u128(if eid.is_some() { 160002 } else { 160001 }),
        company_id: cid,
        employee_id: eid,
        revision: 0,
        details: normalize(&values, eid.is_some(), now.date_naive()).unwrap(),
        reference: "FICTIONAL-REGISTRATION-REFERENCE".into(),
        note: "Fictional renderer data only; no actual agency verification.".into(),
        created_by: actor,
        updated_by: actor,
        created_at: now,
        updated_at: now,
    };
    Snapshot {
        version: "government-registration-v1".into(),
        company_id: cid,
        employee_id: eid,
        record: Some(r.clone()),
        history: vec![Event {
            id: Uuid::from_u128(160020),
            company_id: cid,
            record_id: r.id,
            revision: 0,
            actor_id: actor,
            actor_name: "Fictional HR preparer".into(),
            reason: "Record explicit registration facts from fictional evidence.".into(),
            before_record: None,
            after_record: r,
            recorded_at: now,
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn known_fields_preserve_ids_names_and_unknowns_without_splitting_or_zero_defaults() {
        let d = NaiveDate::from_ymd_opt(2026, 10, 25).unwrap();
        let mut v = Details::new();
        v.insert("last_name".into(), "Dela Cruz".into());
        v.insert("middle_name".into(), "Ángel".into());
        v.insert("bir_tin".into(), "012-345-678".into());
        v.insert("bir_branch".into(), "0000".into());
        let p = normalize(&v, true, d).unwrap();
        assert_eq!(p["bir_tin"], "012345678");
        assert_eq!(p["first_name"], "");
        assert_eq!(p["middle_name"], "Ángel");
        assert_eq!(p["philhealth_number"], "");
        for (k, bad) in [
            ("bir_tin", "000000000"),
            ("birth_date", "1900-02-29"),
            ("birth_date", "2200-01-01"),
            ("birth_date", "1990-1-01"),
            ("sex", "Other"),
            ("sss_number", "O123456789"),
            ("last_name", "Name\nInjected"),
        ] {
            let mut bads = v.clone();
            bads.insert(k.into(), bad.into());
            assert!(normalize(&bads, true, d).is_err(), "{k}");
        }
        assert!(normalize(&v, false, d).is_err());
    }
    #[test]
    #[ignore = "Requires local PostgreSQL registration vectors."]
    fn postgres_registration_snapshots_preserve_scopes_and_complete_revision_chains() {
        let path =
            std::env::var("HRIS_REGISTRATION_VECTORS").expect("Set the registration vector path.");
        let values: Vec<Snapshot> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(values.len(), 3);
        for v in values {
            v.validate(v.company_id, v.employee_id, "2026-10-25".parse().unwrap())
                .unwrap();
            assert!(
                v.validate(Uuid::new_v4(), v.employee_id, "2026-10-25".parse().unwrap())
                    .is_err()
            );
            if v.record.is_some() {
                let mut bad = v.clone();
                bad.history.pop();
                assert!(
                    bad.validate(v.company_id, v.employee_id, "2026-10-25".parse().unwrap())
                        .is_err()
                );
            }
        }
    }
}
