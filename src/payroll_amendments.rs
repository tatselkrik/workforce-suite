//! Linked supplemental pay and overpayment reviews. Original slips and receipts never change.
use crate::{
    access::Role,
    domain::{Mode, Snapshot},
    payroll::PayrollAction,
};
use anyhow::{Context, Result, ensure};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;
pub const RULE_VERSION: &str = "PH-payroll-amendment-2026-10-24-v1";
const LIMIT: i64 = 100_000_000_000;
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Worksheet {
    /// payout or review_only. A review is never a collectible employee debt.
    pub purpose: String,
    pub basic_cents: i64,
    /// Additional regular taxable earnings BEFORE additional mandatory contributions.
    pub taxable_cents: i64,
    pub shared_cents: i64,
    pub other_benefits_cents: i64,
    pub exempt_cents: i64,
    pub sss_cents: i64,
    pub philhealth_cents: i64,
    pub pagibig_cents: i64,
    /// Signed: positive withholding, negative reviewed same-year refund.
    pub withholding_cents: i64,
    pub lawful_deduction_cents: i64,
    pub review_amount_cents: i64,
    pub evidence: String,
    pub tax_note: String,
    pub contribution_note: String,
    pub review_note: String,
    pub reviewed: bool,
    pub tax_reviewed: bool,
    pub contributions_reviewed: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Calculation {
    pub basic_cents: i64,
    pub gross_cents: i64,
    pub taxable_cents: i64,
    pub shared_cents: i64,
    pub benefit_exempt_cents: i64,
    pub contribution_cents: i64,
    pub withholding_cents: i64,
    pub deduction_cents: i64,
    pub net_cents: i64,
    pub review_amount_cents: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub kind: String,
    pub slip_id: Uuid,
    pub employee_id: Uuid,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    pub paid_on: NaiveDate,
    pub cash_date: NaiveDate,
    pub original_net_cents: i64,
    pub sss_refundable_cents: i64,
    pub philhealth_refundable_cents: i64,
    pub pagibig_refundable_cents: i64,
    pub withholding_refundable_cents: i64,
    pub shared_floor_cents: i64,
}
pub fn text(s: &str, max: usize, required: bool) -> Result<()> {
    ensure!(
        s.chars().count() <= max
            && (!required || !s.trim().is_empty())
            && !s.chars().any(|c| c.is_control() && c != '\n' && c != '\t'),
        "Provide bounded readable amendment details and evidence."
    );
    Ok(())
}
pub fn calculate(s: &Source, w: &Worksheet) -> Result<Calculation> {
    ensure!(
        ["monthly", "cutoff", "thirteenth", "final"].contains(&s.kind.as_str())
            && !s.slip_id.is_nil()
            && !s.employee_id.is_nil()
            && s.starts_on <= s.ends_on
            && s.ends_on <= s.cash_date
            && s.paid_on <= s.cash_date
            && s.cash_date.year() == 2026
            && s.cash_date >= NaiveDate::from_ymd_opt(2026, 1, 6).unwrap(),
        "Link an actually paid original slip and use the reviewed 2026 payment year from 6 January."
    );
    for v in [
        s.original_net_cents,
        s.sss_refundable_cents,
        s.philhealth_refundable_cents,
        s.pagibig_refundable_cents,
        s.withholding_refundable_cents,
        s.shared_floor_cents,
        w.basic_cents,
        w.taxable_cents,
        w.shared_cents,
        w.other_benefits_cents,
        w.exempt_cents,
        w.lawful_deduction_cents,
    ] {
        ensure!(
            (0..=LIMIT).contains(&v),
            "Use bounded nonnegative centavos for owed earnings and source balances."
        );
    }
    for v in [
        w.sss_cents,
        w.philhealth_cents,
        w.pagibig_cents,
        w.withholding_cents,
        w.review_amount_cents,
    ] {
        ensure!(
            (-LIMIT..=LIMIT).contains(&v),
            "Signed amendment amounts exceed the supported limit."
        );
    }
    for v in [
        &w.evidence,
        &w.tax_note,
        &w.contribution_note,
        &w.review_note,
    ] {
        text(v, 2000, false)?;
    }
    ensure!(
        ["payout", "review_only"].contains(&w.purpose.as_str()),
        "Choose supplemental payout or overpayment review."
    );
    if w.purpose == "review_only" {
        ensure!(
            w.review_amount_cents <= 0
                && w.review_amount_cents >= -s.original_net_cents
                && [
                    w.basic_cents,
                    w.taxable_cents,
                    w.shared_cents,
                    w.other_benefits_cents,
                    w.exempt_cents,
                    w.sss_cents,
                    w.philhealth_cents,
                    w.pagibig_cents,
                    w.withholding_cents,
                    w.lawful_deduction_cents
                ]
                .iter()
                .all(|n| *n == 0),
            "Overpayment reviews contain only a negative reviewed amount. They never reduce wages, tax or contributions or create a collectible debt."
        );
        return Ok(Calculation {
            review_amount_cents: w.review_amount_cents,
            ..Default::default()
        });
    }
    ensure!(
        w.review_amount_cents == 0 && w.basic_cents <= w.taxable_cents,
        "Basic pay is part of additional taxable earnings; review-only amounts do not belong in payouts."
    );
    ensure!(
        (w.basic_cents == 0 || ["monthly", "cutoff"].contains(&s.kind.as_str()))
            && (s.kind != "thirteenth" || w.taxable_cents == 0),
        "Basic corrections link salary/cutoff slips; additional 13th-month earnings use the shared-benefit amount."
    );
    for (delta, available) in [
        (w.sss_cents, s.sss_refundable_cents),
        (w.philhealth_cents, s.philhealth_refundable_cents),
        (w.pagibig_cents, s.pagibig_refundable_cents),
        (w.withholding_cents, s.withholding_refundable_cents),
    ] {
        ensure!(
            delta >= -available,
            "Refund exceeds the original charge plus earlier approved linked adjustments."
        );
        ensure!(
            delta >= 0 || s.paid_on.year() == s.cash_date.year(),
            "Prior-year withholding/contribution refunds need a separate year-specific correction; do not credit them against current-year income."
        );
    }
    ensure!(
        w.shared_cents == 0 || w.other_benefits_cents >= s.shared_floor_cents,
        "Retain all known annual shared-benefit facts before allocating another exclusion."
    );
    let contributions = w.sss_cents + w.philhealth_cents + w.pagibig_cents;
    ensure!(
        w.taxable_cents >= contributions.max(0),
        "Additional employee contributions cannot exceed the additional regular taxable earnings; review any source reclassification separately."
    );
    let exempt = w
        .shared_cents
        .min((crate::payroll_tax::BENEFIT_CAP - w.other_benefits_cents).max(0));
    let gross = w.taxable_cents + w.shared_cents + w.exempt_cents;
    let net = gross - contributions - w.withholding_cents - w.lawful_deduction_cents;
    ensure!(
        (0..=LIMIT).contains(&gross)
            && (0..=LIMIT).contains(&net)
            && (0..=LIMIT).contains(&(w.taxable_cents + w.shared_cents - exempt - contributions))
            && (-LIMIT..=LIMIT).contains(&contributions),
        "Use bounded supplemental income with nonnegative cash. An overpayment needs its separate review, without automatic recovery."
    );
    Ok(Calculation {
        basic_cents: w.basic_cents,
        gross_cents: gross,
        taxable_cents: w.taxable_cents + w.shared_cents - exempt - contributions,
        shared_cents: w.shared_cents,
        benefit_exempt_cents: exempt,
        contribution_cents: contributions,
        withholding_cents: w.withholding_cents,
        deduction_cents: w.lawful_deduction_cents,
        net_cents: net,
        review_amount_cents: 0,
    })
}
pub fn ready(s: &Source, w: &Worksheet) -> Result<Calculation> {
    let c = calculate(s, w)?;
    ensure!(
        if w.purpose == "review_only" {
            w.review_amount_cents < 0
        } else {
            [
                w.taxable_cents,
                w.shared_cents,
                w.exempt_cents,
                w.sss_cents,
                w.philhealth_cents,
                w.pagibig_cents,
                w.withholding_cents,
                w.lawful_deduction_cents,
            ]
            .iter()
            .any(|n| *n != 0)
        },
        "A reviewed amendment must contain a correction."
    );
    ensure!(
        w.reviewed && !w.evidence.trim().is_empty() && !w.review_note.trim().is_empty(),
        "Review the linked original, earlier amendments, owed difference and legal/evidence basis."
    );
    ensure!(
        w.purpose == "review_only"
            || (w.tax_reviewed
                && w.contributions_reviewed
                && !w.tax_note.trim().is_empty()
                && !w.contribution_note.trim().is_empty()),
        "Review supplementary/cumulative withholding, benefit classification and original-month contribution reconciliation, including zero. Amounts are reviewed corrections, not a second full statutory charge."
    );
    Ok(c)
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Record {
    pub id: Uuid,
    pub company_id: Uuid,
    pub employee_id: Uuid,
    pub target_kind: String,
    pub target_id: Uuid,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    pub cash_date: NaiveDate,
    pub title: String,
    pub employee_name: String,
    pub employee_code: String,
    pub department: String,
    pub status: String,
    pub revision: i64,
    pub created_by: Uuid,
    pub prepared_by: Uuid,
    pub submitted_by: Option<Uuid>,
    pub approved_by: Option<Uuid>,
    pub approved_at: Option<DateTime<Utc>>,
    pub paid_on: Option<NaiveDate>,
    pub payment_reference: String,
    pub source_basis: Value,
    pub worksheet: Worksheet,
    pub totals: Calculation,
    pub rule_version: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Slip {
    pub id: Uuid,
    pub company_id: Uuid,
    pub employee_id: Uuid,
    pub target_kind: String,
    pub target_id: Uuid,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    pub cash_date: NaiveDate,
    pub title: String,
    pub company_name: String,
    pub employee_name: String,
    pub employee_code: String,
    pub department: String,
    pub purpose: String,
    pub totals: Calculation,
    pub approved_by: Uuid,
    pub approved_at: DateTime<Utc>,
    pub rule_version: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Status {
    pub id: Uuid,
    pub company_id: Uuid,
    pub employee_id: Uuid,
    pub kind: String,
    pub recorded_on: NaiveDate,
    pub net_cents: i64,
}
#[derive(Clone, Debug, Default)]
pub struct Setup {
    pub records: Vec<Record>,
    pub slips: Vec<Slip>,
    pub statuses: Vec<Status>,
}
pub fn source(r: &Record) -> Result<Source> {
    Ok(serde_json::from_value(
        r.source_basis["calculation_source"].clone(),
    )?)
}
pub fn payload(a: &PayrollAction, data: &Snapshot) -> Result<Value> {
    ensure!(
        data.mode != Mode::SignedOut
            && data.access.company_id == data.company.id
            && data
                .access
                .allows(crate::staff_roles::Capability::PayrollPreparation),
        "Only HR and Administrators manage payroll amendments."
    );
    let op = a
        .operation
        .strip_prefix("amendment_")
        .context("Unknown amendment operation.")?;
    let fields: &[&str] = match op {
        "create" => &["target_kind", "target_id", "cash_date", "title", "purpose"],
        "save" => &["worksheet"],
        "pay" => &["paid_on", "payment_reference", "received_confirmed"],
        "withdraw" => &["unpaid_confirmed"],
        "refresh" | "submit" | "return" | "cancel" | "approve" => &[],
        _ => anyhow::bail!("Unknown amendment operation."),
    };
    let v = &a.record;
    let obj = v.as_object().context("Enter amendment details.")?;
    ensure!(
        v.to_string().len() <= 65536
            && obj.keys().all(
                |k| ["id", "expected_revision", "reason"].contains(&k.as_str())
                    || fields.contains(&k.as_str())
            ),
        "Unsupported amendment fields."
    );
    let id: Uuid = serde_json::from_value(v["id"].clone())?;
    ensure!(!id.is_nil(), "Amendment identity required.");
    text(v["reason"].as_str().context("Give a reason.")?, 500, true)?;
    if op == "create" {
        let target: Uuid = serde_json::from_value(v["target_id"].clone())?;
        ensure!(
            !target.is_nil()
                && ["monthly", "cutoff", "thirteenth", "final"]
                    .contains(&v["target_kind"].as_str().unwrap_or(""))
                && ["payout", "review_only"].contains(&v["purpose"].as_str().unwrap_or(""))
                && v["expected_revision"].is_null(),
            "Select a paid original slip and amendment purpose."
        );
        let d: NaiveDate = serde_json::from_value(v["cash_date"].clone())?;
        ensure!(
            d.year() == 2026 && d >= NaiveDate::from_ymd_opt(2026, 1, 6).unwrap(),
            "Use reviewed 2026 payment dates from 6 January."
        );
        text(v["title"].as_str().context("Give a title.")?, 120, true)?;
    } else {
        ensure!(
            v["expected_revision"].as_i64().is_some_and(|n| n >= 0),
            "Refresh the amendment revision."
        );
    }
    if op == "save" {
        let r = data
            .payroll
            .amendments
            .records
            .iter()
            .find(|r| r.id == id && r.company_id == data.company.id)
            .context("Refresh this amendment.")?;
        let w: Worksheet = serde_json::from_value(v["worksheet"].clone())?;
        ensure!(
            w.purpose == r.worksheet.purpose,
            "Amendment purpose cannot change."
        );
        calculate(&source(r)?, &w)?;
    }
    if ["approve", "pay", "withdraw"].contains(&op) {
        ensure!(
            data.access.role == Role::Administrator,
            "Only Administrators approve or record payment/withdrawal."
        );
    }
    if op == "pay" {
        let _: NaiveDate = serde_json::from_value(v["paid_on"].clone())?;
        text(
            v["payment_reference"]
                .as_str()
                .context("Give an actual payment reference.")?,
            500,
            true,
        )?;
        ensure!(
            v["received_confirmed"] == true,
            "Confirm actual supplemental cash received, including zero."
        );
    }
    if op == "withdraw" {
        ensure!(
            v["unpaid_confirmed"] == true,
            "Confirm this supplemental payout was never paid."
        );
    }
    Ok(json!({"p_operation":op,"p_record":v}))
}
mod sources;
mod workflow;
pub(crate) use sources::{annual as augment_annual, benefits as augment_benefits};
pub use workflow::{Target, apply_demo, demo_basis, targets};
pub fn totals_rows(t: &Calculation) -> Vec<(&'static str, i64)> {
    vec![
        ("Additional basic (included in earnings)", t.basic_cents),
        ("Supplemental gross", t.gross_cents),
        ("Taxable after contributions / exclusions", t.taxable_cents),
        ("Shared benefit gross", t.shared_cents),
        ("Shared benefit exclusion allocated", t.benefit_exempt_cents),
        (
            "Employee contributions (+) / refund (-)",
            t.contribution_cents,
        ),
        ("Withholding (+) / refund (-)", t.withholding_cents),
        ("Lawful deductions", t.deduction_cents),
        ("Supplemental cash", t.net_cents),
    ]
}
pub fn status(data: &Snapshot, id: Uuid) -> Option<&Status> {
    data.payroll
        .amendments
        .statuses
        .iter()
        .find(|s| s.id == id && s.company_id == data.company.id)
}
pub fn export_csv(data: &Snapshot, kind: &str, id: Option<Uuid>) -> Result<Vec<u8>> {
    ensure!(
        kind == "amendment_statement"
            && data.mode != Mode::SignedOut
            && data.access.company_id == data.company.id,
        "Choose an issued amendment statement."
    );
    let s = data
        .payroll
        .amendments
        .slips
        .iter()
        .find(|s| {
            Some(s.id) == id
                && s.company_id == data.company.id
                && (data
                    .access
                    .allows(crate::staff_roles::Capability::PayrollPreparation)
                    || data.access.employee_id == Some(s.employee_id))
        })
        .context("Only an available own issued amendment may be exported.")?;
    let mut rows = vec![
        ("Company", s.company_name.clone()),
        ("Employee code", s.employee_code.clone()),
        ("Employee name", s.employee_name.clone()),
        ("Department", s.department.clone()),
        ("Statement", s.title.clone()),
        ("Statement ID", s.id.to_string()),
        ("Original kind", s.target_kind.clone()),
        ("Original ID", s.target_id.to_string()),
        ("Earned from", s.starts_on.to_string()),
        ("Earned through", s.ends_on.to_string()),
        ("Planned supplemental payment", s.cash_date.to_string()),
        ("Purpose", s.purpose.clone()),
    ];
    if s.purpose == "review_only" {
        rows.push((
            "Overpayment discrepancy review only",
            crate::payroll::money(s.totals.review_amount_cents),
        ));
    } else {
        rows.extend(
            totals_rows(&s.totals)
                .into_iter()
                .map(|(label, n)| (label, crate::payroll::money(n))),
        );
    }
    if let Some(st) = status(data, s.id) {
        rows.push(("Recorded status", st.kind.clone()));
        rows.push(("Recorded date", st.recorded_on.to_string()));
        rows.push((
            "Recorded supplemental cash",
            crate::payroll::money(st.net_cents),
        ));
    }
    rows.push(("Meaning","Original payslip/receipt unchanged. Supplemental cash only; a discrepancy review creates no automatic recovery or employee debt. No transfer, remittance or filing was made.".into()));
    let mut out = "\u{feff}Field,Value\r\n".to_owned();
    for (k, v) in rows {
        out.push_str(&crate::reports::csv_cell(k)?);
        out.push(',');
        out.push_str(&crate::reports::csv_cell(&v)?);
        out.push_str("\r\n");
    }
    Ok(out.into_bytes())
}
