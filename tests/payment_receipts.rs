use hris_desktop::{
    access::Role,
    domain::{Action, DemoStore},
    payroll::{self, PayrollAction},
    payroll_payments as p, payroll_tax,
};
use serde_json::{Value, json};
use uuid::Uuid;
fn apply(s: &mut DemoStore, op: &str, v: Value) {
    let a = PayrollAction::new(op, v);
    a.payload(&s.data)
        .unwrap_or_else(|e| panic!("{op} payload: {e}"));
    s.execute(Action::Payroll(a))
        .unwrap_or_else(|e| panic!("{op} action: {e}"));
}
fn fixture() -> (DemoStore, Uuid, Uuid, Uuid) {
    let mut s = DemoStore::new();
    let run = Uuid::new_v4();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let employees = [s.data.employees[1].id, s.data.employees[2].id];
    for employee in employees {
        apply(
            &mut s,
            "profile_save",
            json!({"id":Uuid::new_v4(),"employee_id":employee,"effective_on":"2026-02-01","pay":payroll::Pay{rate_cents:3_000_000,sss:false,philhealth:false,pagibig:false,coverage_note:"Synthetic contribution exception reviewed for this fixture".into(),..Default::default()},"reason":"Synthetic pay source"}),
        );
    }
    apply(
        &mut s,
        "run_create",
        json!({"id":run,"month":"2026-05-01","title":"Synthetic multiple recipients","reason":"Synthetic payment fixture"}),
    );
    for (employee, id) in employees.into_iter().zip([a, b]) {
        apply(
            &mut s,
            "item_add",
            json!({"id":id,"run_id":run,"employee_id":employee,"reason":"Synthetic source worksheet"}),
        );
        let item = s.data.payroll.items.iter().find(|i| i.id == id).unwrap();
        let w = payroll::Worksheet {
            reviewed: true,
            review_note: "PRIVATE_RECEIPT_SOURCE".into(),
            ..item.worksheet.clone()
        };
        let rev = item.revision;
        apply(
            &mut s,
            "item_save",
            json!({"id":id,"expected_revision":rev,"worksheet":w,"reason":"Synthetic source review"}),
        );
    }
    let rev = s
        .data
        .payroll
        .runs
        .iter()
        .find(|r| r.id == run)
        .unwrap()
        .revision;
    apply(
        &mut s,
        "run_submit",
        json!({"id":run,"expected_revision":rev,"scope_reviewed":true,"reason":"Synthetic independent submission"}),
    );
    s.data.access.user_id = Uuid::new_v4();
    let rev = s
        .data
        .payroll
        .runs
        .iter()
        .find(|r| r.id == run)
        .unwrap()
        .revision;
    apply(
        &mut s,
        "run_approve",
        json!({"id":run,"expected_revision":rev,"reason":"Synthetic independent approval"}),
    );
    (s, run, a, b)
}
fn request(s: &DemoStore, slip: Uuid, date: &str) -> PayrollAction {
    let c = p::candidates(&s.data)
        .unwrap()
        .into_iter()
        .find(|r| r.slip_id == slip)
        .unwrap();
    PayrollAction::new(
        "receipt_record",
        json!({"id":Uuid::new_v4(),"kind":c.kind,"slip_id":slip,"expected_revision":c.revision,"paid_on":date,"payment_reference":"PRIVATE_ACTUAL_PAYMENT_REFERENCE","reason":"PRIVATE_RECEIPT_EVIDENCE"}),
    )
}
#[test]
fn individual_payments_preserve_sibling_sources_and_complete_only_after_last_receipt() {
    let (mut s, run, a, b) = fixture();
    let slips = serde_json::to_value(&s.data.payroll.slips).unwrap();
    let req = request(&s, a, "2026-09-29");
    s.execute(Action::Payroll(req.clone())).unwrap();
    assert!(req.saved(&s.data));
    assert_eq!(s.data.payroll.receipts.len(), 1);
    assert_eq!(
        s.data
            .payroll
            .runs
            .iter()
            .find(|r| r.id == run)
            .unwrap()
            .status,
        "approved"
    );
    let candidates = p::candidates(&s.data).unwrap();
    assert!(
        candidates
            .iter()
            .find(|c| c.slip_id == b)
            .unwrap()
            .paid_on
            .is_none()
    );
    assert_eq!(
        s.data.payroll.receipts[0].amount_cents,
        s.data
            .payroll
            .slips
            .iter()
            .find(|p| p.id == a)
            .unwrap()
            .item
            .totals
            .net
    );
    s.execute(Action::Payroll(req.clone())).unwrap();
    assert_eq!(s.data.payroll.receipts.len(), 1);
    let mut changed = req.clone();
    changed.record["payment_reference"] = json!("Changed retry");
    assert!(s.execute(Action::Payroll(changed)).is_err());
    let second = request(&s, b, "2026-09-30");
    s.execute(Action::Payroll(second)).unwrap();
    let r = s.data.payroll.runs.iter().find(|r| r.id == run).unwrap();
    assert_eq!(r.status, "paid");
    assert_eq!(r.paid_on.unwrap().to_string(), "2026-09-30");
    assert_eq!(s.data.payroll.receipts.len(), 2);
    assert_eq!(serde_json::to_value(&s.data.payroll.slips).unwrap(), slips);
    assert_eq!(
        p::candidates(&s.data)
            .unwrap()
            .iter()
            .find(|c| c.slip_id == a)
            .unwrap()
            .paid_on
            .unwrap()
            .to_string(),
        "2026-09-29"
    );
    s.execute(Action::Payroll(req)).unwrap();
    assert_eq!(s.data.payroll.receipts.len(), 2);
}
#[test]
fn actual_individual_date_survives_later_batch_date_in_annual_tax_source() {
    let (mut s, run, a, b) = fixture();
    let req = request(&s, a, "2026-09-30");
    s.execute(Action::Payroll(req)).unwrap();
    let e = s
        .data
        .payroll
        .slips
        .iter()
        .find(|p| p.id == a)
        .unwrap()
        .employee_id;
    let eid_b = s
        .data
        .payroll
        .slips
        .iter()
        .find(|p| p.id == b)
        .unwrap()
        .employee_id;
    let tid = Uuid::new_v4();
    apply(
        &mut s,
        "tax_create",
        json!({"id":tid,"employee_id":e,"year":2026,"kind":"termination","as_of":"2026-09-30","service_end":"2026-09-30","title":"Synthetic individual tax attribution","reason":"Synthetic tax source check"}),
    );
    let r = &s.data.payroll.tax.records[0];
    let source = payroll_tax::source(r).unwrap();
    assert_eq!(source.income.len(), 1);
    assert_eq!(source.pending_payments, 0);
    let rev = s
        .data
        .payroll
        .runs
        .iter()
        .find(|r| r.id == run)
        .unwrap()
        .revision;
    apply(
        &mut s,
        "run_paid",
        json!({"id":run,"expected_revision":rev,"paid_on":"2026-10-01","payment_reference":"Remaining synthetic batch","reason":"Record remaining only"}),
    );
    let rev = s.data.payroll.tax.records[0].revision;
    apply(
        &mut s,
        "tax_refresh",
        json!({"id":tid,"expected_revision":rev,"reason":"Refresh changed batch status"}),
    );
    assert_eq!(
        payroll_tax::source(&s.data.payroll.tax.records[0])
            .unwrap()
            .income
            .len(),
        1
    );
    apply(
        &mut s,
        "tax_create",
        json!({"id":Uuid::new_v4(),"employee_id":eid_b,"year":2026,"kind":"termination","as_of":"2026-09-30","service_end":"2026-09-30","title":"Synthetic sibling receipt attribution","reason":"Synthetic sibling tax source check"}),
    );
    assert_eq!(
        payroll_tax::source(&s.data.payroll.tax.records[1])
            .unwrap()
            .income
            .len(),
        0
    );
}
#[test]
fn payment_requests_reject_non_administrators_unissued_extra_amounts_and_stale_revisions() {
    let (mut s, _run, a, b) = fixture();
    let req = request(&s, a, "2026-09-30");
    for role in [Role::HrGeneralist, Role::Manager, Role::Employee] {
        s.data.access.role = role;
        assert!(req.payload(&s.data).is_err());
    }
    s.data.access.role = Role::Administrator;
    for (field, value) in [
        ("amount_cents", json!(1)),
        ("paid_on", json!("2200-01-01")),
        ("paid_on", json!("2026-05-01")),
        ("slip_id", json!(Uuid::new_v4())),
        ("expected_revision", Value::Null),
        ("kind", json!("unknown")),
    ] {
        let mut bad = req.clone();
        bad.record[field] = value;
        assert!(bad.payload(&s.data).is_err());
    }
    let stale = request(&s, b, "2026-09-30");
    s.execute(Action::Payroll(req)).unwrap();
    assert!(s.execute(Action::Payroll(stale)).is_err());
    s.data.access.role = Role::Manager;
    assert!(p::candidates(&s.data).is_err());
}
