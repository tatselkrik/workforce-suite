use super::*;
use crate::payroll_payments;
#[derive(Clone, Debug)]
pub struct Target {
    pub kind: String,
    pub id: Uuid,
    pub employee_id: Uuid,
    pub employee_name: String,
    pub employee_code: String,
    pub department: String,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    pub paid_on: NaiveDate,
    pub net_cents: i64,
    pub sss_cents: i64,
    pub philhealth_cents: i64,
    pub pagibig_cents: i64,
    pub withholding_cents: i64,
    pub facts: Value,
}
pub fn targets(data: &Snapshot) -> Result<Vec<Target>> {
    ensure!(
        data.access.allows(crate::staff_roles::Capability::PayrollPreparation)
            && data.access.company_id == data.company.id
            && data.mode != Mode::SignedOut,
        "Only current Payroll Officers and Administrators select original payroll sources."
    );
    let mut rows = vec![];
    for p in payroll_payments::candidates(data)?
        .into_iter()
        .filter(|p| p.paid_on.is_some())
    {
        let (sss, ph, pi, wh, facts) = match p.kind.as_str() {
            "monthly" | "cutoff" => {
                let item = if p.kind == "monthly" {
                    &data
                        .payroll
                        .slips
                        .iter()
                        .find(|s| s.id == p.slip_id)
                        .context("Refresh salary source.")?
                        .item
                } else {
                    &data
                        .payroll
                        .cutoffs
                        .slips
                        .iter()
                        .find(|s| s.base.id == p.slip_id)
                        .context("Refresh cutoff source.")?
                        .base
                        .item
                };
                let t = &item.totals;
                (
                    t.sss_employee,
                    t.philhealth_employee,
                    t.pagibig_employee,
                    t.withholding,
                    serde_json::to_value(t)?,
                )
            }
            _ => {
                let t = &data
                    .payroll
                    .thirteenth
                    .slips
                    .iter()
                    .find(|s| s.id == p.slip_id)
                    .context("Refresh annual benefit source.")?
                    .item
                    .totals;
                (
                    t.sss_adjustment_cents,
                    t.philhealth_adjustment_cents,
                    t.pagibig_adjustment_cents,
                    t.withholding_cents,
                    serde_json::to_value(t)?,
                )
            }
        };
        let (lo, department) = match p.kind.as_str() {
            "monthly" => {
                let s = data
                    .payroll
                    .slips
                    .iter()
                    .find(|s| s.id == p.slip_id)
                    .unwrap();
                (s.month, s.item.department.clone())
            }
            "cutoff" => {
                let s = data
                    .payroll
                    .cutoffs
                    .slips
                    .iter()
                    .find(|s| s.base.id == p.slip_id)
                    .unwrap();
                (s.starts_on, s.base.item.department.clone())
            }
            _ => {
                let s = data
                    .payroll
                    .thirteenth
                    .slips
                    .iter()
                    .find(|s| s.id == p.slip_id)
                    .unwrap();
                (s.as_of, s.item.department.clone())
            }
        };
        rows.push(Target {
            kind: p.kind,
            id: p.slip_id,
            employee_id: p.employee_id,
            employee_name: p.employee_name,
            employee_code: p.employee_code,
            department,
            starts_on: lo,
            ends_on: p.ends_on,
            paid_on: p.paid_on.unwrap(),
            net_cents: p.amount_cents,
            sss_cents: sss,
            philhealth_cents: ph,
            pagibig_cents: pi,
            withholding_cents: wh,
            facts,
        });
    }
    for s in &data.payroll.finals.slips {
        if s.company_id == data.company.id
            && data
                .payroll
                .finals
                .statuses
                .iter()
                .any(|p| p.id == s.id && p.kind == "settled")
        {
            let r = data
                .payroll
                .finals
                .records
                .iter()
                .find(|r| r.id == s.id)
                .context("Refresh settled final statement.")?;
            rows.push(Target {
                kind: "final".into(),
                id: s.id,
                employee_id: s.employee_id,
                employee_name: s.employee_name.clone(),
                employee_code: s.employee_code.clone(),
                department: s.department.clone(),
                starts_on: s.employment_start,
                ends_on: s.service_end,
                paid_on: s.cash_date,
                net_cents: s.totals.net_cents,
                sss_cents: r.worksheet.sss_adjustment_cents,
                philhealth_cents: r.worksheet.philhealth_adjustment_cents,
                pagibig_cents: r.worksheet.pagibig_adjustment_cents,
                withholding_cents: s.totals.tax.adjustment_cents.max(0),
                facts: serde_json::to_value(&s.totals)?,
            });
        }
    }
    rows.sort_by(|a, b| {
        a.employee_code
            .cmp(&b.employee_code)
            .then(a.ends_on.cmp(&b.ends_on))
            .then(a.kind.cmp(&b.kind))
            .then(a.id.cmp(&b.id))
    });
    Ok(rows)
}
fn active(r: &Record) -> bool {
    ["approved", "paid"].contains(&r.status.as_str()) && r.worksheet.purpose == "payout"
}
pub(crate) fn benefit_facts(data: &Snapshot, eid: Uuid, year: i32, exclude: Uuid) -> Value {
    let mut gross = 0;
    let mut ext = 0;
    let mut other = 0;
    for s in data
        .payroll
        .thirteenth
        .slips
        .iter()
        .filter(|s| s.company_id == data.company.id && s.employee_id == eid && s.year == year)
    {
        gross += s.item.totals.gross_cents;
        ext = ext.max(s.item.worksheet.external_thirteenth_cents);
        let b = data
            .payroll
            .thirteenth
            .items
            .iter()
            .find(|i| i.id == s.id)
            .map(|i| &i.source_basis);
        let known_final = b
            .and_then(|v| v["final_shared_benefits_cents"].as_i64())
            .unwrap_or(0);
        let known_amend = b
            .and_then(|v| v["amendment_shared_benefits_cents"].as_i64())
            .unwrap_or(0);
        other =
            other.max((s.item.worksheet.other_benefits_cents - known_final - known_amend).max(0));
    }
    let final_cents = data
        .payroll
        .finals
        .records
        .iter()
        .filter(|r| {
            r.employee_id == eid
                && r.cash_date.year() == year
                && ["approved", "settled"].contains(&r.status.as_str())
        })
        .map(|r| r.totals.shared_extras_cents)
        .sum::<i64>();
    let amendment_cents = data
        .payroll
        .amendments
        .records
        .iter()
        .filter(|r| {
            r.id != exclude && r.employee_id == eid && r.cash_date.year() == year && active(r)
        })
        .map(|r| r.totals.shared_cents)
        .sum::<i64>();
    let outside = data
        .payroll
        .amendments
        .records
        .iter()
        .filter(|r| {
            r.id != exclude && r.employee_id == eid && r.cash_date.year() == year && active(r)
        })
        .map(external_pool)
        .max()
        .unwrap_or(0);
    other = other.max(outside - ext);
    json!({"floor_cents":gross+ext+other+final_cents+amendment_cents,"issued_thirteenth_cents":gross,"external_thirteenth_cents":ext,"external_other_cents":other,"final_cents":final_cents,"amendment_cents":amendment_cents})
}
pub(crate) fn external_pool(r: &Record) -> i64 {
    let b = &r.source_basis["benefits"];
    (r.worksheet.other_benefits_cents
        - b["issued_thirteenth_cents"].as_i64().unwrap_or(0)
        - b["final_cents"].as_i64().unwrap_or(0)
        - b["amendment_cents"].as_i64().unwrap_or(0))
    .max(0)
}
pub fn demo_basis(
    data: &Snapshot,
    kind: &str,
    id: Uuid,
    cash: NaiveDate,
    exclude: Uuid,
) -> Result<Value> {
    let t=targets(data)?.into_iter().find(|t|t.kind==kind&&t.id==id).context("Select an actually paid original slip. Unpaid final statements may be withdrawn; original receipts remain immutable.")?;
    let mut prior = data
        .payroll
        .amendments
        .records
        .iter()
        .filter(|r| {
            r.id != exclude
                && r.target_kind == kind
                && r.target_id == id
                && ["approved", "paid", "reviewed"].contains(&r.status.as_str())
        })
        .collect::<Vec<_>>();
    prior.sort_by_key(|r| r.id);
    let refund = |base: i64, f: fn(&Worksheet) -> i64| {
        base + prior
            .iter()
            .filter(|r| active(r))
            .map(|r| f(&r.worksheet))
            .sum::<i64>()
    };
    let credits = (targets(data)?
        .iter()
        .filter(|p| {
            p.employee_id == t.employee_id && p.kind != "final" && p.paid_on.year() == cash.year()
        })
        .map(|p| p.withholding_cents)
        .sum::<i64>()
        + data
            .payroll
            .finals
            .records
            .iter()
            .filter(|r| {
                r.employee_id == t.employee_id
                    && r.status == "settled"
                    && r.cash_date.year() == cash.year()
            })
            .map(|r| r.totals.tax.adjustment_cents)
            .sum::<i64>()
        + data
            .payroll
            .tax
            .records
            .iter()
            .filter(|r| {
                r.employee_id == t.employee_id && r.status == "settled" && r.year == cash.year()
            })
            .map(|r| r.totals.adjustment_cents)
            .sum::<i64>()
        + data
            .payroll
            .amendments
            .records
            .iter()
            .filter(|r| {
                r.id != exclude
                    && r.employee_id == t.employee_id
                    && r.status == "paid"
                    && r.cash_date.year() == cash.year()
            })
            .map(|r| r.totals.withholding_cents)
            .sum::<i64>())
    .max(0);
    let benefits = benefit_facts(data, t.employee_id, cash.year(), exclude);
    let s = Source {
        kind: kind.into(),
        slip_id: id,
        employee_id: t.employee_id,
        starts_on: t.starts_on,
        ends_on: t.ends_on,
        paid_on: t.paid_on,
        cash_date: cash,
        original_net_cents: t.net_cents,
        sss_refundable_cents: refund(t.sss_cents, |w| w.sss_cents),
        philhealth_refundable_cents: refund(t.philhealth_cents, |w| w.philhealth_cents),
        pagibig_refundable_cents: refund(t.pagibig_cents, |w| w.pagibig_cents),
        withholding_refundable_cents: refund(t.withholding_cents, |w| w.withholding_cents)
            .min(credits),
        shared_floor_cents: benefits["floor_cents"].as_i64().unwrap(),
    };
    Ok(
        json!({"calculation_source":s,"original_totals":t.facts,"employee":{"id":t.employee_id,"full_name":t.employee_name,"employee_code":t.employee_code,"department":t.department},"prior":prior.iter().map(|r|json!({"id":r.id,"status":r.status,"revision":r.revision,"totals":r.totals,"sss_cents":r.worksheet.sss_cents,"philhealth_cents":r.worksheet.philhealth_cents,"pagibig_cents":r.worksheet.pagibig_cents,"withholding_cents":r.worksheet.withholding_cents})).collect::<Vec<_>>(),"benefits":benefits,"remaining_tax_credit_cents":credits}),
    )
}
fn check(data: &Snapshot, r: &Record) -> Result<()> {
    ensure!(
        demo_basis(data, &r.target_kind, r.target_id, r.cash_date, r.id)? == r.source_basis,
        "Linked pay, earlier adjustments or benefit facts changed. Refresh and review; withdraw an unpaid issued amendment if necessary."
    );
    ensure!(
        ready(&source(r)?, &r.worksheet)? == r.totals && r.rule_version == RULE_VERSION,
        "Amendment totals/rule changed. Refresh."
    );
    Ok(())
}
pub fn apply_demo(data: &mut Snapshot, a: &PayrollAction) -> Result<()> {
    let mut copy = data.clone();
    inner(&mut copy, a)?;
    *data = copy;
    Ok(())
}
fn inner(data: &mut Snapshot, a: &PayrollAction) -> Result<()> {
    payload(a, data)?;
    let op = a.operation.strip_prefix("amendment_").unwrap();
    let v = &a.record;
    let id: Uuid = serde_json::from_value(v["id"].clone())?;
    // Actor-bound idempotency survives a completed write followed by a failed refresh.
    if ["create", "pay", "withdraw"].contains(&op) {
        if let Some(e) = data
            .payroll
            .events
            .iter()
            .find(|e| e.record_id == id && e.kind == a.operation)
        {
            ensure!(
                e.after_record["actor_id"] == json!(data.access.user_id)
                    && e.after_record.get("request") == Some(v),
                "This amendment operation identity was already used."
            );
            return Ok(());
        }
    }
    let before = data
        .payroll
        .amendments
        .records
        .iter()
        .find(|r| r.id == id)
        .cloned();
    let mut r;
    if op == "create" {
        ensure!(before.is_none(), "This amendment identity already exists.");
        let kind = v["target_kind"].as_str().unwrap();
        let target: Uuid = serde_json::from_value(v["target_id"].clone())?;
        let cash: NaiveDate = serde_json::from_value(v["cash_date"].clone())?;
        let b = demo_basis(data, kind, target, cash, id)?;
        let s: Source = serde_json::from_value(b["calculation_source"].clone())?;
        let purpose = v["purpose"].as_str().unwrap();
        let w = Worksheet {
            purpose: purpose.into(),
            other_benefits_cents: if purpose == "payout" {
                s.shared_floor_cents
            } else {
                0
            },
            ..Default::default()
        };
        let totals = calculate(&s, &w)?;
        r = Record {
            id,
            company_id: data.company.id,
            employee_id: s.employee_id,
            target_kind: kind.into(),
            target_id: target,
            starts_on: s.starts_on,
            ends_on: s.ends_on,
            cash_date: cash,
            title: v["title"].as_str().unwrap().into(),
            employee_name: b["employee"]["full_name"].as_str().unwrap().into(),
            employee_code: b["employee"]["employee_code"].as_str().unwrap().into(),
            department: b["employee"]["department"].as_str().unwrap().into(),
            status: "draft".into(),
            revision: 0,
            created_by: data.access.user_id,
            prepared_by: data.access.user_id,
            submitted_by: None,
            approved_by: None,
            approved_at: None,
            paid_on: None,
            payment_reference: String::new(),
            source_basis: b,
            worksheet: w,
            totals,
            rule_version: RULE_VERSION.into(),
        };
    } else {
        r = before.clone().context("Amendment unavailable.")?;
        ensure!(
            Some(r.revision) == v["expected_revision"].as_i64(),
            "Amendment changed. Refresh."
        );
        match op {
            "save" | "refresh" => {
                ensure!(r.status == "draft", "Only draft amendments may change.");
                let fresh = demo_basis(data, &r.target_kind, r.target_id, r.cash_date, r.id)?;
                if op == "save" {
                    ensure!(
                        fresh == r.source_basis,
                        "Amendment sources changed. Refresh."
                    );
                    r.worksheet = serde_json::from_value(v["worksheet"].clone())?;
                } else {
                    r.source_basis = fresh;
                    r.worksheet.reviewed = false;
                    r.worksheet.tax_reviewed = false;
                    r.worksheet.contributions_reviewed = false;
                    r.worksheet.review_note.clear();
                    if r.worksheet.purpose == "payout" {
                        r.worksheet.other_benefits_cents = r
                            .worksheet
                            .other_benefits_cents
                            .max(source(&r)?.shared_floor_cents);
                    }
                }
                r.totals = match calculate(&source(&r)?, &r.worksheet) {
                    Ok(c) => c,
                    Err(_) if op == "refresh" => {
                        r.worksheet = Worksheet {
                            purpose: r.worksheet.purpose.clone(),
                            other_benefits_cents: if r.worksheet.purpose == "payout" {
                                source(&r)?.shared_floor_cents
                            } else {
                                0
                            },
                            ..Default::default()
                        };
                        calculate(&source(&r)?, &r.worksheet)?
                    }
                    Err(e) => return Err(e),
                };
                r.prepared_by = data.access.user_id;
            }
            "submit" => {
                ensure!(r.status == "draft", "Submit a draft amendment.");
                check(data, &r)?;
                r.status = "submitted".into();
                r.submitted_by = Some(data.access.user_id);
            }
            "approve" => {
                ensure!(
                    r.status == "submitted",
                    "Submit before independent approval."
                );
                ensure!(
                    ![Some(r.created_by), Some(r.prepared_by), r.submitted_by]
                        .contains(&Some(data.access.user_id)),
                    "An independent Administrator must approve this amendment."
                );
                check(data, &r)?;
                ensure!(
                    r.worksheet.purpose == "review_only"
                        || (!data
                            .payroll
                            .amendments
                            .records
                            .iter()
                            .any(|p| p.employee_id == r.employee_id
                                && p.id != r.id
                                && p.status == "approved")
                            && !data
                                .payroll
                                .finals
                                .records
                                .iter()
                                .any(|p| p.employee_id == r.employee_id && p.status == "approved")
                            && !data
                                .payroll
                                .tax
                                .records
                                .iter()
                                .any(|p| p.employee_id == r.employee_id && p.status == "approved")),
                    "Settle or withdraw the earlier pending amendment, final statement or tax advice first."
                );
                r.approved_by = Some(data.access.user_id);
                r.approved_at = Some(Utc::now());
                r.status = if r.worksheet.purpose == "review_only" {
                    "reviewed"
                } else {
                    "approved"
                }
                .into();
                data.payroll.amendments.slips.push(Slip {
                    id: r.id,
                    company_id: r.company_id,
                    employee_id: r.employee_id,
                    target_kind: r.target_kind.clone(),
                    target_id: r.target_id,
                    starts_on: r.starts_on,
                    ends_on: r.ends_on,
                    cash_date: r.cash_date,
                    title: r.title.clone(),
                    company_name: data.company.name.clone(),
                    employee_name: r.employee_name.clone(),
                    employee_code: r.employee_code.clone(),
                    department: r.department.clone(),
                    purpose: r.worksheet.purpose.clone(),
                    totals: r.totals.clone(),
                    approved_by: r.approved_by.unwrap(),
                    approved_at: r.approved_at.unwrap(),
                    rule_version: r.rule_version.clone(),
                });
            }
            "return" => {
                ensure!(r.status == "submitted", "Return a submitted review.");
                r.status = "draft".into();
                r.submitted_by = None;
            }
            "cancel" => {
                ensure!(
                    ["draft", "submitted"].contains(&r.status.as_str()),
                    "Issued amendments remain immutable."
                );
                r.status = "cancelled".into();
                r.submitted_by = None;
            }
            "withdraw" => {
                ensure!(
                    r.status == "approved",
                    "Withdraw only an unpaid approved supplement."
                );
                r.status = "withdrawn".into();
                data.payroll.amendments.statuses.push(Status {
                    id,
                    company_id: r.company_id,
                    employee_id: r.employee_id,
                    kind: "withdrawn".into(),
                    recorded_on: data.company.today(),
                    net_cents: 0,
                });
            }
            "pay" => {
                ensure!(
                    r.status == "approved",
                    "Record an approved supplemental payout."
                );
                let on: NaiveDate = serde_json::from_value(v["paid_on"].clone())?;
                let zone: chrono_tz::Tz = data.company.timezone.parse()?;
                ensure!(
                    on == r.cash_date
                        && on <= data.company.today()
                        && on >= r.approved_at.unwrap().with_timezone(&zone).date_naive(),
                    "Confirm actual cash on the approved date, after approval. Withdraw an unpaid amendment to reschedule."
                );
                check(data, &r)?;
                r.status = "paid".into();
                r.paid_on = Some(on);
                r.payment_reference = v["payment_reference"].as_str().unwrap().into();
                data.payroll.amendments.statuses.push(Status {
                    id,
                    company_id: r.company_id,
                    employee_id: r.employee_id,
                    kind: "paid".into(),
                    recorded_on: on,
                    net_cents: r.totals.net_cents,
                });
            }
            _ => anyhow::bail!("Unknown amendment operation."),
        }
        r.revision += 1;
    }
    data.payroll.amendments.records.retain(|x| x.id != id);
    data.payroll.amendments.records.push(r.clone());
    let after = json!({"record":r,"request":v,"actor_id":data.access.user_id});
    data.payroll.events.push(crate::payroll::Event {
        id: Uuid::new_v4(),
        company_id: data.company.id,
        record_id: id,
        kind: a.operation.clone(),
        reason: v["reason"].as_str().unwrap().into(),
        actor_name: data
            .employee_name(data.access.employee_id.unwrap_or_default())
            .into(),
        actor_role: data.access.role,
        before_record: before.map(|r| serde_json::to_value(r).unwrap()),
        after_record: after,
        recorded_at: Utc::now(),
    });
    Ok(())
}
