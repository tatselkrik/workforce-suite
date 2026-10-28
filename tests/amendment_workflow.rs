use hris_desktop::{
    access::Role,
    domain::{Action, DemoStore},
    payroll::{Pay, PayrollAction as Change, Worksheet},
    payroll_amendments as p, payroll_reporting as reports,
};
use serde_json::{Value, json};
use uuid::Uuid;
fn send(s: &mut DemoStore, op: &str, v: Value) {
    s.execute(Action::Payroll(Change::new(op, v)))
        .unwrap_or_else(|e| panic!("{op}: {e:#}"));
}
fn act(s: &mut DemoStore, op: &str, id: Uuid, extra: Value) {
    let r = s
        .data
        .payroll
        .amendments
        .records
        .iter()
        .find(|r| r.id == id)
        .unwrap();
    let mut v = json!({"id":id,"expected_revision":r.revision,"reason":"PRIVATE_AMEND_ACTION"});
    for (k, vv) in extra.as_object().unwrap() {
        v[k] = vv.clone();
    }
    send(s, &format!("amendment_{op}"), v);
}
fn original() -> (DemoStore, Uuid, Uuid, Uuid, Uuid) {
    let mut s = DemoStore::new();
    let admin = s.data.access.user_id;
    let hr = Uuid::from_u128(41100);
    let eid = s.data.employees[1].id;
    s.data.employees[1].joined_on = Some("2026-01-01".parse().unwrap());
    s.data.access.role = Role::HrGeneralist;
    s.data.access.user_id = hr;
    let run = Uuid::from_u128(41101);
    let slip = Uuid::from_u128(41102);
    send(
        &mut s,
        "profile_save",
        json!({"id":Uuid::from_u128(41103),"employee_id":eid,"effective_on":"2026-02-01","pay":Pay{rate_cents:5000000,sss:false,philhealth:false,pagibig:false,coverage_note:"PRIVATE_ORIGINAL_COVERAGE".into(),..Default::default()},"reason":"PRIVATE_ORIGINAL_PROFILE"}),
    );
    send(
        &mut s,
        "cutoff_run_create",
        json!({"id":run,"month":"2026-09-01","part":1,"pay_date":"2026-09-15","title":"Fictional original","reason":"PRIVATE_ORIGINAL_RUN"}),
    );
    send(
        &mut s,
        "cutoff_item_add",
        json!({"id":slip,"run_id":run,"employee_id":eid,"reason":"PRIVATE_ORIGINAL_ITEM"}),
    );
    let rev = s.data.payroll.cutoffs.items[0].base.revision;
    send(
        &mut s,
        "cutoff_item_save",
        json!({"id":slip,"expected_revision":rev,"worksheet":Worksheet{reviewed:true,review_note:"PRIVATE_ORIGINAL_REVIEW".into(),..Default::default()},"reason":"PRIVATE_ORIGINAL_REVIEW"}),
    );
    let rev = s.data.payroll.cutoffs.runs[0].base.revision;
    send(
        &mut s,
        "cutoff_run_submit",
        json!({"id":run,"expected_revision":rev,"scope_reviewed":true,"reason":"PRIVATE_ORIGINAL_SUBMIT"}),
    );
    s.data.access.role = Role::Administrator;
    s.data.access.user_id = admin;
    let rev = s.data.payroll.cutoffs.runs[0].base.revision;
    send(
        &mut s,
        "cutoff_run_approve",
        json!({"id":run,"expected_revision":rev,"reason":"PRIVATE_ORIGINAL_APPROVE"}),
    );
    let rev = s.data.payroll.cutoffs.runs[0].base.revision;
    let today = s.data.company.today();
    send(
        &mut s,
        "receipt_record",
        json!({"id":Uuid::from_u128(41104),"kind":"cutoff","slip_id":slip,"expected_revision":rev,"paid_on":today,"payment_reference":"PRIVATE_ORIGINAL_RECEIPT","reason":"PRIVATE_ORIGINAL_PAID"}),
    );
    (s, admin, hr, eid, slip)
}
#[test]
fn demo_linked_correction_preserves_original_and_exports_own_aggregate() {
    let (mut s, admin, hr, eid, target) = original();
    let original = serde_json::to_value(&s.data.payroll.cutoffs.slips).unwrap();
    let receipt = serde_json::to_value(&s.data.payroll.receipts).unwrap();
    let id = Uuid::from_u128(41110);
    let today = s.data.company.today();
    s.data.access.user_id = hr;
    s.data.access.role = Role::HrGeneralist;
    let create = json!({"id":id,"target_kind":"cutoff","target_id":target,"cash_date":today,"purpose":"payout","title":"Fictional linked correction","reason":"PRIVATE_AMEND_CREATE"});
    send(&mut s, "amendment_create", create.clone());
    send(&mut s, "amendment_create", create);
    assert_eq!(s.data.payroll.amendments.records.len(), 1);
    let r = s.data.payroll.amendments.records[0].clone();
    let w = p::Worksheet {
        basic_cents: 100000,
        taxable_cents: 200000,
        shared_cents: 300000,
        withholding_cents: 10000,
        evidence: "PRIVATE_AMEND_EVIDENCE".into(),
        tax_note: "PRIVATE_AMEND_TAX".into(),
        contribution_note: "PRIVATE_AMEND_CONTRIBUTIONS".into(),
        review_note: "PRIVATE_AMEND_REVIEW".into(),
        reviewed: true,
        tax_reviewed: true,
        contributions_reviewed: true,
        ..r.worksheet
    };
    act(&mut s, "save", id, json!({"worksheet":w}));
    act(&mut s, "submit", id, json!({}));
    s.data.access.role = Role::Administrator;
    let r = &s.data.payroll.amendments.records[0];
    let before = serde_json::to_value(r).unwrap();
    let self_approval = Change::new(
        "amendment_approve",
        json!({"id":id,"expected_revision":r.revision,"reason":"Self approval denied"}),
    );
    assert!(s.execute(Action::Payroll(self_approval)).is_err());
    assert_eq!(
        serde_json::to_value(&s.data.payroll.amendments.records[0]).unwrap(),
        before
    );
    s.data.access.user_id = admin;
    act(&mut s, "approve", id, json!({}));
    act(
        &mut s,
        "pay",
        id,
        json!({"paid_on":today,"payment_reference":"PRIVATE_SUPPLEMENTAL_RECEIPT","received_confirmed":true}),
    );
    assert_eq!(
        serde_json::to_value(&s.data.payroll.cutoffs.slips).unwrap(),
        original
    );
    assert_eq!(
        serde_json::to_value(&s.data.payroll.receipts).unwrap(),
        receipt
    );
    let f = reports::Filter {
        starts_on: "2026-01-01".parse().unwrap(),
        ends_on: "2026-12-31".parse().unwrap(),
        axis: "received".into(),
        kind: "amendment".into(),
        employee_id: Some(eid),
        search: String::new(),
    };
    let report = reports::demo(&s.data, &f, chrono::Utc::now()).unwrap();
    assert_eq!(report.totals.recorded_cash_cents, 490000);
    assert_eq!(report.totals.source_net_cents, 0);
    assert_eq!(report.wire.rows[0].linked_id, Some(target));
    s.data.access.role = Role::Manager;
    s.data.access.employee_id = Some(eid);
    let csv = String::from_utf8(p::export_csv(&s.data, "amendment_statement", Some(id)).unwrap())
        .unwrap();
    assert!(!csv.contains("PRIVATE_"));
    assert!(csv.contains(&target.to_string()));
    let slip = &s.data.payroll.amendments.slips[0];
    let pdf = hris_desktop::payroll_pdf::build_amendment(slip, p::status(&s.data, id)).unwrap();
    assert!(pdf.starts_with(b"%PDF"));
    assert!(!String::from_utf8_lossy(&pdf).contains("PRIVATE_"));
    s.data.access.employee_id = Some(Uuid::new_v4());
    assert!(p::export_csv(&s.data, "amendment_statement", Some(id)).is_err());
    assert!(reports::demo(&s.data, &f, chrono::Utc::now()).is_err());
}
#[test]
fn version_two_postgres_rows_preserve_cash_links_and_review_boundaries() {
    let rows: Vec<reports::Wire> =
        serde_json::from_str(include_str!("fixtures/amendment-report-vectors.json")).unwrap();
    assert!(rows.len() >= 5);
    for w in rows {
        let f = w.filter.clone();
        let report = reports::Report::from_wire(w.clone(), w.company_id, &f).unwrap();
        assert_eq!(
            report.totals.recorded_cash_cents,
            w.rows.iter().map(|r| r.cash_cents).sum::<i64>()
        );
        assert!(
            !String::from_utf8(report.csv().unwrap())
                .unwrap()
                .contains("PRIVATE_")
        );
        if f.kind == "amendment" && f.axis == "issued" {
            assert_eq!(report.totals.overpayment_review_cents, -100000);
            assert_eq!(report.totals.amendment_outstanding_cents, 0);
        }
    }
}
