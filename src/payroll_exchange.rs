//! Local exchange snapshots. No accounting network or payroll mutation.
use crate::{
    access::{Access, Role},
    domain::Snapshot,
    payroll_reporting as p,
};
use anyhow::{Context, Result, ensure};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    path::Path,
};
use uuid::Uuid;

pub const KEYS: &[&str] = &[
    "employee_name",
    "department",
    "period_start",
    "period_end",
    "issued_on",
    "received_on",
    "status",
    "basic_cents",
    "gross_cents",
    "employee_contributions_cents",
    "employer_contributions_cents",
    "withholding_cents",
    "net_cents",
    "contained_source_net_cents",
    "cash_cents",
    "offset_cents",
    "tax_adjustment_cents",
    "outstanding_cents",
    "reserved_cents",
    "rule_version",
    "linked_kind",
    "linked_id",
    "review_amount_cents",
];
pub const FIXED: &[&str] = &[
    "exchange_id",
    "record_key",
    "row_sha256",
    "employee_code",
    "source_kind",
    "payment_state",
    "amount_unit",
];
pub const ACK_HEADERS: &[&str] = &[
    "exchange_id",
    "record_key",
    "row_sha256",
    "employee_code",
    "employee_name",
    "source_kind",
    "service_period",
    "expected_revision",
    "status",
    "reference",
    "note",
];
fn label(v: &str, max: usize, required: bool) -> Result<()> {
    ensure!(
        v.chars().count() <= max
            && (!required || !v.trim().is_empty())
            && !v.chars().any(char::is_control),
        "Use bounded text without control characters."
    );
    Ok(())
}
fn hash(v: &str) -> bool {
    v.len() == 64
        && v.bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
pub fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn scope(cid: Uuid, uid: Uuid, role: Role, a: &Access) -> Result<()> {
    ensure!(
        cid == a.company_id
            && uid == a.user_id
            && role == a.role
            && a.allows(crate::staff_roles::Capability::PayrollPreparation),
        "Current HR/Admin exchange scope changed. Sign in or refresh."
    );
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Column {
    pub key: String,
    pub header: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub id: Uuid,
    pub revision: i64,
    pub name: String,
    pub enabled: bool,
    pub columns: Vec<Column>,
    pub created_at: DateTime<Utc>,
}
fn columns(cols: &[Column]) -> Result<()> {
    ensure!(
        !cols.is_empty() && cols.len() <= KEYS.len(),
        "Choose1–23 columns."
    );
    let mut keys = BTreeSet::new();
    let mut headers: BTreeSet<String> = FIXED.iter().map(|s| s.to_string()).collect();
    for c in cols {
        let mut chars = c.header.chars();
        ensure!(
            KEYS.contains(&c.key.as_str())
                && keys.insert(&c.key)
                && c.header.len() <= 64
                && chars.next().is_some_and(|c| c.is_ascii_alphabetic())
                && chars.all(|c| c.is_ascii_alphanumeric() || " _.-".contains(c))
                && headers.insert(c.header.to_ascii_lowercase()),
            "Use distinct whitelisted fields and safe unique headers."
        );
    }
    Ok(())
}
impl Profile {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.id.is_nil() && (0..=99).contains(&self.revision),
            "Invalid profile identity/revision."
        );
        label(&self.name, 80, true)?;
        columns(&self.columns)
    }
}
pub fn default_columns() -> Vec<Column> {
    [
        "employee_name",
        "department",
        "period_start",
        "period_end",
        "issued_on",
        "received_on",
        "gross_cents",
        "net_cents",
        "cash_cents",
        "offset_cents",
        "contained_source_net_cents",
        "tax_adjustment_cents",
        "review_amount_cents",
    ]
    .into_iter()
    .map(|k| Column {
        key: k.into(),
        header: k.into(),
    })
    .collect()
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProfileChange {
    pub request_id: Uuid,
    pub id: Uuid,
    pub expected_revision: Option<i64>,
    pub name: String,
    pub enabled: bool,
    pub columns: Vec<Column>,
    pub reason: String,
}
impl ProfileChange {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.request_id.is_nil()
                && !self.id.is_nil()
                && self.expected_revision.is_none_or(|r| (0..=98).contains(&r)),
            "Invalid profile/retry identity or revision."
        );
        label(&self.name, 80, true)?;
        label(&self.reason, 500, true)?;
        columns(&self.columns)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub id: Uuid,
    pub profile_id: Uuid,
    pub expected_revision: i64,
    pub filter: p::Filter,
    pub reason: String,
}
impl Request {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.id.is_nil()
                && !self.profile_id.is_nil()
                && (0..=99).contains(&self.expected_revision),
            "Invalid exchange/profile identity."
        );
        self.filter.validate()?;
        label(&self.reason, 500, true)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct History {
    pub id: Uuid,
    pub profile_name: String,
    pub profile_revision: i64,
    pub row_count: usize,
    pub sha256: String,
    pub created_at: DateTime<Utc>,
    pub accepted: usize,
    pub rejected: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub version: String,
    pub company_id: Uuid,
    pub user_id: Uuid,
    pub role: Role,
    pub profiles: Vec<Profile>,
    pub history: Vec<History>,
}
impl State {
    pub fn validate(&self, a: &Access) -> Result<()> {
        scope(self.company_id, self.user_id, self.role, a)?;
        ensure!(
            self.version == "payroll-exchange-state-v1"
                && self.profiles.len() <= 50
                && self.history.len() <= 100,
            "Invalid exchange state/limit."
        );
        let mut ids = BTreeSet::new();
        for p in &self.profiles {
            p.validate()?;
            ensure!(ids.insert(p.id), "Duplicate exchange profile.");
        }
        ids.clear();
        for h in &self.history {
            label(&h.profile_name, 80, true)?;
            ensure!(
                !h.id.is_nil()
                    && ids.insert(h.id)
                    && (0..=99).contains(&h.profile_revision)
                    && (1..=1000).contains(&h.row_count)
                    && h.accepted <= h.row_count
                    && h.rejected <= h.row_count
                    && h.accepted + h.rejected <= h.row_count
                    && hash(&h.sha256),
                "Invalid exchange history/control counts."
            );
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Control {
    pub record_key: String,
    pub row_sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Basis {
    pub company_id: Uuid,
    pub exchange_id: Uuid,
    pub profile: Profile,
    pub report: p::Wire,
    pub controls: Vec<Control>,
    pub csv: String,
    pub sha256: String,
    pub fingerprint: String,
}
fn csv_literal(v: &str) -> String {
    let clean = v.trim_start_matches(|c: char| c.is_whitespace() || c == '\u{feff}');
    let formula = clean.starts_with(['=', '+', '-', '@']) || v.starts_with(['\t', '\r', '\n']);
    format!("{}{}", if formula { "'" } else { "" }, v)
}
fn csv_quote(v: &str, literal: bool) -> String {
    let s = if literal { csv_literal(v) } else { v.into() };
    format!("\"{}\"", s.replace('"', "\"\""))
}
fn period(r: &p::Row) -> String {
    format!("{} through {}", r.period_start, r.period_end)
}
impl Basis {
    pub fn validate(&self, a: &Access) -> Result<p::Report> {
        ensure!(
            a.allows(crate::staff_roles::Capability::PayrollPreparation)
                && self.company_id == a.company_id
                && !self.exchange_id.is_nil()
                && self.controls.len() == self.report.rows.len()
                && (1..=1000).contains(&self.controls.len())
                && self.csv.len() <= 4194304
                && hash(&self.sha256)
                && hash(&self.fingerprint),
            "Invalid exchange snapshot or scope."
        );
        self.profile.validate()?;
        let report = p::Report::from_wire(self.report.clone(), a.company_id, &self.report.filter)?;
        ensure!(
            self.sha256 == sha(self.csv.as_bytes()) && self.csv == self.render()?,
            "Frozen exchange bytes do not match their checksum or selected fields."
        );
        Ok(report)
    }
    pub fn matches(&self, request: &Request) -> Result<()> {
        request.validate()?;
        ensure!(
            self.exchange_id == request.id
                && self.profile.id == request.profile_id
                && self.profile.revision == request.expected_revision
                && self.report.filter == request.filter
                && self.profile.enabled,
            "Exchange preview no longer matches this request."
        );
        Ok(())
    }
    pub fn render(&self) -> Result<String> {
        ensure!(
            self.controls.len() == self.report.rows.len(),
            "Exchange controls/rows differ."
        );
        let mut out = FIXED
            .iter()
            .map(|s| csv_quote(s, true))
            .chain(
                self.profile
                    .columns
                    .iter()
                    .map(|c| csv_quote(&c.header, true)),
            )
            .collect::<Vec<_>>()
            .join(",");
        out.push('\n');
        for (r, c) in self.report.rows.iter().zip(&self.controls) {
            ensure!(
                c.record_key == format!("{}:{}", r.kind, r.id) && hash(&c.row_sha256),
                "Exchange row identity/digest mismatch."
            );
            let value = serde_json::to_value(r)?;
            let mut cells = vec![
                csv_quote(&self.exchange_id.to_string(), true),
                csv_quote(&c.record_key, true),
                csv_quote(&c.row_sha256, true),
                csv_quote(&r.employee_code, true),
                csv_quote(&r.kind, true),
                csv_quote(&r.payment_state, true),
                csv_quote("PHP_CENTAVOS", true),
            ];
            for col in &self.profile.columns {
                let v = &value[&col.key];
                let text = if let Some(s) = v.as_str() {
                    s.to_string()
                } else if v.is_null() {
                    String::new()
                } else {
                    v.to_string()
                };
                cells.push(csv_quote(&text, !col.key.ends_with("_cents")));
            }
            out.push_str(&cells.join(","));
            out.push('\n');
        }
        Ok(out)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preview {
    pub version: String,
    pub company_id: Uuid,
    pub user_id: Uuid,
    pub role: Role,
    pub basis: Basis,
}
impl Preview {
    pub fn validate(&self, a: &Access, r: &Request) -> Result<()> {
        scope(self.company_id, self.user_id, self.role, a)?;
        ensure!(
            self.version == "payroll-exchange-preview-v1",
            "Unknown exchange preview."
        );
        self.basis.validate(a)?;
        self.basis.matches(r)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub record_key: String,
    pub revision: i64,
    pub batch_id: Uuid,
    pub status: String,
    pub reference: String,
    pub note: String,
    pub created_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Detail {
    pub version: String,
    pub company_id: Uuid,
    pub user_id: Uuid,
    pub role: Role,
    pub basis: Basis,
    pub created_at: DateTime<Utc>,
    pub events: Vec<Event>,
}
impl Detail {
    pub fn source(&self, key: &str) -> Option<&p::Row> {
        self.basis
            .controls
            .iter()
            .position(|c| c.record_key == key)
            .and_then(|n| self.basis.report.rows.get(n))
    }
    pub fn current(&self, key: &str) -> Option<&Event> {
        self.events
            .iter()
            .filter(|e| e.record_key == key)
            .max_by_key(|e| e.revision)
    }
    pub fn validate(&self, a: &Access, id: Uuid) -> Result<()> {
        scope(self.company_id, self.user_id, self.role, a)?;
        ensure!(
            self.version == "payroll-exchange-detail-v1"
                && self.basis.exchange_id == id
                && self.events.len() <= 10000,
            "Invalid exchange detail/history."
        );
        self.basis.validate(a)?;
        let keys: BTreeSet<_> = self
            .basis
            .controls
            .iter()
            .map(|c| c.record_key.as_str())
            .collect();
        let mut grouped: BTreeMap<&str, Vec<&Event>> = BTreeMap::new();
        for e in &self.events {
            ensure!(
                keys.contains(e.record_key.as_str())
                    && !e.batch_id.is_nil()
                    && (1..=100).contains(&e.revision)
                    && ["accepted", "rejected", "pending"].contains(&e.status.as_str()),
                "Invalid acknowledgement history."
            );
            label(&e.reference, 200, false)?;
            label(&e.note, 500, false)?;
            grouped.entry(&e.record_key).or_default().push(e);
        }
        for mut events in grouped.into_values() {
            events.sort_by_key(|e| e.revision);
            let mut prior = "pending";
            for (i, e) in events.iter().enumerate() {
                ensure!(
                    e.revision == i as i64 + 1
                        && ((prior == "pending" && e.status != "pending")
                            || (prior != "pending"
                                && e.status == "pending"
                                && !e.note.trim().is_empty())),
                    "Acknowledgement sequence changed or has a gap."
                );
                prior = &e.status;
            }
        }
        Ok(())
    }
    pub fn template(&self) -> Result<Vec<u8>> {
        crate::imports::csv(
            ACK_HEADERS,
            self.basis
                .controls
                .iter()
                .zip(&self.basis.report.rows)
                .map(|(c, r)| {
                    vec![
                        self.basis.exchange_id.to_string(),
                        c.record_key.clone(),
                        c.row_sha256.clone(),
                        r.employee_code.clone(),
                        r.employee_name.clone(),
                        r.kind.clone(),
                        period(r),
                        self.current(&c.record_key)
                            .map_or(0, |e| e.revision)
                            .to_string(),
                        String::new(),
                        String::new(),
                        String::new(),
                    ]
                }),
        )
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AckRow {
    pub record_key: String,
    pub row_sha256: String,
    pub expected_revision: i64,
    pub status: String,
    pub reference: String,
    pub note: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Ack {
    pub id: Uuid,
    pub exchange_id: Uuid,
    pub reason: String,
    pub rows: Vec<AckRow>,
}
impl Ack {
    pub fn validate(&self, detail: &Detail, a: &Access) -> Result<()> {
        detail.validate(a, self.exchange_id)?;
        ensure!(
            !self.id.is_nil() && (1..=1000).contains(&self.rows.len()),
            "Use1–1000 acknowledgement rows and a retry identity."
        );
        label(&self.reason, 500, true)?;
        let mut keys = BTreeSet::new();
        for r in &self.rows {
            let c = detail
                .basis
                .controls
                .iter()
                .find(|c| c.record_key == r.record_key)
                .context("Acknowledgement record is outside this frozen exchange.")?;
            let prior = detail.current(&r.record_key);
            ensure!(
                keys.insert(&r.record_key)
                    && r.row_sha256 == c.row_sha256
                    && r.expected_revision == prior.map_or(0, |e| e.revision)
                    && r.expected_revision < 100,
                "Acknowledgement digest/revision changed or row duplicated. Reload its template."
            );
            ensure!(
                ["accepted", "rejected", "pending"].contains(&r.status.as_str()),
                "Choose accepted, rejected or an Admin reset to pending."
            );
            label(&r.reference, 200, false)?;
            label(&r.note, 500, false)?;
            if r.status == "pending" {
                ensure!(
                    a.role == Role::Administrator
                        && prior.is_some_and(|e| e.status != "pending")
                        && !r.note.trim().is_empty(),
                    "Only an Administrator can reset an outcome with a row reason."
                );
            } else {
                ensure!(
                    prior.is_none_or(|e| e.status == "pending"),
                    "An Administrator must reset the previous terminal outcome first."
                );
            }
        }
        Ok(())
    }
    pub fn from_csv(bytes: &[u8], detail: &Detail, reason: String, a: &Access) -> Result<Self> {
        let text = std::str::from_utf8(bytes).context("Use a UTF-8 HRIS acknowledgement CSV.")?;
        let mut rows = crate::imports::parse_bounded(text, 1000, ACK_HEADERS.len())?.into_iter();
        let header = rows.next().context("Include the acknowledgement header.")?;
        ensure!(
            header
                .cells
                .iter()
                .map(String::as_str)
                .eq(ACK_HEADERS.iter().copied()),
            "Use the unchanged HRIS acknowledgement headers."
        );
        let mut batch = Self {
            id: Uuid::new_v4(),
            exchange_id: detail.basis.exchange_id,
            reason,
            rows: Vec::new(),
        };
        for r in rows {
            ensure!(
                r.cells.len() == ACK_HEADERS.len(),
                "Line{}: use the unchanged acknowledgement template fields.",
                r.line
            );
            if r.cells[8].is_empty() {
                ensure!(
                    r.cells[9].is_empty() && r.cells[10].is_empty(),
                    "Line{}: enter a status for a reference/note.",
                    r.line
                );
                continue;
            }
            ensure!(
                r.cells[0] == batch.exchange_id.to_string(),
                "Line{}: exchange identity differs.",
                r.line
            );
            let source = detail
                .source(&r.cells[1])
                .context("Acknowledgement source is outside this frozen export.")?;
            ensure!(
                r.cells[3..7]
                    == [
                        csv_literal(&source.employee_code),
                        csv_literal(&source.employee_name),
                        source.kind.clone(),
                        period(source)
                    ],
                "Line{}: employee, source or service-period context changed. Keep the template context unchanged.",
                r.line
            );
            ensure!(
                !r.cells[7].is_empty()
                    && r.cells[7].bytes().all(|c| c.is_ascii_digit())
                    && r.cells[7].len() <= 2,
                "Line{}: use the exact template revision.",
                r.line
            );
            batch.rows.push(AckRow {
                record_key: r.cells[1].clone(),
                row_sha256: r.cells[2].clone(),
                expected_revision: r.cells[7].parse()?,
                status: r.cells[8].clone(),
                reference: r.cells[9].clone(),
                note: r.cells[10].clone(),
            });
        }
        batch.validate(detail, a)?;
        Ok(batch)
    }
    pub fn load(path: &Path, detail: &Detail, reason: String, a: &Access) -> Result<Self> {
        ensure!(
            path.extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("csv")),
            "Use the HRIS acknowledgement CSV template."
        );
        let file = std::fs::File::open(path).context("Could not open acknowledgement file.")?;
        let mut bytes = Vec::new();
        file.take(2 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() <= 2 * 1024 * 1024,
            "Acknowledgement file exceeds2MiB."
        );
        Self::from_csv(&bytes, detail, reason, a)
    }
}
#[derive(Clone)]
pub enum Operation {
    State,
    SaveProfile(ProfileChange),
    Preview(Request),
    Prepare {
        request: Request,
        fingerprint: String,
    },
    Detail {
        id: Uuid,
        destination: Option<std::path::PathBuf>,
    },
    Template {
        id: Uuid,
        destination: std::path::PathBuf,
    },
    Acknowledge(Ack),
    Resolve {
        kind: String,
        id: Uuid,
        payload: serde_json::Value,
    },
}
impl Operation {
    pub fn attempt(&self) -> Option<(String, Uuid, serde_json::Value)> {
        match self {
            Self::SaveProfile(c) => Some((
                "profile".into(),
                c.request_id,
                serde_json::to_value(c).ok()?,
            )),
            Self::Prepare {
                request,
                fingerprint,
            } => Some((
                "prepare".into(),
                request.id,
                serde_json::json!({"request":request,"fingerprint":fingerprint}),
            )),
            Self::Acknowledge(a) => Some(("ack".into(), a.id, serde_json::to_value(a).ok()?)),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attempt {
    pub version: String,
    pub company_id: Uuid,
    pub user_id: Uuid,
    pub role: Role,
    pub kind: String,
    pub id: Uuid,
    pub committed: bool,
}
impl Attempt {
    pub fn validate(&self, a: &Access, kind: &str, id: Uuid) -> Result<()> {
        scope(self.company_id, self.user_id, self.role, a)?;
        ensure!(
            self.version == "payroll-exchange-attempt-v1"
                && self.kind == kind
                && self.id == id
                && ["profile", "prepare", "ack"].contains(&kind),
            "Retained attempt receipt changed."
        );
        Ok(())
    }
}
#[derive(Clone, Default)]
pub struct Update {
    pub state: Option<State>,
    pub preview: Option<Preview>,
    pub detail: Option<Detail>,
    pub profile_saved: Option<Uuid>,
    pub resolved: Option<Attempt>,
}
pub fn authorize(data: &Snapshot) -> Result<()> {
    p::authorize(data)
}
