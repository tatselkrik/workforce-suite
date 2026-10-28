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
    pub year: i32,
    pub kind: String,
    pub as_of: NaiveDate,
    pub service_end: NaiveDate,
    pub title: String,
    pub employee_name: String,
    pub employee_code: String,
    pub department: String,
    pub status: String,
    pub revision: i64,
    pub created_by: Uuid,
    pub submitted_by: Option<Uuid>,
    pub approved_by: Option<Uuid>,
    pub approved_at: Option<DateTime<Utc>>,
    pub settled_on: Option<NaiveDate>,
    pub settlement_reference: String,
    pub rule_version: String,
    pub source_basis: Value,
    pub worksheet: Worksheet,
    pub totals: Calculation,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Slip {
    pub id: Uuid,
    pub company_id: Uuid,
    pub employee_id: Uuid,
    pub year: i32,
    pub kind: String,
    pub as_of: NaiveDate,
    pub service_end: NaiveDate,
    pub company_name: String,
    pub employee_name: String,
    pub employee_code: String,
    pub department: String,
    pub totals: Calculation,
    pub approved_by: Uuid,
    pub approved_at: DateTime<Utc>,
    pub rule_version: String,
}
#[derive(Clone, Debug, Default)]
pub struct Setup {
    pub records: Vec<Record>,
    pub slips: Vec<Slip>,
}
pub fn source(r: &Record) -> Result<Source> {
    Ok(serde_json::from_value(
        r.source_basis["calculation_source"].clone(),
    )?)
}
fn required(v: &Value, k: &str, max: usize) -> Result<()> {
    text(
        v[k].as_str()
            .context("Enter the required annual tax text.")?,
        max,
        true,
    )
}
pub fn dates(year: i32, kind: &str, end: NaiveDate, asof: NaiveDate) -> Result<()> {
    ensure!(
        (2026..=2200).contains(&year) && end.year() >= 1900 && asof.year() == year && end <= asof,
        "Use the actual service end and a payment as-of date in the tax year."
    );
    ensure!(
        kind == "termination"
            || (kind == "year_end" && end.month() == 12 && end.day() == 31 && end == asof),
        "Year-end closes31December. A termination close needs the actual service end date."
    );
    Ok(())
}
pub fn payload(a: &PayrollAction, data: &Snapshot) -> Result<Value> {
    ensure!(
        data.mode != Mode::SignedOut
            && data.access.allows(crate::staff_roles::Capability::PayrollPreparation)
            && data.company.id == data.access.company_id,
        "Only Payroll Officers and Administrators manage annual tax reconciliation."
    );
    let op = a
        .operation
        .strip_prefix("tax_")
        .context("Unknown annual tax operation.")?;
    let fields: &[&str] = match op {
        "create" => &[
            "employee_id",
            "year",
            "kind",
            "as_of",
            "service_end",
            "title",
        ],
        "save" => &["worksheet"],
        "settle" => &["settled_on", "settlement_reference"],
        "refresh" | "submit" | "return" | "cancel" | "approve" => &[],
        _ => anyhow::bail!("Unknown annual tax operation."),
    };
    let v = &a.record;
    let obj = v.as_object().context("Enter annual tax details.")?;
    ensure!(
        v.to_string().len() <= 131072
            && obj.keys().all(
                |k| ["id", "expected_revision", "reason"].contains(&k.as_str())
                    || fields.contains(&k.as_str())
            ),
        "Unsupported annual tax fields."
    );
    let id: Uuid = serde_json::from_value(v["id"].clone())?;
    ensure!(!id.is_nil(), "Record identity required.");
    required(v, "reason", 500)?;
    if op == "create" {
        ensure!(
            v["expected_revision"].is_null(),
            "New tax record must not have a revision."
        );
        let eid: Uuid = serde_json::from_value(v["employee_id"].clone())?;
        ensure!(!eid.is_nil(), "Employee identity required.");
        dates(
            serde_json::from_value(v["year"].clone())?,
            v["kind"].as_str().unwrap_or(""),
            serde_json::from_value(v["service_end"].clone())?,
            serde_json::from_value(v["as_of"].clone())?,
        )?;
        required(v, "title", 120)?;
    } else {
        ensure!(
            v["expected_revision"].as_i64().is_some_and(|n| n >= 0),
            "Refresh the record revision."
        );
    }
    if op == "save" {
        let r = data
            .payroll
            .tax
            .records
            .iter()
            .find(|r| r.id == id)
            .context("Refresh this annual tax record.")?;
        let w: Worksheet = serde_json::from_value(v["worksheet"].clone())?;
        calculate(&source(r)?, &w)?;
    }
    if op == "approve" || op == "settle" {
        ensure!(
            data.access.role == Role::Administrator,
            "Only Administrators approve or record tax settlements."
        );
    }
    if op == "settle" {
        let _: NaiveDate = serde_json::from_value(v["settled_on"].clone())?;
        required(v, "settlement_reference", 500)?;
    }
    Ok(json!({"p_operation":op,"p_record":v}))
}
pub fn demo_basis(data: &Snapshot, r: &Record) -> Result<Value> {
    let e = data
        .employees
        .iter()
        .find(|e| e.id == r.employee_id)
        .context("Employee unavailable.")?;
    let lo = e
        .joined_on
        .unwrap_or(NaiveDate::from_ymd_opt(r.year, 1, 1).unwrap())
        .max(NaiveDate::from_ymd_opt(r.year, 1, 1).unwrap());
    ensure!(
        e.joined_on.is_none_or(|d| d <= r.service_end),
        "Employee joined after this close's service end."
    );
    let mut income = vec![];
    let mut payments = vec![];
    let mut pending = 0;
    let mut add = |kind: &str,
                   id: Uuid,
                   first: NaiveDate,
                   last: NaiveDate,
                   paid: Option<NaiveDate>,
                   status: &str,
                   revision: i64,
                   taxable: i64,
                   withheld: i64,
                   gross: i64,
                   exempt: i64,
                   contribution: i64,
                   coverage: bool| {
        let (paid, received) = crate::payroll_payments::effective(data,kind,id,paid,status);
        let status=if received {"paid"}else{status};
        if last > r.as_of {
            return;
        }
        let id = format!("{kind}:{id}");
        payments.push(json!({"id":id,"paid_on":paid,"status":status,"revision":revision}));
        if status == "approved" {
            pending += 1;
        }
        if status != "paid" || !paid.is_some_and(|p| p.year() == r.year && p <= r.as_of) {
            return;
        }
        let a = first.max(lo);
        let b = last.min(r.service_end);
        income.push(Income {
            id,
            starts_on: if coverage && a <= b { Some(a) } else { None },
            ends_on: if coverage && a <= b { Some(b) } else { None },
            taxable_cents: taxable,
            withholding_cents: withheld,
            benefit_gross_cents: gross,
            benefit_exempt_cents: exempt,
            contribution_cents: contribution,
        });
    };
    for s in &data.payroll.slips {
        if s.employee_id == r.employee_id {
            let run = data
                .payroll
                .runs
                .iter()
                .find(|p| p.id == s.run_id)
                .context("Refresh monthly payment records.")?;
            add(
                "monthly",
                s.id,
                s.month,
                crate::payroll::month_end(s.month),
                run.paid_on,
                &run.status,
                run.revision,
                s.item.totals.taxable,
                s.item.totals.withholding,
                0,
                0,
                0,
                true,
            );
        }
    }
    for s in &data.payroll.cutoffs.slips {
        if s.base.employee_id == r.employee_id {
            let run = data
                .payroll
                .cutoffs
                .runs
                .iter()
                .find(|p| p.base.id == s.base.run_id)
                .context("Refresh cutoff payment records.")?;
            add(
                "cutoff",
                s.base.id,
                s.starts_on,
                s.ends_on,
                run.base.paid_on,
                &run.base.status,
                run.base.revision,
                s.base.item.totals.taxable,
                s.base.item.totals.withholding,
                0,
                0,
                0,
                true,
            );
        }
    }
    let (mut ext, mut other) = (0, 0);
    for s in &data.payroll.thirteenth.slips {
        if s.employee_id == r.employee_id {
            let run = data
                .payroll
                .thirteenth
                .runs
                .iter()
                .find(|p| p.id == s.run_id)
                .context("Refresh annual benefit payment records.")?;
            add(
                "thirteenth",
                s.id,
                s.as_of,
                s.as_of,
                run.paid_on,
                &run.status,
                run.revision,
                s.item.totals.taxable_cents,
                s.item.totals.withholding_cents,
                s.item.totals.gross_cents,
                s.item.totals.exempt_cents,
                s.item.totals.sss_adjustment_cents
                    + s.item.totals.philhealth_adjustment_cents
                    + s.item.totals.pagibig_adjustment_cents,
                false,
            );
            let (paid, received) = crate::payroll_payments::effective(data,"thirteenth",s.id,run.paid_on,&run.status);
            if s.year == r.year
                && s.as_of <= r.as_of
                && received
                && paid
                    .is_some_and(|p| p.year() == r.year && p <= r.as_of)
            {
                ext = ext.max(s.item.worksheet.external_thirteenth_cents);
                other = other.max(s.item.worksheet.other_benefits_cents);
            }
        }
    }
    income.sort_by(|a, b| a.id.cmp(&b.id));
    payments.sort_by_key(|p| p["id"].as_str().unwrap().to_owned());
    let mut previous = vec![];
    let mut adjustment = 0;
    let mut pending_adjustments = 0;
    for p in &data.payroll.tax.records {
        if p.id != r.id
            && p.employee_id == r.employee_id
            && p.year == r.year
            && ["approved", "settled"].contains(&p.status.as_str())
        {
            ensure!(
                p.as_of.max(p.settled_on.unwrap_or(p.as_of)) <= r.as_of,
                "Use an as-of date at least as late as the earlier tax close and settlement."
            );
            if p.status == "approved" {
                pending_adjustments += 1;
            } else {
                adjustment += p.totals.adjustment_cents;
            }
            previous.push(json!({"id":p.id,"revision":p.revision,"status":p.status,"as_of":p.as_of,"adjustment_cents":p.totals.adjustment_cents,"settled_on":p.settled_on}));
        }
    }
    previous.sort_by_key(|p| p["id"].as_str().unwrap().to_owned());
    let b=crate::payroll_final::augment_annual_demo_basis(data,r,json!({"employee":{"id":e.id,"full_name":e.full_name,"employee_code":e.employee_code,"department":e.department,"joined_on":e.joined_on,"status":e.status},"payments":payments,"previous":previous,"calculation_source":Source{starts_on:lo,service_end:r.service_end,as_of:r.as_of,income,prior_adjustment_cents:adjustment,pending_payments:pending,pending_adjustments,external_thirteenth_floor:ext,other_benefits_floor:other}}))?;
    crate::payroll_amendments::augment_annual(data,r,b)
}
pub fn apply_demo(data: &mut Snapshot, a: &PayrollAction) -> Result<()> {
    payload(a, data)?;
    let v = &a.record;
    let id: Uuid = serde_json::from_value(v["id"].clone())?;
    let op = a.operation.strip_prefix("tax_").unwrap();
    let ix = data.payroll.tax.records.iter().position(|r| r.id == id);
    let before = ix.map(|n| serde_json::to_value(&data.payroll.tax.records[n]).unwrap());
    let mut r;
    if op == "create" {
        if ix.is_some() {
            ensure!(
                data.payroll.events.iter().any(|e| e.record_id == id
                    && e.kind == a.operation
                    && e.after_record["created_by"] == json!(data.access.user_id)
                    && e.after_record["employee_id"] == v["employee_id"]
                    && e.after_record["year"] == v["year"]
                    && e.after_record["kind"] == v["kind"]
                    && e.after_record["as_of"] == v["as_of"]
                    && e.after_record["service_end"] == v["service_end"]
                    && e.after_record["title"] == v["title"]
                    && e.reason == v["reason"].as_str().unwrap().trim()),
                "Tax creation identity was already used."
            );
            return Ok(());
        }
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
            year: serde_json::from_value(v["year"].clone())?,
            kind: v["kind"].as_str().unwrap().into(),
            as_of: serde_json::from_value(v["as_of"].clone())?,
            service_end: serde_json::from_value(v["service_end"].clone())?,
            title: v["title"].as_str().unwrap().trim().into(),
            employee_name: e.full_name.clone(),
            employee_code: e.employee_code.clone(),
            department: e.department.clone(),
            status: "draft".into(),
            revision: 0,
            created_by: data.access.user_id,
            submitted_by: None,
            approved_by: None,
            approved_at: None,
            settled_on: None,
            settlement_reference: String::new(),
            rule_version: RULE_VERSION.into(),
            source_basis: Value::Null,
            worksheet: Worksheet::default(),
            totals: Calculation::default(),
        };
        r.source_basis = demo_basis(data, &r)?;
        r.totals = calculate(&source(&r)?, &r.worksheet)?;
    } else {
        r = data.payroll.tax.records[ix.context("Tax record unavailable.")?].clone();
        ensure!(
            v["expected_revision"].as_i64() == Some(r.revision),
            "Tax record changed. Refresh."
        );
        if ["save", "refresh", "submit", "approve"].contains(&op) {
            let fresh = demo_basis(data, &r)?;
            if op == "refresh" {
                ensure!(r.status == "draft", "Only a draft tax close can refresh.");
                r.source_basis = fresh;
                r.worksheet.final_compensation_reviewed = false;
                r.worksheet.previous_employers_reviewed = false;
                r.worksheet.benefits_reviewed = false;
                r.worksheet.review_note.clear();
                r.totals = match calculate(&source(&r)?, &r.worksheet) {
                    Ok(c) => c,
                    Err(_) => {
                        r.worksheet = Worksheet::default();
                        calculate(&source(&r)?, &r.worksheet)?
                    }
                };
                let e = data
                    .employees
                    .iter()
                    .find(|e| e.id == r.employee_id)
                    .unwrap();
                r.employee_name = e.full_name.clone();
                r.employee_code = e.employee_code.clone();
                r.department = e.department.clone();
            } else {
                ensure!(
                    fresh == r.source_basis,
                    "Tax/payment sources changed. Refresh and review again."
                );
            }
        }
        match op {
            "save" => {
                ensure!(r.status == "draft", "Only draft tax worksheets may change.");
                r.worksheet = serde_json::from_value(v["worksheet"].clone())?;
                r.totals = calculate(&source(&r)?, &r.worksheet)?;
            }
            "refresh" => (),
            "submit" | "approve" => {
                ensure!(
                    r.year == 2026 && r.as_of <= data.company.today(),
                    "Close completed2026payment facts only; future years need a reviewed rule pack."
                );
                ensure!(
                    ready(&source(&r)?, &r.worksheet)? == r.totals,
                    "Tax totals do not reconcile."
                );
                if op == "submit" {
                    ensure!(r.status == "draft", "Submit a draft tax close.");
                    r.status = "submitted".into();
                    r.submitted_by = Some(data.access.user_id);
                } else {
                    ensure!(
                        r.status == "submitted" && r.submitted_by != Some(data.access.user_id),
                        "Another Administrator must approve this tax close."
                    );
                    r.status = "approved".into();
                    r.approved_by = Some(data.access.user_id);
                    r.approved_at = Some(Utc::now());
                    data.payroll.tax.slips.push(Slip {
                        id: r.id,
                        company_id: r.company_id,
                        employee_id: r.employee_id,
                        year: r.year,
                        kind: r.kind.clone(),
                        as_of: r.as_of,
                        service_end: r.service_end,
                        company_name: data.company.name.clone(),
                        employee_name: r.employee_name.clone(),
                        employee_code: r.employee_code.clone(),
                        department: r.department.clone(),
                        totals: r.totals.clone(),
                        approved_by: data.access.user_id,
                        approved_at: r.approved_at.unwrap(),
                        rule_version: r.rule_version.clone(),
                    });
                }
            }
            "return" => {
                ensure!(r.status == "submitted", "Return a submitted tax close.");
                r.status = "draft".into();
                r.submitted_by = None;
            }
            "cancel" => {
                ensure!(
                    ["draft", "submitted"].contains(&r.status.as_str()),
                    "Issued annual tax advice remains immutable."
                );
                r.status = "cancelled".into();
                r.submitted_by = None;
            }
            "settle" => {
                ensure!(
                    r.status == "approved",
                    "Only approved tax advice can be recorded as settled."
                );
                let date: NaiveDate = serde_json::from_value(v["settled_on"].clone())?;
                let tz = data.company.timezone.parse::<chrono_tz::Tz>()?;
                ensure!(
                    date >= r.as_of
                        && date
                            >= r.approved_at
                                .context("Approval date missing.")?
                                .with_timezone(&tz)
                                .date_naive()
                        && date <= data.company.today(),
                    "Use an actual settlement date after approval and the payment as-of date."
                );
                r.status = "settled".into();
                r.settled_on = Some(date);
                r.settlement_reference = v["settlement_reference"].as_str().unwrap().trim().into();
            }
            _ => anyhow::bail!("Unsupported tax action."),
        }
        r.revision += 1;
    }
    let after = serde_json::to_value(&r)?;
    if let Some(ix) = ix {
        data.payroll.tax.records[ix] = r;
    } else {
        data.payroll.tax.records.push(r);
    }
    data.payroll.events.insert(
        0,
        crate::payroll::Event {
            id: Uuid::new_v4(),
            company_id: data.company.id,
            record_id: id,
            kind: a.operation.clone(),
            reason: v["reason"].as_str().unwrap().trim().into(),
            actor_name: data
                .employee_name(data.access.employee_id.unwrap_or_default())
                .to_string(),
            actor_role: data.access.role,
            before_record: before,
            after_record: after,
            recorded_at: Utc::now(),
        },
    );
    Ok(())
}
