use chrono::NaiveDate;
use hris_desktop::{
    access::Role,
    domain::{Action, DemoStore},
    payroll::PayrollAction,
    payroll_tax as t,
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;
#[test]
fn annual_centavos_match_postgres_for_brackets_refunds_prior_employers_shared_cap_and_reclassification()
 {
    #[derive(Deserialize)]
    struct Vector {
        source: t::Source,
        worksheet: t::Worksheet,
        result: t::Calculation,
    }
    let cases: Vec<Vector> =
        serde_json::from_str(include_str!("fixtures/annual-tax-rule-vectors.json")).unwrap();
    assert_eq!(cases.len(), 29);
    for c in cases {
        assert_eq!(t::calculate(&c.source, &c.worksheet).unwrap(), c.result);
    }
}
fn create(s: &mut DemoStore, eid: Uuid, id: Uuid, asof: &str) {
    s.execute(Action::Payroll(PayrollAction::new("tax_create",json!({"id":id,"employee_id":eid,"year":2026,"kind":"termination","as_of":asof,"service_end":"2026-09-30","title":"Synthetic annual tax","reason":"Synthetic annual tax fixture"})))).unwrap();
}
fn action(op: &str, r: &t::Record) -> PayrollAction {
    PayrollAction::new(
        &format!("tax_{op}"),
        json!({"id":r.id,"expected_revision":r.revision,"reason":"Synthetic reviewed action"}),
    )
}
fn save(s: &mut DemoStore, id: Uuid, w: &t::Worksheet) {
    let r = s
        .data
        .payroll
        .tax
        .records
        .iter()
        .find(|r| r.id == id)
        .unwrap();
    s.execute(Action::Payroll(PayrollAction::new("tax_save",json!({"id":id,"expected_revision":r.revision,"worksheet":w,"reason":"PRIVATE_TAX_WORKSHEET"})))).unwrap();
}
fn w() -> t::Worksheet {
    t::Worksheet {
        openings: vec![t::Opening {
            starts_on: "2026-07-01".parse().unwrap(),
            ends_on: "2026-09-30".parse().unwrap(),
            taxable_cents: 30000000,
            withholding_cents: 1500000,
            benefit_gross_cents: 0,
            benefit_exempt_cents: 0,
            reference: "PRIVATE_TAX_OPENING".into(),
        }],
        previous_employers: vec![t::PreviousEmployer {
            starts_on: "2026-01-01".parse().unwrap(),
            ends_on: "2026-06-30".parse().unwrap(),
            employer: "PRIVATE_TAX_EMPLOYER".into(),
            taxable_cents: 30000000,
            withholding_cents: 1500000,
            benefit_gross_cents: 0,
            benefit_exempt_cents: 0,
            reference: "PRIVATE_TAX_2316".into(),
        }],
        settlement_note: "PRIVATE_TAX_TERMINATION".into(),
        review_note: "PRIVATE_TAX_REVIEW".into(),
        final_compensation_reviewed: true,
        previous_employers_reviewed: true,
        benefits_reviewed: true,
        ..Default::default()
    }
}
#[test]
fn native_demo_protects_independent_approval_own_receipts_and_new_review_on_source_change() {
    let mut s = DemoStore::new();
    let eid = s.data.employees[1].id;
    s.data.employees[1].joined_on = Some("2026-07-01".parse().unwrap());
    let id = Uuid::new_v4();
    create(&mut s, eid, id, "2026-09-30");
    save(&mut s, id, &w());
    let r = s.data.payroll.tax.records[0].clone();
    s.execute(Action::Payroll(action("submit", &r))).unwrap();
    let r = s.data.payroll.tax.records[0].clone();
    assert!(s.execute(Action::Payroll(action("approve", &r))).is_err());
    s.data.access.user_id = Uuid::new_v4();
    s.execute(Action::Payroll(action("approve", &r))).unwrap();
    assert_eq!(s.data.payroll.tax.slips.len(), 1);
    let bytes = serde_json::to_string(&s.data.payroll.tax.slips[0]).unwrap();
    assert!(!bytes.contains("PRIVATE_TAX"));
    assert!(!bytes.contains("source_basis"));
    assert!(!bytes.contains("worksheet"));
    let csv = t::export_csv(&s.data, "tax_payslip", Some(id)).unwrap();
    assert!(!String::from_utf8(csv).unwrap().contains("PRIVATE_TAX"));
    assert!(
        hris_desktop::payroll_pdf::build_tax(&s.data.payroll.tax.slips[0])
            .unwrap()
            .starts_with(b"%PDF-")
    );
    assert!(String::from_utf8_lossy(&hris_desktop::payroll_pdf::build_tax(&s.data.payroll.tax.slips[0]).unwrap()).contains("ANNUAL TAX ADVICE"));
    s.data.access.role = Role::Manager;
    s.data.access.employee_id = Some(eid);
    assert!(t::export_csv(&s.data, "tax_payslip", Some(id)).is_ok());
    assert!(t::export_csv(&s.data, "tax_register", None).is_err());
    s.data.access.employee_id = Some(Uuid::new_v4());
    assert!(t::export_csv(&s.data, "tax_payslip", Some(id)).is_err());
    s.data.access.role = Role::Administrator;
    let later = Uuid::new_v4();
    let date = s.data.company.today().to_string();
    create(&mut s, eid, later, &date);
    save(&mut s, later, &w());
    let r = s.data.payroll.tax.records[1].clone();
    assert!(s.execute(Action::Payroll(action("submit", &r))).is_err());
    assert_eq!(t::source(&r).unwrap().pending_adjustments, 1);
}
#[test]
fn engine_rejects_overlapping_openings_concurrent_prior_employers_and_unreviewed_facts() {
    let source = t::Source {
        starts_on: "2026-07-01".parse().unwrap(),
        service_end: "2026-09-30".parse().unwrap(),
        as_of: "2026-09-30".parse().unwrap(),
        income: vec![],
        prior_adjustment_cents: 0,
        pending_payments: 0,
        pending_adjustments: 0,
        external_thirteenth_floor: 0,
        other_benefits_floor: 0,
    };
    let mut x = w();
    assert!(t::ready(&source, &x).is_ok());
    x.openings.push(x.openings[0].clone());
    assert!(t::calculate(&source, &x).is_err());
    x = w();
    x.previous_employers[0].ends_on = "2026-07-01".parse().unwrap();
    assert!(t::calculate(&source, &x).is_err());
    x = w();
    x.benefits_reviewed = false;
    assert!(t::ready(&source, &x).is_err());
    let empty = t::calculate(&source, &t::Worksheet::default()).unwrap();
    assert_eq!(empty.coverage_gaps.len(), 1);
    let mut s = source;
    s.pending_payments = 1;
    assert!(t::ready(&s, &w()).is_err());
    s.pending_payments = 0;
    s.other_benefits_floor = 1;
    assert!(t::ready(&s, &w()).is_err());
    assert!(serde_json::from_value::<t::Worksheet>(json!({"openings":[],"extra":true})).is_err());
    assert!(t::annual_tax(-1).is_err());
    assert!(t::annual_tax(t::LIMIT + 1).is_err());
    let _: NaiveDate = "2026-09-30".parse().unwrap();
}
