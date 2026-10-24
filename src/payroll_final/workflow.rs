use super::*;
use crate::{
    access::Role,
    domain::{Mode, Snapshot},
    payroll::PayrollAction,
};
use anyhow::Context;
use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use uuid::Uuid;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Record {
    pub id: Uuid,
    pub company_id: Uuid,
    pub employee_id: Uuid,
    pub employment_start: NaiveDate,
    pub service_end: NaiveDate,
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
    pub settled_on: Option<NaiveDate>,
    pub payment_reference: String,
    pub handling_reference: String,
    pub rule_version: String,
    pub source_basis: Value,
    pub worksheet: Worksheet,
    #[serde(default)]
    pub cash_year_reviewed: bool,
    #[serde(default)]
    pub cash_year_note: String,
    pub totals: Calculation,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Slip {
    pub id: Uuid,
    pub company_id: Uuid,
    pub employee_id: Uuid,
    pub employment_start: NaiveDate,
    pub service_end: NaiveDate,
    pub cash_date: NaiveDate,
    pub company_name: String,
    pub employee_name: String,
    pub employee_code: String,
    pub department: String,
    pub totals: Calculation,
    pub sources: Value,
    pub components: Value,
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
fn seed(s: &Source) -> Worksheet {
    let mut w = Worksheet::default();
    w.vacation.previous_days_hundredths = s.previous_vacation_days_hundredths;
    if s.previous_vacation_days_hundredths > 0 {
        w.vacation.reference =
            "Earlier recorded vacation conversion retained. Review the original evidence.".into();
    }
    w
}
pub fn reset_reviews(w: &mut Worksheet) {
    w.cash_year_reviewed = false;
    w.entitlements_reviewed = false;
    w.source_scope_reviewed = false;
    w.contributions_reviewed = false;
    w.review_note.clear();
    w.tax.final_compensation_reviewed = false;
    w.tax.previous_employers_reviewed = false;
    w.tax.benefits_reviewed = false;
    w.tax.review_note.clear();
}
fn field(v: &Value, k: &str, max: usize, required: bool) -> Result<()> {
    let value = v[k]
        .as_str()
        .context("Enter the required final-pay details.")?;
    ensure!(
        !required || !value.trim().is_empty(),
        "{}",
        match k {
            "reason" => "Enter a reason for this action.",
            "payment_reference" => "Enter the actual payment reference.",
            "handling_reference" => "Enter the employer tax-handling reference.",
            "title" => "Enter a title for this review.",
            _ => "Complete the required final-pay details.",
        }
    );
    text(value, max, required)
}

pub fn payload(a: &PayrollAction, data: &Snapshot) -> Result<Value> {
    ensure!(
        data.mode != Mode::SignedOut
            && data.access.company_id == data.company.id
            && data.access.role.manages_hr(),
        "Only HR and Administrators manage final pay."
    );
    let op = a
        .operation
        .strip_prefix("final_")
        .context("Unknown final-pay operation.")?;
    let fields: &[&str] = match op {
        "create" => &["employee_id", "service_end", "cash_date", "title"],
        "save" => &["worksheet"],
        "settle" => &[
            "settled_on",
            "payment_reference",
            "handling_reference",
            "cash_received_confirmed",
            "employer_handling_confirmed",
        ],
        "withdraw" => &["unpaid_confirmed"],
        "refresh" | "submit" | "return" | "cancel" | "approve" => &[],
        _ => anyhow::bail!("Unknown final-pay operation."),
    };
    let v = &a.record;
    let o = v.as_object().context("Enter final-pay details.")?;
    ensure!(
        v.to_string().len() <= 262144
            && o.keys().all(
                |k| ["id", "expected_revision", "reason"].contains(&k.as_str())
                    || fields.contains(&k.as_str())
            ),
        "Unsupported final-pay fields."
    );
    let id: Uuid = serde_json::from_value(v["id"].clone())?;
    ensure!(!id.is_nil(), "Final-pay identity required.");
    field(v, "reason", 500, true)?;
    if op == "create" {
        ensure!(
            v["expected_revision"].is_null(),
            "New final-pay cases must not have a revision."
        );
        let eid: Uuid = serde_json::from_value(v["employee_id"].clone())?;
        ensure!(!eid.is_nil(), "Employee required.");
        let end: NaiveDate = serde_json::from_value(v["service_end"].clone())?;
        let cash: NaiveDate = serde_json::from_value(v["cash_date"].clone())?;
        ensure!(
            end >= NaiveDate::from_ymd_opt(2025, 1, 1).unwrap()
                && cash.year() == 2026
                && cash >= NaiveDate::from_ymd_opt(2026, 1, 6).unwrap()
                && end <= cash,
            "Use a completed 2025/2026 separation and a reviewed2026 cash date from6January."
        );
        field(v, "title", 120, true)?;
    } else {
        ensure!(
            v["expected_revision"].as_i64().is_some_and(|n| n >= 0),
            "Refresh the final-pay revision."
        );
    }
    if op == "save" {
        let r = data
            .payroll
            .finals
            .records
            .iter()
            .find(|r| r.id == id && r.company_id == data.company.id)
            .context("Refresh this final-pay case.")?;
        let w: Worksheet = serde_json::from_value(v["worksheet"].clone())?;
        calculate(&source(r)?, &w)?;
    }
    if ["approve", "settle", "withdraw"].contains(&op) {
        ensure!(
            data.access.role == Role::Administrator,
            "Only Administrators approve or record final settlement/withdrawal."
        );
    }
    if op == "settle" {
        let _: NaiveDate = serde_json::from_value(v["settled_on"].clone())?;
        field(v, "payment_reference", 500, true)?;
        field(v, "handling_reference", 1000, false)?;
        ensure!(
            v["cash_received_confirmed"] == true && v["employer_handling_confirmed"] == true,
            "Confirm actual combined cash and tax handling, including zero."
        );
    }
    if op == "withdraw" {
        ensure!(
            v["unpaid_confirmed"] == true,
            "Confirm that no final cash/tax settlement occurred."
        );
    }
    Ok(json!({"p_operation":op,"p_record":v}))
}
fn tax_record(r: &Record) -> tax::Record {
    tax::Record {
        id: r.id,
        company_id: r.company_id,
        employee_id: r.employee_id,
        year: r.cash_date.year(),
        kind: "termination".into(),
        as_of: r.cash_date,
        service_end: r.service_end,
        title: r.title.clone(),
        employee_name: r.employee_name.clone(),
        employee_code: r.employee_code.clone(),
        department: r.department.clone(),
        status: "draft".into(),
        revision: 0,
        created_by: r.created_by,
        submitted_by: None,
        approved_by: None,
        approved_at: None,
        settled_on: None,
        settlement_reference: String::new(),
        rule_version: tax::RULE_VERSION.into(),
        source_basis: Value::Null,
        worksheet: tax::Worksheet::default(),
        totals: tax::Calculation::default(),
    }
}
fn external_other(data: &Snapshot, eid: Uuid, year: i32, asof: Option<NaiveDate>) -> i64 {
    data.payroll
        .thirteenth
        .slips
        .iter()
        .filter(|s| s.company_id == data.company.id && s.employee_id == eid && s.year == year)
        .filter(|s| {
            asof.is_none_or(|d| {
                data.payroll
                    .thirteenth
                    .runs
                    .iter()
                    .find(|r| r.id == s.run_id)
                    .is_some_and(|r| {
                        let (pd, paid) = crate::payroll_payments::effective(
                            data,
                            "thirteenth",
                            s.id,
                            r.paid_on,
                            &r.status,
                        );
                        s.as_of <= d && paid && pd.is_some_and(|p| p.year() == year && p <= d)
                    })
            })
        })
        .map(|s| {
            let known = data
                .payroll
                .thirteenth
                .items
                .iter()
                .find(|i| i.id == s.id)
                .and_then(|i| i.source_basis["final_shared_benefits_cents"].as_i64())
                .unwrap_or(0);
            (s.item.worksheet.other_benefits_cents - known).max(0)
        })
        .max()
        .unwrap_or(0)
}
pub(crate) fn augment_annual_demo_basis(
    data: &Snapshot,
    r: &tax::Record,
    mut b: Value,
) -> Result<Value> {
    let mut s: tax::Source = serde_json::from_value(b["calculation_source"].clone())?;
    let mut facts = vec![];
    for p in data.payroll.finals.records.iter().filter(|p| {
        p.company_id == data.company.id
            && p.employee_id == r.employee_id
            && p.id != r.id
            && p.cash_date.year() == r.year
            && ["approved", "settled"].contains(&p.status.as_str())
    }) {
        ensure!(
            p.cash_date <= r.as_of,
            "Use an as-of date at least as late as the earlier final statement."
        );
        if p.status == "approved" {
            s.pending_adjustments += 1;
        } else {
            s.income.push(tax::Income {
                id: format!("final:{}", p.id),
                starts_on: None,
                ends_on: None,
                taxable_cents: p.totals.taxable_extras_cents + p.totals.shared_extras_cents,
                withholding_cents: 0,
                benefit_gross_cents: p.totals.shared_extras_cents,
                benefit_exempt_cents: 0,
                contribution_cents: p.totals.additional_contributions_cents,
            });
            s.prior_adjustment_cents += p.totals.tax.adjustment_cents;
        }
        facts.push(json!({"id":p.id,"status":p.status,"cash_date":p.cash_date,"revision":p.revision,"totals":p.totals}));
    }
    facts.sort_by_key(|p| p["id"].as_str().unwrap().to_owned());
    s.other_benefits_floor = external_other(data, r.employee_id, r.year, Some(r.as_of));
    b["calculation_source"] = serde_json::to_value(s)?;
    b["final_payments"] = json!(facts);
    Ok(b)
}
pub(crate) fn augment_benefit_demo_basis(
    data: &Snapshot,
    eid: Uuid,
    r: &crate::payroll_thirteenth::Run,
    mut b: Value,
) -> Result<Value> {
    let mut facts = vec![];
    let mut known = 0;
    for p in data.payroll.finals.records.iter().filter(|p| {
        p.company_id == data.company.id
            && p.employee_id == eid
            && p.cash_date.year() == r.year
            && p.status == "settled"
    }) {
        known += p.totals.shared_extras_cents;
        facts.push(
            json!({"id":p.id,"cash_date":p.cash_date,"shared_cents":p.totals.shared_extras_cents}),
        );
    }
    facts.sort_by_key(|p| p["id"].as_str().unwrap().to_owned());
    b["calculation_source"]["other_benefits_floor"] =
        json!(external_other(data, eid, r.year, None) + known);
    b["final_shared_benefits_cents"] = json!(known);
    b["final_benefits"] = json!(facts);
    Ok(b)
}
fn candidate_income(
    data: &Snapshot,
    p: &crate::payroll_payments::Candidate,
    lo: NaiveDate,
    end: NaiveDate,
) -> Result<(tax::Income, i64, i64)> {
    let (mut gross, mut exempt, mut contribution, mut ext, mut other) = (0, 0, 0, 0, 0);
    let (first, last, taxable, withheld, coverage) = match p.kind.as_str() {
        "monthly" => {
            let s = data
                .payroll
                .slips
                .iter()
                .find(|s| s.id == p.slip_id)
                .context("Refresh salary source.")?;
            (
                s.month,
                payroll::month_end(s.month),
                s.item.totals.taxable,
                s.item.totals.withholding,
                true,
            )
        }
        "cutoff" => {
            let s = data
                .payroll
                .cutoffs
                .slips
                .iter()
                .find(|s| s.base.id == p.slip_id)
                .context("Refresh cutoff source.")?;
            (
                s.starts_on,
                s.ends_on,
                s.base.item.totals.taxable,
                s.base.item.totals.withholding,
                true,
            )
        }
        _ => {
            let s = data
                .payroll
                .thirteenth
                .slips
                .iter()
                .find(|s| s.id == p.slip_id)
                .context("Refresh13th source.")?;
            let t = &s.item.totals;
            gross = t.gross_cents;
            exempt = t.exempt_cents;
            contribution =
                t.sss_adjustment_cents + t.philhealth_adjustment_cents + t.pagibig_adjustment_cents;
            ext = s.item.worksheet.external_thirteenth_cents;
            let known = data
                .payroll
                .thirteenth
                .items
                .iter()
                .find(|i| i.id == s.id)
                .and_then(|i| i.source_basis["final_shared_benefits_cents"].as_i64())
                .unwrap_or(0);
            other = (s.item.worksheet.other_benefits_cents - known).max(0);
            (
                s.as_of,
                s.as_of,
                t.taxable_cents,
                t.withholding_cents,
                false,
            )
        }
    };
    let (a, b) = (first.max(lo), last.min(end));
    Ok((
        tax::Income {
            id: format!("{}:{}", p.kind, p.slip_id),
            starts_on: if coverage && a <= b { Some(a) } else { None },
            ends_on: if coverage && a <= b { Some(b) } else { None },
            taxable_cents: taxable,
            withholding_cents: withheld,
            benefit_gross_cents: gross,
            benefit_exempt_cents: exempt,
            contribution_cents: contribution,
        },
        ext,
        other,
    ))
}
pub fn demo_basis(data: &Snapshot, r: &Record) -> Result<Value> {
    let e = data
        .employees
        .iter()
        .find(|e| e.id == r.employee_id)
        .context("Employee unavailable.")?;
    let start = e
        .joined_on
        .context("Set the evidenced employment start before final pay.")?;
    let mut b = tax::demo_basis(data, &tax_record(r))?;
    let mut annual: tax::Source = serde_json::from_value(b["calculation_source"].clone())?;
    let mut outstanding = vec![];
    let mut selected = vec![];
    let mut candidates = crate::payroll_payments::candidates(data)?
        .into_iter()
        .filter(|p| {
            p.employee_id == r.employee_id && p.paid_on.is_none() && p.ends_on <= r.cash_date
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|a, b| a.kind.cmp(&b.kind).then(a.slip_id.cmp(&b.slip_id)));
    for p in candidates {
        let (income, ext, other) = candidate_income(data, &p, annual.starts_on, r.service_end)?;
        annual.external_thirteenth_floor = annual.external_thirteenth_floor.max(ext);
        annual.other_benefits_floor = annual.other_benefits_floor.max(other);
        outstanding.push(Pending {
            id: income.id.clone(),
            income,
            net_cents: p.amount_cents,
        });
        selected.push(json!({"kind":p.kind,"slip_id":p.slip_id,"run_id":p.run_id,"ends_on":p.ends_on,"amount_cents":p.amount_cents}));
    }
    if let Some(employee) = b["employee"].as_object_mut() {
        employee.remove("status");
    }
    if let Some(payments) = b["payments"].as_array_mut() {
        for p in payments {
            p.as_object_mut().unwrap().remove("revision");
        }
    }
    let mut prior = vec![];
    let mut vacation = 0;
    for p in data.payroll.finals.records.iter().filter(|p| {
        p.company_id == data.company.id
            && p.employee_id == r.employee_id
            && p.id != r.id
            && ["approved", "settled"].contains(&p.status.as_str())
    }) {
        prior.push(json!({"id":p.id,"status":p.status,"employment_start":p.employment_start,"service_end":p.service_end,"cash_date":p.cash_date}));
        if p.status == "settled" && p.cash_date.year() == r.cash_date.year() {
            vacation = vacation.max(
                p.worksheet.vacation.previous_days_hundredths
                    + p.worksheet.vacation.days_hundredths,
            );
        }
    }
    prior.sort_by_key(|p| p["id"].as_str().unwrap().to_owned());
    Ok(
        json!({"annual_basis":b,"prior_final":prior,"selected":selected,"calculation_source":Source{employment_start:start,service_end:r.service_end,cash_date:r.cash_date,annual,outstanding,previous_vacation_days_hundredths:vacation}}),
    )
}
pub(crate) fn guard_payment(data: &Snapshot, a: &PayrollAction) -> Result<()> {
    let k = match a.operation.as_str() {
        "receipt_record" => a.record["kind"].as_str().unwrap_or(""),
        "run_paid" => "monthly",
        "cutoff_run_paid" => "cutoff",
        "thirteenth_run_paid" => "thirteenth",
        _ => return Ok(()),
    };
    let id = if a.operation == "receipt_record" {
        &a.record["slip_id"]
    } else {
        &a.record["id"]
    };
    let key = if a.operation == "receipt_record" {
        "slip_id"
    } else {
        "run_id"
    };
    ensure!(
        !data
            .payroll
            .finals
            .records
            .iter()
            .any(|r| r.company_id == data.company.id
                && r.status == "approved"
                && r.source_basis["selected"]
                    .as_array()
                    .is_some_and(|ps| ps.iter().any(|p| p["kind"] == k && p[key] == *id))),
        "A selected payslip belongs to an approved final statement. Use final settlement or withdraw that unpaid statement."
    );
    Ok(())
}
fn check(data: &Snapshot, r: &Record) -> Result<()> {
    ensure!(
        demo_basis(data, r)? == r.source_basis,
        "Final-pay income, payments or employment changed. Refresh a draft, or withdraw the unpaid issued statement and prepare a new review."
    );
    ensure!(
        ready(&source(r)?, &r.worksheet)? == r.totals
            && (r.rule_version == RULE_VERSION
                || (r.service_end.year() == r.cash_date.year()
                    && r.rule_version == "PH-final-pay-2026-10-24-v1")),
        "Final-pay totals changed."
    );
    ensure!(
        !data
            .payroll
            .finals
            .records
            .iter()
            .any(|p| p.company_id == r.company_id
                && p.employee_id == r.employee_id
                && p.id != r.id
                && ["approved", "settled"].contains(&p.status.as_str())
                && p.employment_start <= r.service_end
                && p.service_end >= r.employment_start),
        "This employment already has an overlapping issued final statement."
    );
    Ok(())
}
fn event(data: &mut Snapshot, a: &PayrollAction, before: Option<Value>, r: &Record) {
    data.payroll.events.push(payroll::Event {
        id: Uuid::new_v4(),
        company_id: data.company.id,
        record_id: r.id,
        kind: a.operation.clone(),
        reason: a.record["reason"].as_str().unwrap().trim().into(),
        actor_name: data
            .employee_name(data.access.employee_id.unwrap_or_default())
            .into(),
        actor_role: data.access.role,
        before_record: before,
        after_record: json!({"record":r,"request":a.record,"actor_id":data.access.user_id}),
        recorded_at: Utc::now(),
    });
}
pub fn apply_demo(data: &mut Snapshot, a: &PayrollAction) -> Result<()> {
    payload(a, data)?;
    let mut next = data.clone();
    inner(&mut next, a)?;
    *data = next;
    Ok(())
}
fn inner(data: &mut Snapshot, a: &PayrollAction) -> Result<()> {
    let v = &a.record;
    let id: Uuid = serde_json::from_value(v["id"].clone())?;
    let op = a.operation.strip_prefix("final_").unwrap();
    if ["create", "settle", "withdraw"].contains(&op) {
        if let Some(e) = data
            .payroll
            .events
            .iter()
            .find(|e| e.record_id == id && e.kind == a.operation)
        {
            ensure!(
                e.after_record["request"] == *v
                    && e.after_record["actor_id"] == json!(data.access.user_id),
                "This final-pay operation identity was already used."
            );
            return Ok(());
        }
    }
    let ix = data
        .payroll
        .finals
        .records
        .iter()
        .position(|r| r.id == id && r.company_id == data.company.id);
    let before = ix.map(|i| serde_json::to_value(&data.payroll.finals.records[i]).unwrap());
    let mut r;
    if op == "create" {
        ensure!(ix.is_none(), "Final-pay identity already exists.");
        let eid: Uuid = serde_json::from_value(v["employee_id"].clone())?;
        let e = data
            .employees
            .iter()
            .find(|e| e.id == eid)
            .context("Employee unavailable.")?;
        r = Record {
            id,
            company_id: data.company.id,
            employee_id: eid,
            employment_start: e.joined_on.context("Set the employment start first.")?,
            service_end: serde_json::from_value(v["service_end"].clone())?,
            cash_date: serde_json::from_value(v["cash_date"].clone())?,
            title: v["title"].as_str().unwrap().trim().into(),
            employee_name: e.full_name.clone(),
            employee_code: e.employee_code.clone(),
            department: e.department.clone(),
            status: "draft".into(),
            revision: 0,
            created_by: data.access.user_id,
            prepared_by: data.access.user_id,
            submitted_by: None,
            approved_by: None,
            approved_at: None,
            settled_on: None,
            payment_reference: String::new(),
            handling_reference: String::new(),
            rule_version: RULE_VERSION.into(),
            cash_year_reviewed: false,
            cash_year_note: String::new(),
            source_basis: Value::Null,
            worksheet: Worksheet::default(),
            totals: Calculation::default(),
        };
        r.source_basis = demo_basis(data, &r)?;
        r.worksheet = seed(&source(&r)?);
        r.totals = calculate(&source(&r)?, &r.worksheet)?;
    } else {
        r = data.payroll.finals.records[ix.context("Final-pay case unavailable.")?].clone();
        ensure!(
            v["expected_revision"] == json!(r.revision),
            "Final-pay record changed. Refresh."
        );
        match op {
            "save" | "refresh" => {
                ensure!(
                    r.status == "draft",
                    "Only draft final-pay worksheets may change."
                );
                let fresh = demo_basis(data, &r)?;
                if op == "save" {
                    ensure!(
                        fresh == r.source_basis,
                        "Final-pay sources changed. Refresh and review again."
                    );
                    r.worksheet = serde_json::from_value(v["worksheet"].clone())?;
                } else {
                    r.source_basis = fresh;
                    reset_reviews(&mut r.worksheet);
                    r.worksheet.vacation.previous_days_hundredths = r
                        .worksheet
                        .vacation
                        .previous_days_hundredths
                        .max(source(&r)?.previous_vacation_days_hundredths);
                }
                if r.worksheet.vacation.previous_days_hundredths > 0
                    && r.worksheet.vacation.reference.trim().is_empty()
                {
                    r.worksheet.vacation.reference="Earlier recorded vacation conversion retained. Review the original evidence.".into();
                }
                r.totals = match calculate(&source(&r)?, &r.worksheet) {
                    Ok(c) => c,
                    Err(_) if op == "refresh" => {
                        r.worksheet = seed(&source(&r)?);
                        calculate(&source(&r)?, &r.worksheet)?
                    }
                    Err(e) => return Err(e),
                };
                let e = data
                    .employees
                    .iter()
                    .find(|e| e.id == r.employee_id)
                    .unwrap();
                r.employment_start = e.joined_on.unwrap();
                r.employee_name = e.full_name.clone();
                r.employee_code = e.employee_code.clone();
                r.department = e.department.clone();
                r.prepared_by = data.access.user_id;
                r.rule_version = RULE_VERSION.into();
                r.cash_year_reviewed = r.worksheet.cash_year_reviewed;
                r.cash_year_note = r.worksheet.cash_year_note.clone();
            }
            "submit" | "approve" => {
                ensure!(
                    r.service_end <= data.company.today(),
                    "Review completed employment and issued pay only."
                );
                check(data, &r)?;
                if op == "submit" {
                    ensure!(r.status == "draft", "Submit a draft final-pay review.");
                    r.status = "submitted".into();
                    r.submitted_by = Some(data.access.user_id);
                } else {
                    ensure!(
                        r.status == "submitted"
                            && r.created_by != data.access.user_id
                            && r.prepared_by != data.access.user_id
                            && r.submitted_by != Some(data.access.user_id),
                        "An independent Administrator must approve final pay."
                    );
                    r.status = "approved".into();
                    r.approved_by = Some(data.access.user_id);
                    r.approved_at = Some(Utc::now());
                    let mut sources = r.source_basis["selected"].clone();
                    for s in sources.as_array_mut().unwrap() {
                        s.as_object_mut().unwrap().remove("run_id");
                    }
                    let components=r.worksheet.lines.iter().map(|l|json!({"label":l.label,"kind":l.kind,"amount_cents":l.amount_cents})).collect::<Vec<_>>();
                    data.payroll.finals.slips.push(Slip {
                        id: r.id,
                        company_id: r.company_id,
                        employee_id: r.employee_id,
                        employment_start: r.employment_start,
                        service_end: r.service_end,
                        cash_date: r.cash_date,
                        company_name: data.company.name.clone(),
                        employee_name: r.employee_name.clone(),
                        employee_code: r.employee_code.clone(),
                        department: r.department.clone(),
                        totals: r.totals.clone(),
                        sources,
                        components: json!(components),
                        approved_by: data.access.user_id,
                        approved_at: r.approved_at.unwrap(),
                        rule_version: r.rule_version.clone(),
                    });
                }
            }
            "return" => {
                ensure!(r.status == "submitted", "Return a submitted final review.");
                r.status = "draft".into();
                r.submitted_by = None;
            }
            "cancel" => {
                ensure!(
                    ["draft", "submitted"].contains(&r.status.as_str()),
                    "Issued statements remain immutable."
                );
                r.status = "cancelled".into();
                r.submitted_by = None;
            }
            "withdraw" => {
                ensure!(
                    r.status == "approved",
                    "Withdraw an unpaid approved statement only."
                );
                r.status = "withdrawn".into();
                data.payroll.finals.statuses.push(Status {
                    id: r.id,
                    company_id: r.company_id,
                    employee_id: r.employee_id,
                    kind: r.status.clone(),
                    recorded_on: data.company.today(),
                    net_cents: 0,
                });
            }
            "settle" => {
                ensure!(
                    r.status == "approved",
                    "Only approved final statements can settle."
                );
                let d: NaiveDate = serde_json::from_value(v["settled_on"].clone())?;
                let tz = data.company.timezone.parse::<chrono_tz::Tz>()?;
                ensure!(
                    d == r.cash_date
                        && d <= data.company.today()
                        && d >= r.approved_at.unwrap().with_timezone(&tz).date_naive(),
                    "Use the approved actual cash date after approval; withdraw an unpaid statement to reschedule."
                );
                check(data, &r)?;
                field(
                    v,
                    "handling_reference",
                    1000,
                    r.totals.tax.employer_advance_cents > 0,
                )?;
                let candidates = crate::payroll_payments::candidates(data)?;
                r.status = "settled".into();
                r.settled_on = Some(d);
                r.payment_reference = v["payment_reference"].as_str().unwrap().trim().into();
                r.handling_reference = v["handling_reference"].as_str().unwrap().trim().into();
                for part in r.source_basis["selected"].as_array().unwrap() {
                    let p = candidates
                        .iter()
                        .find(|p| {
                            json!(p.slip_id) == part["slip_id"]
                                && p.kind == part["kind"]
                                && p.employee_id == r.employee_id
                                && p.paid_on.is_none()
                        })
                        .context("Final source payment changed.")?;
                    data.payroll.receipts.push(crate::payroll_payments::Receipt{id:Uuid::new_v4(),company_id:r.company_id,employee_id:r.employee_id,kind:p.kind.clone(),slip_id:p.slip_id,run_id:p.run_id,amount_cents:p.amount_cents,paid_on:d,payment_reference:r.payment_reference.clone(),reason:"Source net discharged in combined final settlement; actual cash is on the final statement.".into(),origin:"final_settlement".into(),recorded_by:Some(data.access.user_id),recorded_at:Utc::now(),final_case_id:Some(r.id)});
                    let rows = crate::payroll_payments::candidates(data)?;
                    let remaining = rows
                        .iter()
                        .any(|q| q.run_id == p.run_id && q.kind == p.kind && q.paid_on.is_none());
                    let latest = rows
                        .iter()
                        .filter(|q| q.run_id == p.run_id && q.kind == p.kind)
                        .filter_map(|q| q.paid_on)
                        .max();
                    macro_rules! update {
                        ($run:expr) => {{
                            let run = $run;
                            run.revision += 1;
                            if !remaining {
                                run.status = "paid".into();
                                run.paid_on = latest;
                                run.payment_reference = "Per-employee payment records".into();
                            }
                        }};
                    }
                    match p.kind.as_str() {
                        "monthly" => update!(
                            data.payroll
                                .runs
                                .iter_mut()
                                .find(|q| q.id == p.run_id)
                                .unwrap()
                        ),
                        "cutoff" => update!(
                            &mut data
                                .payroll
                                .cutoffs
                                .runs
                                .iter_mut()
                                .find(|q| q.base.id == p.run_id)
                                .unwrap()
                                .base
                        ),
                        _ => update!(
                            data.payroll
                                .thirteenth
                                .runs
                                .iter_mut()
                                .find(|q| q.id == p.run_id)
                                .unwrap()
                        ),
                    };
                }
                data.payroll.finals.statuses.push(Status {
                    id: r.id,
                    company_id: r.company_id,
                    employee_id: r.employee_id,
                    kind: r.status.clone(),
                    recorded_on: d,
                    net_cents: r.totals.net_cents,
                });
            }
            _ => anyhow::bail!("Unsupported final-pay operation."),
        }
        r.revision += 1;
    }
    if let Some(i) = ix {
        data.payroll.finals.records[i] = r.clone();
    } else {
        data.payroll.finals.records.push(r.clone());
    }
    event(data, a, before, &r);
    Ok(())
}
