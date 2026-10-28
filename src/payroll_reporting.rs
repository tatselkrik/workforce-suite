//! Read-only consolidated payroll facts. Advice, offsets and cash stay distinct.
use crate::{
    domain::{Mode, Snapshot},
    payroll::month_end,
};
use anyhow::{Context, Result, ensure};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
pub const VERSION: &str = "payroll-report-v2";
const LIMIT: i64 = 1_000_000_000_000_000;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Filter {
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    pub axis: String,
    pub kind: String,
    pub employee_id: Option<Uuid>,
    pub search: String,
}
impl Filter {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.starts_on <= self.ends_on
                && self.starts_on.year() >= 2026
                && self.ends_on.year() <= 2200
                && (self.ends_on - self.starts_on).num_days() <= 365,
            "Use at most366 days from2026 onward."
        );
        ensure!(
            ["issued", "received"].contains(&self.axis.as_str())
                && [
                    "all",
                    "monthly",
                    "cutoff",
                    "thirteenth",
                    "annual_tax",
                    "final",
                    "amendment"
                ]
                .contains(&self.kind.as_str()),
            "Choose the report date basis and source kind."
        );
        ensure!(
            self.search.chars().count() <= 80
                && !self.search.chars().any(char::is_control)
                && self.employee_id.is_none_or(|id| !id.is_nil()),
            "Use a bounded employee search."
        );
        Ok(())
    }
    pub fn matches(&self, r: &Row) -> bool {
        let on = if self.axis == "received" {
            r.received_on
        } else {
            Some(r.issued_on)
        };
        on.is_some_and(|d| d >= self.starts_on && d <= self.ends_on)
            && (self.kind == "all" || self.kind == r.kind)
            && self.employee_id.is_none_or(|id| id == r.employee_id)
            && (self.search.trim_matches(' ').is_empty()
                || format!("{} {} {}", r.employee_code, r.employee_name, r.department)
                    .to_lowercase()
                    .contains(&self.search.trim_matches(' ').to_lowercase()))
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Row {
    pub id: Uuid,
    pub company_id: Uuid,
    pub employee_id: Uuid,
    pub kind: String,
    pub employee_code: String,
    pub employee_name: String,
    pub department: String,
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
    pub issued_on: NaiveDate,
    pub received_on: Option<NaiveDate>,
    pub status: String,
    pub payment_state: String,
    pub basic_cents: Option<i64>,
    pub gross_cents: Option<i64>,
    pub employee_contributions_cents: Option<i64>,
    pub employer_contributions_cents: Option<i64>,
    pub withholding_cents: Option<i64>,
    pub net_cents: Option<i64>,
    pub contained_source_net_cents: Option<i64>,
    pub cash_cents: i64,
    pub offset_cents: i64,
    pub tax_adjustment_cents: Option<i64>,
    pub outstanding_cents: Option<i64>,
    pub reserved_cents: i64,
    pub reserved_case_id: Option<Uuid>,
    pub rule_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linked_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linked_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review_amount_cents: Option<i64>,
}
impl Row {
    fn empty(
        cid: Uuid,
        id: Uuid,
        eid: Uuid,
        kind: &str,
        code: &str,
        name: &str,
        dept: &str,
        lo: NaiveDate,
        hi: NaiveDate,
        on: NaiveDate,
        rule: &str,
    ) -> Self {
        Self {
            id,
            company_id: cid,
            employee_id: eid,
            kind: kind.into(),
            employee_code: code.into(),
            employee_name: name.into(),
            department: dept.into(),
            period_start: lo,
            period_end: hi,
            issued_on: on,
            received_on: None,
            status: "approved".into(),
            payment_state: "unpaid".into(),
            basic_cents: None,
            gross_cents: None,
            employee_contributions_cents: None,
            employer_contributions_cents: None,
            withholding_cents: None,
            net_cents: None,
            contained_source_net_cents: None,
            cash_cents: 0,
            offset_cents: 0,
            tax_adjustment_cents: None,
            outstanding_cents: None,
            reserved_cents: 0,
            reserved_case_id: None,
            rule_version: rule.into(),
            linked_kind: None,
            linked_id: None,
            review_amount_cents: None,
        }
    }
    pub fn is_source(&self) -> bool {
        ["monthly", "cutoff", "thirteenth"].contains(&self.kind.as_str())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Wire {
    pub version: String,
    pub company_name: String,
    pub company_id: Uuid,
    pub generated_at: DateTime<Utc>,
    pub filter: Filter,
    pub rows: Vec<Row>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Summary {
    pub amendment_gross_cents: i64,
    pub amendment_net_cents: i64,
    pub amendment_contributions_cents: i64,
    pub amendment_withholding_cents: i64,
    pub amendment_outstanding_cents: i64,
    pub overpayment_review_cents: i64,
    pub source_gross_cents: i64,
    pub source_net_cents: i64,
    pub source_employee_contributions_cents: i64,
    pub source_employer_contributions_cents: i64,
    pub source_withholding_cents: i64,
    pub recorded_cash_cents: i64,
    pub source_offset_cents: i64,
    pub current_source_outstanding_cents: i64,
    pub current_source_reserved_cents: i64,
    pub final_statement_net_cents: i64,
    pub final_contained_source_net_cents: i64,
    pub final_additional_net_cents: i64,
    pub annual_advised_adjustment_cents: i64,
    pub annual_settled_adjustment_cents: i64,
    pub missing_receipts: usize,
}
#[derive(Clone, Debug)]
pub struct Report {
    pub wire: Wire,
    pub totals: Summary,
}
fn add(to: &mut i64, n: i64) -> Result<()> {
    *to = to
        .checked_add(n)
        .context("Report total exceeds supported integer centavos.")?;
    Ok(())
}
pub fn authorize(data: &Snapshot) -> Result<()> {
    ensure!(
        data.mode != Mode::SignedOut
            && data.access.company_id == data.company.id
            && data
                .access
                .allows(crate::staff_roles::Capability::PayrollReports),
        "Only current HR/Admin can generate company payroll reports."
    );
    Ok(())
}
impl Report {
    pub fn from_wire(wire: Wire, cid: Uuid, filter: &Filter) -> Result<Self> {
        filter.validate()?;
        ensure!(
            (wire.version == VERSION
                || (wire.version == "payroll-report-v1"
                    && wire.rows.iter().all(|r| r.kind != "amendment")))
                && wire.company_id == cid
                && wire.filter == *filter
                && wire.rows.len() <= 5000,
            "Payroll report scope, contract or row limit changed. Refresh."
        );
        ensure!(
            !wire.company_name.trim().is_empty()
                && wire.company_name.chars().count() <= 240
                && !wire.company_name.chars().any(char::is_control),
            "Invalid report company label."
        );
        let mut t = Summary::default();
        let mut ids = std::collections::BTreeSet::new();
        for r in &wire.rows {
            ensure!(
                r.company_id == cid
                    && !r.id.is_nil()
                    && !r.employee_id.is_nil()
                    && ids.insert((r.kind.clone(), r.id))
                    && filter.matches(r)
                    && r.period_start <= r.period_end,
                "Report contains duplicate or out-of-scope facts."
            );
            ensure!(
                [
                    "monthly",
                    "cutoff",
                    "thirteenth",
                    "annual_tax",
                    "final",
                    "amendment"
                ]
                .contains(&r.kind.as_str()),
                "Unknown payroll report source."
            );
            for s in [
                &r.employee_code,
                &r.employee_name,
                &r.department,
                &r.status,
                &r.payment_state,
                &r.rule_version,
            ] {
                ensure!(
                    s.chars().count() <= 240 && !s.chars().any(char::is_control),
                    "Invalid report label."
                );
            }
            for v in [
                r.basic_cents,
                r.gross_cents,
                if r.kind == "amendment" {
                    None
                } else {
                    r.employee_contributions_cents
                },
                r.employer_contributions_cents,
                if r.kind == "amendment" {
                    None
                } else {
                    r.withholding_cents
                },
                r.net_cents,
                r.contained_source_net_cents,
                r.outstanding_cents,
                Some(r.cash_cents),
                Some(r.offset_cents),
                Some(r.reserved_cents),
            ]
            .into_iter()
            .flatten()
            {
                ensure!((0..=LIMIT).contains(&v), "Invalid recorded payroll amount.");
            }
            for v in [
                r.tax_adjustment_cents,
                r.review_amount_cents,
                if r.kind == "amendment" {
                    r.employee_contributions_cents
                } else {
                    None
                },
                if r.kind == "amendment" {
                    r.withholding_cents
                } else {
                    None
                },
            ]
            .into_iter()
            .flatten()
            {
                ensure!(
                    (-LIMIT..=LIMIT).contains(&v),
                    "Invalid recorded tax adjustment."
                );
            }
            ensure!(
                r.reserved_cents <= r.outstanding_cents.unwrap_or(0)
                    && ((r.cash_cents == 0 && r.offset_cents == 0) || r.received_on.is_some()),
                "Report payment/balance facts do not reconcile."
            );
            add(&mut t.recorded_cash_cents, r.cash_cents)?;
            add(&mut t.source_offset_cents, r.offset_cents)?;
            ensure!(
                r.kind == "amendment"
                    || (r.linked_kind.is_none()
                        && r.linked_id.is_none()
                        && r.review_amount_cents.is_none()),
                "Original report rows cannot acquire amendment links or review amounts."
            );
            if r.is_source() {
                let net = r.net_cents.context("Issued source net missing.")?;
                ensure!(
                    ["approved", "paid"].contains(&r.status.as_str()),
                    "Issued source status is invalid."
                );
                let reconciles = match r.payment_state.as_str() {
                    "received" => {
                        r.received_on.is_some()
                            && r.cash_cents == net
                            && r.offset_cents == 0
                            && r.outstanding_cents == Some(0)
                            && r.reserved_cents == 0
                            && r.reserved_case_id.is_none()
                    }
                    "final_offset" => {
                        r.received_on.is_some()
                            && r.cash_cents == 0
                            && r.offset_cents == net
                            && r.outstanding_cents == Some(0)
                            && r.reserved_cents == 0
                            && r.reserved_case_id.is_some()
                    }
                    "unpaid" => {
                        r.received_on.is_none()
                            && r.cash_cents == 0
                            && r.offset_cents == 0
                            && r.outstanding_cents == Some(net)
                            && r.reserved_cents == 0
                            && r.reserved_case_id.is_none()
                    }
                    "reserved" => {
                        r.received_on.is_none()
                            && r.cash_cents == 0
                            && r.offset_cents == 0
                            && r.outstanding_cents == Some(net)
                            && r.reserved_cents == net
                            && r.reserved_case_id.is_some()
                    }
                    "receipt_missing" => {
                        r.received_on.is_none()
                            && r.cash_cents == 0
                            && r.offset_cents == 0
                            && r.outstanding_cents.is_none()
                            && r.reserved_cents == 0
                            && r.reserved_case_id.is_none()
                    }
                    _ => false,
                };
                ensure!(
                    reconciles,
                    "Source payment, offset and balance states do not reconcile."
                );

                ensure!(
                    r.contained_source_net_cents.is_none()
                        && r.tax_adjustment_cents.is_none()
                        && r.cash_cents + r.offset_cents <= net,
                    "Source facts misclassified."
                );
                if r.payment_state == "final_offset" {
                    ensure!(
                        r.cash_cents == 0 && r.reserved_case_id.is_some(),
                        "Final source offsets are not another cash payment."
                    );
                }
                add(
                    &mut t.source_gross_cents,
                    r.gross_cents.context("Source gross missing.")?,
                )?;
                add(&mut t.source_net_cents, net)?;
                add(
                    &mut t.source_employee_contributions_cents,
                    r.employee_contributions_cents.unwrap_or(0),
                )?;
                add(
                    &mut t.source_employer_contributions_cents,
                    r.employer_contributions_cents.unwrap_or(0),
                )?;
                add(
                    &mut t.source_withholding_cents,
                    r.withholding_cents.unwrap_or(0),
                )?;
                add(
                    &mut t.current_source_outstanding_cents,
                    r.outstanding_cents.unwrap_or(0),
                )?;
                add(&mut t.current_source_reserved_cents, r.reserved_cents)?;
                if r.payment_state == "receipt_missing" {
                    t.missing_receipts += 1;
                }
            } else if r.kind == "amendment" {
                let net = r.net_cents.context("Supplemental net missing.")?;
                ensure!(
                    ["monthly", "cutoff", "thirteenth", "final"]
                        .contains(&r.linked_kind.as_deref().unwrap_or(""))
                        && r.linked_id.is_some_and(|id| !id.is_nil())
                        && r.offset_cents == 0
                        && r.reserved_cents == 0
                        && r.reserved_case_id.is_none()
                        && r.contained_source_net_cents.is_none()
                        && r.tax_adjustment_cents.is_none()
                        && r.payment_state == r.status,
                    "Invalid linked supplemental report facts."
                );
                ensure!(
                    ["approved", "paid", "withdrawn", "reviewed"].contains(&r.status.as_str())
                        && (r.status == "paid") == r.received_on.is_some()
                        && r.cash_cents == if r.status == "paid" { net } else { 0 },
                    "Supplemental cash and status do not reconcile."
                );
                if r.status == "reviewed" {
                    ensure!(
                        r.review_amount_cents.is_some_and(|v| v < 0)
                            && net == 0
                            && r.gross_cents == Some(0)
                            && r.employee_contributions_cents == Some(0)
                            && r.withholding_cents == Some(0)
                            && r.outstanding_cents.is_none(),
                        "Overpayment review is not cash or an automatic receivable."
                    );
                    add(
                        &mut t.overpayment_review_cents,
                        r.review_amount_cents.unwrap(),
                    )?;
                } else {
                    ensure!(
                        r.review_amount_cents.is_none()
                            && r.outstanding_cents
                                == Some(if r.status == "approved" { net } else { 0 }),
                        "Supplemental outstanding balance does not reconcile."
                    );
                    add(
                        &mut t.amendment_gross_cents,
                        r.gross_cents.context("Supplemental gross missing.")?,
                    )?;
                    add(&mut t.amendment_net_cents, net)?;
                    add(
                        &mut t.amendment_contributions_cents,
                        r.employee_contributions_cents
                            .context("Supplemental contributions missing.")?,
                    )?;
                    add(
                        &mut t.amendment_withholding_cents,
                        r.withholding_cents
                            .context("Supplemental withholding missing.")?,
                    )?;
                    add(
                        &mut t.amendment_outstanding_cents,
                        r.outstanding_cents.unwrap(),
                    )?;
                }
            } else if r.kind == "final" {
                let net = r.net_cents.context("Final statement net missing.")?;
                ensure!(
                    ["approved", "settled", "withdrawn"].contains(&r.status.as_str())
                        && r.payment_state == r.status
                        && r.reserved_case_id.is_none()
                        && (r.status == "settled") == r.received_on.is_some(),
                    "Final statement status/date invalid."
                );

                let contained = r
                    .contained_source_net_cents
                    .context("Final contained sources missing.")?;
                ensure!(
                    r.offset_cents == 0
                        && r.outstanding_cents.is_none()
                        && r.reserved_cents == 0
                        && r.tax_adjustment_cents.is_none()
                        && r.cash_cents == if r.status == "settled" { net } else { 0 },
                    "Final statement/cash facts do not reconcile."
                );
                add(&mut t.final_statement_net_cents, net)?;
                add(&mut t.final_contained_source_net_cents, contained)?;
                add(&mut t.final_additional_net_cents, net - contained)?;
            } else {
                ensure!(
                    ["approved", "settled"].contains(&r.status.as_str())
                        && r.reserved_case_id.is_none()
                        && (r.status == "settled") == r.received_on.is_some(),
                    "Annual tax status/date invalid."
                );
                ensure!(
                    r.cash_cents == 0
                        && r.offset_cents == 0
                        && r.net_cents.is_none()
                        && r.outstanding_cents.is_none()
                        && r.reserved_cents == 0,
                    "Tax advice is not a wage or cash receipt."
                );
                let adj = r
                    .tax_adjustment_cents
                    .context("Tax advice adjustment missing.")?;
                add(&mut t.annual_advised_adjustment_cents, adj)?;
                if r.status == "settled" {
                    add(&mut t.annual_settled_adjustment_cents, adj)?;
                }
            }
        }
        Ok(Self { wire, totals: t })
    }
    pub fn csv(&self) -> Result<Vec<u8>> {
        let line = |cells: Vec<String>| -> Result<String> {
            Ok(cells
                .iter()
                .map(|s| crate::reports::csv_cell(s))
                .collect::<Result<Vec<_>>>()?
                .join(",")
                + "\r\n")
        };
        let mut out = String::from("\u{feff}");
        out += &line(
            [
                "contract",
                "company",
                "generated UTC",
                "date basis",
                "from",
                "through",
                "source kind",
                "employee filter",
                "search",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        )?;
        out += &line(vec![
            VERSION.into(),
            self.wire.company_name.clone(),
            self.wire.generated_at.to_rfc3339(),
            self.wire.filter.axis.clone(),
            self.wire.filter.starts_on.to_string(),
            self.wire.filter.ends_on.to_string(),
            self.wire.filter.kind.clone(),
            self.wire
                .filter
                .employee_id
                .map(|x| x.to_string())
                .unwrap_or_default(),
            self.wire.filter.search.clone(),
        ])?;
        out+=&line(vec!["Amounts are PHP. Source offsets are not cash; advice/settled tax adjustments remain separate. Balances are current at generation for selected issued sources.".into()])?;
        out += &line(
            [
                "kind",
                "record",
                "employee code",
                "employee name",
                "saved department",
                "period from",
                "period through",
                "approved on",
                "received/settled on",
                "status",
                "payment state",
                "basic",
                "gross",
                "employee contributions",
                "employer contributions",
                "withholding",
                "statement net",
                "contained source net",
                "recorded payroll cash",
                "source offset",
                "tax adjustment",
                "current source outstanding",
                "reserved subset",
                "reserved/final case",
                "rule version",
                "linked original kind",
                "linked original record",
                "overpayment review only",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        )?;
        let money = |v: Option<i64>| {
            v.map(|n| {
                format!(
                    "{}{}.{:02}",
                    if n < 0 { "-" } else { "" },
                    n.unsigned_abs() / 100,
                    n.unsigned_abs() % 100
                )
            })
            .unwrap_or_default()
        };
        for r in &self.wire.rows {
            out += &line(vec![
                r.kind.clone(),
                r.id.to_string(),
                r.employee_code.clone(),
                r.employee_name.clone(),
                r.department.clone(),
                r.period_start.to_string(),
                r.period_end.to_string(),
                r.issued_on.to_string(),
                r.received_on.map(|d| d.to_string()).unwrap_or_default(),
                r.status.clone(),
                r.payment_state.clone(),
                money(r.basic_cents),
                money(r.gross_cents),
                money(r.employee_contributions_cents),
                money(r.employer_contributions_cents),
                money(r.withholding_cents),
                money(r.net_cents),
                money(r.contained_source_net_cents),
                money(Some(r.cash_cents)),
                money(Some(r.offset_cents)),
                money(r.tax_adjustment_cents),
                money(r.outstanding_cents),
                money(Some(r.reserved_cents)),
                r.reserved_case_id
                    .map(|id| id.to_string())
                    .unwrap_or_default(),
                r.rule_version.clone(),
                r.linked_kind.clone().unwrap_or_default(),
                r.linked_id.map(|id| id.to_string()).unwrap_or_default(),
                money(r.review_amount_cents),
            ])?;
        }
        Ok(out.into_bytes())
    }
}
fn source_payment(data: &Snapshot, r: &mut Row, status: &str) -> Result<()> {
    r.status = status.into();
    let receipts: Vec<_> = data
        .payroll
        .receipts
        .iter()
        .filter(|p| p.company_id == r.company_id && p.kind == r.kind && p.slip_id == r.id)
        .collect();
    ensure!(receipts.len() <= 1, "Duplicate source receipts.");
    let net = r.net_cents.unwrap();
    if let Some(p) = receipts.first() {
        ensure!(
            p.employee_id == r.employee_id && p.amount_cents == net,
            "Receipt does not match issued source net."
        );
        r.received_on = Some(p.paid_on);
        r.outstanding_cents = Some(0);
        if p.origin == "final_settlement" {
            ensure!(p.final_case_id.is_some(), "Final receipt binding missing.");
            r.payment_state = "final_offset".into();
            r.offset_cents = net;
            r.reserved_case_id = p.final_case_id;
        } else {
            r.payment_state = "received".into();
            r.cash_cents = net;
        }
    } else if status == "paid" {
        r.payment_state = "receipt_missing".into();
    } else {
        r.outstanding_cents = Some(net);
        let cases: Vec<_> = data
            .payroll
            .finals
            .records
            .iter()
            .filter(|c| {
                c.company_id == r.company_id
                    && c.employee_id == r.employee_id
                    && c.status == "approved"
                    && c.source_basis["selected"].as_array().is_some_and(|ps| {
                        ps.iter()
                            .any(|p| p["kind"] == r.kind && p["slip_id"] == r.id.to_string())
                    })
            })
            .collect();
        ensure!(
            cases.len() <= 1,
            "A source is reserved by multiple final statements."
        );
        if let Some(c) = cases.first() {
            r.reserved_cents = net;
            r.reserved_case_id = Some(c.id);
            r.payment_state = "reserved".into();
        }
    }
    Ok(())
}
pub fn demo(data: &Snapshot, filter: &Filter, now: DateTime<Utc>) -> Result<Report> {
    authorize(data)?;
    filter.validate()?;
    if let Some(eid) = filter.employee_id {
        ensure!(
            data.employees
                .iter()
                .any(|e| e.id == eid && e.company_id == data.company.id),
            "Employee outside this company."
        );
    }
    let tz = data.company.timezone.parse::<chrono_tz::Tz>()?;
    let cid = data.company.id;
    let mut rows = vec![];
    for s in data.payroll.slips.iter().filter(|s| s.company_id == cid) {
        let t = &s.item.totals;
        let mut r = Row::empty(
            cid,
            s.id,
            s.employee_id,
            "monthly",
            &s.item.employee_code,
            &s.item.employee_name,
            &s.item.department,
            s.month,
            month_end(s.month),
            s.approved_at.with_timezone(&tz).date_naive(),
            &s.rule_version,
        );
        regular(&mut r, t);
        let run = data
            .payroll
            .runs
            .iter()
            .find(|p| p.id == s.run_id && p.company_id == cid)
            .context("Issued monthly status missing.")?;
        source_payment(data, &mut r, &run.status)?;
        rows.push(r);
    }
    for s in data
        .payroll
        .cutoffs
        .slips
        .iter()
        .filter(|s| s.base.company_id == cid)
    {
        let p = &s.base;
        let mut r = Row::empty(
            cid,
            p.id,
            p.employee_id,
            "cutoff",
            &p.item.employee_code,
            &p.item.employee_name,
            &p.item.department,
            s.starts_on,
            s.ends_on,
            p.approved_at.with_timezone(&tz).date_naive(),
            &p.rule_version,
        );
        regular(&mut r, &p.item.totals);
        let run = data
            .payroll
            .cutoffs
            .runs
            .iter()
            .find(|x| x.base.id == p.run_id && x.base.company_id == cid)
            .context("Issued cutoff status missing.")?;
        source_payment(data, &mut r, &run.base.status)?;
        rows.push(r);
    }
    for s in data
        .payroll
        .thirteenth
        .slips
        .iter()
        .filter(|s| s.company_id == cid)
    {
        let t = &s.item.totals;
        let mut r = Row::empty(
            cid,
            s.id,
            s.employee_id,
            "thirteenth",
            &s.item.employee_code,
            &s.item.employee_name,
            &s.item.department,
            NaiveDate::from_ymd_opt(s.year, 1, 1).unwrap(),
            s.as_of,
            s.approved_at.with_timezone(&tz).date_naive(),
            &s.rule_version,
        );
        r.basic_cents = None;
        r.gross_cents = Some(t.gross_cents);
        r.net_cents = Some(t.net_cents);
        r.employee_contributions_cents = Some(
            t.sss_adjustment_cents + t.philhealth_adjustment_cents + t.pagibig_adjustment_cents,
        );
        r.employer_contributions_cents = None;
        r.withholding_cents = Some(t.withholding_cents);
        let run = data
            .payroll
            .thirteenth
            .runs
            .iter()
            .find(|x| x.id == s.run_id && x.company_id == cid)
            .context("Issued13th status missing.")?;
        source_payment(data, &mut r, &run.status)?;
        rows.push(r);
    }
    for s in data
        .payroll
        .tax
        .slips
        .iter()
        .filter(|s| s.company_id == cid)
    {
        let c = data
            .payroll
            .tax
            .records
            .iter()
            .find(|c| c.id == s.id && c.company_id == cid)
            .context("Issued tax status missing.")?;
        let mut r = Row::empty(
            cid,
            s.id,
            s.employee_id,
            "annual_tax",
            &s.employee_code,
            &s.employee_name,
            &s.department,
            NaiveDate::from_ymd_opt(s.year, 1, 1)
                .unwrap()
                .min(s.service_end),
            s.service_end,
            s.approved_at.with_timezone(&tz).date_naive(),
            &s.rule_version,
        );
        r.status = c.status.clone();
        r.payment_state = if c.status == "settled" {
            "settled_adjustment"
        } else {
            "advice_only"
        }
        .into();
        r.received_on = c.settled_on;
        r.tax_adjustment_cents = Some(s.totals.adjustment_cents);
        rows.push(r);
    }
    for s in data
        .payroll
        .finals
        .slips
        .iter()
        .filter(|s| s.company_id == cid)
    {
        let c = data
            .payroll
            .finals
            .records
            .iter()
            .find(|c| c.id == s.id && c.company_id == cid)
            .context("Issued final status missing.")?;
        let mut r = Row::empty(
            cid,
            s.id,
            s.employee_id,
            "final",
            &s.employee_code,
            &s.employee_name,
            &s.department,
            s.employment_start,
            s.service_end,
            s.approved_at.with_timezone(&tz).date_naive(),
            &s.rule_version,
        );
        r.status = c.status.clone();
        r.payment_state = c.status.clone();
        r.net_cents = Some(s.totals.net_cents);
        r.contained_source_net_cents = Some(s.totals.source_net_cents);
        r.received_on = c.settled_on;
        if c.status == "settled" {
            r.cash_cents = s.totals.net_cents;
        }
        rows.push(r);
    }
    for s in data
        .payroll
        .amendments
        .slips
        .iter()
        .filter(|s| s.company_id == cid)
    {
        let c = data
            .payroll
            .amendments
            .records
            .iter()
            .find(|c| c.id == s.id && c.company_id == cid)
            .context("Issued amendment status missing.")?;
        let mut r = Row::empty(
            cid,
            s.id,
            s.employee_id,
            "amendment",
            &s.employee_code,
            &s.employee_name,
            &s.department,
            s.starts_on,
            s.ends_on,
            s.approved_at.with_timezone(&tz).date_naive(),
            &s.rule_version,
        );
        r.status = c.status.clone();
        r.payment_state = c.status.clone();
        r.basic_cents = Some(s.totals.basic_cents);
        r.gross_cents = Some(s.totals.gross_cents);
        r.employee_contributions_cents = Some(s.totals.contribution_cents);
        r.withholding_cents = Some(s.totals.withholding_cents);
        r.net_cents = Some(s.totals.net_cents);
        r.linked_kind = Some(s.target_kind.clone());
        r.linked_id = Some(s.target_id);
        if s.purpose == "review_only" {
            r.review_amount_cents = Some(s.totals.review_amount_cents);
        } else {
            r.outstanding_cents = Some(if c.status == "approved" {
                s.totals.net_cents
            } else {
                0
            });
        }
        if c.status == "paid" {
            r.received_on = c.paid_on;
            r.cash_cents = s.totals.net_cents;
        }
        rows.push(r);
    }
    rows.retain(|r| filter.matches(r));
    rows.sort_by_key(|r| {
        (
            std::cmp::Reverse(if filter.axis == "received" {
                r.received_on.unwrap()
            } else {
                r.issued_on
            }),
            r.employee_code.clone(),
            r.kind.clone(),
            r.id,
        )
    });
    Report::from_wire(
        Wire {
            version: VERSION.into(),
            company_name: data.company.name.clone(),
            company_id: cid,
            generated_at: now,
            filter: filter.clone(),
            rows,
        },
        cid,
        filter,
    )
}
fn regular(r: &mut Row, t: &crate::payroll::Totals) {
    r.basic_cents = Some(t.basic);
    r.gross_cents = Some(t.gross);
    r.net_cents = Some(t.net);
    r.withholding_cents = Some(t.withholding);
    r.employee_contributions_cents =
        Some(t.sss_employee + t.philhealth_employee + t.pagibig_employee);
    r.employer_contributions_cents =
        Some(t.sss_employer + t.philhealth_employer + t.pagibig_employer + t.ec);
}
