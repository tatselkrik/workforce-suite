//! Local current-registration concordance. No name heuristics, expected tax,
//! historical identity certification or agency acceptance is inferred.
use super::{annual, bir_file as bir, *};
use std::path::Path;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Registration {
    pub id: Uuid,
    pub company_id: Uuid,
    pub employee_id: Option<Uuid>,
    pub revision: i64,
    pub tin: String,
    pub branch: String,
}
impl Registration {
    fn validate(&self, cid: Uuid, eid: Option<Uuid>) -> Result<()> {
        ensure!(
            !self.id.is_nil()
                && self.company_id == cid
                && self.employee_id == eid
                && self.revision >= 0
                && (self.tin.is_empty()
                    || (self.tin.len() == 9
                        && self.tin.bytes().all(|b| b.is_ascii_digit())
                        && self.tin.bytes().any(|b| b != b'0')))
                && (self.branch.is_empty()
                    || (self.branch.len() == 4 && self.branch.bytes().all(|b| b.is_ascii_digit()))),
            "BIR registration binding is outside the current protected scope or is not canonical."
        );
        Ok(())
    }
    fn usable(&self) -> bool {
        bir::matchable_identity(&self.tin, &self.branch)
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Person {
    pub label: EmployeeLabel,
    pub registration: Option<Registration>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Wire {
    pub version: String,
    pub ledger: annual::Wire,
    pub employer: Option<Registration>,
    pub people: Vec<Person>,
}
pub struct Basis {
    pub ledger: annual::Ledger,
    pub employer: Option<Registration>,
    pub people: Vec<Person>,
}
impl Basis {
    pub fn from_wire(wire: Wire, cid: Uuid, year: i32) -> Result<Self> {
        ensure!(
            wire.version == "government-bir-identities-v1" && wire.people.len() <= 20_000,
            "BIR identity response is unsupported or exceeds the roster limit."
        );
        let ledger = annual::Ledger::from_wire(wire.ledger, cid, year)?;
        let mut people = HashMap::new();
        let mut codes = HashSet::new();
        let mut registrations = HashSet::new();
        if let Some(e) = &wire.employer {
            e.validate(cid, None)?;
            registrations.insert(e.id);
        }
        for (i, p) in wire.people.iter().enumerate() {
            let e = &p.label;
            ensure!(
                !e.employee_id.is_nil()
                    && people.insert(e.employee_id, i).is_none()
                    && codes.insert(e.employee_code.clone())
                    && [&e.employee_code, &e.employee_name]
                        .iter()
                        .all(|s| !s.trim().is_empty()
                            && s.chars().count() <= 200
                            && !s.chars().any(char::is_control)),
                "Current BIR roster has duplicate, invalid or foreign employee labels."
            );
            if let Some(r) = &p.registration {
                r.validate(cid, Some(e.employee_id))?;
                ensure!(
                    registrations.insert(r.id),
                    "Duplicate BIR registration binding."
                );
            }
        }
        for e in &ledger.wire.employees {
            ensure!(
                people.get(&e.employee_id).is_some_and(|i| {
                    let p = &wire.people[*i].label;
                    p.employee_code == e.employee_code && p.employee_name == e.employee_name
                }),
                "Annual source labels do not match the same-statement current roster."
            );
        }
        Ok(Self {
            ledger,
            employer: wire.employer,
            people: wire.people,
        })
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum Status {
    CurrentPairMatched,
    Unmatched,
    Ambiguous,
}
impl Status {
    pub fn label(&self) -> &'static str {
        match self {
            Self::CurrentPairMatched => "Current registration pair matched",
            Self::Unmatched => "No current registration pair matched",
            Self::Ambiguous => "Multiple current registrations use this pair",
        }
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct Row {
    pub line: usize,
    pub status: Status,
    pub employee: Option<EmployeeLabel>,
    pub candidates: usize,
    pub observed_sources: Option<usize>,
    pub received_sources: Option<usize>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub company_id: Uuid,
    pub user_id: Uuid,
    pub year: i32,
    pub as_of: NaiveDate,
    pub generated_at: DateTime<Utc>,
    pub source_sha256: String,
    pub file_name: String,
    pub schedule: bir::Schedule,
    pub preflight_reviews: usize,
    pub roster_people: usize,
    pub usable_registrations: usize,
    pub received_people_not_uniquely_matched: usize,
    pub rows: Vec<Row>,
}
impl Report {
    pub fn export(&self, path: &Path) -> Result<Vec<u8>> {
        let headers = [
            "Scope",
            "Source SHA256",
            "Year",
            "Current snapshot UTC",
            "As of",
            "File line",
            "Identity review",
            "Current employee code",
            "Current employee name",
            "Candidate registrations",
            "Observed annual sources",
            "Actual receipt sources in year",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        let rows = self
            .rows
            .iter()
            .map(|r| {
                vec![
                    "INTERNAL CURRENT IDENTITY REVIEW; NOT EXPECTED TAX OR OFFICIAL VALIDATION"
                        .into(),
                    self.source_sha256.clone(),
                    self.year.to_string(),
                    self.generated_at.to_rfc3339(),
                    self.as_of.to_string(),
                    r.line.to_string(),
                    r.status.label().into(),
                    r.employee
                        .as_ref()
                        .map(|e| e.employee_code.clone())
                        .unwrap_or_default(),
                    r.employee
                        .as_ref()
                        .map(|e| e.employee_name.clone())
                        .unwrap_or_default(),
                    r.candidates.to_string(),
                    r.observed_sources
                        .map(|n| n.to_string())
                        .unwrap_or_default(),
                    r.received_sources
                        .map(|n| n.to_string())
                        .unwrap_or_default(),
                ]
            })
            .collect();
        super::export::Table { headers, rows }.render(path)
    }
}
pub fn review(
    basis: Basis,
    bytes: &[u8],
    name: &str,
    today: NaiveDate,
    uid: Uuid,
) -> Result<Report> {
    ensure!(!uid.is_nil(), "Sign in before reviewing BIR identities.");
    let employer=basis.employer.as_ref().filter(|e|e.usable()).ok_or_else(||anyhow::anyhow!("Complete the company's current BIR TIN and branch in Government registration details before identity review."))?;
    let w = &basis.ledger.wire;
    let checked = bir::inspect(
        bytes,
        name,
        today,
        Some(&bir::Employer {
            tin: employer.tin.clone(),
            branch: employer.branch.clone(),
        }),
        w.company_id,
        uid,
    )?;
    ensure!(
        checked.errors == 0 && checked.employer_match == Some(true) && checked.year == Some(w.year),
        "Use the selected year's matching employer file and resolve all structure/control errors in Check BIR 1604C file before identity review."
    );
    let identities = bir::checked_identities(bytes, &checked)?;
    let mut pairs: HashMap<(&str, &str), Vec<&Person>> = HashMap::new();
    for p in &basis.people {
        if let Some(r) = p.registration.as_ref().filter(|r| r.usable()) {
            pairs.entry((&r.tin, &r.branch)).or_default().push(p);
        }
    }
    let mut counts: HashMap<Uuid, (usize, usize)> = HashMap::new();
    let mut received_people = HashSet::new();
    for e in &w.entries {
        let c = counts.entry(e.fact.employee_id).or_default();
        c.0 += 1;
        if e.fact.state == State::Received {
            c.1 += 1;
            received_people.insert(e.fact.employee_id);
        }
    }
    let mut matched_people = HashSet::new();
    let mut rows = vec![];
    for row in identities {
        let candidates = pairs
            .get(&(row.tin.as_str(), row.branch.as_str()))
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let (status, employee, observed, received) = match candidates {
            [p] => {
                matched_people.insert(p.label.employee_id);
                let c = counts
                    .get(&p.label.employee_id)
                    .copied()
                    .unwrap_or_default();
                (
                    Status::CurrentPairMatched,
                    Some(p.label.clone()),
                    Some(c.0),
                    Some(c.1),
                )
            }
            [] => (Status::Unmatched, None, None, None),
            _ => (Status::Ambiguous, None, None, None),
        };
        rows.push(Row {
            line: row.line,
            status,
            employee,
            candidates: candidates.len(),
            observed_sources: observed,
            received_sources: received,
        });
    }
    Ok(Report {
        company_id: w.company_id,
        user_id: uid,
        year: w.year,
        as_of: w.as_of,
        generated_at: w.generated_at,
        source_sha256: checked.sha256,
        file_name: checked.file_name,
        schedule: checked.schedule,
        preflight_reviews: checked.reviews,
        roster_people: basis.people.len(),
        usable_registrations: basis
            .people
            .iter()
            .filter(|p| p.registration.as_ref().is_some_and(Registration::usable))
            .count(),
        received_people_not_uniquely_matched: received_people.difference(&matched_people).count(),
        rows,
    })
}

#[cfg(test)]
pub(crate) mod tests;
