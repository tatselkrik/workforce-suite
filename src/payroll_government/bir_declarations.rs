//! Literal local declarations beside current recorded sources. No expected
//! return, tax calculation, variance, certificate or official acceptance.
use super::{annual, bir_file as bir, bir_identity as identity, *};
use std::path::Path;

#[derive(Clone, Debug, Serialize)]
pub struct Row {
    pub identity: identity::Row,
    pub declared: Vec<bir::DeclaredAmount>,
    pub sources: Vec<annual::Entry>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub identity: identity::Report,
    pub rows: Vec<Row>,
    /// Includes all observed states, not only actual receipt sources.
    pub sources_without_unique_file_match: usize,
}

pub fn review(
    basis: identity::Basis,
    bytes: &[u8],
    name: &str,
    today: NaiveDate,
    user: Uuid,
) -> Result<Report> {
    // Both records originate in the existing protected, same-statement wire.
    let ledger = basis.ledger.clone();
    let employer = basis.employer.as_ref().map(|r| bir::Employer {
        tin: r.tin.clone(),
        branch: r.branch.clone(),
    });
    let identity = identity::review(basis, bytes, name, today, user)?;
    let preflight = bir::inspect(
        bytes,
        name,
        today,
        employer.as_ref(),
        identity.company_id,
        identity.user_id,
    )?;
    ensure!(
        preflight.sha256 == identity.source_sha256 && preflight.schedule == identity.schedule,
        "BIR declaration and identity fingerprints differ."
    );
    let values = bir::checked_declared_money(bytes, &preflight)?;
    ensure!(
        values.len() == identity.rows.len(),
        "Recheck the complete BIR file."
    );
    let mut sources: HashMap<Uuid, Vec<annual::Entry>> = HashMap::new();
    for entry in ledger.wire.entries {
        sources
            .entry(entry.fact.employee_id)
            .or_default()
            .push(entry);
    }
    let mut linked = HashSet::new();
    let mut rows = Vec::with_capacity(values.len());
    for (matched, (line, declared)) in identity.rows.iter().zip(values) {
        ensure!(
            matched.line == line,
            "BIR declaration physical lines differ."
        );
        let entries = if let Some(person) = &matched.employee {
            ensure!(
                matched.status == identity::Status::CurrentPairMatched
                    && linked.insert(person.employee_id),
                "A person cannot receive duplicate or uncertain file-source links."
            );
            sources.remove(&person.employee_id).unwrap_or_default()
        } else {
            ensure!(
                matched.status != identity::Status::CurrentPairMatched,
                "A current identity match lacks its employee."
            );
            Vec::new()
        };
        rows.push(Row {
            identity: matched.clone(),
            declared,
            sources: entries,
        });
    }
    Ok(Report {
        identity,
        rows,
        sources_without_unique_file_match: sources.values().map(Vec::len).sum(),
    })
}

impl Report {
    pub fn export(&self, path: &Path) -> Result<Vec<u8>> {
        let headers = [
            "Worksheet",
            "Source SHA256",
            "Year",
            "As of",
            "Current snapshot UTC",
            "Schedule",
            "File line",
            "Identity review",
            "Current employee code",
            "Current employee name",
            "Record type",
            "File column",
            "Declared field",
            "File-declared PHP",
            "Source kind",
            "Source ID",
            "Service from",
            "Service to",
            "Actual receipt",
            "Source state",
            "Recorded withholding PHP",
            "Received-in-year PHP",
            "Source treatment",
            "Observed months",
        ]
        .map(String::from)
        .to_vec();
        let mut rows = Vec::new();
        for r in &self.rows {
            let base = vec![
                "INTERNAL DECLARATIONS AND RECORDED SOURCES; NOT EXPECTED TAX OR A BIR RETURN"
                    .into(),
                self.identity.source_sha256.clone(),
                self.identity.year.to_string(),
                self.identity.as_of.to_string(),
                self.identity.generated_at.to_rfc3339(),
                self.identity.schedule.label().into(),
                r.identity.line.to_string(),
                r.identity.status.label().into(),
                r.identity
                    .employee
                    .as_ref()
                    .map(|p| p.employee_code.clone())
                    .unwrap_or_default(),
                r.identity
                    .employee
                    .as_ref()
                    .map(|p| p.employee_name.clone())
                    .unwrap_or_default(),
            ];
            for d in &r.declared {
                let mut row = base.clone();
                row.extend([
                    "FILE_DECLARATION".into(),
                    d.column.to_string(),
                    d.field.clone(),
                    decimal(d.cents),
                ]);
                row.resize(headers.len(), String::new());
                rows.push(row);
            }
            for e in &r.sources {
                let f = &e.fact;
                let mut row = base.clone();
                row.extend([
                    "RECORDED_SOURCE".into(),
                    String::new(),
                    String::new(),
                    String::new(),
                    f.kind.label().into(),
                    f.id.to_string(),
                    f.starts_on.to_string(),
                    f.ends_on.to_string(),
                    f.received_on.map(|d| d.to_string()).unwrap_or_default(),
                    f.state.label().into(),
                    f.parts
                        .employee_cents
                        .map(decimal)
                        .unwrap_or_else(|| "Unknown".into()),
                    if f.state == State::Received {
                        f.parts
                            .employee_cents
                            .map(decimal)
                            .unwrap_or_else(|| "Unknown".into())
                    } else {
                        String::new()
                    },
                    e.treatment().into(),
                    e.observed_months
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("; "),
                ]);
                rows.push(row);
            }
            if r.sources.is_empty() {
                let mut row = base;
                row.resize(headers.len(), String::new());
                row[10] = "SOURCE_REVIEW".into();
                row[22] = if r.identity.employee.is_some() {
                    "No sources observed for the current match; not a zero-tax declaration"
                } else {
                    "No unique current employee selected; no payroll sources linked"
                }
                .into();
                rows.push(row);
            }
        }
        super::export::Table { headers, rows }.render(path)
    }
}

#[cfg(test)]
mod tests;
