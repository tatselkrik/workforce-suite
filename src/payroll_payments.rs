//! Full issued-slip payment records; recording a payment never moves money.
use crate::{
    access::Role,
    domain::{Mode, Snapshot},
    payroll::{self, PayrollAction},
};
use anyhow::{Context, Result, ensure};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Receipt {
    pub id: Uuid,
    pub company_id: Uuid,
    pub employee_id: Uuid,
    pub kind: String,
    pub slip_id: Uuid,
    pub run_id: Uuid,
    pub amount_cents: i64,
    pub paid_on: NaiveDate,
    pub payment_reference: String,
    pub reason: String,
    pub origin: String,
    pub recorded_by: Option<Uuid>,
    pub recorded_at: DateTime<Utc>,
    #[serde(default)]
    pub final_case_id: Option<Uuid>,
}
#[derive(Clone, Debug)]
pub struct Candidate {
    pub kind: String,
    pub slip_id: Uuid,
    pub run_id: Uuid,
    pub employee_id: Uuid,
    pub employee_name: String,
    pub employee_code: String,
    pub title: String,
    pub ends_on: NaiveDate,
    pub revision: i64,
    pub amount_cents: i64,
    pub paid_on: Option<NaiveDate>,
    pub payment_reference: String,
}
pub fn effective(
    data: &Snapshot,
    kind: &str,
    slip: Uuid,
    legacy: Option<NaiveDate>,
    status: &str,
) -> (Option<NaiveDate>, bool) {
    if let Some(r) = data
        .payroll
        .receipts
        .iter()
        .find(|r| r.company_id == data.company.id && r.kind == kind && r.slip_id == slip)
    {
        (Some(r.paid_on), true)
    } else {
        (legacy, status == "paid")
    }
}
pub fn candidates(data: &Snapshot) -> Result<Vec<Candidate>> {
    ensure!(
        data.mode != Mode::SignedOut
            && data.access.allows(crate::staff_roles::Capability::PayrollPreparation)
            && data.access.company_id == data.company.id,
        "HR access is required to review payment records."
    );
    let mut rows = vec![];
    let mut add = |kind: &str,
                   slip: Uuid,
                   run: Uuid,
                   employee: Uuid,
                   name: &str,
                   code: &str,
                   title: &str,
                   end: NaiveDate,
                   revision: i64,
                   net: i64,
                   status: &str,
                   paid: Option<NaiveDate>,
                   reference: &str| {
        if !["approved", "paid"].contains(&status) {
            return;
        }
        let (date, is_paid) = effective(data, kind, slip, paid, status);
        let reference = data
            .payroll
            .receipts
            .iter()
            .find(|r| r.kind == kind && r.slip_id == slip && r.company_id == data.company.id)
            .map(|r| r.payment_reference.clone())
            .unwrap_or_else(|| reference.into());
        rows.push(Candidate {
            kind: kind.into(),
            slip_id: slip,
            run_id: run,
            employee_id: employee,
            employee_name: name.into(),
            employee_code: code.into(),
            title: title.into(),
            ends_on: end,
            revision,
            amount_cents: net,
            paid_on: if is_paid { date } else { None },
            payment_reference: reference,
        });
    };
    for s in data
        .payroll
        .slips
        .iter()
        .filter(|s| s.company_id == data.company.id)
    {
        let r = data
            .payroll
            .runs
            .iter()
            .find(|r| r.id == s.run_id && r.company_id == data.company.id)
            .context("Refresh monthly run payment status.")?;
        add(
            "monthly",
            s.id,
            r.id,
            s.employee_id,
            &s.item.employee_name,
            &s.item.employee_code,
            &r.title,
            payroll::month_end(s.month),
            r.revision,
            s.item.totals.net,
            &r.status,
            r.paid_on,
            &r.payment_reference,
        );
    }
    for s in data
        .payroll
        .cutoffs
        .slips
        .iter()
        .filter(|s| s.base.company_id == data.company.id)
    {
        let r = data
            .payroll
            .cutoffs
            .runs
            .iter()
            .find(|r| r.base.id == s.base.run_id && r.base.company_id == data.company.id)
            .context("Refresh cutoff run payment status.")?;
        let p = &s.base;
        add(
            "cutoff",
            p.id,
            r.base.id,
            p.employee_id,
            &p.item.employee_name,
            &p.item.employee_code,
            &r.base.title,
            s.ends_on,
            r.base.revision,
            p.item.totals.net,
            &r.base.status,
            r.base.paid_on,
            &r.base.payment_reference,
        );
    }
    for s in data
        .payroll
        .thirteenth
        .slips
        .iter()
        .filter(|s| s.company_id == data.company.id)
    {
        let r = data
            .payroll
            .thirteenth
            .runs
            .iter()
            .find(|r| r.id == s.run_id && r.company_id == data.company.id)
            .context("Refresh benefit run payment status.")?;
        add(
            "thirteenth",
            s.id,
            r.id,
            s.employee_id,
            &s.item.employee_name,
            &s.item.employee_code,
            &r.title,
            s.as_of,
            r.revision,
            s.item.totals.net_cents,
            &r.status,
            r.paid_on,
            &r.payment_reference,
        );
    }
    rows.sort_by(|a, b| {
        b.ends_on
            .cmp(&a.ends_on)
            .then_with(|| a.employee_code.cmp(&b.employee_code))
            .then_with(|| a.kind.cmp(&b.kind))
    });
    Ok(rows)
}
pub fn payload(a: &PayrollAction, data: &Snapshot) -> Result<Value> {
    ensure!(
        data.mode != Mode::SignedOut
            && data.access.company_id == data.company.id
            && data.access.role == Role::Administrator,
        "Only Administrators record actual payments."
    );
    ensure!(
        a.operation == "receipt_record",
        "Unsupported payment operation."
    );
    let v = &a.record;
    let o = v.as_object().context("Enter payment details.")?;
    ensure!(
        v.to_string().len() <= 8192
            && o.keys().all(|k| [
                "id",
                "kind",
                "slip_id",
                "expected_revision",
                "paid_on",
                "payment_reference",
                "reason"
            ]
            .contains(&k.as_str())),
        "Unsupported payment fields."
    );
    let id: Uuid = serde_json::from_value(v["id"].clone())?;
    let sid: Uuid = serde_json::from_value(v["slip_id"].clone())?;
    ensure!(
        !id.is_nil() && !sid.is_nil() && v["expected_revision"].as_i64().is_some_and(|n| n >= 0),
        "Payment identity and current run revision required."
    );
    let k = v["kind"].as_str().unwrap_or("");
    ensure!(
        ["monthly", "cutoff", "thirteenth"].contains(&k),
        "Choose a supported payslip."
    );
    for key in ["payment_reference", "reason"] {
        payroll::text(
            v[key]
                .as_str()
                .context("Enter a payment reference and reason.")?,
            500,
            true,
        )?;
    }
    let date: NaiveDate = serde_json::from_value(v["paid_on"].clone())?;
    let c = candidates(data)?
        .into_iter()
        .find(|c| c.kind == k && c.slip_id == sid)
        .context("Refresh this issued payslip.")?;
    ensure!(
        date >= c.ends_on && date <= data.company.today(),
        "Use an actual payment date after this completed period."
    );
    ensure!(
        c.paid_on.is_none()
            || data
                .payroll
                .receipts
                .iter()
                .any(|r| r.id == id && r.slip_id == sid && r.kind == k),
        "This payslip already has a payment record. Refresh."
    );
    Ok(json!({"p_operation":"record","p_record":v}))
}
pub fn apply_demo(data: &mut Snapshot, a: &PayrollAction) -> Result<()> {
    payload(a, data)?;
    let v = &a.record;
    let id: Uuid = serde_json::from_value(v["id"].clone())?;
    if let Some(r) = data.payroll.receipts.iter().find(|r| r.id == id) {
        ensure!(
            r.recorded_by == Some(data.access.user_id)
                && data.payroll.events.iter().any(|e| e.record_id == id
                    && e.kind == "receipt_record"
                    && e.after_record["request"] == *v),
            "This payment identity was already used."
        );
        return Ok(());
    }
    let sid: Uuid = serde_json::from_value(v["slip_id"].clone())?;
    let k = v["kind"].as_str().unwrap();
    let c = candidates(data)?
        .into_iter()
        .find(|c| c.kind == k && c.slip_id == sid)
        .unwrap();
    ensure!(
        v["expected_revision"] == json!(c.revision),
        "Payroll payment status changed. Refresh."
    );
    let r = Receipt {
        id,
        company_id: data.company.id,
        employee_id: c.employee_id,
        kind: k.into(),
        slip_id: sid,
        run_id: c.run_id,
        amount_cents: c.amount_cents,
        paid_on: serde_json::from_value(v["paid_on"].clone())?,
        payment_reference: v["payment_reference"].as_str().unwrap().trim().into(),
        reason: v["reason"].as_str().unwrap().trim().into(),
        origin: "individual".into(),
        final_case_id: None,
        recorded_by: Some(data.access.user_id),
        recorded_at: Utc::now(),
    };
    data.payroll.receipts.push(r.clone());
    let remaining = candidates(data)?
        .iter()
        .filter(|p| p.kind == k && p.run_id == c.run_id && p.paid_on.is_none())
        .count();
    let latest = data
        .payroll
        .receipts
        .iter()
        .filter(|p| p.kind == k && p.run_id == c.run_id)
        .map(|p| p.paid_on)
        .max();
    macro_rules! update {
        ($run:expr) => {{
            let r = $run;
            r.revision += 1;
            if remaining == 0 {
                r.status = "paid".into();
                r.paid_on = latest;
                r.payment_reference = "Per-employee payment records".into();
            }
        }};
    }
    match k {
        "monthly" => update!(
            data.payroll
                .runs
                .iter_mut()
                .find(|r| r.id == c.run_id)
                .unwrap()
        ),
        "cutoff" => update!(
            &mut data
                .payroll
                .cutoffs
                .runs
                .iter_mut()
                .find(|r| r.base.id == c.run_id)
                .unwrap()
                .base
        ),
        _ => update!(
            data.payroll
                .thirteenth
                .runs
                .iter_mut()
                .find(|r| r.id == c.run_id)
                .unwrap()
        ),
    }
    data.payroll.events.push(payroll::Event {
        id: Uuid::new_v4(),
        company_id: data.company.id,
        record_id: id,
        kind: "receipt_record".into(),
        reason: r.reason.clone(),
        actor_name: data
            .employee_name(data.access.employee_id.unwrap_or_default())
            .into(),
        actor_role: data.access.role,
        before_record: None,
        after_record: json!({"receipt":r,"request":v}),
        recorded_at: Utc::now(),
    });
    Ok(())
}
