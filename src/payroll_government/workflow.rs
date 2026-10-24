//! Reviewed allocations and recorded remittances. No original payroll is edited.
use super::*;
use crate::domain::Snapshot;
use anyhow::Context;
use serde_json::Value;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Filter {
    pub month: NaiveDate,
    pub agency: Agency,
}
impl Filter {
    pub fn validate(&self, today: NaiveDate) -> Result<()> {
        ensure!(
            month_ok(self.month) && self.month <= today,
            "Choose a past/current calendar month from 2025 onward."
        );
        Ok(())
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Basis {
    pub company_id: Uuid,
    pub employee_id: Uuid,
    pub id: Uuid,
    pub agency: Agency,
    pub kind: Kind,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    pub contribution_month: Option<NaiveDate>,
    pub parts: Parts,
}
impl From<&Fact> for Basis {
    fn from(f: &Fact) -> Self {
        Self {
            company_id: f.company_id,
            employee_id: f.employee_id,
            id: f.id,
            agency: f.agency,
            kind: f.kind,
            starts_on: f.starts_on,
            ends_on: f.ends_on,
            contribution_month: f.contribution_month,
            parts: f.parts,
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Allocation {
    pub month: NaiveDate,
    pub employee_cents: i64,
    pub employer_cents: i64,
    pub ec_cents: i64,
}
impl Allocation {
    pub fn parts(&self) -> Parts {
        Parts {
            employee_cents: Some(self.employee_cents),
            employer_cents: Some(self.employer_cents),
            ec_cents: Some(self.ec_cents),
        }
    }
}
fn text(value: &str, max: usize, required: bool) -> Result<()> {
    ensure!(
        (!required || !value.trim().is_empty())
            && value.chars().count() <= max
            && !value
                .chars()
                .any(|c| c.is_control() && c != '\n' && c != '\t'),
        "Check government reference/note length and required text."
    );
    Ok(())
}
pub fn validate_plan(src: &Basis, plan: &[Allocation], today: NaiveDate) -> Result<()> {
    ensure!(
        src.agency != Agency::Bir
            && matches!(
                src.kind,
                Kind::Thirteenth | Kind::Amendment | Kind::FinalContribution
            )
            && !src.id.is_nil()
            && (1..=24).contains(&plan.len()),
        "Choose an available special contribution source and 1–24 covered months."
    );
    let expected = src
        .parts
        .employee_cents
        .context("The issued employee contribution is unavailable.")?;
    let mut total = 0;
    let mut dates = HashSet::new();
    for a in plan {
        ensure!(
            month_ok(a.month)
                && a.month <= today
                && a.month
                    >= src
                        .starts_on
                        .with_day(1)
                        .context("Invalid service start.")?
                && a.month <= src.ends_on.with_day(1).context("Invalid service end.")?
                && src.contribution_month.is_none_or(|d| d == a.month)
                && dates.insert(a.month),
            "Use unique supported months within the original service and known coverage."
        );
        for n in [a.employee_cents, a.employer_cents, a.ec_cents] {
            ensure!(
                n.unsigned_abs() <= 100_000_000_000_000,
                "Contribution review amounts exceed the supported centavo limit."
            );
        }
        ensure!(
            (src.agency == Agency::Sss || a.ec_cents == 0)
                && (expected < 0 || a.employee_cents >= 0)
                && (expected > 0 || a.employee_cents <= 0),
            "Do not invent EC or offset employee charges with new refunds."
        );
        total = add(total, a.employee_cents)?;
    }
    ensure!(
        total == expected,
        "Allocated employee centavos must exactly retain the issued deduction/adjustment."
    );
    Ok(())
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Review {
    pub id: Uuid,
    pub company_id: Uuid,
    pub employee_id: Uuid,
    pub source_kind: Kind,
    pub source_id: Uuid,
    pub agency: Agency,
    pub status: String,
    pub revision: i64,
    pub previous_id: Option<Uuid>,
    pub source_basis: Basis,
    pub allocations: Vec<Allocation>,
    pub reference: String,
    pub note: String,
    pub created_by: Uuid,
    pub prepared_by: Uuid,
    pub contributors: Vec<Uuid>,
    pub submitted_by: Option<Uuid>,
    pub approved_by: Option<Uuid>,
    pub approved_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
impl Review {
    fn validate(&self, cid: Uuid, today: NaiveDate) -> Result<()> {
        let b = &self.source_basis;
        ensure!(
            self.company_id == cid
                && !self.id.is_nil()
                && self.employee_id == b.employee_id
                && self.source_id == b.id
                && self.source_kind == b.kind
                && self.agency == b.agency
                && cid == b.company_id
                && self.revision >= 0
                && ["draft", "submitted", "approved", "cancelled"].contains(&self.status.as_str()),
            "Government review identity/status is inconsistent."
        );
        let contributors: HashSet<_> = self.contributors.iter().copied().collect();
        ensure!(
            (1..=128).contains(&contributors.len())
                && contributors.len() == self.contributors.len()
                && !contributors.contains(&Uuid::nil())
                && contributors.contains(&self.created_by)
                && contributors.contains(&self.prepared_by)
                && self
                    .submitted_by
                    .is_none_or(|id| contributors.contains(&id))
                && self
                    .previous_id
                    .is_none_or(|id| !id.is_nil() && id != self.id),
            "Government reviewer/creator history is inconsistent."
        );
        let submitted = ["submitted", "approved"].contains(&self.status.as_str());
        ensure!(
            submitted == self.submitted_by.is_some()
                && (self.status == "approved") == self.approved_by.is_some()
                && self.approved_at.is_some() == self.approved_by.is_some()
                && self
                    .approved_by
                    .is_none_or(|id| !id.is_nil() && !contributors.contains(&id)),
            "Government review lacks independent approval metadata."
        );
        text(&self.reference, 1000, true)?;
        text(&self.note, 2000, true)?;
        validate_plan(b, &self.allocations, today)
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReceiptAllocation {
    pub month: NaiveDate,
    pub amount_cents: i64,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub id: Uuid,
    pub company_id: Uuid,
    pub agency: Agency,
    pub paid_on: NaiveDate,
    pub amount_cents: i64,
    pub allocations: Vec<ReceiptAllocation>,
    pub reference: String,
    pub evidence: String,
    pub note: String,
    pub replaces_id: Option<Uuid>,
    pub recorded_by: Uuid,
    pub recorded_at: DateTime<Utc>,
    pub void_id: Option<Uuid>,
    pub void_reason: Option<String>,
    pub voided_by: Option<Uuid>,
    pub voided_at: Option<DateTime<Utc>>,
    pub replacement_id: Option<Uuid>,
}
impl Receipt {
    fn validate(
        &self,
        cid: Uuid,
        agency: Agency,
        month: NaiveDate,
        today: NaiveDate,
    ) -> Result<()> {
        ensure!(
            self.company_id == cid
                && self.agency == agency
                && !self.id.is_nil()
                && !self.recorded_by.is_nil()
                && self.paid_on >= NaiveDate::from_ymd_opt(2025, 1, 1).unwrap()
                && self.paid_on <= today
                && (1..=100_000_000_000_000).contains(&self.amount_cents)
                && (1..=24).contains(&self.allocations.len()),
            "Recorded government receipt is outside the expected scope or range."
        );
        text(&self.reference, 1000, true)?;
        text(&self.evidence, 2000, true)?;
        text(&self.note, 2000, false)?;
        ensure!(
            !self.reference.chars().any(char::is_control),
            "Receipt reference must be a single line."
        );
        let mut total = 0;
        let mut dates = HashSet::new();
        for a in &self.allocations {
            ensure!(
                month_ok(a.month)
                    && a.month <= today
                    && (1..=100_000_000_000_000).contains(&a.amount_cents)
                    && dates.insert(a.month),
                "Receipt coverage must contain unique supported months and positive exact centavos."
            );
            total = add(total, a.amount_cents)?;
        }
        ensure!(
            total == self.amount_cents && dates.contains(&month),
            "Receipt allocations do not reconcile to this period and its full total."
        );
        let voided = self.void_id.is_some();
        ensure!(
            voided == self.void_reason.is_some()
                && voided == self.voided_by.is_some()
                && voided == self.voided_at.is_some()
                && self.void_id.is_none_or(|id| !id.is_nil())
                && self.voided_by.is_none_or(|id| !id.is_nil())
                && self
                    .replacement_id
                    .is_none_or(|id| voided && !id.is_nil() && id != self.id)
                && self
                    .replaces_id
                    .is_none_or(|id| !id.is_nil() && id != self.id),
            "Receipt void/replacement history is inconsistent."
        );
        if let Some(s) = &self.void_reason {
            text(s, 2000, true)?;
        }
        Ok(())
    }
    pub fn covered_cents(&self, month: NaiveDate) -> i64 {
        self.allocations
            .iter()
            .find(|a| a.month == month)
            .map(|a| a.amount_cents)
            .unwrap_or(0)
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub id: Uuid,
    pub company_id: Uuid,
    pub record_id: Uuid,
    pub kind: String,
    pub actor_name: String,
    pub reason: String,
    pub recorded_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EmployeeLabel {
    pub employee_id: Uuid,
    pub employee_code: String,
    pub employee_name: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewWire {
    pub version: String,
    pub source: Wire,
    pub effective_facts: Vec<Fact>,
    pub reviews: Vec<Review>,
    pub remittances: Vec<Receipt>,
    pub history: Vec<Event>,
    pub employees: Vec<EmployeeLabel>,
}
#[derive(Clone, Debug)]
pub struct Reconciliation {
    pub wire: ReviewWire,
    pub summary: Summary,
}
impl Reconciliation {
    pub fn from_wire(wire: ReviewWire, cid: Uuid, filter: Filter) -> Result<Self> {
        ensure!(
            wire.version == "government-reconciliation-v1"
                && wire.reviews.len() <= 10000
                && wire.remittances.len() <= 5000
                && wire.history.len() <= 20000,
            "Government workflow response version/size is unsupported."
        );
        wire.source.validate(cid, filter.agency, filter.month)?;
        filter.validate(wire.source.as_of)?;
        let mut review_ids = HashSet::new();
        for r in &wire.reviews {
            r.validate(cid, wire.source.as_of)?;
            ensure!(
                r.agency == filter.agency
                    && review_ids.insert(r.id)
                    && wire.source.facts.iter().any(|f| f.kind == r.source_kind
                        && f.id == r.source_id
                        && f.employee_id == r.employee_id),
                "Review belongs to a foreign source/agency or is duplicated."
            );
        }
        for r in &wire.reviews {
            if let Some(previous) = r.previous_id {
                let p = wire
                    .reviews
                    .iter()
                    .find(|p| p.id == previous)
                    .context("Prior approved review is missing.")?;
                ensure!(
                    p.status == "approved"
                        && p.source_kind == r.source_kind
                        && p.source_id == r.source_id
                        && p.agency == r.agency
                        && p.employee_id == r.employee_id,
                    "Linked government review crosses source boundaries."
                );
                let mut seen = HashSet::new();
                let mut current = Some(r.id);
                while let Some(id) = current {
                    ensure!(seen.insert(id), "Government review version cycle.");
                    current = wire
                        .reviews
                        .iter()
                        .find(|p| p.id == id)
                        .and_then(|p| p.previous_id);
                }
            }
        }
        let mut computed = vec![];
        for f in &wire.source.facts {
            let leaves: Vec<_> = wire
                .reviews
                .iter()
                .filter(|r| {
                    r.status == "approved"
                        && r.source_kind == f.kind
                        && r.source_id == f.id
                        && !wire
                            .reviews
                            .iter()
                            .any(|n| n.status == "approved" && n.previous_id == Some(r.id))
                })
                .collect();
            ensure!(leaves.len() <= 1, "Conflicting approved coverage versions.");
            let mut out = f.clone();
            if !matches!(f.state, State::Withdrawn | State::ReviewOnly) {
                if let Some(r) = leaves.first().filter(|r| r.source_basis == Basis::from(f)) {
                    let Some(a) = r.allocations.iter().find(|a| a.month == filter.month) else {
                        continue;
                    };
                    out.contribution_month = Some(a.month);
                    out.parts = a.parts();
                }
            }
            computed.push(out);
        }
        ensure!(
            computed == wire.effective_facts,
            "Server contribution allocation results do not match their retained sources/reviews."
        );
        let mut receipt_ids = HashSet::new();
        let mut active_refs = HashSet::new();
        let mut payments = vec![];
        for r in &wire.remittances {
            r.validate(cid, filter.agency, filter.month, wire.source.as_of)?;
            ensure!(receipt_ids.insert(r.id), "Duplicate government remittance.");
            if r.void_id.is_none() {
                ensure!(
                    active_refs.insert(r.reference.trim().to_lowercase()),
                    "Duplicate active government payment reference."
                );
            }
            payments.push(Remittance {
                id: r.id,
                company_id: r.company_id,
                agency: r.agency,
                month: filter.month,
                paid_on: r.paid_on,
                amount_cents: r.covered_cents(filter.month),
                voided: r.void_id.is_some(),
            });
        }
        let mut employee_ids = HashSet::new();
        for e in &wire.employees {
            ensure!(
                !e.employee_id.is_nil()
                    && employee_ids.insert(e.employee_id)
                    && wire
                        .source
                        .facts
                        .iter()
                        .any(|f| f.employee_id == e.employee_id),
                "Employee label outside the current government source set."
            );
            text(&e.employee_code, 200, true)?;
            text(&e.employee_name, 200, true)?;
        }
        ensure!(
            wire.source
                .facts
                .iter()
                .all(|f| employee_ids.contains(&f.employee_id)),
            "A government source lacks its employee label."
        );
        let mut event_ids = HashSet::new();
        for e in &wire.history {
            ensure!(
                e.company_id == cid
                    && !e.id.is_nil()
                    && event_ids.insert(e.id)
                    && (review_ids.contains(&e.record_id) || receipt_ids.contains(&e.record_id))
                    && [
                        "allocation_create",
                        "allocation_save",
                        "allocation_submit",
                        "allocation_approve",
                        "allocation_return",
                        "allocation_cancel",
                        "remittance_record",
                        "remittance_void"
                    ]
                    .contains(&e.kind.as_str()),
                "Government audit history is outside the current scope."
            );
            text(&e.actor_name, 200, true)?;
            text(&e.reason, 2000, true)?;
        }
        let summary = reconcile(
            cid,
            filter.agency,
            filter.month,
            wire.source.as_of,
            &computed,
            &payments,
        )?;
        Ok(Self { wire, summary })
    }
    pub fn filter(&self) -> Filter {
        Filter {
            month: self.wire.source.month,
            agency: self.wire.source.agency,
        }
    }
    pub fn employee(&self, id: Uuid) -> Option<&EmployeeLabel> {
        self.wire.employees.iter().find(|e| e.employee_id == id)
    }
    pub fn approved(&self, f: &Fact) -> Option<&Review> {
        self.wire.reviews.iter().find(|r| {
            r.status == "approved"
                && r.source_kind == f.kind
                && r.source_id == f.id
                && !self
                    .wire
                    .reviews
                    .iter()
                    .any(|n| n.status == "approved" && n.previous_id == Some(r.id))
        })
    }
    pub fn pending(&self, f: &Fact) -> Option<&Review> {
        self.wire.reviews.iter().find(|r| {
            ["draft", "submitted"].contains(&r.status.as_str())
                && r.source_kind == f.kind
                && r.source_id == f.id
        })
    }
}
#[derive(Clone, Debug)]
pub struct Action {
    pub operation: String,
    pub record: Value,
}
impl Action {
    pub fn endpoint(&self) -> Result<(&'static str, &str)> {
        let (kind, op) = self
            .operation
            .split_once('_')
            .context("Choose a government action.")?;
        match (kind, op) {
            ("allocation", "create" | "save" | "submit" | "approve" | "return" | "cancel") => {
                Ok(("rpc/manage_government_allocation", op))
            }
            ("remittance", "record" | "void") => Ok(("rpc/manage_government_remittance", op)),
            _ => anyhow::bail!("Choose a supported government action."),
        }
    }
    pub fn id(&self) -> Result<Uuid> {
        let id = Uuid::parse_str(
            self.record["id"]
                .as_str()
                .context("Government operation identity missing.")?,
        )?;
        ensure!(!id.is_nil(), "Government operation identity required.");
        Ok(id)
    }
    pub fn authorize(&self, data: &Snapshot) -> Result<()> {
        crate::payroll_reporting::authorize(data)?;
        self.endpoint()?;
        self.id()?;
        ensure!(
            self.record.is_object() && self.record.to_string().len() <= 65536,
            "Government action is too large."
        );
        if self.operation == "allocation_approve" || self.operation.starts_with("remittance_") {
            ensure!(
                data.access.role == crate::access::Role::Administrator,
                "Only Administrators approve or record/void remittances."
            );
        }
        Ok(())
    }
}

pub fn decimal(n: i64) -> String {
    format!(
        "{}{}.{:02}",
        if n < 0 { "-" } else { "" },
        n.unsigned_abs() / 100,
        n.unsigned_abs() % 100
    )
}
pub fn parse_cents(value: &str) -> Result<i64> {
    let s = value.trim();
    let (negative, s) = s.strip_prefix('-').map_or((false, s), |s| (true, s));
    let mut p = s.split('.');
    let whole = p.next().unwrap_or("");
    let minor = p.next().unwrap_or("");
    ensure!(
        !whole.is_empty()
            && whole.len() <= 13
            && whole.bytes().all(|c| c.is_ascii_digit())
            && minor.len() <= 2
            && minor.bytes().all(|c| c.is_ascii_digit())
            && p.next().is_none(),
        "Enter a peso amount with at most two decimals."
    );
    let major = whole.parse::<i64>()?;
    let minor = if minor.is_empty() {
        0
    } else {
        minor.parse::<i64>()? * if minor.len() == 1 { 10 } else { 1 }
    };
    let n = major
        .checked_mul(100)
        .and_then(|n| n.checked_add(minor))
        .context("Amount is too large.")?;
    ensure!(
        n <= 100_000_000_000_000,
        "Amount exceeds the supported centavo limit."
    );
    Ok(if negative { -n } else { n })
}
impl Reconciliation {
    pub fn included(&self, f: &Fact) -> bool {
        if matches!(f.state, State::Withdrawn | State::ReviewOnly) {
            return false;
        }
        if f.agency == Agency::Bir {
            f.state == State::Received
                && f.received_on.and_then(|d| d.with_day(1)) == Some(self.wire.source.month)
        } else {
            f.contribution_month == Some(self.wire.source.month)
        }
    }
    pub fn csv(&self) -> Result<Vec<u8>> {
        let s = &self.wire.source;
        let summary = &self.summary;
        let mut out = String::from("\u{feff}");
        let line = |values: Vec<String>| -> Result<String> {
            ensure!(
                values.len() == 22,
                "Government worksheet columns do not match."
            );
            Ok(values
                .iter()
                .map(|v| crate::reports::csv_cell(v))
                .collect::<Result<Vec<_>>>()?
                .join(",")
                + "\r\n")
        };
        out.push_str(&line(
            [
                "record_type",
                "company",
                "agency",
                "report_month",
                "generated_utc",
                "HRIS_record_id",
                "employee_code_current",
                "employee_name_current",
                "source",
                "period_start",
                "period_end",
                "received_on",
                "status",
                "included_in_liability",
                "known_employee_php",
                "known_employer_php",
                "known_ec_php",
                "known_total_php",
                "complete_liability_php",
                "recorded_remitted_php",
                "difference_php",
                "review_note",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        )?);
        let prefix = |kind: &str| {
            vec![
                kind.into(),
                s.company_name.clone(),
                s.agency.label().into(),
                s.month.to_string(),
                s.generated_at.to_rfc3339(),
            ]
        };
        let known = summary.known_parts;
        let total = known.total().context("Known totals are invalid.")?;
        let mut row = prefix("summary");
        row.extend([String::new(),String::new(),String::new(),String::new(),String::new(),String::new(),String::new(),if summary.complete{"Sources reconciled".into()}else{"UNRESOLVED".into()},decimal(known.employee_cents.unwrap()),decimal(known.employer_cents.unwrap()),decimal(known.ec_cents.unwrap()),decimal(total),if summary.complete{decimal(total)}else{"UNRESOLVED".into()},decimal(summary.remitted_cents),summary.difference_cents.map(decimal).unwrap_or("UNRESOLVED".into()),format!("{} unallocated; {} missing receipts; {} postdated receipts; {} negative person balances. Review worksheet; filing/payment is separate.",summary.unallocated_count,summary.missing_receipt_count,summary.dated_after_as_of_count,summary.negative_balance_count)]);
        row.insert(13, String::new());
        out.push_str(&line(row)?);
        for f in &self.wire.effective_facts {
            let e = self
                .employee(f.employee_id)
                .context("Employee label missing.")?;
            let mut row = prefix("source");
            row.extend([f.id.to_string(),e.employee_code.clone(),e.employee_name.clone(),f.kind.label().into(),f.starts_on.to_string(),f.ends_on.to_string(),f.received_on.map(|d|d.to_string()).unwrap_or_default(),f.state.label().into(),f.parts.employee_cents.map(decimal).unwrap_or("UNKNOWN".into()),f.parts.employer_cents.map(decimal).unwrap_or("UNKNOWN".into()),f.parts.ec_cents.map(decimal).unwrap_or("UNKNOWN".into()),f.parts.total().map(decimal).unwrap_or("UNKNOWN".into()),String::new(),String::new(),String::new(),if matches!(f.state,State::Withdrawn|State::ReviewOnly){"Excluded from liability".into()}else if f.agency!=Agency::Bir&&f.contribution_month.is_none(){"Contribution coverage requires review".into()}else{"Original source / reviewed allocation; receipt and inclusion basis remain distinct".into()}]);
            row.insert(13, self.included(f).to_string());
            out.push_str(&line(row)?);
        }
        for r in &self.wire.remittances {
            let mut row = prefix("remittance");
            row.extend([
                r.id.to_string(),
                String::new(),
                String::new(),
                "Government receipt coverage".into(),
                s.month.to_string(),
                crate::payroll::month_end(s.month).to_string(),
                r.paid_on.to_string(),
                if r.void_id.is_some() {
                    "Voided".into()
                } else {
                    "Recorded".into()
                },
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                decimal(if r.void_id.is_some() {
                    0
                } else {
                    r.covered_cents(s.month)
                }),
                String::new(),
                "Selected month allocation only; private payment references/evidence are omitted"
                    .into(),
            ]);
            row.insert(13, String::new());
            out.push_str(&line(row)?);
        }
        Ok(out.into_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn csv_separates_source_inclusion_from_amounts_and_month_share_from_whole_receipt() {
        let report = crate::payroll_government::preview();
        let csv = String::from_utf8(report.csv().unwrap()).unwrap();
        let rows: Vec<Vec<&str>> = csv
            .lines()
            .map(|s| {
                s.trim_start_matches('\u{feff}')
                    .trim_matches('"')
                    .split("\",\"")
                    .collect()
            })
            .collect();
        assert!(rows.iter().all(|r| r.len() == 22));
        assert_eq!(rows[0][13], "included_in_liability");
        assert_eq!(rows[2][13], "true");
        assert_eq!(rows[3][13], "false");
        assert_eq!(rows[4][19], "4600.00"); // Only September, not the 5,000 whole receipt.
        assert!(!csv.contains("Fictional bank receipt"));
        assert!(!csv.contains("FICTIONAL-SSS-RECEIPT"));
        let mut w = report.wire;
        w.source.agency = Agency::Bir;
        w.source.facts.truncate(1);
        let f = &mut w.source.facts[0];
        f.agency = Agency::Bir;
        f.state = State::Approved;
        f.received_on = None;
        f.parts.employer_cents = Some(0);
        f.parts.ec_cents = Some(0);
        w.effective_facts = w.source.facts.clone();
        w.reviews.clear();
        w.remittances.clear();
        w.history.clear();
        w.employees.truncate(1);
        let r = Reconciliation::from_wire(
            w,
            Uuid::from_u128(1),
            Filter {
                month: "2026-09-01".parse().unwrap(),
                agency: Agency::Bir,
            },
        )
        .unwrap();
        assert!(!r.included(&r.wire.effective_facts[0]));
        assert_eq!(r.summary.known_parts.employee_cents, Some(0));
        let csv = String::from_utf8(r.csv().unwrap()).unwrap();
        assert!(csv.contains("\"Approved, not received\",\"false\",\"1500.00\""));
    }
    #[test]
    fn currency_input_is_exact_and_zero_deductions_cannot_be_split_into_refunds() {
        for (s, n) in [
            ("0", 0),
            ("0.01", 1),
            ("-0.01", -1),
            ("123.4", 12340),
            (" 1.23 ", 123),
        ] {
            assert_eq!(parse_cents(s).unwrap(), n);
        }
        for s in [
            "",
            ".1",
            "1.001",
            "1e2",
            "+1",
            "1,000.00",
            "1.2.3",
            "NaN",
            "1000000000000.01",
        ] {
            assert!(parse_cents(s).is_err(), "{s}");
        }
        let src = Basis {
            company_id: Uuid::new_v4(),
            employee_id: Uuid::new_v4(),
            id: Uuid::new_v4(),
            agency: Agency::Sss,
            kind: Kind::Thirteenth,
            starts_on: "2026-01-01".parse().unwrap(),
            ends_on: "2026-09-30".parse().unwrap(),
            contribution_month: None,
            parts: Parts {
                employee_cents: Some(0),
                employer_cents: None,
                ec_cents: None,
            },
        };
        let p = vec![
            Allocation {
                month: "2026-01-01".parse().unwrap(),
                employee_cents: 100,
                employer_cents: 0,
                ec_cents: 0,
            },
            Allocation {
                month: "2026-02-01".parse().unwrap(),
                employee_cents: -100,
                employer_cents: 0,
                ec_cents: 0,
            },
        ];
        assert!(validate_plan(&src, &p, "2026-10-25".parse().unwrap()).is_err());
    }
    #[test]
    fn one_persons_contribution_refund_cannot_be_hidden_by_another_persons_positive_charge() {
        let cid = Uuid::new_v4();
        let mut f = Fact {
            company_id: cid,
            employee_id: Uuid::new_v4(),
            id: Uuid::new_v4(),
            agency: Agency::Sss,
            kind: Kind::Amendment,
            state: State::Approved,
            starts_on: "2026-09-01".parse().unwrap(),
            ends_on: "2026-09-30".parse().unwrap(),
            contribution_month: Some("2026-09-01".parse().unwrap()),
            received_on: None,
            parts: Parts {
                employee_cents: Some(0),
                employer_cents: Some(100),
                ec_cents: Some(0),
            },
        };
        let mut bad = f.clone();
        bad.id = Uuid::new_v4();
        bad.employee_id = Uuid::new_v4();
        bad.parts.employer_cents = Some(-1);
        f.id = Uuid::new_v4();
        let s = reconcile(
            cid,
            Agency::Sss,
            "2026-09-01".parse().unwrap(),
            "2026-10-25".parse().unwrap(),
            &[f, bad],
            &[],
        )
        .unwrap();
        assert_eq!(s.negative_balance_count, 1);
        assert_eq!(s.difference_cents, None);
    }
    #[test]
    #[ignore = "Requires explicit local PostgreSQL workflow vectors."]
    fn postgres_workflows_validate_and_export_without_private_evidence() {
        let path = std::env::var("HRIS_GOVERNMENT_RECON_VECTORS")
            .expect("Set the local workflow-vector path.");
        let values: Vec<Value> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(values.len(), 4);
        for value in values {
            let w: ReviewWire = serde_json::from_value(value.clone()).unwrap();
            let cid = w.source.company_id;
            let filter = Filter {
                month: w.source.month,
                agency: w.source.agency,
            };
            let report = Reconciliation::from_wire(w, cid, filter).unwrap();
            let csv = String::from_utf8(report.csv().unwrap()).unwrap();
            assert!(!csv.contains("PRIVATE_"));
            assert!(csv.contains("record_type"));
            let mut foreign = value.clone();
            foreign["source"]["company_id"] = json!(Uuid::new_v4());
            let w: ReviewWire = serde_json::from_value(foreign).unwrap();
            assert!(Reconciliation::from_wire(w, cid, filter).is_err());
            if let Some(record) = value["effective_facts"].as_array().and_then(|a| a.first()) {
                let mut tampered = value.clone();
                tampered["effective_facts"][0]["parts"]["employee_cents"] =
                    json!(record["parts"]["employee_cents"].as_i64().unwrap_or(0) + 1);
                let w: ReviewWire = serde_json::from_value(tampered).unwrap();
                assert!(Reconciliation::from_wire(w, cid, filter).is_err());
            }
        }
    }
}
