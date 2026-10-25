//! Bounded, HR-assisted pending leave request exchange. Normal leave rules apply.
use super::{Accrual, ApprovalStep, Balance, DayCharge, DayPart};
use crate::{
    access::{Access, Role},
    domain::*,
    imports, tabular,
};
use anyhow::{Context, Result, ensure};
use chrono::{Datelike, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeSet, HashMap},
    io::Read,
    path::Path,
};
use uuid::Uuid;
pub const HEADERS: [&str; 7] = [
    "employee_code",
    "leave_type_code",
    "starts_on",
    "ends_on",
    "day_part",
    "reason",
    "reference",
];
pub const MAX_ROWS: usize = 100;
fn staff(data: &Snapshot) -> Result<()> {
    ensure!(
        data.access.role.manages_hr() && data.company.id == data.access.company_id,
        "Current HR or Administrator access is required for leave imports."
    );
    Ok(())
}
fn note(s: &str, max: usize, required: bool) -> Result<()> {
    ensure!(
        (!required || !s.trim().is_empty())
            && s.chars().count() <= max
            && !s.chars().any(char::is_control),
        "Use {}text up to {max} characters without control characters.",
        if required { "a private note with " } else { "" }
    );
    Ok(())
}
fn digest(v: &impl Serialize) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(v)?)))
}
fn fingerprint(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn iso(s: &str) -> Result<NaiveDate> {
    let d = s.parse::<NaiveDate>().context("Use YYYY-MM-DD dates.")?;
    ensure!(d.to_string() == s, "Use exact YYYY-MM-DD dates.");
    Ok(d)
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub id: Uuid,
    pub source_row: usize,
    pub employee_code: String,
    pub type_code: String,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    pub day_part: DayPart,
    pub reason: String,
    pub reference: String,
}
impl Record {
    fn shape(&self) -> Result<()> {
        ensure!(
            !self.id.is_nil() && (2..=501).contains(&self.source_row),
            "Choose valid row identities/source rows."
        );
        ensure!(
            (1900..=2200).contains(&self.starts_on.year())
                && self.starts_on <= self.ends_on
                && self.starts_on.year() == self.ends_on.year()
                && (self.day_part == DayPart::Full || self.starts_on == self.ends_on),
            "Use same-year dates and one date for a half-day."
        );
        note(&self.employee_code, 50, true)?;
        note(&self.type_code, 40, true)?;
        note(&self.reason, 500, false)?;
        note(&self.reference, 200, false)
    }
    fn employee<'a>(&self, data: &'a Snapshot) -> Result<&'a Employee> {
        let e = data
            .employees
            .iter()
            .filter(|e| {
                e.company_id == data.company.id
                    && e.status == EmployeeStatus::Active
                    && e.employee_code.eq_ignore_ascii_case(&self.employee_code)
            })
            .collect::<Vec<_>>();
        ensure!(
            e.len() == 1,
            "Employee code must match one active employee in this company."
        );
        Ok(e[0])
    }
    fn prepare(&self, data: &Snapshot) -> Result<LeaveRequest> {
        self.shape()?;
        let e = self.employee(data)?;
        let draft = LeaveDraft {
            employee_id: Some(e.id),
            kind: LeaveKind::Custom,
            starts_on: self.starts_on.to_string(),
            ends_on: self.ends_on.to_string(),
            reason: self.reason.clone(),
            options: super::LeaveOptions {
                type_code: self.type_code.clone(),
                day_part: self.day_part,
            },
        };
        let mut r = super::prepare(data, &draft)?;
        r.id = self.id;
        Ok(r)
    }
    fn balance(&self, data: &Snapshot) -> Result<Option<Balance>> {
        let e = self.employee(data)?;
        let t = super::dated::policy_on(data, self.starts_on)
            .leave_type(&self.type_code)
            .context("Choose an active leave type.")?;
        if t.accrual == Accrual::Unlimited {
            Ok(None)
        } else {
            Ok(Some(super::balance_checked(
                data,
                e,
                t,
                self.starts_on.year(),
                self.starts_on,
                None,
            )?))
        }
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
        let rows = if tabular::is_xlsx(path) {
            tabular::load_xlsx(path, MAX_ROWS, HEADERS.len())?
                .into_iter()
                .map(|r| imports::CsvRow {
                    line: r.number,
                    cells: r.cells,
                })
                .collect()
        } else {
            ensure!(
                path.extension()
                    .is_some_and(|x| x.eq_ignore_ascii_case("csv")),
                "Choose CSV or Excel (.xlsx)."
            );
            let mut bytes = Vec::new();
            std::fs::File::open(path)?
                .take((imports::MAX_BYTES + 1) as u64)
                .read_to_end(&mut bytes)?;
            ensure!(
                bytes.len() <= imports::MAX_BYTES,
                "Leave source exceeds 2 MiB."
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
            .context("Include the leave template header.")?;
        ensure!(
            header
                .cells
                .iter()
                .map(|s| s.trim().trim_start_matches('\u{feff}'))
                .eq(HEADERS),
            "Use the seven leave template columns in their original order."
        );
        let mut next = data.clone();
        let mut result = Vec::new();
        let mut span = 0;
        for raw in source {
            ensure!(
                result.len() < MAX_ROWS,
                "Import at most 100 leave rows per file."
            );
            let mut errors = Vec::new();
            let record = (|| -> Result<Record> {
                ensure!(
                    raw.cells.len() == 7 && raw.cells.iter().all(|c| c.len() <= 4096),
                    "Use seven bounded template cells in each row."
                );
                let c = |i: usize| raw.cells[i].trim();
                let part = match c(4) {
                    "full" => DayPart::Full,
                    "am" => DayPart::Am,
                    "pm" => DayPart::Pm,
                    _ => anyhow::bail!("day_part must be full, am or pm."),
                };
                let r = Record {
                    id: Uuid::new_v4(),
                    source_row: raw.line,
                    employee_code: c(0).to_uppercase(),
                    type_code: c(1).into(),
                    starts_on: iso(c(2))?,
                    ends_on: iso(c(3))?,
                    day_part: part,
                    reason: c(5).into(),
                    reference: c(6).into(),
                };
                r.shape()?;
                span += (r.ends_on - r.starts_on).num_days() + 1;
                ensure!(
                    span <= 3660,
                    "Split sources exceeding 3,660 combined calendar dates."
                );
                let proposal = r.prepare(&next)?;
                next.leave.push(proposal);
                Ok(r)
            })();
            let record = match record {
                Ok(r) => Some(r),
                Err(e) => {
                    errors.push(e.to_string());
                    None
                }
            };
            result.push(Row {
                number: raw.line,
                cells: raw.cells,
                errors,
                record,
            });
        }
        ensure!(!result.is_empty(), "Add at least one leave request row.");
        Ok(Self {
            id: Uuid::new_v4(),
            scope: data.access.clone(),
            timezone: data.company.timezone.clone(),
            filename,
            reason: String::new(),
            reference: String::new(),
            rows: result,
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
            "Leave import access changed. Reopen the file."
        );
        ensure!(
            self.valid_rows(),
            "Fix every source row before protected preview."
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
            "leave_type_code",
            "starts_on",
            "ends_on",
            "day_part",
            "reason",
            "reference",
            "errors",
        ];
        let rows = self
            .rows
            .iter()
            .map(|r| {
                let mut v = vec![r.number.to_string()];
                v.extend((0..7).map(|i| r.cells.get(i).cloned().unwrap_or_default()));
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
        if tabular::is_xlsx(path) {
            tabular::text_xlsx(&headers, &rows)
        } else {
            imports::csv(&headers, rows)
        }
    }
}
pub fn template(excel: bool) -> Result<Vec<u8>> {
    if excel {
        tabular::text_xlsx(&HEADERS, &[])
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
    fn shape(&self) -> Result<()> {
        ensure!(
            !self.id.is_nil() && (1..=MAX_ROWS).contains(&self.rows.len()),
            "Use 1–100 leave import rows."
        );
        note(&self.reason, 500, true)?;
        note(&self.reference, 200, false)?;
        let mut ids = BTreeSet::new();
        let mut lines = BTreeSet::new();
        let mut span = 0;
        for r in &self.rows {
            r.shape()?;
            ensure!(
                ids.insert(r.id) && lines.insert(r.source_row),
                "Use distinct row identities and source rows."
            );
            span += (r.ends_on - r.starts_on).num_days() + 1;
        }
        ensure!(
            span <= 3660,
            "Split sources exceeding 3,660 combined calendar dates."
        );
        ensure!(
            serde_json::to_vec(self)?.len() <= 262144,
            "The reviewed leave import exceeds its size limit."
        );
        Ok(())
    }
    pub fn validate(&self, data: &Snapshot) -> Result<()> {
        staff(data)?;
        self.shape()?;
        ensure!(
            self.expected_timezone == data.company.timezone,
            "Company time zone changed. Reopen the file."
        );
        let mut next = data.clone();
        for r in &self.rows {
            let request = r.prepare(&next)?;
            next.leave.push(request);
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreviewRow {
    pub id: Uuid,
    pub source_row: usize,
    pub employee_code: String,
    pub employee_id: Option<Uuid>,
    pub employee_name: Option<String>,
    pub type_code: String,
    pub starts_on: Option<NaiveDate>,
    pub ends_on: Option<NaiveDate>,
    pub day_part: DayPart,
    pub type_name: Option<String>,
    pub charged_units: Option<i32>,
    pub charged_dates: Option<Vec<DayCharge>>,
    pub policy_revision: Option<i64>,
    pub approval_steps: Option<Vec<ApprovalStep>>,
    pub balance_before: Option<Balance>,
    pub balance_after: Option<Balance>,
    pub errors: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
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
        batch.shape()?;
        ensure!(
            self.version == "leave-import-v1"
                && self.company_id == data.company.id
                && self.user_id == data.access.user_id
                && self.role == data.access.role
                && self.as_of == data.company.today()
                && self.timezone == data.company.timezone
                && self.timezone == batch.expected_timezone
                && self.batch_id == batch.id
                && fingerprint(&self.fingerprint)
                && self.rows.len() == batch.rows.len(),
            "Leave preview has an invalid or stale scope."
        );
        ensure!(
            self.ready == self.rows.iter().all(|r| r.errors.is_empty()),
            "Leave preview readiness disagrees with its errors."
        );
        let mut next = data.clone();
        for (r, b) in self.rows.iter().zip(&batch.rows) {
            ensure!(
                r.id == b.id
                    && r.source_row == b.source_row
                    && r.employee_code.eq_ignore_ascii_case(&b.employee_code)
                    && r.type_code == b.type_code
                    && r.day_part == b.day_part
                    && r.errors.len() <= 8
                    && r.errors.iter().all(|s| !s.is_empty() && s.len() <= 2000),
                "Leave preview does not match the selected source rows."
            );
            if r.errors.is_empty() {
                let before = b.balance(&next)?;
                let request = b.prepare(&next)?;
                let e = b.employee(&next)?;
                ensure!(
                    r.employee_id == Some(e.id)
                        && r.employee_name.as_deref() == Some(e.full_name.as_str())
                        && r.starts_on == Some(b.starts_on)
                        && r.ends_on == Some(b.ends_on)
                        && r.type_name.as_deref() == Some(request.type_name())
                        && r.charged_units == Some(request.units())
                        && r.charged_dates.as_deref()
                            == Some(request.policy.charged_dates.as_slice())
                        && r.policy_revision == Some(request.policy.policy_revision)
                        && r.approval_steps.as_deref()
                            == Some(request.policy.approval_steps.as_slice())
                        && r.balance_before == before,
                    "Protected leave preview disagrees with current charges, policy, approval or funding. Refresh."
                );
                next.leave.push(request);
                ensure!(
                    r.balance_after == b.balance(&next)?,
                    "Protected leave preview disagrees with running reservations. Refresh."
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
        self.batch.shape()?;
        ensure!(
            access.role.manages_hr()
                && self.scope == *access
                && self.preview.company_id == access.company_id
                && self.preview.user_id == access.user_id
                && self.preview.role == access.role
                && self.preview.version == "leave-import-v1"
                && self.preview.batch_id == self.batch.id
                && fingerprint(&self.preview.fingerprint)
                && self.preview.ready
                && self.preview.rows.len() == self.batch.rows.len(),
            "Leave import access or submission identity changed."
        );
        for (r, b) in self.preview.rows.iter().zip(&self.batch.rows) {
            ensure!(
                r.id == b.id
                    && r.source_row == b.source_row
                    && r.employee_code.eq_ignore_ascii_case(&b.employee_code)
                    && r.employee_id.is_some_and(|id| !id.is_nil())
                    && r.type_code == b.type_code
                    && r.starts_on == Some(b.starts_on)
                    && r.ends_on == Some(b.ends_on)
                    && r.day_part == b.day_part
                    && r.errors.is_empty(),
                "Exact leave submission no longer matches its preview."
            );
        }
        Ok(())
    }
    pub fn validate(&self, data: &Snapshot) -> Result<()> {
        self.identity(&data.access)?;
        self.preview.validate(data, &self.batch)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReceiptItem {
    pub id: Uuid,
    pub source_row: usize,
    pub employee_id: Uuid,
    pub request_id: Uuid,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
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
            self.version == "leave-import-receipt-v1"
                && self.company_id == data.company.id
                && self.user_id == data.access.user_id
                && self.role == data.access.role
                && self.batch_id == post.batch.id
                && fingerprint(&self.payload_sha256)
                && self.row_count == post.batch.rows.len()
                && self.items.len() == self.row_count,
            "Leave receipt scope/row count is invalid."
        );
        let mut ids = BTreeSet::new();
        let mut rows = BTreeSet::new();
        for i in &self.items {
            let expected = post
                .preview
                .rows
                .iter()
                .find(|r| r.id == i.id)
                .context("Receipt contains an unknown leave row.")?;
            ensure!(
                rows.insert(i.id)
                    && ids.insert(i.request_id)
                    && !i.request_id.is_nil()
                    && Some(i.employee_id) == expected.employee_id
                    && i.source_row == expected.source_row,
                "Receipt does not match the protected leave subjects."
            );
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
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
    pub created_at: chrono::DateTime<Utc>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
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
    pub type_code: String,
    pub type_name: String,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    pub day_part: DayPart,
    pub charged_units: i32,
    pub charged_dates: Vec<DayCharge>,
    pub policy_revision: i64,
    pub approval_steps: Vec<ApprovalStep>,
    pub status: LeaveStatus,
    pub request_revision: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct History {
    pub version: String,
    pub company_id: Uuid,
    pub user_id: Uuid,
    pub role: Role,
    pub as_of: NaiveDate,
    pub timezone: String,
    pub batches: Vec<BatchHistory>,
    pub items: Vec<Item>,
}
impl History {
    pub fn validate(&self, data: &Snapshot) -> Result<()> {
        staff(data)?;
        ensure!(
            self.version == "leave-import-context-v1"
                && self.company_id == data.company.id
                && self.user_id == data.access.user_id
                && self.role == data.access.role
                && self.as_of == data.company.today()
                && self.timezone == data.company.timezone
                && self.batches.len() <= 500
                && self.items.len() <= 5000,
            "Leave import history has a stale/invalid scope or exceeds limits."
        );
        let mut batches = BTreeSet::new();
        for b in &self.batches {
            ensure!(
                !b.id.is_nil()
                    && batches.insert(b.id)
                    && b.company_id == self.company_id
                    && b.actor_role.manages_hr()
                    && (1..=MAX_ROWS).contains(&b.row_count)
                    && fingerprint(&b.source_fingerprint)
                    && fingerprint(&b.payload_sha256)
                    && b.timezone.parse::<chrono_tz::Tz>().is_ok()
                    && self.items.iter().filter(|i| i.batch_id == b.id).count() == b.row_count,
                "Leave import history contains an invalid batch."
            );
            note(&b.reason, 500, true)?;
            note(&b.reference, 200, false)?;
        }
        let mut rows = BTreeSet::new();
        let mut requests = BTreeSet::new();
        let mut lines = BTreeSet::new();
        for i in &self.items {
            ensure!(
                i.company_id == self.company_id
                    && batches.contains(&i.batch_id)
                    && !i.id.is_nil()
                    && rows.insert(i.id)
                    && !i.request_id.is_nil()
                    && requests.insert(i.request_id)
                    && lines.insert((i.batch_id, i.source_row))
                    && (2..=501).contains(&i.source_row)
                    && i.starts_on <= i.ends_on
                    && i.starts_on.year() == i.ends_on.year()
                    && i.request_revision >= 0
                    && i.policy_revision >= 0
                    && (1..=732).contains(&i.charged_units)
                    && (1..=366).contains(&i.charged_dates.len())
                    && i.charged_dates.iter().all(|d| d.date >= i.starts_on
                        && d.date <= i.ends_on
                        && d.units == if i.day_part == DayPart::Full { 2 } else { 1 })
                    && i.charged_dates.windows(2).all(|p| p[0].date < p[1].date)
                    && i.charged_dates
                        .iter()
                        .try_fold(0i32, |n, d| n.checked_add(d.units))
                        == Some(i.charged_units)
                    && (1..=5).contains(&i.approval_steps.len())
                    && data
                        .employees
                        .iter()
                        .any(|e| e.id == i.employee_id && e.company_id == self.company_id),
                "Leave import history contains an invalid/foreign request."
            );
            note(&i.reference, 200, false)?;
        }
        Ok(())
    }
}
#[derive(Default)]
pub struct Demo {
    batches: Vec<BatchHistory>,
    items: Vec<Item>,
    payloads: HashMap<Uuid, Value>,
}
impl Demo {
    pub fn history(&self, data: &Snapshot) -> Result<History> {
        staff(data)?;
        let mut items = self.items.clone();
        for i in &mut items {
            let r = data
                .leave
                .iter()
                .find(|r| r.id == i.request_id)
                .context("Imported leave history is incomplete.")?;
            i.status = r.status;
            i.request_revision = r.policy.revision;
            i.approval_steps = r.policy.approval_steps.clone();
        }
        let h = History {
            version: "leave-import-context-v1".into(),
            company_id: data.company.id,
            user_id: data.access.user_id,
            role: data.access.role,
            as_of: data.company.today(),
            timezone: data.company.timezone.clone(),
            batches: self.batches.clone(),
            items,
        };
        h.validate(data)?;
        Ok(h)
    }
    pub fn preview(data: &Snapshot, batch: &Batch) -> Result<Preview> {
        staff(data)?;
        batch.shape()?;
        ensure!(
            batch.expected_timezone == data.company.timezone,
            "Company time zone changed."
        );
        let mut next = data.clone();
        let mut rows = Vec::new();
        for b in &batch.rows {
            let e = b.employee(&next).ok().cloned();
            let mut staged = next.clone();
            let result = (|| -> Result<(LeaveRequest, Option<Balance>, Option<Balance>)> {
                let before = b.balance(&next)?;
                let r = b.prepare(&next)?;
                staged.leave.push(r.clone());
                let after = b.balance(&staged)?;
                Ok((r, before, after))
            })();
            let (r, before, after, errors) = match result {
                Ok((r, b, a)) => {
                    next = staged;
                    (Some(r), b, a, Vec::new())
                }
                Err(e) => (None, None, None, vec![e.to_string()]),
            };
            rows.push(PreviewRow {
                id: b.id,
                source_row: b.source_row,
                employee_code: e
                    .as_ref()
                    .map_or(b.employee_code.clone(), |e| e.employee_code.clone()),
                employee_id: e.as_ref().map(|e| e.id),
                employee_name: e.map(|e| e.full_name),
                type_code: b.type_code.clone(),
                starts_on: Some(b.starts_on),
                ends_on: Some(b.ends_on),
                day_part: b.day_part,
                type_name: r.as_ref().map(|r| r.type_name().into()),
                charged_units: r.as_ref().map(|r| r.units()),
                charged_dates: r.as_ref().map(|r| r.policy.charged_dates.clone()),
                policy_revision: r.as_ref().map(|r| r.policy.policy_revision),
                approval_steps: r.as_ref().map(|r| r.policy.approval_steps.clone()),
                balance_before: before,
                balance_after: after,
                errors,
            });
        }
        Ok(Preview {
            version: "leave-import-v1".into(),
            company_id: data.company.id,
            user_id: data.access.user_id,
            role: data.access.role,
            as_of: data.company.today(),
            timezone: data.company.timezone.clone(),
            batch_id: batch.id,
            fingerprint: digest(
                &json!({"batch":batch,"company":data.company,"scope":[data.access.company_id,data.access.user_id],"role":data.access.role,"as_of":data.company.today(),"employees":data.employees,"leave":data.leave,"policy":data.leave_setup.policy,"dated":data.leave_setup.dated,"credits":data.leave_setup.credits}),
            )?,
            ready: rows.iter().all(|r| r.errors.is_empty()),
            rows,
        })
    }
    pub fn post(&mut self, data: &mut Snapshot, post: &Post) -> Result<Receipt> {
        staff(data)?;
        post.identity(&data.access)?;
        let payload = serde_json::to_value(&post.batch)?;
        if let Some(old) = self.payloads.get(&post.batch.id) {
            let b = self.batches.iter().find(|b| b.id == post.batch.id).unwrap();
            ensure!(
                b.company_id == data.company.id
                    && b.actor_id == data.access.user_id
                    && old == &payload,
                "Leave import identity was already used."
            );
            return self.receipt(data, post.batch.id);
        }
        post.validate(data)?;
        let fresh = Self::preview(data, &post.batch)?;
        ensure!(
            fresh.ready && fresh.fingerprint == post.preview.fingerprint,
            "Leave import sources changed. Preview all rows again."
        );
        ensure!(
            self.batches.len() < 500 && self.items.len() + post.batch.rows.len() <= 5000,
            "Leave import history limit reached."
        );
        let mut next = data.clone();
        let mut items = Vec::new();
        for row in &post.batch.rows {
            let mut r = row.prepare(&next)?;
            r.id = Uuid::new_v4();
            items.push(Item {
                id: row.id,
                company_id: data.company.id,
                batch_id: post.batch.id,
                employee_id: r.employee_id,
                request_id: r.id,
                source_row: row.source_row,
                employee_code: row.employee_code.clone(),
                employee_name: row.employee(data)?.full_name.clone(),
                reference: row.reference.clone(),
                type_code: r.type_code().into(),
                type_name: r.type_name().into(),
                starts_on: r.starts_on,
                ends_on: r.ends_on,
                day_part: r.policy.day_part,
                charged_units: r.units(),
                charged_dates: r.policy.charged_dates.clone(),
                policy_revision: r.policy.policy_revision,
                approval_steps: r.policy.approval_steps.clone(),
                status: r.status,
                request_revision: r.policy.revision,
            });
            let rid = r.id;
            next.leave.push(r);
            super::demo_event(&mut next, rid, "submitted", "");
        }
        let name = data
            .employees
            .iter()
            .find(|e| Some(e.id) == data.access.employee_id)
            .map_or("Staff", |e| e.full_name.as_str());
        self.batches.insert(
            0,
            BatchHistory {
                id: post.batch.id,
                company_id: data.company.id,
                actor_id: data.access.user_id,
                actor_name: name.into(),
                actor_role: data.access.role,
                timezone: post.batch.expected_timezone.clone(),
                row_count: post.batch.rows.len(),
                reason: post.batch.reason.clone(),
                reference: post.batch.reference.clone(),
                source_fingerprint: post.preview.fingerprint.clone(),
                payload_sha256: digest(&post.batch)?,
                created_at: Utc::now(),
            },
        );
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
            .context("Leave import receipt unavailable.")?;
        Ok(Receipt {
            version: "leave-import-receipt-v1".into(),
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
}
