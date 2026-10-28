use crate::{
    access::{Access, Role},
    corrections::{CorrectionChange, CorrectionDraft},
    domain::{Employee, EmployeeStatus, Snapshot},
    imports,
    requests::{self, Status},
};
use anyhow::{Context, Result, ensure};
use chrono::{DateTime, NaiveDate, Timelike, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeSet, HashMap},
    io::Read,
    path::Path,
};
use uuid::Uuid;

pub const HEADERS: [&str; 6] = [
    "employee_code",
    "work_date",
    "checked_in_at",
    "checked_out_at",
    "reason",
    "reference",
];
pub const MAX_ROWS: usize = 100;
fn staff(data: &Snapshot) -> Result<()> {
    ensure!(
        data.access
            .allows(crate::staff_roles::Capability::AttendanceAdministration)
            && data.access.company_id == data.company.id,
        "Current HR or Administrator access is required for attendance imports."
    );
    Ok(())
}
fn note(value: &str, max: usize, required: bool) -> Result<()> {
    ensure!(
        (!required || !value.trim().is_empty())
            && value.chars().count() <= max
            && !value.chars().any(char::is_control),
        "Enter {}text up to {max} characters without control characters.",
        if required { "a reason with " } else { "" }
    );
    Ok(())
}
fn digest(value: &impl Serialize) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(value)?)))
}
fn fingerprint(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
pub fn timestamp(text: &str) -> Result<DateTime<Utc>> {
    ensure!(
        text.is_ascii()
            && text.len() >= 20
            && text.len() <= 32
            && text.as_bytes()[10] == b'T'
            && text.as_bytes()[13] == b':'
            && text.as_bytes()[16] == b':',
        "Use YYYY-MM-DDTHH:MM:SS with an explicit Z or offset, for example 2026-10-25T08:00:00+08:00."
    );
    let suffix = &text[19..];
    let zone = if let Some(rest) = suffix.strip_prefix('.') {
        let n = rest.bytes().take_while(u8::is_ascii_digit).count();
        ensure!(
            (1..=6).contains(&n),
            "Use at most six fractional timestamp digits."
        );
        &rest[n..]
    } else {
        suffix
    };
    ensure!(
        zone == "Z"
            || (zone.len() == 6 && zone.starts_with(['+', '-']) && zone.as_bytes()[3] == b':'),
        "Include an explicit Z or HH:MM offset."
    );
    ensure!(
        zone != "-00:00",
        "An unknown -00:00 offset is unsupported; use Z or a known offset."
    );
    let parsed = DateTime::parse_from_rfc3339(text)
        .context("Enter a valid ISO timestamp with seconds and an offset.")?;
    ensure!(
        parsed.nanosecond() < 1_000_000_000
            && parsed.nanosecond() % 1000 == 0
            && parsed.offset().local_minus_utc().abs() <= 14 * 3600,
        "Use a supported timestamp without leap seconds, submicrosecond precision or offsets beyond 14 hours."
    );
    Ok(parsed.with_timezone(&Utc))
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub id: Uuid,
    pub source_row: usize,
    pub employee_code: String,
    pub work_date: NaiveDate,
    pub checked_in_at: DateTime<Utc>,
    pub checked_out_at: Option<DateTime<Utc>>,
    pub reason: String,
    pub reference: String,
}
impl Record {
    fn employee<'a>(&self, data: &'a Snapshot) -> Result<&'a Employee> {
        let found = data
            .employees
            .iter()
            .filter(|e| {
                e.company_id == data.company.id
                    && e.employee_code.eq_ignore_ascii_case(&self.employee_code)
                    && e.status == EmployeeStatus::Active
            })
            .collect::<Vec<_>>();
        ensure!(
            found.len() == 1,
            "Employee code must match one active employee in this company."
        );
        Ok(found[0])
    }
    fn proposal(&self, data: &Snapshot, now: DateTime<Utc>) -> Result<CorrectionChange> {
        let e = self.employee(data)?;
        ensure!(
            !self.id.is_nil() && (2..=501).contains(&self.source_row),
            "Use a valid row identity/source row."
        );
        note(&self.reason, 500, true)?;
        note(&self.reference, 200, false)?;
        ensure!(
            !data.requests.records.iter().any(|r| r.employee_id == e.id
                && r.work_date == self.work_date
                && r.status == Status::Pending),
            "A pending correction request already exists for this employee and date."
        );
        let originals = data
            .attendance
            .iter()
            .filter(|a| a.employee_id == e.id && a.work_date == self.work_date)
            .collect::<Vec<_>>();
        ensure!(
            originals.len() <= 1,
            "Attendance is ambiguous for this date. Refresh and review the source."
        );
        let mut d = CorrectionDraft::new(
            e.id,
            self.work_date,
            originals.first().map(|a| (*a).clone()),
            data,
        );
        d.reason = self.reason.clone();
        let mut c = d.proposal_from_instants(data, now, self.checked_in_at, self.checked_out_at)?;
        if c.expected_revision.is_none() {
            c.record.id = self.id;
        } // Stable demo preview identity, not a written attendance row.
        Ok(c)
    }
}
#[derive(Clone, Debug)]
pub struct Row {
    pub number: usize,
    pub cells: Vec<String>,
    pub errors: Vec<String>,
    pub record: Option<Record>,
}
#[derive(Clone, Debug)]
pub struct Import {
    pub id: Uuid,
    pub scope: Access,
    pub timezone: String,
    pub filename: String,
    pub reason: String,
    pub reference: String,
    pub rows: Vec<Row>,
}
impl Import {
    pub fn load(data: &Snapshot, path: &Path) -> Result<Self> {
        staff(data)?;
        let rows = if crate::tabular::is_xlsx(path) {
            crate::tabular::load_xlsx(path, MAX_ROWS, HEADERS.len())?
                .into_iter()
                .map(|r| imports::CsvRow {
                    line: r.number,
                    cells: r.cells,
                })
                .collect()
        } else {
            ensure!(
                path.extension()
                    .is_some_and(|s| s.eq_ignore_ascii_case("csv")),
                "Choose CSV or Excel (.xlsx)."
            );
            let mut bytes = Vec::new();
            std::fs::File::open(path)?
                .take((imports::MAX_BYTES + 1) as u64)
                .read_to_end(&mut bytes)?;
            ensure!(
                bytes.len() <= imports::MAX_BYTES,
                "Attendance source exceeds 2 MiB."
            );
            imports::parse(std::str::from_utf8(&bytes).context("Save CSV as UTF-8.")?)?
        };
        Self::from_rows(
            data,
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            rows,
        )
    }
    pub fn from_csv(data: &Snapshot, filename: &str, text: &str) -> Result<Self> {
        Self::from_rows(data, filename.into(), imports::parse(text)?)
    }
    fn from_rows(data: &Snapshot, filename: String, rows: Vec<imports::CsvRow>) -> Result<Self> {
        staff(data)?;
        let mut source = rows.into_iter();
        let header = source
            .next()
            .context("Include the attendance template header.")?;
        ensure!(
            header
                .cells
                .iter()
                .map(|s| s.trim().trim_start_matches('\u{feff}'))
                .eq(HEADERS),
            "Use the six attendance-template columns in their original order."
        );
        let mut rows = Vec::new();
        for raw in source {
            ensure!(
                rows.len() < MAX_ROWS,
                "Import at most 100 attendance rows per file."
            );
            let mut errors = Vec::new();
            let get = |i: usize| raw.cells.get(i).map_or("", |s| s.trim());
            if raw.cells.len() != HEADERS.len() {
                errors.push("Use all six template cells in this row.".into());
            }
            let record = (|| -> Result<Record> {
                ensure!(
                    raw.cells.iter().all(|s| s.len() <= 4096),
                    "A field exceeds 4 KiB."
                );
                let date = get(1)
                    .parse::<NaiveDate>()
                    .context("work_date must be YYYY-MM-DD.")?;
                ensure!(
                    date.to_string() == get(1),
                    "Use an exact YYYY-MM-DD work date."
                );
                let r = Record {
                    id: Uuid::new_v4(),
                    source_row: raw.line,
                    employee_code: get(0).to_uppercase(),
                    work_date: date,
                    checked_in_at: timestamp(get(2))?,
                    checked_out_at: if get(3).is_empty() {
                        None
                    } else {
                        Some(timestamp(get(3))?)
                    },
                    reason: get(4).into(),
                    reference: get(5).into(),
                };
                r.proposal(data, Utc::now())?;
                Ok(r)
            })();
            let record = match record {
                Ok(r) => Some(r),
                Err(e) => {
                    errors.push(e.to_string());
                    None
                }
            };
            rows.push(Row {
                number: raw.line,
                cells: raw.cells,
                errors,
                record,
            });
        }
        ensure!(
            !rows.is_empty(),
            "Add at least one attendance row below the template header."
        );
        for i in 0..rows.len() {
            for j in i + 1..rows.len() {
                if let (Some(a), Some(b)) = (&rows[i].record, &rows[j].record) {
                    let same = a.employee_code.eq_ignore_ascii_case(&b.employee_code);
                    let message = if same && a.work_date == b.work_date {
                        Some("Duplicate employee and work date in this file.")
                    } else if same
                        && a.checked_out_at.is_none_or(|end| b.checked_in_at < end)
                        && b.checked_out_at.is_none_or(|end| a.checked_in_at < end)
                    {
                        Some("Proposed intervals overlap another row in this file.")
                    } else {
                        None
                    };
                    if let Some(message) = message {
                        rows[i].errors.push(message.into());
                        rows[j].errors.push(message.into());
                    }
                }
            }
        }
        Ok(Self {
            id: Uuid::new_v4(),
            scope: data.access.clone(),
            timezone: data.company.timezone.clone(),
            filename,
            reason: String::new(),
            reference: String::new(),
            rows,
        })
    }
    pub fn valid_rows(&self) -> bool {
        !self.rows.is_empty()
            && self.rows.len() <= MAX_ROWS
            && self
                .rows
                .iter()
                .all(|r| r.errors.is_empty() && r.record.is_some())
    }
    pub fn batch(&self, data: &Snapshot) -> Result<Batch> {
        staff(data)?;
        ensure!(
            self.scope == data.access,
            "Import access changed. Reopen the source file."
        );
        ensure!(
            self.valid_rows(),
            "Fix every source row before requesting a protected preview."
        );
        let b = Batch {
            id: self.id,
            expected_timezone: self.timezone.clone(),
            reason: self.reason.trim().into(),
            reference: self.reference.trim().into(),
            rows: self
                .rows
                .iter()
                .map(|r| r.record.clone().unwrap())
                .collect(),
        };
        b.validate(data)?;
        Ok(b)
    }
    pub fn error_bytes(&self, path: &Path, server: Option<&Preview>) -> Result<Vec<u8>> {
        let headers = [
            "source_row",
            "employee_code",
            "work_date",
            "checked_in_at",
            "checked_out_at",
            "reason",
            "reference",
            "errors",
        ];
        let rows = self
            .rows
            .iter()
            .map(|r| {
                let mut v = vec![r.number.to_string()];
                v.extend((0..6).map(|i| r.cells.get(i).cloned().unwrap_or_default()));
                let mut errors = r.errors.clone();
                if let Some(p) =
                    server.and_then(|p| p.rows.iter().find(|p| p.source_row == r.number))
                {
                    errors.extend(p.errors.clone());
                }
                v.push(errors.join("; "));
                v
            })
            .collect::<Vec<_>>();
        if crate::tabular::is_xlsx(path) {
            crate::tabular::text_xlsx(&headers, &rows)
        } else {
            imports::csv(&headers, rows)
        }
    }
}
pub fn template(excel: bool) -> Result<Vec<u8>> {
    if excel {
        crate::tabular::text_xlsx(&HEADERS, &[])
    } else {
        imports::csv(&HEADERS, std::iter::empty())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Batch {
    pub id: Uuid,
    pub expected_timezone: String,
    pub reason: String,
    pub reference: String,
    pub rows: Vec<Record>,
}
impl Batch {
    pub fn validate(&self, data: &Snapshot) -> Result<()> {
        staff(data)?;
        ensure!(
            !self.id.is_nil() && !self.rows.is_empty() && self.rows.len() <= MAX_ROWS,
            "Use an attendance batch of 1–100 rows."
        );
        note(&self.reason, 500, true)?;
        note(&self.reference, 200, false)?;
        ensure!(
            self.expected_timezone == data.company.timezone,
            "Company time zone changed. Reopen and review the file."
        );
        let mut ids = BTreeSet::new();
        let mut lines = BTreeSet::new();
        let mut days = BTreeSet::new();
        for r in &self.rows {
            ensure!(
                ids.insert(r.id) && lines.insert(r.source_row),
                "Use distinct row identities/source rows."
            );
            let e = r.employee(data)?;
            ensure!(
                days.insert((e.id, r.work_date)),
                "Duplicate employee and work date in this file."
            );
            r.proposal(data, Utc::now())?;
        }
        for (i, a) in self.rows.iter().enumerate() {
            for b in self.rows.iter().skip(i + 1) {
                ensure!(
                    !a.employee_code.eq_ignore_ascii_case(&b.employee_code)
                        || !a.checked_out_at.is_none_or(|end| b.checked_in_at < end)
                        || !b.checked_out_at.is_none_or(|end| a.checked_in_at < end),
                    "Proposed intervals overlap another row in this file."
                );
            }
        }
        ensure!(
            serde_json::to_vec(self)?.len() <= 262144,
            "The reviewed import exceeds the request size limit."
        );
        Ok(())
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PreviewRow {
    pub id: Uuid,
    pub source_row: usize,
    pub employee_code: String,
    pub employee_id: Option<Uuid>,
    pub employee_name: Option<String>,
    pub work_date: Option<NaiveDate>,
    pub proposed_in: Option<DateTime<Utc>>,
    pub proposed_out: Option<DateTime<Utc>>,
    pub base_revision: Option<i64>,
    pub errors: Vec<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Preview {
    pub version: String,
    pub company_id: Uuid,
    pub user_id: Uuid,
    pub role: Role,
    pub as_of: NaiveDate,
    pub timezone: String,
    pub batch_id: Uuid,
    pub fingerprint: String,
    pub ready: bool,
    pub rows: Vec<PreviewRow>,
}
impl Preview {
    pub fn validate(&self, data: &Snapshot, batch: &Batch) -> Result<()> {
        staff(data)?;
        ensure!(
            self.version == "attendance-import-v1"
                && self.company_id == data.company.id
                && self.user_id == data.access.user_id
                && self.role == data.access.role
                && self.as_of == data.company.today()
                && self.timezone == data.company.timezone
                && self.timezone == batch.expected_timezone
                && self.batch_id == batch.id
                && fingerprint(&self.fingerprint)
                && self.rows.len() == batch.rows.len(),
            "Protected attendance preview has an invalid or stale scope."
        );
        ensure!(
            self.ready == self.rows.iter().all(|r| r.errors.is_empty()),
            "Preview readiness does not match its row errors."
        );
        for (r, expected) in self.rows.iter().zip(&batch.rows) {
            ensure!(
                r.id == expected.id
                    && r.source_row == expected.source_row
                    && r.employee_code
                        .eq_ignore_ascii_case(&expected.employee_code)
                    && r.errors.len() <= 8
                    && r.errors.iter().all(|e| !e.is_empty() && e.len() <= 2000),
                "Protected preview does not match the selected source rows."
            );
            if r.errors.is_empty() {
                let e = expected.employee(data)?;
                let base = data
                    .attendance
                    .iter()
                    .find(|a| a.employee_id == e.id && a.work_date == expected.work_date);
                ensure!(
                    r.employee_id == Some(e.id)
                        && r.employee_name.as_deref() == Some(&e.full_name)
                        && r.work_date == Some(expected.work_date)
                        && r.proposed_in == Some(expected.checked_in_at)
                        && r.proposed_out == expected.checked_out_at
                        && r.base_revision == base.map(|a| a.revision),
                    "Protected preview disagrees with the current employee or clock source. Refresh."
                );
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug)]
pub struct Post {
    pub scope: Access,
    pub batch: Batch,
    pub preview: Preview,
}
impl Post {
    pub fn identity(&self, access: &Access) -> Result<()> {
        ensure!(
            access.allows(crate::staff_roles::Capability::AttendanceAdministration)
                && self.scope == *access
                && self.preview.company_id == access.company_id
                && self.preview.user_id == access.user_id
                && self.preview.role == access.role
                && self.preview.version == "attendance-import-v1"
                && self.preview.batch_id == self.batch.id
                && fingerprint(&self.preview.fingerprint)
                && self.preview.ready
                && self.preview.rows.len() == self.batch.rows.len()
                && !self.batch.rows.is_empty(),
            "Import access or protected submission identity changed."
        );
        for (r, b) in self.preview.rows.iter().zip(&self.batch.rows) {
            ensure!(
                r.id == b.id
                    && r.source_row == b.source_row
                    && r.employee_code.eq_ignore_ascii_case(&b.employee_code)
                    && r.employee_id.is_some_and(|id| !id.is_nil())
                    && r.errors.is_empty()
                    && r.work_date == Some(b.work_date)
                    && r.proposed_in == Some(b.checked_in_at)
                    && r.proposed_out == b.checked_out_at,
                "The exact submission no longer matches its protected preview."
            );
        }
        Ok(())
    }
    pub fn validate(&self, data: &Snapshot) -> Result<()> {
        self.identity(&data.access)?;
        self.preview.validate(data, &self.batch)?;
        Ok(())
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ReceiptItem {
    pub id: Uuid,
    pub source_row: usize,
    pub employee_id: Uuid,
    pub request_id: Uuid,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Receipt {
    pub version: String,
    pub company_id: Uuid,
    pub user_id: Uuid,
    pub role: Role,
    pub batch_id: Uuid,
    pub payload_sha256: String,
    pub row_count: usize,
    pub items: Vec<ReceiptItem>,
}
impl Receipt {
    pub fn validate(&self, data: &Snapshot, post: &Post) -> Result<()> {
        post.identity(&data.access)?;
        ensure!(
            self.version == "attendance-import-receipt-v1"
                && self.company_id == data.company.id
                && self.user_id == data.access.user_id
                && self.role == data.access.role
                && self.batch_id == post.batch.id
                && fingerprint(&self.payload_sha256)
                && self.row_count == post.batch.rows.len()
                && self.items.len() == self.row_count,
            "Import receipt has an invalid scope or row count."
        );
        let mut ids = BTreeSet::new();
        let mut rows = BTreeSet::new();
        for item in &self.items {
            let expected = post
                .preview
                .rows
                .iter()
                .find(|r| r.id == item.id)
                .context("Receipt contains an unknown source row.")?;
            ensure!(
                item.source_row == expected.source_row
                    && Some(item.employee_id) == expected.employee_id
                    && !item.request_id.is_nil()
                    && rows.insert(item.id)
                    && ids.insert(item.request_id),
                "Import receipt does not match the protected subjects."
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BatchHistory {
    pub id: Uuid,
    pub company_id: Uuid,
    pub actor_id: Uuid,
    pub actor_name: String,
    pub actor_role: Role,
    pub timezone: String,
    pub row_count: usize,
    pub reason: String,
    pub reference: String,
    pub source_fingerprint: String,
    pub payload_sha256: String,
    pub created_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Item {
    pub id: Uuid,
    pub company_id: Uuid,
    pub batch_id: Uuid,
    pub employee_id: Uuid,
    pub request_id: Uuid,
    pub source_row: usize,
    pub employee_code: String,
    pub employee_name: String,
    pub reference: String,
    pub work_date: NaiveDate,
    pub proposed_in: DateTime<Utc>,
    pub proposed_out: Option<DateTime<Utc>>,
    pub status: Status,
    pub request_revision: i64,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Withdrawal {
    pub id: Uuid,
    pub company_id: Uuid,
    pub item_id: Uuid,
    pub actor_id: Uuid,
    pub actor_name: String,
    pub actor_role: Role,
    pub reason: String,
    pub created_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct History {
    pub version: String,
    pub company_id: Uuid,
    pub user_id: Uuid,
    pub role: Role,
    pub as_of: NaiveDate,
    pub timezone: String,
    pub batches: Vec<BatchHistory>,
    pub items: Vec<Item>,
    pub withdrawals: Vec<Withdrawal>,
}
impl History {
    pub fn validate(&self, data: &Snapshot) -> Result<()> {
        staff(data)?;
        ensure!(
            self.version == "attendance-import-context-v1"
                && self.company_id == data.company.id
                && self.user_id == data.access.user_id
                && self.role == data.access.role
                && self.as_of == data.company.today()
                && self.timezone == data.company.timezone
                && self.batches.len() <= 500
                && self.items.len() <= 5000
                && self.withdrawals.len() <= 5000,
            "Attendance import history has an invalid/stale scope or exceeds limits."
        );
        let mut ids = BTreeSet::new();
        for b in &self.batches {
            ensure!(
                !b.id.is_nil()
                    && ids.insert(b.id)
                    && b.company_id == self.company_id
                    && (1..=100).contains(&b.row_count)
                    && fingerprint(&b.source_fingerprint)
                    && fingerprint(&b.payload_sha256)
                    && self.items.iter().filter(|i| i.batch_id == b.id).count() == b.row_count
                    && b.timezone.parse::<chrono_tz::Tz>().is_ok(),
                "Import history contains an invalid batch."
            );
            note(&b.reason, 500, true)?;
            note(&b.reference, 200, false)?;
        }
        let mut items = BTreeSet::new();
        let mut requests = BTreeSet::new();
        let mut source_rows = BTreeSet::new();
        for i in &self.items {
            ensure!(
                i.company_id == self.company_id
                    && items.insert(i.id)
                    && !i.id.is_nil()
                    && requests.insert(i.request_id)
                    && !i.request_id.is_nil()
                    && ids.contains(&i.batch_id)
                    && (2..=501).contains(&i.source_row)
                    && source_rows.insert((i.batch_id, i.source_row))
                    && i.request_revision >= 0
                    && i.proposed_out.is_none_or(|end| end >= i.proposed_in)
                    && data
                        .employees
                        .iter()
                        .any(|e| e.id == i.employee_id && e.company_id == self.company_id),
                "Import history contains an invalid or foreign row."
            );
            note(&i.reference, 200, false)?;
        }
        let mut withdrawals = BTreeSet::new();
        let mut withdrawn_items = BTreeSet::new();
        for w in &self.withdrawals {
            ensure!(
                w.company_id == self.company_id
                    && !w.id.is_nil()
                    && withdrawals.insert(w.id)
                    && withdrawn_items.insert(w.item_id)
                    && items.contains(&w.item_id)
                    && self.items.iter().any(|i| i.id == w.item_id
                        && i.status == Status::Cancelled
                        && self
                            .batches
                            .iter()
                            .any(|b| b.id == i.batch_id && b.actor_id == w.actor_id)),
                "Import withdrawal history is invalid."
            );
            note(&w.reason, 500, true)?;
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Withdraw {
    pub id: Uuid,
    pub item_id: Uuid,
    pub expected_revision: i64,
    pub reason: String,
}
impl Withdraw {
    pub fn validate(&self, data: &Snapshot, history: &History) -> Result<()> {
        staff(data)?;
        history.validate(data)?;
        ensure!(!self.id.is_nil(), "Choose a withdrawal identity.");
        note(&self.reason, 500, true)?;
        let item = history
            .items
            .iter()
            .find(|i| i.id == self.item_id)
            .context("Refresh the imported request history.")?;
        let b = history
            .batches
            .iter()
            .find(|b| b.id == item.batch_id)
            .unwrap();
        ensure!(
            b.actor_id == data.access.user_id
                && item.status == Status::Pending
                && item.request_revision == self.expected_revision,
            "Only the original HR actor can withdraw an unchanged pending imported request."
        );
        Ok(())
    }
}

#[derive(Default)]
pub struct Demo {
    batches: Vec<BatchHistory>,
    items: Vec<Item>,
    withdrawals: Vec<Withdrawal>,
    payloads: HashMap<Uuid, Value>,
    withdraw_payloads: HashMap<Uuid, Withdraw>,
}
impl Demo {
    pub fn history(&self, data: &Snapshot) -> Result<History> {
        staff(data)?;
        let mut items = self.items.clone();
        for item in &mut items {
            let r = data
                .requests
                .records
                .iter()
                .find(|r| r.id == item.request_id)
                .context("Imported request history is incomplete.")?;
            item.status = r.status;
            item.request_revision = r.revision;
        }
        let h = History {
            version: "attendance-import-context-v1".into(),
            company_id: data.company.id,
            user_id: data.access.user_id,
            role: data.access.role,
            as_of: data.company.today(),
            timezone: data.company.timezone.clone(),
            batches: self.batches.clone(),
            items,
            withdrawals: self.withdrawals.clone(),
        };
        h.validate(data)?;
        Ok(h)
    }
    pub fn preview(data: &Snapshot, batch: &Batch) -> Result<Preview> {
        batch.validate(data)?;
        let mut facts = Vec::new();
        let mut rows = Vec::new();
        for r in &batch.rows {
            let e = r.employee(data)?;
            let c = r.proposal(data, Utc::now())?;
            facts.push(json!({"employee":e,"proposal":c.record,"attendance":data.attendance.iter().filter(|a|a.employee_id==e.id).collect::<Vec<_>>(),"requests":data.requests.records.iter().filter(|a|a.employee_id==e.id).collect::<Vec<_>>(),"breaks":data.breaks.records,"rules":data.calculations.policy}));
            rows.push(PreviewRow {
                id: r.id,
                source_row: r.source_row,
                employee_code: e.employee_code.clone(),
                employee_id: Some(e.id),
                employee_name: Some(e.full_name.clone()),
                work_date: Some(r.work_date),
                proposed_in: Some(r.checked_in_at),
                proposed_out: r.checked_out_at,
                base_revision: c.expected_revision,
                errors: Vec::new(),
            });
        }
        Ok(Preview {
            version: "attendance-import-v1".into(),
            company_id: data.company.id,
            user_id: data.access.user_id,
            role: data.access.role,
            as_of: data.company.today(),
            timezone: data.company.timezone.clone(),
            batch_id: batch.id,
            fingerprint: digest(
                &json!({"batch":batch,"scope":[data.access.company_id,data.access.user_id],"role":data.access.role,"as_of":data.company.today(),"facts":facts}),
            )?,
            ready: true,
            rows,
        })
    }
    pub fn post(&mut self, data: &mut Snapshot, post: &Post) -> Result<Receipt> {
        staff(data)?;
        let payload = serde_json::to_value(&post.batch)?;
        if let Some(old) = self.payloads.get(&post.batch.id) {
            let b = self.batches.iter().find(|b| b.id == post.batch.id).unwrap();
            ensure!(
                b.company_id == data.company.id
                    && b.actor_id == data.access.user_id
                    && old == &payload,
                "Import identity was already used."
            );
            return self.receipt(data, post.batch.id);
        }
        post.validate(data)?;
        let fresh = Self::preview(data, &post.batch)?;
        ensure!(
            fresh.fingerprint == post.preview.fingerprint,
            "Import sources changed. Preview every row again."
        );
        ensure!(
            self.batches.len() < 500 && self.items.len() + post.batch.rows.len() <= 5000,
            "Import history limit reached."
        );
        let mut next = data.clone();
        let mut items = Vec::new();
        let now = Utc::now();
        let actor = actor(data);
        for row in &post.batch.rows {
            let e = row.employee(data)?;
            let c = row.proposal(data, now)?;
            let rid = Uuid::new_v4();
            let base = data
                .attendance
                .iter()
                .find(|a| a.employee_id == e.id && a.work_date == row.work_date)
                .cloned();
            next.requests.records.push(requests::Request {
                id: rid,
                company_id: data.company.id,
                employee_id: e.id,
                employee_name: e.full_name.clone(),
                employee_code: e.employee_code.clone(),
                submitted_by: data.access.user_id,
                work_date: row.work_date,
                timezone: data.company.timezone.clone(),
                base_record: base,
                proposed_in: row.checked_in_at,
                proposed_out: row.checked_out_at,
                planned_shift: c.record.planned_shift,
                reason: row.reason.clone(),
                status: Status::Pending,
                revision: 0,
                applied_attendance_id: None,
                correction_id: None,
                created_at: now,
            });
            next.requests
                .events
                .push(event(data, rid, e.id, Status::Pending, &row.reason, now));
            items.push(Item {
                id: row.id,
                company_id: data.company.id,
                batch_id: post.batch.id,
                employee_id: e.id,
                request_id: rid,
                source_row: row.source_row,
                employee_code: e.employee_code.clone(),
                employee_name: e.full_name.clone(),
                reference: row.reference.clone(),
                work_date: row.work_date,
                proposed_in: row.checked_in_at,
                proposed_out: row.checked_out_at,
                status: Status::Pending,
                request_revision: 0,
            });
        }
        self.batches.push(BatchHistory {
            id: post.batch.id,
            company_id: data.company.id,
            actor_id: data.access.user_id,
            actor_name: actor,
            actor_role: data.access.role,
            timezone: data.company.timezone.clone(),
            row_count: items.len(),
            reason: post.batch.reason.clone(),
            reference: post.batch.reference.clone(),
            source_fingerprint: fresh.fingerprint,
            payload_sha256: digest(&post.batch)?,
            created_at: now,
        });
        self.items.extend(items);
        self.payloads.insert(post.batch.id, payload);
        *data = next;
        self.receipt(data, post.batch.id)
    }
    fn receipt(&self, data: &Snapshot, bid: Uuid) -> Result<Receipt> {
        let b = self
            .batches
            .iter()
            .find(|b| b.id == bid)
            .context("Import receipt unavailable.")?;
        Ok(Receipt {
            version: "attendance-import-receipt-v1".into(),
            company_id: data.company.id,
            user_id: data.access.user_id,
            role: data.access.role,
            batch_id: bid,
            payload_sha256: b.payload_sha256.clone(),
            row_count: b.row_count,
            items: self
                .items
                .iter()
                .filter(|i| i.batch_id == bid)
                .map(|i| ReceiptItem {
                    id: i.id,
                    source_row: i.source_row,
                    employee_id: i.employee_id,
                    request_id: i.request_id,
                })
                .collect(),
        })
    }
    pub fn withdraw(&mut self, data: &mut Snapshot, change: &Withdraw) -> Result<()> {
        staff(data)?;
        if let Some(old) = self.withdraw_payloads.get(&change.id) {
            let w = self.withdrawals.iter().find(|w| w.id == change.id).unwrap();
            ensure!(
                old == change
                    && w.company_id == data.company.id
                    && w.actor_id == data.access.user_id,
                "Withdrawal identity was already used."
            );
            return Ok(());
        }
        change.validate(data, &self.history(data)?)?;
        let item = self.items.iter().find(|i| i.id == change.item_id).unwrap();
        let r = data
            .requests
            .records
            .iter_mut()
            .find(|r| r.id == item.request_id)
            .context("Request unavailable.")?;
        r.status = Status::Cancelled;
        r.revision += 1;
        let rid = r.id;
        let eid = r.employee_id;
        let now = Utc::now();
        data.requests.events.push(event(
            data,
            rid,
            eid,
            Status::Cancelled,
            &change.reason,
            now,
        ));
        self.withdrawals.push(Withdrawal {
            id: change.id,
            company_id: data.company.id,
            item_id: item.id,
            actor_id: data.access.user_id,
            actor_name: actor(data),
            actor_role: data.access.role,
            reason: change.reason.clone(),
            created_at: now,
        });
        self.withdraw_payloads.insert(change.id, change.clone());
        Ok(())
    }
}
fn actor(data: &Snapshot) -> String {
    data.employees
        .iter()
        .find(|e| Some(e.id) == data.access.employee_id)
        .map(|e| e.full_name.clone())
        .unwrap_or_else(|| "Staff".into())
}
fn event(
    data: &Snapshot,
    rid: Uuid,
    eid: Uuid,
    status: Status,
    reason: &str,
    now: DateTime<Utc>,
) -> requests::Event {
    requests::Event {
        id: Uuid::new_v4(),
        company_id: data.company.id,
        request_id: rid,
        employee_id: eid,
        actor_id: data.access.user_id,
        actor_name: actor(data),
        actor_role: data.access.role,
        status,
        reason: reason.into(),
        before_record: None,
        after_record: None,
        created_at: now,
    }
}
