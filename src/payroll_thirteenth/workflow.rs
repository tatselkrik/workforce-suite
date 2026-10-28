use super::*;
use crate::{
    access::Role,
    domain::{Mode, Snapshot},
    payroll::PayrollAction,
};
use anyhow::Context;
use chrono::{DateTime, Utc};
use serde_json::{Value, json};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Run {
    pub id: Uuid,
    pub company_id: Uuid,
    pub year: i32,
    pub as_of: NaiveDate,
    pub pay_date: NaiveDate,
    pub title: String,
    pub status: String,
    pub revision: i64,
    pub created_by: Uuid,
    pub submitted_by: Option<Uuid>,
    pub approved_by: Option<Uuid>,
    pub paid_on: Option<NaiveDate>,
    pub payment_reference: String,
    pub rule_version: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Item {
    pub id: Uuid,
    pub company_id: Uuid,
    pub run_id: Uuid,
    pub employee_id: Uuid,
    pub employee_name: String,
    pub employee_code: String,
    pub department: String,
    #[serde(default)]
    pub source_basis: Value,
    pub worksheet: Worksheet,
    pub totals: Calculation,
    pub revision: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Slip {
    pub id: Uuid,
    pub company_id: Uuid,
    pub run_id: Uuid,
    pub employee_id: Uuid,
    pub year: i32,
    pub as_of: NaiveDate,
    pub pay_date: NaiveDate,
    pub company_name: String,
    pub item: Item,
    pub approved_by: Uuid,
    pub approved_at: DateTime<Utc>,
    pub rule_version: String,
}
#[derive(Clone, Debug, Default)]
pub struct Setup {
    pub runs: Vec<Run>,
    pub items: Vec<Item>,
    pub slips: Vec<Slip>,
}
pub fn source(i: &Item) -> Result<Source> {
    Ok(serde_json::from_value(
        i.source_basis["calculation_source"].clone(),
    )?)
}
fn field(v: &Value, k: &str, max: usize) -> Result<()> {
    super::text(
        v[k].as_str().context("Enter the required text.")?,
        max,
        true,
    )
}
pub fn payload(a: &PayrollAction, data: &Snapshot) -> Result<Value> {
    ensure!(
        data.mode != Mode::SignedOut
            && data
                .access
                .allows(crate::staff_roles::Capability::PayrollPreparation)
            && data.access.company_id == data.company.id,
        "Only Payroll Officers and Administrators manage annual benefit pay."
    );
    let op = a
        .operation
        .strip_prefix("thirteenth_")
        .context("Unknown benefit action.")?;
    let fields: &[&str] = match op {
        "run_create" => &["year", "as_of", "pay_date", "title"],
        "run_update" => &["pay_date", "title"],
        "item_add" => &["run_id", "employee_id"],
        "item_save" => &["worksheet"],
        "run_submit" => &["scope_reviewed"],
        "run_paid" => &["paid_on", "payment_reference"],
        "item_refresh" | "item_remove" | "run_return" | "run_cancel" | "run_approve" => &[],
        _ => anyhow::bail!("Unknown annual benefit operation."),
    };
    let v = &a.record;
    let obj = v.as_object().context("Enter benefit details.")?;
    ensure!(
        v.to_string().len() <= 131072
            && obj.keys().all(
                |k| ["id", "expected_revision", "reason"].contains(&k.as_str())
                    || fields.contains(&k.as_str())
            ),
        "Unsupported benefit fields."
    );
    let id: Uuid = serde_json::from_value(v["id"].clone())?;
    ensure!(!id.is_nil(), "Record identity required.");
    field(v, "reason", 500)?;
    if !v["expected_revision"].is_null() {
        ensure!(
            v["expected_revision"].as_i64().is_some_and(|n| n >= 0),
            "Refresh the record revision."
        );
    }
    if op == "run_create" || op == "run_update" {
        let (year, end) = if op == "run_create" {
            (
                serde_json::from_value::<i32>(v["year"].clone())?,
                serde_json::from_value::<NaiveDate>(v["as_of"].clone())?,
            )
        } else {
            let r = data
                .payroll
                .thirteenth
                .runs
                .iter()
                .find(|r| r.id == id)
                .context("Refresh this benefit run.")?;
            (r.year, r.as_of)
        };
        ensure!(
            (2026..=2200).contains(&year) && end.year() == year,
            "Use coverage in the benefit year."
        );
        let pay: NaiveDate = serde_json::from_value(v["pay_date"].clone())?;
        ensure!(
            pay >= end && pay <= NaiveDate::from_ymd_opt(year + 1, 1, 31).unwrap(),
            "Review the planned pay date after earned coverage, through the following January."
        );
        field(v, "title", 120)?;
    }
    if op == "item_save" {
        let i = data
            .payroll
            .thirteenth
            .items
            .iter()
            .find(|i| i.id == id)
            .context("Refresh the worksheet.")?;
        let w: Worksheet = serde_json::from_value(v["worksheet"].clone())?;
        calculate(&source(i)?, &w)?;
    }
    if op == "run_submit" {
        ensure!(
            v["scope_reviewed"] == true,
            "Review included and omitted employees."
        );
    }
    if op == "run_approve" || op == "run_paid" {
        ensure!(
            data.access.role == Role::Administrator,
            "Only Administrators approve or record benefit payments."
        );
    }
    if op == "run_paid" {
        let _: NaiveDate = serde_json::from_value(v["paid_on"].clone())?;
        field(v, "payment_reference", 500)?;
    }
    Ok(json!({"p_operation":op,"p_record":v}))
}
pub fn published(i: &Item) -> Item {
    let mut i = i.clone();
    i.source_basis = Value::Null;
    for note in [
        &mut i.worksheet.eligibility_note,
        &mut i.worksheet.adjustment_note,
        &mut i.worksheet.policy_note,
        &mut i.worksheet.opening_note,
        &mut i.worksheet.tax_note,
        &mut i.worksheet.contribution_note,
        &mut i.worksheet.review_note,
    ] {
        note.clear();
    }
    for o in &mut i.worksheet.openings {
        o.reference.clear();
    }
    i
}
pub fn demo_basis(data: &Snapshot, eid: Uuid, r: &Run) -> Result<Value> {
    let e = data
        .employees
        .iter()
        .find(|e| e.id == eid)
        .context("Employee unavailable.")?;
    let lo = e
        .joined_on
        .unwrap_or(NaiveDate::from_ymd_opt(r.year, 1, 1).unwrap())
        .max(NaiveDate::from_ymd_opt(r.year, 1, 1).unwrap());
    ensure!(
        lo <= r.as_of,
        "Employee joined after this benefit coverage."
    );
    let mut periods = vec![];
    for s in &data.payroll.slips {
        if s.employee_id == eid
            && s.month.year() == r.year
            && crate::payroll::month_end(s.month) <= r.as_of
        {
            periods.push(Period {
                id: s.id,
                starts_on: s.month.max(lo),
                ends_on: crate::payroll::month_end(s.month),
                basic_cents: s.item.totals.basic,
            });
        }
    }
    for s in &data.payroll.cutoffs.slips {
        if s.base.employee_id == eid && s.base.month.year() == r.year && s.ends_on <= r.as_of {
            periods.push(Period {
                id: s.base.id,
                starts_on: s.starts_on.max(lo),
                ends_on: s.ends_on,
                basic_cents: s.base.item.totals.basic,
            });
        }
    }
    periods.sort_by_key(|p| (p.starts_on, p.id));
    let mut prior = vec![];
    let (mut external, mut other) = (0, 0);
    for s in &data.payroll.thirteenth.slips {
        if s.employee_id == eid && s.year == r.year {
            ensure!(
                s.as_of <= r.as_of,
                "Reconcile through the already issued later coverage or beyond."
            );
            prior.push(Prior {
                id: s.id,
                gross_cents: s.item.totals.gross_cents,
                exempt_cents: s.item.totals.exempt_cents,
            });
            external = external.max(s.item.worksheet.external_thirteenth_cents);
            other = other.max(s.item.worksheet.other_benefits_cents);
        }
    }
    prior.sort_by_key(|p| p.id);
    let b = crate::payroll_final::augment_benefit_demo_basis(
        data,
        eid,
        r,
        json!({"employee":{"id":e.id,"full_name":e.full_name,"employee_code":e.employee_code,"department":e.department,"joined_on":e.joined_on,"status":e.status},"calculation_source":Source{starts_on:lo,as_of:r.as_of,periods,prior,external_thirteenth_floor:external,other_benefits_floor:other}}),
    )?;
    crate::payroll_amendments::augment_benefits(data, eid, r, b)
}
pub fn apply_demo(data: &mut Snapshot, a: &PayrollAction) -> Result<()> {
    payload(a, data)?;
    let v = &a.record;
    let id: Uuid = serde_json::from_value(v["id"].clone())?;
    let expected = v["expected_revision"].as_i64();
    let op = a.operation.strip_prefix("thirteenth_").unwrap();
    let mut before = None;
    let after;
    if op == "run_create" {
        ensure!(
            !data.payroll.thirteenth.runs.iter().any(|r| r.id == id),
            "Benefit identity already exists."
        );
        let r = Run {
            id,
            company_id: data.company.id,
            year: serde_json::from_value(v["year"].clone())?,
            as_of: serde_json::from_value(v["as_of"].clone())?,
            pay_date: serde_json::from_value(v["pay_date"].clone())?,
            title: v["title"].as_str().unwrap().trim().into(),
            status: "draft".into(),
            revision: 0,
            created_by: data.access.user_id,
            submitted_by: None,
            approved_by: None,
            paid_on: None,
            payment_reference: String::new(),
            rule_version: RULE_VERSION.into(),
        };
        after = serde_json::to_value(&r)?;
        data.payroll.thirteenth.runs.push(r);
    } else if op.starts_with("item_") {
        let ix = data
            .payroll
            .thirteenth
            .items
            .iter()
            .position(|i| i.id == id);
        let old = ix.map(|n| data.payroll.thirteenth.items[n].clone());
        let (rid, eid) = if op == "item_add" {
            ensure!(old.is_none(), "Worksheet identity exists.");
            (
                serde_json::from_value(v["run_id"].clone())?,
                serde_json::from_value(v["employee_id"].clone())?,
            )
        } else {
            let i = old.as_ref().context("Worksheet unavailable.")?;
            ensure!(Some(i.revision) == expected, "Worksheet changed. Refresh.");
            before = Some(serde_json::to_value(i)?);
            (i.run_id, i.employee_id)
        };
        let ri = data
            .payroll
            .thirteenth
            .runs
            .iter()
            .position(|r| r.id == rid)
            .context("Benefit run unavailable.")?;
        let r = data.payroll.thirteenth.runs[ri].clone();
        ensure!(r.status == "draft", "Only draft worksheets may change.");
        if op == "item_remove" {
            after = before.clone().unwrap();
            data.payroll.thirteenth.items.remove(ix.unwrap());
        } else {
            ensure!(
                op != "item_add"
                    || !data
                        .payroll
                        .thirteenth
                        .items
                        .iter()
                        .any(|i| i.run_id == rid && i.employee_id == eid),
                "Employee already included."
            );
            let fresh = demo_basis(data, eid, &r)?;
            if op == "item_save" {
                ensure!(
                    Some(&fresh) == old.as_ref().map(|i| &i.source_basis),
                    "Sources changed. Refresh and review again."
                );
            }
            let mut w = if op == "item_save" {
                serde_json::from_value(v["worksheet"].clone())?
            } else {
                old.as_ref()
                    .map(|i| i.worksheet.clone())
                    .unwrap_or_default()
            };
            if op == "item_refresh" {
                w.reviewed = false;
                w.review_note.clear();
                w.tax_reviewed = false;
                w.contributions_reviewed = false;
            }
            let src: Source = serde_json::from_value(fresh["calculation_source"].clone())?;
            let seed = || Worksheet {
                external_thirteenth_cents: src.external_thirteenth_floor,
                other_benefits_cents: src.other_benefits_floor,
                opening_note: if src.external_thirteenth_floor > 0 || src.other_benefits_floor > 0 {
                    "Previously approved external payment and benefit facts retained. Review the original evidence.".into()
                } else {
                    String::new()
                },
                ..Default::default()
            };
            if op == "item_add" {
                w = seed();
            }
            if op == "item_refresh" {
                w.external_thirteenth_cents = w
                    .external_thirteenth_cents
                    .max(src.external_thirteenth_floor);
                w.other_benefits_cents = w.other_benefits_cents.max(src.other_benefits_floor);
            }
            let t = match calculate(&src, &w) {
                Ok(t) => t,
                Err(_) if op == "item_refresh" => {
                    w = seed();
                    calculate(&src, &w)?
                }
                Err(e) => return Err(e),
            };
            let e = data.employees.iter().find(|e| e.id == eid).unwrap();
            let i = Item {
                id,
                company_id: data.company.id,
                run_id: rid,
                employee_id: eid,
                employee_name: e.full_name.clone(),
                employee_code: e.employee_code.clone(),
                department: e.department.clone(),
                source_basis: fresh,
                worksheet: w,
                totals: t,
                revision: old.as_ref().map_or(0, |i| i.revision + 1),
            };
            after = serde_json::to_value(&i)?;
            if let Some(ix) = ix {
                data.payroll.thirteenth.items[ix] = i;
            } else {
                data.payroll.thirteenth.items.push(i);
            }
        }
        data.payroll.thirteenth.runs[ri].revision += 1;
    } else {
        let ri = data
            .payroll
            .thirteenth
            .runs
            .iter()
            .position(|r| r.id == id)
            .context("Benefit run unavailable.")?;
        let mut r = data.payroll.thirteenth.runs[ri].clone();
        ensure!(
            Some(r.revision) == expected,
            "Benefit run changed. Refresh."
        );
        before = Some(serde_json::to_value(&r)?);
        if op == "run_submit" || op == "run_approve" {
            ensure!(
                r.as_of <= data.company.today() && r.year == 2026,
                "Approve earned2026coverage only."
            );
            let items: Vec<_> = data
                .payroll
                .thirteenth
                .items
                .iter()
                .filter(|i| i.run_id == id)
                .collect();
            ensure!(!items.is_empty(), "Add at least one worksheet.");
            for i in items {
                ensure!(
                    demo_basis(data, i.employee_id, &r)? == i.source_basis,
                    "Benefit sources changed. Refresh and review."
                );
                ensure!(
                    ready(&source(i)?, &i.worksheet)? == i.totals,
                    "Benefit calculation does not reconcile."
                );
            }
        }
        match op {
            "run_update" => {
                ensure!(r.status == "draft", "Only draft title/pay date may change.");
                r.title = v["title"].as_str().unwrap().trim().into();
                r.pay_date = serde_json::from_value(v["pay_date"].clone())?;
            }
            "run_submit" => {
                ensure!(r.status == "draft", "Submit a draft benefit.");
                r.status = "submitted".into();
                r.submitted_by = Some(data.access.user_id);
            }
            "run_return" => {
                ensure!(r.status == "submitted", "Return submitted benefit only.");
                r.status = "draft".into();
                r.submitted_by = None;
            }
            "run_cancel" => {
                ensure!(
                    ["draft", "submitted"].contains(&r.status.as_str()),
                    "Issued pay stays immutable."
                );
                r.status = "cancelled".into();
                r.submitted_by = None;
            }
            "run_approve" => {
                ensure!(
                    r.status == "submitted" && r.submitted_by != Some(data.access.user_id),
                    "Another Administrator must approve."
                );
                for i in data
                    .payroll
                    .thirteenth
                    .items
                    .iter()
                    .filter(|i| i.run_id == id)
                {
                    data.payroll.thirteenth.slips.push(Slip {
                        id: i.id,
                        company_id: data.company.id,
                        run_id: id,
                        employee_id: i.employee_id,
                        year: r.year,
                        as_of: r.as_of,
                        pay_date: r.pay_date,
                        company_name: data.company.name.clone(),
                        item: published(i),
                        approved_by: data.access.user_id,
                        approved_at: Utc::now(),
                        rule_version: RULE_VERSION.into(),
                    });
                }
                r.status = "approved".into();
                r.approved_by = Some(data.access.user_id);
            }
            "run_paid" => {
                ensure!(
                    r.status == "approved",
                    "Record payment for approved benefit only."
                );
                let date: NaiveDate = serde_json::from_value(v["paid_on"].clone())?;
                ensure!(
                    date >= r.as_of && date <= data.company.today(),
                    "Use an actual payment date after coverage."
                );
                r.status = "paid".into();
                r.paid_on = Some(date);
                r.payment_reference = v["payment_reference"].as_str().unwrap().trim().into();
            }
            _ => anyhow::bail!("Unsupported benefit change."),
        }
        r.revision += 1;
        after = serde_json::to_value(&r)?;
        data.payroll.thirteenth.runs[ri] = r;
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
                .employees
                .iter()
                .find(|e| Some(e.id) == data.access.employee_id)
                .map(|e| e.full_name.clone())
                .unwrap_or_else(|| "Prototype staff".into()),
            actor_role: data.access.role,
            before_record: before,
            after_record: after,
            recorded_at: Utc::now(),
        },
    );
    Ok(())
}
