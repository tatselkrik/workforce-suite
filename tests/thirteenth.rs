use hris_desktop::payroll_thirteenth::{Calculation, Source, Worksheet, calculate};
use serde::Deserialize;
#[test]
fn rust_matches_postgres_for_earned_basic_coverage_prior_payments_and_shared_exemption() {
    #[derive(Deserialize)]
    struct Vector {
        source: Source,
        worksheet: Worksheet,
        result: Calculation,
    }
    let cases: Vec<Vector> =
        serde_json::from_str(include_str!("fixtures/thirteenth-rule-vectors.json")).unwrap();
    assert_eq!(cases.len(), 18);
    for c in cases {
        assert_eq!(calculate(&c.source, &c.worksheet).unwrap(), c.result);
    }
}

#[test]
fn demo_workflow_preserves_independent_approval_private_reviews_and_own_exports() {
    use hris_desktop::{
        access::Role,
        domain::{Action, DemoStore},
        payroll::PayrollAction,
        payroll_thirteenth as b,
    };
    use serde_json::json;
    use uuid::Uuid;
    let mut store = DemoStore::new();
    let eid = store.data.employees[1].id;
    let run = Uuid::new_v4();
    let item = Uuid::new_v4();
    let apply = |s: &mut DemoStore, op: &str, v: serde_json::Value| {
        s.execute(Action::Payroll(PayrollAction::new(op, v)))
            .unwrap()
    };
    apply(
        &mut store,
        "thirteenth_run_create",
        json!({"id":run,"year":2026,"as_of":"2026-09-30","pay_date":"2026-10-01","title":"Synthetic annual benefit","reason":"Synthetic scope"}),
    );
    apply(
        &mut store,
        "thirteenth_item_add",
        json!({"id":item,"run_id":run,"employee_id":eid,"reason":"Synthetic employee"}),
    );
    let src = b::source(&store.data.payroll.thirteenth.items[0]).unwrap();
    let w = b::Worksheet {
        openings: vec![b::Opening {
            starts_on: src.starts_on,
            ends_on: src.as_of,
            basic_cents: 27000000,
            reference: "PRIVATE_13_OPENING".into(),
        }],
        classification: "rank_and_file".into(),
        served_one_month: true,
        eligibility_note: "PRIVATE_13_ELIGIBILITY".into(),
        other_benefits_cents: 7500000,
        opening_note: "PRIVATE_13_OTHER".into(),
        reviewed: true,
        review_note: "PRIVATE_13_REVIEW".into(),
        tax_reviewed: true,
        tax_note: "PRIVATE_13_TAX".into(),
        contributions_reviewed: true,
        contribution_note: "PRIVATE_13_CONTRIBUTIONS".into(),
        ..Default::default()
    };
    apply(
        &mut store,
        "thirteenth_item_save",
        json!({"id":item,"expected_revision":0,"worksheet":w,"reason":"Synthetic review"}),
    );
    let revision = store.data.payroll.thirteenth.runs[0].revision;
    apply(
        &mut store,
        "thirteenth_run_submit",
        json!({"id":run,"expected_revision":revision,"scope_reviewed":true,"reason":"Synthetic submit"}),
    );
    let revision = store.data.payroll.thirteenth.runs[0].revision;
    assert!(
        store
            .execute(Action::Payroll(PayrollAction::new(
                "thirteenth_run_approve",
                json!({"id":run,"expected_revision":revision,"reason":"Self approval must fail"})
            )))
            .is_err()
    );
    store.data.access.user_id = Uuid::new_v4();
    apply(
        &mut store,
        "thirteenth_run_approve",
        json!({"id":run,"expected_revision":revision,"reason":"Independent synthetic approval"}),
    );
    let slip = &store.data.payroll.thirteenth.slips[0];
    assert!(!serde_json::to_string(slip).unwrap().contains("PRIVATE_13"));
    assert_eq!(slip.item.totals.gross_cents, 2250000);
    let pdf = hris_desktop::payroll_pdf::build_thirteenth(slip).unwrap();
    assert!(pdf.starts_with(b"%PDF-1.4"));
    assert!(String::from_utf8_lossy(&pdf).contains("13TH-MONTH PAY"));
    store.data.access.role = Role::Manager;
    store.data.access.employee_id = Some(eid);
    assert!(b::export_csv(&store.data, "thirteenth_payslip", Some(item)).is_ok());
    assert!(b::export_csv(&store.data, "thirteenth_register", Some(run)).is_err());
    store.data.access.employee_id = Some(Uuid::new_v4());
    assert!(b::export_csv(&store.data, "thirteenth_payslip", Some(item)).is_err());
}
