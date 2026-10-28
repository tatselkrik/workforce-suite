//! Bounded UTF-8 opening CSV. All row errors are retained; posting is atomic.
use super::dated::{OpeningRecord, Preview};
use crate::domain::Snapshot;
use anyhow::{Context, Result, ensure};
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    io::Read,
    path::Path,
};
use uuid::Uuid;
pub const HEADERS: [&str; 6] = [
    "employee_code",
    "leave_type_code",
    "available_days",
    "remaining_carry_days",
    "reason",
    "reference",
];
pub fn quantity(text: &str) -> Result<i32> {
    const MESSAGE: &str = "Enter 0–366 days in whole or half days, such as 2 or 0.5.";
    let text = text.trim();
    ensure!(text.len() <= 64, MESSAGE);
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    ensure!(
        !whole.is_empty() && whole.bytes().all(|b| b.is_ascii_digit()),
        MESSAGE
    );
    let fraction = fraction.trim_end_matches('0');
    ensure!(matches!(fraction, "" | "5"), MESSAGE);
    let whole: u16 = whole.parse().map_err(|_| anyhow::anyhow!(MESSAGE))?;
    ensure!(whole <= 366, MESSAGE);
    let units = i32::from(whole) * 2 + if fraction == "5" { 1 } else { 0 };
    ensure!(units <= 732, MESSAGE);
    Ok(units)
}
#[derive(Clone, Debug)]
pub struct Row {
    pub line: usize,
    pub employee_code: String,
    pub cells: Vec<String>,
    pub errors: Vec<String>,
    pub record: Option<OpeningRecord>,
}
#[derive(Clone, Debug)]
pub struct Import {
    pub id: Uuid,
    pub filename: String,
    pub rows: Vec<Row>,
}
impl Import {
    pub fn load(data: &Snapshot, path: &Path) -> Result<Self> {
        if path
            .extension()
            .is_some_and(|s| s.eq_ignore_ascii_case("xlsx"))
        {
            let rows = crate::tabular::load_xlsx(path, 100, HEADERS.len())?;
            return Self::from_rows(
                data,
                path.file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("opening.xlsx"),
                rows.into_iter()
                    .map(|r| crate::imports::CsvRow {
                        line: r.number,
                        cells: r.cells,
                    })
                    .collect(),
            );
        }
        ensure!(
            path.extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("csv")),
            "Choose the UTF-8 CSV template for opening balances."
        );
        let mut bytes = Vec::new();
        std::fs::File::open(path)?
            .take(2_097_153)
            .read_to_end(&mut bytes)?;
        ensure!(bytes.len() <= 2_097_152, "Opening CSV exceeds 2 MiB.");
        Self::parse(
            data,
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("opening.csv"),
            std::str::from_utf8(&bytes).context("Save the CSV with UTF-8 encoding.")?,
        )
    }
    pub fn parse(data: &Snapshot, filename: &str, text: &str) -> Result<Self> {
        Self::from_rows(data, filename, crate::imports::parse(text)?)
    }
    fn from_rows(
        data: &Snapshot,
        filename: &str,
        rows: Vec<crate::imports::CsvRow>,
    ) -> Result<Self> {
        ensure!(
            data.access
                .allows(crate::staff_roles::Capability::LeaveAdministration),
            "HR access is required for opening imports."
        );
        let c = data
            .leave_setup
            .dated
            .as_ref()
            .context("Refresh dated leave setup first.")?;
        c.validate(&data.access, data.company.today())?;
        let mut parsed = rows.into_iter();
        let header = parsed.next().context("Include a header row.")?;
        ensure!(
            header.cells.len() == HEADERS.len()
                && header
                    .cells
                    .iter()
                    .map(|s| s.trim().trim_start_matches('\u{feff}'))
                    .eq(HEADERS),
            "Use the six opening-template columns in their original order."
        );
        let mut rows = Vec::new();
        for raw in parsed {
            let mut errors = Vec::new();
            if raw.cells.len() != 6 {
                errors.push(format!("Expected 6 cells; found {}.", raw.cells.len()));
            }
            let get = |i: usize| raw.cells.get(i).map_or("", |s| s.trim());
            let code = get(0).to_uppercase();
            let people = data
                .employees
                .iter()
                .filter(|e| e.employee_code.eq_ignore_ascii_case(&code))
                .collect::<Vec<_>>();
            if people.len() != 1 {
                errors.push("Employee code must match one existing employee.".into());
            }
            let target = quantity(get(2));
            if let Err(e) = &target {
                errors.push(format!("available_days: {e}"));
            }
            let carry = quantity(if get(3).is_empty() { "0" } else { get(3) });
            if let Err(e) = &carry {
                errors.push(format!("remaining_carry_days: {e}"));
            }
            let mut record = None;
            if let (Some(person), Ok(target), Ok(carry)) = (people.first(), target, carry) {
                let r = OpeningRecord {
                    id: Uuid::new_v4(),
                    employee_id: person.id,
                    type_code: get(1).into(),
                    effective_on: c.as_of,
                    target_available_units: target,
                    remaining_carry_units: carry,
                    source_requests_reviewed: true,
                    expected_revision: c.revision,
                    reason: get(4).into(),
                    reference: get(5).into(),
                };
                if let Err(e) = r.validate(data) {
                    errors.push(e.to_string());
                } else {
                    record = Some(r);
                }
            }
            rows.push(Row {
                line: raw.line,
                employee_code: code,
                cells: raw.cells,
                errors,
                record,
            });
        }
        ensure!(
            (1..=100).contains(&rows.len()),
            "Import 1–100 opening rows at a time."
        );
        let mut counts = HashMap::new();
        for row in &rows {
            if let Some(r) = &row.record {
                *counts
                    .entry((r.employee_id, r.type_code.clone()))
                    .or_insert(0) += 1;
            }
        }
        for row in &mut rows {
            if row.record.as_ref().is_some_and(|r| {
                counts
                    .get(&(r.employee_id, r.type_code.clone()))
                    .copied()
                    .unwrap_or(0)
                    > 1
            }) {
                row.errors
                    .push("Duplicate employee/type opening in this file.".into());
            }
        }
        ensure!(
            !filename.chars().any(char::is_control) && filename.chars().count() <= 240,
            "Use a simple CSV filename."
        );
        Ok(Self {
            id: Uuid::new_v4(),
            filename: filename.into(),
            rows,
        })
    }
    pub fn records(&self, data: &Snapshot, reviewed: bool) -> Result<Vec<OpeningRecord>> {
        ensure!(
            reviewed,
            "Review source-system requests and their representation in HRIS first."
        );
        ensure!(
            self.rows
                .iter()
                .all(|r| r.errors.is_empty() && r.record.is_some()),
            "Resolve every CSV row error before previewing; no rows are imported."
        );
        let records = self
            .rows
            .iter()
            .map(|row| {
                let r = row.record.clone().unwrap();
                ensure!(
                    data.employees.iter().any(|e| e.id == r.employee_id
                        && e.employee_code.eq_ignore_ascii_case(&row.employee_code)),
                    "An employee code changed. Reload the CSV and review its target."
                );
                r.validate(data)?;
                Ok(r)
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(records)
    }
    pub fn error_csv(&self) -> Result<Vec<u8>> {
        let mut headers = vec!["source_line"];
        headers.extend(HEADERS);
        headers.push("errors");
        crate::imports::csv(
            &headers,
            self.rows.iter().map(|r| {
                let mut cells = vec![r.line.to_string()];
                cells.extend((0..6).map(|i| r.cells.get(i).cloned().unwrap_or_default()));
                cells.push(r.errors.join("; "));
                cells
            }),
        )
    }
    pub fn error_xlsx(&self) -> Result<Vec<u8>> {
        let mut headers = vec!["source_row"];
        headers.extend(HEADERS);
        headers.push("errors");
        let rows = self
            .rows
            .iter()
            .map(|r| {
                let mut cells = vec![r.line.to_string()];
                cells.extend((0..6).map(|i| r.cells.get(i).cloned().unwrap_or_default()));
                cells.push(r.errors.join("; "));
                cells
            })
            .collect::<Vec<_>>();
        crate::tabular::text_xlsx(&headers, &rows)
    }
}
pub fn template() -> Result<Vec<u8>> {
    crate::imports::csv(&HEADERS, std::iter::empty())
}
pub fn template_xlsx() -> Result<Vec<u8>> {
    crate::tabular::text_xlsx(&HEADERS, &[])
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewRow {
    pub row_number: usize,
    pub record: OpeningRecord,
    pub preview: Option<Preview>,
    pub error: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BatchPreview {
    pub version: String,
    pub company_id: Uuid,
    pub user_id: Uuid,
    pub as_of: NaiveDate,
    pub rows: Vec<PreviewRow>,
}
impl BatchPreview {
    pub fn validate(&self, data: &Snapshot, records: &[OpeningRecord]) -> Result<()> {
        ensure!(
            data.access
                .allows(crate::staff_roles::Capability::LeaveAdministration)
                && self.version == "leave-opening-batch-preview-v1"
                && self.company_id == data.company.id
                && self.user_id == data.access.user_id
                && self.as_of == data.company.today()
                && (1..=100).contains(&self.rows.len())
                && self.rows.len() == records.len(),
            "Opening import preview does not match current access/date or input."
        );
        for (n, (row, record)) in self.rows.iter().zip(records).enumerate() {
            ensure!(
                row.row_number == n + 1 && row.record == *record,
                "Opening import preview row does not match its source."
            );
            match (&row.preview, &row.error) {
                (Some(p), None) => p.validate(data, record)?,
                (None, Some(e)) => ensure!(
                    !e.is_empty() && e.chars().count() <= 250 && !e.chars().any(char::is_control),
                    "Invalid import error response."
                ),
                _ => anyhow::bail!("Each import row needs either a protected preview or an error."),
            }
        }
        ensure!(
            serde_json::to_vec(self)?.len() <= 4_000_000,
            "Opening import preview exceeds the supported size."
        );
        Ok(())
    }
    pub fn records(&self) -> Vec<OpeningRecord> {
        self.rows.iter().map(|r| r.record.clone()).collect()
    }
    pub fn ready(&self) -> bool {
        !self.rows.is_empty()
            && self
                .rows
                .iter()
                .all(|r| r.preview.is_some() && r.error.is_none())
    }
    pub fn expected_revision(&self) -> i64 {
        self.rows.first().map_or(-1, |r| r.record.expected_revision)
    }
    pub fn demo(data: &Snapshot, records: &[OpeningRecord]) -> Self {
        let mut seen = HashSet::new();
        let rows = records
            .iter()
            .enumerate()
            .map(|(n, r)| {
                let result = if seen.insert((r.employee_id, &r.type_code)) {
                    super::dated::preview_demo(data, r)
                } else {
                    Err(anyhow::anyhow!(
                        "Duplicate employee/type opening in this import."
                    ))
                };
                match result {
                    Ok(p) => PreviewRow {
                        row_number: n + 1,
                        record: r.clone(),
                        preview: Some(p),
                        error: None,
                    },
                    Err(e) => PreviewRow {
                        row_number: n + 1,
                        record: r.clone(),
                        preview: None,
                        error: Some(e.to_string()),
                    },
                }
            })
            .collect();
        Self {
            version: "leave-opening-batch-preview-v1".into(),
            company_id: data.company.id,
            user_id: data.access.user_id,
            as_of: data.company.today(),
            rows,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Post {
    pub id: Uuid,
    pub preview: BatchPreview,
}
impl Post {
    pub fn matches(&self, c: &super::dated::Context) -> bool {
        self.preview.ready()
            && self
                .preview
                .rows
                .iter()
                .all(|r| c.openings.iter().any(|o| r.record.matches(o)))
    }
    pub fn payload(&self, data: &Snapshot) -> Result<serde_json::Value> {
        ensure!(
            !self.id.is_nil() && data.access.role.manages_accounts(),
            "Only an Administrator posts an opening import."
        );
        let c = data
            .leave_setup
            .dated
            .as_ref()
            .context("Refresh leave setup.")?;
        c.validate(&data.access, data.company.today())?;
        let records = self.preview.records();
        ensure!(
            self.preview.ready()
                && records
                    .iter()
                    .all(|r| r.expected_revision == self.preview.expected_revision()),
            "Resolve all opening import errors first."
        );
        if !self.matches(c) {
            ensure!(
                c.revision == self.preview.expected_revision(),
                "Leave setup changed. Preview the entire CSV again."
            );
            self.preview.validate(data, &records)?;
        }
        ensure!(
            self.preview.as_of.year() == data.company.today().year() || self.matches(c),
            "Refresh the opening takeover date."
        );
        Ok(
            serde_json::json!({"p_batch_id":self.id,"p_records":records,"p_fingerprints":self.preview.rows.iter().map(|r|&r.preview.as_ref().unwrap().fingerprint).collect::<Vec<_>>(),"p_expected_revision":self.preview.expected_revision()}),
        )
    }
}
