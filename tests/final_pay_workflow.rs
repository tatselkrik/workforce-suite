use hris_desktop::{
    access::Role,
    domain::{Action, DemoStore, EmployeeStatus},
    payroll::{Pay, PayrollAction as Change, Worksheet as PayWorksheet},
    payroll_final as f, payroll_tax as t,
};
use serde_json::{Value, json};
use uuid::Uuid;
fn send(s: &mut DemoStore, op: &str, v: Value) {
    let a=Change::new(op,v);a.payload(&s.data).unwrap_or_else(|e|panic!("{op} payload: {e:#}"));s.execute(Action::Payroll(a)).unwrap_or_else(|e|panic!("{op}: {e:#}"));
}
fn act(s: &mut DemoStore, op: &str, id: Uuid, extra: Value) -> Change {
    let r = s
        .data
        .payroll
        .finals
        .records
        .iter()
        .find(|r| r.id == id)
        .unwrap();
    let mut v = json!({"id":id,"expected_revision":r.revision,"reason":"PRIVATE_FINAL_ACTION"});
    if let Some(o) = extra.as_object() {
        for (k, vv) in o {
            v[k] = vv.clone();
        }
    }
    Change::new(&format!("final_{op}"), v)
}
struct Fixture {
    store: DemoStore,
    id: Uuid,
    admin: Uuid,
    hr: Uuid,
    employee: Uuid,
    source: Uuid,
    run: Uuid,
    w: f::Worksheet,
}
fn fixture() -> Fixture {
    let mut s = DemoStore::new();
    let admin = s.data.access.user_id;
    let hr = Uuid::from_u128(87200);
    let eid = s.data.employees[1].id;
    s.data.employees[1].joined_on = Some("2026-01-01".parse().unwrap());
    s.data.access.user_id = hr;
    s.data.access.role = Role::HrGeneralist;
    let profile = Uuid::from_u128(87201);
    let run = Uuid::from_u128(87202);
    let source = Uuid::from_u128(87203);
    let id = Uuid::from_u128(87204);
    send(
        &mut s,
        "profile_save",
        json!({"id":profile,"employee_id":eid,"effective_on":"2026-02-01","pay":Pay{coverage_note:"Synthetic independently reviewed contribution exception".into(),rate_cents:10_000_000,sss:false,philhealth:false,pagibig:false,..Default::default()},"reason":"PRIVATE_FINAL_PROFILE"}),
    );
    send(
        &mut s,
        "cutoff_run_create",
        json!({"id":run,"month":"2026-10-01","part":1,"pay_date":"2026-10-15","title":"Synthetic final source","reason":"PRIVATE_FINAL_SOURCE"}),
    );
    send(
        &mut s,
        "cutoff_item_add",
        json!({"id":source,"run_id":run,"employee_id":eid,"reason":"PRIVATE_FINAL_EMPLOYEE"}),
    );
    let rev = s.data.payroll.cutoffs.items[0].base.revision;
    send(
        &mut s,
        "cutoff_item_save",
        json!({"id":source,"expected_revision":rev,"worksheet":PayWorksheet{reviewed:true,review_note:"PRIVATE_FINAL_SOURCE_REVIEW".into(),..Default::default()},"reason":"PRIVATE_FINAL_SOURCE_REVIEW"}),
    );
    let rev = s.data.payroll.cutoffs.runs[0].base.revision;
    send(
        &mut s,
        "cutoff_run_submit",
        json!({"id":run,"expected_revision":rev,"scope_reviewed":true,"reason":"PRIVATE_FINAL_SOURCE_SUBMIT"}),
    );
    s.data.access.user_id = admin;
    s.data.access.role = Role::Administrator;
    let rev = s.data.payroll.cutoffs.runs[0].base.revision;
    send(
        &mut s,
        "cutoff_run_approve",
        json!({"id":run,"expected_revision":rev,"reason":"PRIVATE_FINAL_SOURCE_APPROVAL"}),
    );
    s.data.access.user_id = hr;
    s.data.access.role = Role::HrGeneralist;
    let today = s.data.company.today();
    send(
        &mut s,
        "final_create",
        json!({"id":id,"employee_id":eid,"service_end":"2026-10-15","cash_date":today,"title":"Synthetic final review","reason":"PRIVATE_FINAL_CONTEXT"}),
    );
    let w = f::Worksheet {
        entitlements_reviewed: true,
        source_scope_reviewed: true,
        contributions_reviewed: true,
        review_note: "PRIVATE_FINAL_REVIEW".into(),
        contribution_note: "PRIVATE_FINAL_CONTRIBUTIONS".into(),
        vacation: f::Vacation {
            days_hundredths: 1300,
            daily_cents: 100000,
            previous_days_hundredths: 0,
            reference: "PRIVATE_FINAL_VACATION".into(),
        },
        tax: t::Worksheet {
            openings: vec![t::Opening {
                starts_on: "2026-01-01".parse().unwrap(),
                ends_on: "2026-09-30".parse().unwrap(),
                taxable_cents: 0,
                withholding_cents: 0,
                benefit_gross_cents: 0,
                benefit_exempt_cents: 0,
                reference: "PRIVATE_FINAL_OPENING".into(),
            }],
            final_compensation_reviewed: true,
            previous_employers_reviewed: true,
            benefits_reviewed: true,
            review_note: "PRIVATE_FINAL_TAX".into(),
            settlement_note: "PRIVATE_FINAL_HANDLING".into(),
            ..Default::default()
        },
        ..Default::default()
    };
    let a = act(&mut s, "save", id, json!({"worksheet":w}));
    s.execute(Action::Payroll(a)).unwrap();
    Fixture {
        store: s,
        id,
        admin,
        hr,
        employee: eid,
        source,
        run,
        w,
    }
}
fn approve(x: &mut Fixture) {
    let a = act(&mut x.store, "submit", x.id, json!({}));
    x.store.execute(Action::Payroll(a)).unwrap();
    x.store.data.access.user_id = x.admin;
    x.store.data.access.role = Role::Administrator;
    let a = act(&mut x.store, "approve", x.id, json!({}));
    x.store.execute(Action::Payroll(a)).unwrap();
}
fn settlement(x: &mut Fixture) -> Change {
    let d = x.store.data.company.today();
    act(
        &mut x.store,
        "settle",
        x.id,
        json!({"settled_on":d,"payment_reference":"PRIVATE_FINAL_CASH","handling_reference":"","cash_received_confirmed":true,"employer_handling_confirmed":true}),
    )
}
#[test]
fn final_cash_discharge_is_atomic_idempotent_and_annual_adjustment_is_not_repeated() {
    let mut x = fixture();
    approve(&mut x);
    let issued = serde_json::to_value(&x.store.data.payroll.cutoffs.slips).unwrap();
    let original = f::source(&x.store.data.payroll.finals.records[0]).unwrap();
    assert_eq!(original.outstanding.len(), 1);
    let a = settlement(&mut x);
    x.store.execute(Action::Payroll(a.clone())).unwrap();
    let receipts = x.store.data.payroll.receipts.len();
    x.store.execute(Action::Payroll(a)).unwrap();
    assert_eq!(x.store.data.payroll.receipts.len(), receipts);
    let p = &x.store.data.payroll.receipts[0];
    assert_eq!(p.origin, "final_settlement");
    assert_eq!(p.final_case_id, Some(x.id));
    assert_eq!(
        serde_json::to_value(&x.store.data.payroll.cutoffs.slips).unwrap(),
        issued
    );
    assert_eq!(
        x.store.data.payroll.finals.statuses[0].net_cents,
        x.store.data.payroll.finals.slips[0].totals.net_cents
    );
    x.store.data.access.user_id = x.hr;
    x.store.data.access.role = Role::HrGeneralist;
    let d = x.store.data.company.today();
    send(
        &mut x.store,
        "tax_create",
        json!({"id":Uuid::new_v4(),"employee_id":x.employee,"year":2026,"kind":"termination","as_of":d,"service_end":"2026-10-15","title":"Synthetic paid close","reason":"PRIVATE_FINAL_POST_CLOSE"}),
    );
    let r = x.store.data.payroll.tax.records.last().unwrap();
    let source = t::source(r).unwrap();
    assert_eq!(
        source
            .income
            .iter()
            .filter(|i| i.id == format!("final:{}", x.id))
            .count(),
        1
    );
    assert_eq!(t::calculate(&source, &x.w.tax).unwrap().adjustment_cents, 0);
}
#[test]
fn independent_admin_cannot_be_creator_or_preparer_even_after_role_change() {
    let mut x = fixture();
    let a = act(&mut x.store, "submit", x.id, json!({}));
    x.store.execute(Action::Payroll(a)).unwrap();
    x.store.data.access.role = Role::Administrator;
    let a = act(&mut x.store, "approve", x.id, json!({}));
    assert!(x.store.execute(Action::Payroll(a)).is_err());
    assert!(x.store.data.payroll.finals.slips.is_empty());
}
#[test]
fn reserved_final_source_rejects_individual_or_legacy_batch_bypass() {
    let mut x = fixture();
    approve(&mut x);
    let rev = x.store.data.payroll.cutoffs.runs[0].base.revision;
    let d = x.store.data.company.today();
    for a in [
        Change::new(
            "receipt_record",
            json!({"id":Uuid::new_v4(),"kind":"cutoff","slip_id":x.source,"expected_revision":rev,"paid_on":d,"payment_reference":"Synthetic bypass","reason":"Synthetic bypass"}),
        ),
        Change::new(
            "cutoff_run_paid",
            json!({"id":x.run,"expected_revision":rev,"paid_on":d,"payment_reference":"Synthetic batch bypass","reason":"Synthetic bypass"}),
        ),
    ] {
        assert!(x.store.execute(Action::Payroll(a)).is_err());
    }
    assert!(x.store.data.payroll.receipts.is_empty());
}
#[test]
fn changed_employment_blocks_settlement_while_archive_alone_does_not() {
    let mut x = fixture();
    approve(&mut x);
    let e = x
        .store
        .data
        .employees
        .iter_mut()
        .find(|e| e.id == x.employee)
        .unwrap();
    let dept = e.department.clone();
    e.department = "Changed source".into();
    let a = settlement(&mut x);
    assert!(x.store.execute(Action::Payroll(a)).is_err());
    assert!(x.store.data.payroll.receipts.is_empty());
    assert_eq!(x.store.data.payroll.finals.records[0].status, "approved");
    let e = x
        .store
        .data
        .employees
        .iter_mut()
        .find(|e| e.id == x.employee)
        .unwrap();
    e.department = dept;
    e.status = EmployeeStatus::Archived;
    let a = settlement(&mut x);
    x.store.execute(Action::Payroll(a)).unwrap();
}
#[test]
fn unpaid_withdrawal_retains_statement_and_releases_reserved_source() {
    let mut x = fixture();
    approve(&mut x);
    let original = serde_json::to_value(&x.store.data.payroll.finals.slips).unwrap();
    let a = act(
        &mut x.store,
        "withdraw",
        x.id,
        json!({"unpaid_confirmed":true}),
    );
    x.store.execute(Action::Payroll(a.clone())).unwrap();
    x.store.execute(Action::Payroll(a)).unwrap();
    assert_eq!(
        serde_json::to_value(&x.store.data.payroll.finals.slips).unwrap(),
        original
    );
    assert_eq!(x.store.data.payroll.finals.statuses[0].kind, "withdrawn");
    let rev = x.store.data.payroll.cutoffs.runs[0].base.revision;
    let d = x.store.data.company.today();
    send(
        &mut x.store,
        "receipt_record",
        json!({"id":Uuid::new_v4(),"kind":"cutoff","slip_id":x.source,"expected_revision":rev,"paid_on":d,"payment_reference":"Synthetic after withdrawal","reason":"Synthetic own payment"}),
    );
}
#[test]
fn aggregate_own_exports_do_not_expose_private_evidence_or_allow_team_register() {
    let mut x = fixture();
    approve(&mut x);
    let value = serde_json::to_string(&x.store.data.payroll.finals.slips[0]).unwrap();
    assert!(!value.contains("PRIVATE_FINAL_"));
    x.store.data.access.role = Role::Manager;
    x.store.data.access.employee_id = Some(x.employee);
    let csv = f::export_csv(&x.store.data, "final_statement", Some(x.id)).unwrap();
    assert!(!String::from_utf8(csv).unwrap().contains("PRIVATE_FINAL_"));
    let s = &x.store.data.payroll.finals.slips[0];
    let pdf = hris_desktop::payroll_pdf::build_final(s, None).unwrap();
    assert!(pdf.starts_with(b"%PDF-1.4"));
    assert!(!String::from_utf8_lossy(&pdf).contains("PRIVATE_FINAL_"));
    assert!(f::export_csv(&x.store.data, "final_register", None).is_err());
    x.store.data.access.employee_id = Some(Uuid::new_v4());
    assert!(f::export_csv(&x.store.data, "final_statement", Some(x.id)).is_err());
}
