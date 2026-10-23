use super::*;
use chrono::Utc;
fn authorized(data: &Snapshot, eid: Uuid, lo: NaiveDate, hi: NaiveDate) -> i64 {
    let zone = data
        .company
        .timezone
        .parse::<chrono_tz::Tz>()
        .unwrap_or(chrono_tz::Asia::Manila);
    data.overtime
        .records
        .iter()
        .filter(|r| r.employee_id == eid)
        .filter_map(|r| {
            let a = data.attendance.iter().find(|a| a.id == r.attendance_id)?;
            let start = a.checked_in_at.with_timezone(&zone).date_naive();
            let end = a
                .checked_out_at
                .unwrap_or(a.checked_in_at)
                .with_timezone(&zone)
                .date_naive();
            if (a.work_date >= lo && a.work_date <= hi) || (start <= hi && end >= lo) {
                crate::overtime::effective_minutes(data, r).map(i64::from)
            } else {
                None
            }
        })
        .sum()
}
fn basis(data: &Snapshot, eid: Uuid, r: &Run) -> Result<Value> {
    let mut b = payroll::demo_basis(data, eid, r.base.month)?;
    let first = data.payroll.cutoffs.slips.iter().find(|s| {
        s.base.employee_id == eid && s.base.month == r.base.month && s.part == 1 && r.part == 2
    });
    b["first_cutoff"] = serde_json::to_value(first)?;
    b["month_overtime_minutes"] = if r.part == 2 {
        json!(authorized(
            data,
            eid,
            r.base.month,
            payroll::month_end(r.base.month)
        ))
    } else {
        Value::Null
    };
    Ok(b)
}
fn check(data: &Snapshot, i: &Item, r: &Run) -> Result<()> {
    ensure!(
        basis(data, i.base.employee_id, r)? == i.base.source_basis,
        "Cutoff sources changed. Refresh and review again."
    );
    ensure!(
        i.base.worksheet.reviewed && !i.base.worksheet.review_note.trim().is_empty(),
        "Review each employee worksheet."
    );
    let joined = data
        .employees
        .iter()
        .find(|e| e.id == i.base.employee_id)
        .and_then(|e| e.joined_on);
    ensure!(
        r.part != 2
            || previous(&i.base)?.is_some()
            || joined.is_some_and(|d| d > r.base.month.with_day(15).unwrap()),
        "Issue this employee's first cutoff before the second."
    );
    ensure!(
        joined.is_none_or(|d| d <= r.starts_on)
            || i.base.profile.pay.basis != "monthly"
            || i.base.worksheet.basic_override_cents.is_some(),
        "A mid-cutoff joiner needs reviewed proration."
    );
    ensure!(
        i.base
            .worksheet
            .days
            .iter()
            .all(|d| joined.is_none_or(|j| d.date >= j)),
        "Worksheet dates cannot precede employment."
    );
    let ot: i64 = i
        .base
        .worksheet
        .days
        .iter()
        .map(|d| i64::from(d.overtime_minutes))
        .sum();
    let first_used: i64 = if r.part == 2 {
        data.payroll
            .cutoffs
            .slips
            .iter()
            .find(|s| {
                s.base.employee_id == i.base.employee_id
                    && s.base.month == r.base.month
                    && s.part == 1
            })
            .map(|s| {
                s.base
                    .item
                    .worksheet
                    .days
                    .iter()
                    .map(|d| i64::from(d.overtime_minutes))
                    .sum()
            })
            .unwrap_or(0)
    } else {
        0
    };
    let cap = if r.part == 2 {
        authorized(
            data,
            i.base.employee_id,
            r.base.month,
            payroll::month_end(r.base.month),
        ) - first_used
    } else {
        authorized(data, i.base.employee_id, r.starts_on, r.ends_on)
    }
    .max(0);
    ensure!(
        ot <= cap || !i.base.worksheet.exception_note.trim().is_empty(),
        "Review overtime exceeding current authorizations after issued first-cutoff usage."
    );
    ensure!(
        i.calculation
            == calculate(
                &i.base.profile.pay,
                &i.base.worksheet,
                r.base.month,
                r.part,
                previous(&i.base)?.as_ref()
            )?
            && i.base.totals == i.calculation.totals
            && i.base.totals.net >= 0,
        "Review the reconciled calculation and correct negative net pay."
    );
    ensure!(
        !data
            .payroll
            .slips
            .iter()
            .any(|s| s.employee_id == i.base.employee_id && s.month == r.base.month),
        "An issued monthly register overlaps this cutoff."
    );
    Ok(())
}
pub fn apply(data: &mut Snapshot, a: &payroll::PayrollAction) -> Result<()> {
    payload(a, data)?;
    let v = &a.record;
    let id: Uuid = serde_json::from_value(v["id"].clone())?;
    let expected = v["expected_revision"].as_i64();
    let op = a.operation.strip_prefix("cutoff_").unwrap();
    let mut before = None;
    let after: Value;
    if op == "run_create" {
        let month: NaiveDate = serde_json::from_value(v["month"].clone())?;
        let part: u8 = serde_json::from_value(v["part"].clone())?;
        let (start, end) = bounds(month, part)?;
        ensure!(
            !data.payroll.cutoffs.runs.iter().any(|r| r.base.id == id
                || (r.base.month == month && r.part == part && r.base.status != "cancelled")),
            "A live cutoff already exists for this period."
        );
        let r = Run {
            base: payroll::Run {
                id,
                company_id: data.company.id,
                month,
                title: v["title"].as_str().unwrap().trim().into(),
                status: "draft".into(),
                revision: 0,
                created_by: data.access.user_id,
                submitted_by: None,
                approved_by: None,
                paid_on: None,
                payment_reference: String::new(),
                rule_version: RULE_VERSION.into(),
            },
            part,
            starts_on: start,
            ends_on: end,
            pay_date: serde_json::from_value(v["pay_date"].clone())?,
        };
        after = serde_json::to_value(&r)?;
        data.payroll.cutoffs.runs.push(r);
    } else if op.starts_with("item_") {
        let ix = data
            .payroll
            .cutoffs
            .items
            .iter()
            .position(|i| i.base.id == id);
        let (rid, eid, old) = if op == "item_add" {
            (
                serde_json::from_value(v["run_id"].clone())?,
                serde_json::from_value(v["employee_id"].clone())?,
                None,
            )
        } else {
            let i = data
                .payroll
                .cutoffs
                .items
                .get(ix.context("Worksheet unavailable.")?)
                .unwrap();
            ensure!(
                Some(i.base.revision) == expected,
                "Worksheet changed. Refresh."
            );
            (i.base.run_id, i.base.employee_id, Some(i.clone()))
        };
        let ri = data
            .payroll
            .cutoffs
            .runs
            .iter()
            .position(|r| r.base.id == rid)
            .context("Cutoff unavailable.")?;
        let r = data.payroll.cutoffs.runs[ri].clone();
        ensure!(
            r.base.status == "draft",
            "Only draft worksheets may change."
        );
        if let Some(i) = &old {
            before = Some(serde_json::to_value(i)?);
        }
        if op == "item_remove" {
            after = before.clone().unwrap();
            data.payroll.cutoffs.items.remove(ix.unwrap());
        } else {
            let fresh = basis(data, eid, &r)?;
            let employee = data
                .employees
                .iter()
                .find(|e| e.id == eid)
                .context("Employee unavailable.")?;
            ensure!(
                employee.joined_on.is_none_or(|d| d <= r.ends_on),
                "Employee joined after this cutoff."
            );
            let profile = data
                .payroll
                .profile(eid, r.base.month)
                .context("Save a salary profile first.")?
                .clone();
            if op == "item_add" {
                ensure!(
                    ix.is_none()
                        && !data
                            .payroll
                            .cutoffs
                            .items
                            .iter()
                            .any(|i| i.base.run_id == rid && i.base.employee_id == eid),
                    "Employee already included."
                );
            }
            if op == "item_save" {
                ensure!(
                    old.as_ref().unwrap().base.source_basis == fresh,
                    "Sources changed. Refresh before saving."
                );
            }
            let w = if op == "item_save" {
                serde_json::from_value(v["worksheet"].clone())?
            } else if let Some(i) = &old {
                let mut w = i.base.worksheet.clone();
                w.reviewed = false;
                w.review_note.clear();
                w
            } else {
                Worksheet::default()
            };
            let prev: Option<Calculation> = if fresh["first_cutoff"].is_null() {
                None
            } else {
                Some(serde_json::from_value(
                    fresh["first_cutoff"]["calculation"].clone(),
                )?)
            };
            let calculation = calculate(&profile.pay, &w, r.base.month, r.part, prev.as_ref())?;
            let i = Item {
                base: payroll::Item {
                    id,
                    company_id: data.company.id,
                    run_id: rid,
                    employee_id: eid,
                    employee_name: employee.full_name.clone(),
                    employee_code: employee.employee_code.clone(),
                    department: employee.department.clone(),
                    profile,
                    source_basis: fresh,
                    worksheet: w,
                    totals: calculation.totals.clone(),
                    revision: old.as_ref().map_or(0, |i| i.base.revision + 1),
                },
                calculation,
            };
            after = serde_json::to_value(&i)?;
            if let Some(ix) = ix {
                data.payroll.cutoffs.items[ix] = i;
            } else {
                data.payroll.cutoffs.items.push(i);
            }
        }
        data.payroll.cutoffs.runs[ri].base.revision += 1;
    } else {
        let ri = data
            .payroll
            .cutoffs
            .runs
            .iter()
            .position(|r| r.base.id == id)
            .context("Cutoff unavailable.")?;
        let mut r = data.payroll.cutoffs.runs[ri].clone();
        ensure!(
            Some(r.base.revision) == expected,
            "Cutoff changed. Refresh."
        );
        before = Some(serde_json::to_value(&r)?);
        if op == "run_submit" || op == "run_approve" {
            ensure!(
                r.ends_on <= data.company.today() && r.base.month.year() == 2026,
                "Approve completed2026cutoffs only."
            );
            let items: Vec<_> = data
                .payroll
                .cutoffs
                .items
                .iter()
                .filter(|i| i.base.run_id == id)
                .collect();
            ensure!(!items.is_empty(), "Include at least one employee.");
            for i in items {
                check(data, i, &r)?;
            }
        }
        match op {
            "run_update" => {
                ensure!(
                    r.base.status == "draft",
                    "Only draft title/pay date can change."
                );
                r.base.title = v["title"].as_str().unwrap().trim().into();
                r.pay_date = serde_json::from_value(v["pay_date"].clone())?;
            }
            "run_submit" => {
                ensure!(r.base.status == "draft", "Submit a draft cutoff.");
                r.base.status = "submitted".into();
                r.base.submitted_by = Some(data.access.user_id);
            }
            "run_return" => {
                ensure!(
                    r.base.status == "submitted",
                    "Only submitted cutoff returns to draft."
                );
                r.base.status = "draft".into();
                r.base.submitted_by = None;
            }
            "run_cancel" => {
                ensure!(
                    ["draft", "submitted"].contains(&r.base.status.as_str()),
                    "Issued cutoff pay stays immutable."
                );
                r.base.status = "cancelled".into();
                r.base.submitted_by = None;
            }
            "run_approve" => {
                ensure!(
                    r.base.status == "submitted"
                        && r.base.submitted_by != Some(data.access.user_id),
                    "Another Administrator must approve submitted cutoff pay."
                );
                for i in data
                    .payroll
                    .cutoffs
                    .items
                    .iter()
                    .filter(|i| i.base.run_id == id)
                {
                    let mut item = i.base.clone();
                    item.source_basis = Value::Null;
                    item.profile.pay.coverage_note.clear();
                    item.worksheet.review_note.clear();
                    item.worksheet.exception_note.clear();
                    for c in &mut item.profile.pay.components {
                        c.amount_cents = allocated(c.amount_cents, r.part);
                        c.reference.clear();
                    }
                    for c in &mut item.worksheet.adjustments {
                        c.reference.clear();
                    }
                    data.payroll.cutoffs.slips.push(Slip {
                        base: payroll::Slip {
                            id: item.id,
                            company_id: data.company.id,
                            run_id: id,
                            employee_id: item.employee_id,
                            month: r.base.month,
                            company_name: data.company.name.clone(),
                            item,
                            approved_by: data.access.user_id,
                            approved_at: Utc::now(),
                            rule_version: RULE_VERSION.into(),
                        },
                        part: r.part,
                        starts_on: r.starts_on,
                        ends_on: r.ends_on,
                        pay_date: r.pay_date,
                        calculation: i.calculation.clone(),
                    });
                }
                r.base.status = "approved".into();
                r.base.approved_by = Some(data.access.user_id);
            }
            "run_paid" => {
                let d: NaiveDate = serde_json::from_value(v["paid_on"].clone())?;
                ensure!(
                    r.base.status == "approved" && d >= r.ends_on && d <= data.company.today(),
                    "Record an actual payment after cutoff end."
                );
                r.base.status = "paid".into();
                r.base.paid_on = Some(d);
                r.base.payment_reference = v["payment_reference"].as_str().unwrap().trim().into();
            }
            _ => anyhow::bail!("Unknown cutoff action."),
        }
        r.base.revision += 1;
        after = serde_json::to_value(&r)?;
        data.payroll.cutoffs.runs[ri] = r;
    }
    data.payroll.events.push(payroll::Event {
        id: Uuid::new_v4(),
        company_id: data.company.id,
        record_id: id,
        kind: a.operation.clone(),
        reason: v["reason"].as_str().unwrap().trim().into(),
        actor_name: data
            .own_employee()
            .map(|e| e.full_name.clone())
            .unwrap_or_else(|| "HR staff".into()),
        actor_role: data.access.role,
        before_record: before,
        after_record: after,
        recorded_at: Utc::now(),
    });
    Ok(())
}
