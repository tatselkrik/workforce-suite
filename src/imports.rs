use crate::{
    contacts::EmployeeContacts,
    domain::{Action, DemoStore, Employee, EmployeeDraft, EmployeeStatus, Snapshot},
    employment::{EmploymentKind, EmploymentOption},
};
use anyhow::{Context, Result, ensure};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::Read,
    path::{Path, PathBuf},
};
use uuid::Uuid;

pub const MAX_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_ROWS: usize = 500;
pub const HEADERS: [&str; 16] = [
    "employee_code",
    "full_name",
    "department",
    "job_title",
    "joined_on",
    "annual_leave_days",
    "employment_type",
    "work_location",
    "reports_to_code",
    "phone",
    "personal_email",
    "address",
    "emergency_name",
    "emergency_relationship",
    "emergency_phone",
    "emergency_alternate_phone",
];

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Batch {
    pub id: Uuid,
    pub company_id: Uuid,
    pub actor_id: Uuid,
    pub actor_name: String,
    pub actor_role: crate::access::Role,
    pub row_count: u32,
    pub payload_sha256: String,
    pub reason: String,
    pub created_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Item {
    pub id: Uuid,
    pub company_id: Uuid,
    pub batch_id: Uuid,
    pub employee_id: Uuid,
    pub source_row: usize,
}
#[derive(Clone, Debug, Default)]
pub struct Setup {
    pub batches: Vec<Batch>,
    pub items: Vec<Item>,
}
#[derive(Clone, Debug)]
pub struct Row {
    pub source_row: usize,
    pub employee_code: String,
    pub full_name: String,
    pub employee: Option<Employee>,
    pub contacts: EmployeeContacts,
    pub references: Value,
    pub errors: Vec<String>,
}
#[derive(Clone, Debug)]
pub struct Preview {
    pub id: Uuid,
    pub company_id: Uuid,
    pub filename: String,
    pub rows: Vec<Row>,
}
#[derive(Debug)]
pub(crate) struct CsvRow {
    pub(crate) line: usize,
    pub(crate) cells: Vec<String>,
}

// Strict comma CSV with escaped quotes and quoted multiline cells. Track
// physical source lines, so an address newline does not shift error locations.
pub(crate) fn parse(text: &str) -> Result<Vec<CsvRow>> {
    parse_bounded(text, MAX_ROWS, HEADERS.len())
}
pub(crate) fn parse_bounded(
    text: &str,
    max_rows: usize,
    max_columns: usize,
) -> Result<Vec<CsvRow>> {
    ensure!(
        max_rows <= 1000 && max_columns <= 100,
        "Unsupported CSV bounds."
    );
    ensure!(
        text.len() <= MAX_BYTES,
        "CSV exceeds 2 MiB. Split it into smaller files."
    );
    ensure!(
        !text.contains('\0'),
        "Use UTF-8 CSV; null characters and UTF-16 files are unsupported."
    );
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut rows = Vec::new();
    let mut cells = Vec::new();
    let mut field = String::new();
    let mut state = 0u8;
    let mut line = 1usize;
    let mut row_line = 1usize;
    let mut chars = text.chars().peekable();
    let mut pending = false;
    while let Some(c) = chars.next() {
        if c == '\r' || c == '\n' {
            if c == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            line += 1;
            if state == 2 {
                field.push('\n');
                continue;
            }
            cells.push(std::mem::take(&mut field));
            if pending || cells.len() != 1 || !cells[0].is_empty() {
                rows.push(CsvRow {
                    line: row_line,
                    cells: std::mem::take(&mut cells),
                });
            } else {
                cells.clear();
            }
            ensure!(
                rows.len() <= max_rows + 1,
                "Import at most {max_rows} rows per file."
            );
            state = 0;
            row_line = line;
            pending = false;
            continue;
        }
        pending = true;
        match (state, c) {
            (2, '"') => {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    state = 3;
                }
            }
            (2, _) => field.push(c),
            (0, '"') => state = 2,
            (0 | 1 | 3, ',') => {
                cells.push(std::mem::take(&mut field));
                ensure!(
                    cells.len() < max_columns,
                    "Line {line}: too many CSV columns."
                );
                state = 0;
            }
            (1, '"') => anyhow::bail!("Line {line}: quote inside an unquoted field."),
            (3, _) => anyhow::bail!(
                "Line {line}: only a comma or line ending may follow a closing quote."
            ),
            (_, _) => {
                state = 1;
                field.push(c);
            }
        }
    }
    ensure!(
        state != 2,
        "Line {row_line}: quoted field has no closing quote."
    );
    if pending || !cells.is_empty() {
        cells.push(field);
        rows.push(CsvRow {
            line: row_line,
            cells,
        });
    }
    ensure!(
        rows.len() <= max_rows + 1,
        "Import at most {max_rows} rows per file."
    );
    ensure!(
        rows.len() >= 2,
        "Include a header and at least one employee row."
    );
    Ok(rows)
}
fn option(
    data: &Snapshot,
    kind: EmploymentKind,
    name: &str,
    errors: &mut Vec<String>,
) -> Option<EmploymentOption> {
    if name.is_empty() {
        return None;
    }
    let choices: Vec<_> = kind
        .choices(data)
        .iter()
        .filter(|o| {
            o.company_id == data.company.id && o.active && o.name.eq_ignore_ascii_case(name)
        })
        .collect();
    if choices.len() != 1 {
        errors.push(format!(
            "{} must match one existing active option: {name}",
            kind.label()
        ));
        return None;
    }
    Some(choices[0].clone())
}
fn option_reference(option: Option<&EmploymentOption>) -> Value {
    option
        .map(|o| json!({"id":o.id,"name":o.name,"revision":o.revision,"active":o.active}))
        .unwrap_or(Value::Null)
}
impl Preview {
    pub fn from_csv(data: &Snapshot, filename: String, bytes: &[u8]) -> Result<Self> {
        ensure!(
            data.access
                .allows(crate::staff_roles::Capability::Personnel)
                && data.access.company_id == data.company.id,
            "Only HR or Administrators can preview employee imports."
        );
        ensure!(
            bytes.len() <= MAX_BYTES,
            "CSV exceeds 2 MiB. Split it into smaller files."
        );
        let text = std::str::from_utf8(bytes)
            .context("Save the file as UTF-8 comma-separated CSV, then try again.")?;
        Self::from_rows(data, filename, parse(text)?)
    }
    fn from_rows(data: &Snapshot, filename: String, mut parsed: Vec<CsvRow>) -> Result<Self> {
        ensure!(
            data.access
                .allows(crate::staff_roles::Capability::Personnel)
                && data.access.company_id == data.company.id,
            "Only HR or Administrators can preview employee imports."
        );
        ensure!(
            !parsed.is_empty() && parsed.len() <= MAX_ROWS + 1,
            "Include headers and at most 500 employee rows."
        );
        ensure!(
            parsed
                .iter()
                .all(|r| r.cells.iter().all(|c| c.len() <= 4096)),
            "A CSV field exceeds 4 KiB. Use the employee template and field limits."
        );
        let header = parsed.remove(0);
        let mut columns = BTreeMap::new();
        for (i, name) in header.cells.iter().enumerate() {
            let name = name.trim().to_lowercase();
            ensure!(
                HEADERS.contains(&name.as_str()),
                "Unknown CSV column: {name}. Use the employee import template."
            );
            ensure!(
                columns.insert(name, i).is_none(),
                "Duplicate CSV header. Use each column only once."
            );
        }
        ensure!(
            columns.contains_key("employee_code") && columns.contains_key("full_name"),
            "Include employee_code and full_name headers."
        );
        let mut rows = Vec::new();
        for raw in parsed {
            let mut errors = Vec::new();
            if raw.cells.len() != header.cells.len() {
                errors.push(format!(
                    "Expected {} cells; found {}.",
                    header.cells.len(),
                    raw.cells.len()
                ));
            }
            let get = |name: &str| {
                columns
                    .get(name)
                    .and_then(|i| raw.cells.get(*i))
                    .map(|s| s.trim())
                    .unwrap_or("")
            };
            let code = get("employee_code").to_uppercase();
            let full_name = get("full_name").to_owned();
            for name in ["employee_code", "full_name", "department", "job_title"] {
                if get(name).chars().any(char::is_control) {
                    errors.push(format!("{name} cannot contain control characters."));
                }
            }
            if data.employees.iter().any(|e| {
                e.company_id == data.company.id
                    && e.employee_code.trim().eq_ignore_ascii_case(&code)
            }) {
                errors.push("Employee code already exists. Edit the employee record instead; imports create new people only.".into());
            }
            let allowance = if get("annual_leave_days").is_empty() {
                data.company.annual_leave_days
            } else {
                match get("annual_leave_days").parse::<u32>() {
                    Ok(v) => v,
                    Err(_) => {
                        errors
                            .push("annual_leave_days must be a whole number from 0 to 366.".into());
                        0
                    }
                }
            };
            let employment_type = option(
                data,
                EmploymentKind::EmploymentType,
                get("employment_type"),
                &mut errors,
            );
            let location = option(
                data,
                EmploymentKind::WorkLocation,
                get("work_location"),
                &mut errors,
            );
            let manager = if get("reports_to_code").is_empty() {
                None
            } else {
                let matches: Vec<_> = data
                    .employees
                    .iter()
                    .filter(|e| {
                        e.company_id == data.company.id
                            && e.status == EmployeeStatus::Active
                            && e.employee_code.eq_ignore_ascii_case(get("reports_to_code"))
                    })
                    .collect();
                if matches.len() != 1 {
                    errors.push("reports_to_code must match one existing active employee. Import new managers first, then import their reports.".into());
                    None
                } else {
                    Some(matches[0])
                }
            };
            let draft = EmployeeDraft {
                id: None,
                employee_code: code.clone(),
                full_name: full_name.clone(),
                department: get("department").into(),
                job_title: get("job_title").into(),
                joined_on: get("joined_on").into(),
                reports_to: manager.map(|m| m.id),
                employment_type_id: employment_type.as_ref().map(|o| o.id),
                work_location_id: location.as_ref().map(|o| o.id),
                annual_leave_days: allowance,
                status: EmployeeStatus::Active,
            };
            let employee = match draft.employee(data.company.id) {
                Ok(e) => Some(e),
                Err(e) => {
                    errors.push(e.to_string());
                    None
                }
            };
            let mut contacts = EmployeeContacts::empty(data.company.id, Uuid::nil());
            contacts.phone = get("phone").into();
            contacts.personal_email = get("personal_email").into();
            contacts.address = get("address").into();
            contacts.emergency_name = get("emergency_name").into();
            contacts.emergency_relationship = get("emergency_relationship").into();
            contacts.emergency_phone = get("emergency_phone").into();
            contacts.emergency_alternate_phone = get("emergency_alternate_phone").into();
            match contacts.normalized() {
                Ok(c) => contacts = c,
                Err(e) => errors.push(e.to_string()),
            }
            rows.push(Row{source_row:raw.line,employee_code:code,full_name,employee,contacts,references:json!({"manager":manager.map(|m|json!({"id":m.id,"employee_code":m.employee_code,"full_name":m.full_name,"status":m.status})),"employment_type":option_reference(employment_type.as_ref()),"work_location":option_reference(location.as_ref())}),errors});
        }
        let mut seen = BTreeMap::<String, Vec<usize>>::new();
        for (i, row) in rows.iter().enumerate() {
            seen.entry(row.employee_code.clone()).or_default().push(i);
        }
        for indices in seen.values().filter(|v| v.len() > 1) {
            for i in indices {
                rows[*i]
                    .errors
                    .push("Employee code is repeated in this file.".into());
            }
        }
        let mut preview = Self {
            id: Uuid::new_v4(),
            company_id: data.company.id,
            filename,
            rows,
        };
        if preview.valid() {
            if let Err(error) = preview.rows_payload() {
                preview.rows[0].errors.push(error.to_string());
            }
        }
        Ok(preview)
    }
    pub fn load(data: &Snapshot, path: &Path) -> Result<Self> {
        if path
            .extension()
            .is_some_and(|s| s.eq_ignore_ascii_case("xlsx"))
        {
            let rows = crate::tabular::load_xlsx(path, MAX_ROWS, HEADERS.len())?;
            return Self::from_rows(
                data,
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                rows.into_iter()
                    .map(|r| CsvRow {
                        line: r.number,
                        cells: r.cells,
                    })
                    .collect(),
            );
        }
        ensure!(
            path.extension()
                .is_some_and(|s| s.eq_ignore_ascii_case("csv")),
            "Choose a .csv file saved as UTF-8."
        );
        let file = std::fs::File::open(path).context("Could not read the CSV file.")?;
        let mut bytes = Vec::new();
        file.take((MAX_BYTES + 1) as u64).read_to_end(&mut bytes)?;
        Self::from_csv(
            data,
            path.file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Employee import.csv".into()),
            &bytes,
        )
    }
    pub fn valid(&self) -> bool {
        !self.rows.is_empty()
            && self.rows.len() <= MAX_ROWS
            && self
                .rows
                .iter()
                .all(|r| r.errors.is_empty() && r.employee.is_some())
    }
    pub fn valid_count(&self) -> usize {
        self.rows
            .iter()
            .filter(|r| r.errors.is_empty() && r.employee.is_some())
            .count()
    }
    pub fn rows_payload(&self) -> Result<Value> {
        ensure!(
            self.valid(),
            "Fix every preview error and reopen the CSV before importing. No rows have been imported."
        );
        let value=Value::Array(self.rows.iter().map(|r|{
            let e=r.employee.as_ref().unwrap();let c=&r.contacts;
            json!({"source_row":r.source_row,"employee":{"employee_code":e.employee_code,"full_name":e.full_name,"department":e.department,"job_title":e.job_title,"joined_on":e.joined_on,"reports_to":e.reports_to,"employment_type_id":e.employment_type_id,"work_location_id":e.work_location_id,"annual_leave_days":e.annual_leave_days,"status":"active"},"contacts":{"phone":c.phone,"personal_email":c.personal_email,"address":c.address,"emergency_name":c.emergency_name,"emergency_relationship":c.emergency_relationship,"emergency_phone":c.emergency_phone,"emergency_alternate_phone":c.emergency_alternate_phone},"references":r.references})
        }).collect());
        ensure!(
            serde_json::to_vec(&value)?.len() + self.rows.len() * 128 <= MAX_BYTES,
            "The normalized import exceeds the server's 2 MiB request limit. Split the file into smaller batches."
        );
        Ok(value)
    }
    pub fn validate_current(&self, data: &Snapshot) -> Result<()> {
        ensure!(
            data.access
                .allows(crate::staff_roles::Capability::Personnel)
                && data.access.company_id == self.company_id
                && data.company.id == self.company_id,
            "Only HR or Administrators in this company can apply the import."
        );
        self.rows_payload()?;
        for row in &self.rows {
            let e = row.employee.as_ref().unwrap();
            ensure!(
                !data
                    .employees
                    .iter()
                    .any(|p| p.employee_code.eq_ignore_ascii_case(&e.employee_code)),
                "An employee code now exists. Reopen the CSV preview; existing people were preserved."
            );
            let manager=e.reports_to.and_then(|id|data.employees.iter().find(|p|p.company_id==data.company.id&&p.id==id&&p.status==EmployeeStatus::Active)).map(|m|json!({"id":m.id,"employee_code":m.employee_code,"full_name":m.full_name,"status":m.status})).unwrap_or(Value::Null);
            let employment_type = e.employment_type_id.and_then(|id| {
                data.employment_types
                    .iter()
                    .find(|o| o.company_id == data.company.id && o.id == id && o.active)
            });
            let location = e.work_location_id.and_then(|id| {
                data.work_locations
                    .iter()
                    .find(|o| o.company_id == data.company.id && o.id == id && o.active)
            });
            ensure!(
                row.references
                    == json!({"manager":manager,"employment_type":option_reference(employment_type),"work_location":option_reference(location)}),
                "A manager, employment type or work location changed. Reopen the preview."
            );
        }
        Ok(())
    }
    pub fn payload(&self, reason: &str) -> Result<Value> {
        validate_reason(reason)?;
        Ok(json!({"p_batch_id":self.id,"p_rows":self.rows_payload()?,"p_reason":reason.trim()}))
    }
    pub fn error_csv(&self) -> Result<Vec<u8>> {
        csv(
            &["source_row", "employee_code", "full_name", "errors"],
            self.rows.iter().filter(|r| !r.errors.is_empty()).map(|r| {
                vec![
                    r.source_row.to_string(),
                    r.employee_code.clone(),
                    r.full_name.clone(),
                    r.errors.join(" | "),
                ]
            }),
        )
    }
    pub fn error_xlsx(&self) -> Result<Vec<u8>> {
        crate::tabular::text_xlsx(
            &["source_row", "employee_code", "full_name", "errors"],
            &self
                .rows
                .iter()
                .filter(|r| !r.errors.is_empty())
                .map(|r| {
                    vec![
                        r.source_row.to_string(),
                        r.employee_code.clone(),
                        r.full_name.clone(),
                        r.errors.join(" | "),
                    ]
                })
                .collect::<Vec<_>>(),
        )
    }
}
fn validate_reason(reason: &str) -> Result<()> {
    ensure!(
        !reason.trim().is_empty()
            && reason.trim().chars().count() <= 500
            && !reason.chars().any(char::is_control),
        "Enter an import reason of 1–500 characters without control characters."
    );
    Ok(())
}
pub fn csv(headers: &[&str], rows: impl IntoIterator<Item = Vec<String>>) -> Result<Vec<u8>> {
    let mut text = String::from("\u{feff}");
    text.push_str(
        &headers
            .iter()
            .map(|v| crate::reports::csv_cell(v))
            .collect::<Result<Vec<_>>>()?
            .join(","),
    );
    text.push_str("\r\n");
    for row in rows {
        ensure!(row.len() == headers.len(), "CSV columns do not match.");
        text.push_str(
            &row.iter()
                .map(|v| crate::reports::csv_cell(v))
                .collect::<Result<Vec<_>>>()?
                .join(","),
        );
        text.push_str("\r\n");
    }
    Ok(text.into_bytes())
}
pub fn template() -> Result<Vec<u8>> {
    csv(
        &HEADERS,
        [vec![
            "NEW-001",
            "Synthetic Employee",
            "Operations",
            "Associate",
            "2026-01-01",
            "15",
            "",
            "",
            "",
            "000 000 0000",
            "synthetic.employee@example.invalid",
            "Fictional address",
            "Synthetic Contact",
            "Sibling",
            "000 000 0001",
            "",
        ]
        .into_iter()
        .map(String::from)
        .collect()],
    )
}
pub fn template_xlsx() -> Result<Vec<u8>> {
    crate::tabular::text_xlsx(&HEADERS, &[])
}
pub fn apply_demo(store: &mut DemoStore, preview: Preview, reason: String) -> Result<()> {
    ensure!(
        store
            .data
            .access
            .allows(crate::staff_roles::Capability::Personnel)
            && store.data.company.id == preview.company_id,
        "Only HR or Administrators can import employees."
    );
    let payload = preview.payload(&reason)?;
    let fingerprint = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(
            &json!({"rows":payload["p_rows"],"reason":reason.trim()})
        )?)
    );
    if let Some(old) = store
        .data
        .imports
        .batches
        .iter()
        .find(|b| b.id == preview.id)
    {
        ensure!(
            old.actor_id == store.data.access.user_id && old.payload_sha256 == fingerprint,
            "This import ID belongs to different details."
        );
        return Ok(());
    }
    preview.validate_current(&store.data)?;
    let before = store.data.clone();
    let result = (|| -> Result<()> {
        let mut items = Vec::new();
        for row in &preview.rows {
            let mut draft = EmployeeDraft::from(row.employee.as_ref().unwrap());
            draft.id = None;
            store.execute(Action::SaveEmployeeRecord(draft, row.contacts.clone()))?;
            let person = store
                .data
                .employees
                .iter()
                .find(|e| e.employee_code == row.employee_code)
                .context("Imported employee was not found.")?;
            items.push(Item {
                id: Uuid::new_v4(),
                company_id: preview.company_id,
                batch_id: preview.id,
                employee_id: person.id,
                source_row: row.source_row,
            });
        }
        store.data.imports.batches.push(Batch {
            id: preview.id,
            company_id: preview.company_id,
            actor_id: store.data.access.user_id,
            actor_name: store
                .data
                .own_employee()
                .map(|e| e.full_name.clone())
                .unwrap_or_else(|| "HR staff".into()),
            actor_role: store.data.access.role,
            row_count: preview.rows.len() as u32,
            payload_sha256: fingerprint,
            reason: reason.trim().into(),
            created_at: Utc::now(),
        });
        store.data.imports.items.extend(items);
        Ok(())
    })();
    if result.is_err() {
        store.data = before;
    }
    result
}

