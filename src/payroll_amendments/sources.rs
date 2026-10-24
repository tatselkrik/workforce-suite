use super::*;
fn external_other(data: &Snapshot, eid: Uuid, year: i32, asof: Option<NaiveDate>) -> i64 {
    data.payroll
        .thirteenth
        .slips
        .iter()
        .filter(|s| {
            s.company_id == data.company.id
                && s.employee_id == eid
                && s.year == year
                && asof.is_none_or(|d| {
                    s.as_of <= d
                        && data.payroll.receipts.iter().any(|p| {
                            p.kind == "thirteenth"
                                && p.slip_id == s.id
                                && p.paid_on.year() == year
                                && p.paid_on <= d
                        })
                })
        })
        .map(|s| {
            let b = data
                .payroll
                .thirteenth
                .items
                .iter()
                .find(|i| i.id == s.id)
                .map(|i| &i.source_basis);
            (s.item.worksheet.other_benefits_cents
                - b.and_then(|b| b["final_shared_benefits_cents"].as_i64())
                    .unwrap_or(0)
                - b.and_then(|b| b["amendment_shared_benefits_cents"].as_i64())
                    .unwrap_or(0))
            .max(0)
        })
        .max()
        .unwrap_or(0)
}
pub(crate) fn annual(
    data: &Snapshot,
    r: &crate::payroll_tax::Record,
    mut b: Value,
) -> Result<Value> {
    let mut s: crate::payroll_tax::Source =
        serde_json::from_value(b["calculation_source"].clone())?;
    let mut rows = data
        .payroll
        .amendments
        .records
        .iter()
        .filter(|p| {
            p.company_id == data.company.id
                && p.employee_id == r.employee_id
                && p.cash_date.year() == r.year
                && ["approved", "paid"].contains(&p.status.as_str())
        })
        .collect::<Vec<_>>();
    rows.sort_by_key(|p| p.id);
    let mut facts = vec![];
    for p in rows {
        ensure!(
            p.cash_date <= r.as_of,
            "Use an as-of date at least as late as the earlier issued amendment."
        );
        if p.status == "approved" {
            s.pending_adjustments += 1;
        } else {
            s.income.push(crate::payroll_tax::Income {
                id: format!("amendment:{}", p.id),
                starts_on: None,
                ends_on: None,
                taxable_cents: p.totals.taxable_cents,
                withholding_cents: p.totals.withholding_cents.max(0),
                benefit_gross_cents: p.totals.shared_cents,
                benefit_exempt_cents: p.totals.benefit_exempt_cents,
                contribution_cents: 0,
            });
            s.prior_adjustment_cents += p.totals.withholding_cents.min(0);
        }
        facts.push(json!({"id":p.id,"status":p.status,"revision":p.revision,"cash_date":p.cash_date,"totals":p.totals}));
    }
    let outside = data
        .payroll
        .amendments
        .records
        .iter()
        .filter(|p| {
            p.employee_id == r.employee_id
                && p.cash_date.year() == r.year
                && p.cash_date <= r.as_of
                && p.status == "paid"
        })
        .map(super::workflow::external_pool)
        .max()
        .unwrap_or(0);
    s.other_benefits_floor = external_other(data, r.employee_id, r.year, Some(r.as_of))
        .max(outside - s.external_thirteenth_floor);
    b["calculation_source"] = serde_json::to_value(s)?;
    if !facts.is_empty() {
        b["amendments"] = json!(facts);
    }
    Ok(b)
}
pub(crate) fn benefits(
    data: &Snapshot,
    eid: Uuid,
    r: &crate::payroll_thirteenth::Run,
    mut b: Value,
) -> Result<Value> {
    let mut s: crate::payroll_thirteenth::Source =
        serde_json::from_value(b["calculation_source"].clone())?;
    let mut rows = data
        .payroll
        .amendments
        .records
        .iter()
        .filter(|p| {
            p.company_id == data.company.id
                && p.employee_id == eid
                && ["approved", "paid"].contains(&p.status.as_str())
        })
        .collect::<Vec<_>>();
    rows.sort_by_key(|p| p.id);
    let (mut known, mut facts) = (0, vec![]);
    for p in rows {
        if ["monthly", "cutoff"].contains(&p.target_kind.as_str()) {
            if let Some(period) = s.periods.iter_mut().find(|s| s.id == p.target_id) {
                period.basic_cents += p.totals.basic_cents;
            }
        }
        if p.target_kind == "thirteenth" {
            if data
                .payroll
                .thirteenth
                .slips
                .iter()
                .any(|s| s.id == p.target_id && s.year == r.year && s.as_of <= r.as_of)
            {
                s.prior.push(crate::payroll_thirteenth::Prior {
                    id: p.id,
                    gross_cents: p.totals.shared_cents,
                    exempt_cents: p.totals.benefit_exempt_cents,
                });
            }
        } else if p.cash_date.year() == r.year {
            known += p.totals.shared_cents;
            facts.push(json!({"id":p.id,"status":p.status,"target_kind":p.target_kind,"target_id":p.target_id,"totals":p.totals}));
        }
    }
    s.other_benefits_floor =
        super::workflow::benefit_facts(data, eid, r.year, Uuid::nil())["external_other_cents"]
            .as_i64()
            .unwrap()
            + b["final_shared_benefits_cents"].as_i64().unwrap_or(0)
            + known;
    b["calculation_source"] = serde_json::to_value(s)?;
    if !facts.is_empty() {
        b["amendment_shared_benefits_cents"] = json!(known);
        b["amendments"] = json!(facts);
    }
    Ok(b)
}