#[cfg(windows)]
pub fn choose_source() -> Result<Option<PathBuf>> {
    choose_tabular_source("Preview employee CSV or Excel import")
}
#[cfg(windows)]
pub fn choose_tabular_source(caption: &str) -> Result<Option<PathBuf>> {
    use std::{ffi::OsString, os::windows::ffi::OsStringExt};
    use windows_sys::Win32::UI::{Controls::Dialogs::*, Input::KeyboardAndMouse::GetActiveWindow};
    let mut path = vec![0u16; 32768];
    let filter: Vec<u16> = "CSV or Excel (*.csv;*.xlsx)\0*.csv;*.xlsx\0UTF-8 CSV (*.csv)\0*.csv\0Excel workbook (*.xlsx)\0*.xlsx\0\0"
        .encode_utf16()
        .collect();
    ensure!(!caption.contains('\0'), "Invalid file chooser title.");
    let title: Vec<u16> = format!("{caption}\0").encode_utf16().collect();
    // SAFETY: all buffers outlive this synchronous native dialog.
    let mut d: OPENFILENAMEW = unsafe { std::mem::zeroed() };
    d.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    d.hwndOwner = unsafe { GetActiveWindow() };
    d.lpstrFilter = filter.as_ptr();
    d.lpstrTitle = title.as_ptr();
    d.lpstrFile = path.as_mut_ptr();
    d.nMaxFile = path.len() as u32;
    d.Flags = OFN_EXPLORER
        | OFN_FILEMUSTEXIST
        | OFN_PATHMUSTEXIST
        | OFN_NOCHANGEDIR
        | OFN_DONTADDTORECENT;
    if unsafe { GetOpenFileNameW(&mut d) } != 0 {
        let len = path.iter().position(|c| *c == 0).unwrap_or(path.len());
        Ok(Some(PathBuf::from(OsString::from_wide(&path[..len]))))
    } else {
        ensure!(
            unsafe { CommDlgExtendedError() } == 0,
            "Could not open the CSV chooser."
        );
        Ok(None)
    }
}
#[cfg(not(windows))]
pub fn choose_source() -> Result<Option<PathBuf>> {
    anyhow::bail!("CSV selection currently supports Windows only.")
}
#[cfg(not(windows))]
pub fn choose_tabular_source(_caption: &str) -> Result<Option<PathBuf>> {
    anyhow::bail!("CSV/Excel selection currently supports Windows only.")
}
