use chrono::Utc;
use hris_desktop::{
    access::Role,
    domain::{DemoStore, Mode},
    payroll_reporting::{self as p, Filter, Report, Row, Wire},
};
use serde_json::json;
use uuid::Uuid;
fn filter() -> Filter {
    serde_json::from_value(json!({"starts_on":"2026-01-01","ends_on":"2026-12-31","axis":"issued","kind":"all","employee_id":null,"search":""})).unwrap()
}
fn row(kind: &str, n: u128) -> Row {
    serde_json::from_value(json!({"id":Uuid::from_u128(n),"company_id":Uuid::from_u128(1),"employee_id":Uuid::from_u128(2),"kind":kind,"employee_code":"T-1","employee_name":"Fictional person","department":"Operations","period_start":"2026-09-01","period_end":"2026-09-30","issued_on":"2026-10-01","received_on":null,"status":"approved","payment_state":"unpaid","basic_cents":null,"gross_cents":null,"employee_contributions_cents":null,"employer_contributions_cents":null,"withholding_cents":null,"net_cents":null,"contained_source_net_cents":null,"cash_cents":0,"offset_cents":0,"tax_adjustment_cents":null,"outstanding_cents":null,"reserved_cents":0,"reserved_case_id":null,"rule_version":"fictional"})).unwrap()
}
fn source(kind: &str, n: u128, net: i64) -> Row {
    let mut r = row(kind, n);
    r.net_cents = Some(net);
    r.gross_cents = Some(net);
    r.outstanding_cents = Some(net);
    r
}
fn report(rows: Vec<Row>, f: Filter) -> anyhow::Result<Report> {
    Report::from_wire(
        Wire {
            version: p::VERSION.into(),
            company_name: "Fictional company".into(),
            company_id: Uuid::from_u128(1),
            generated_at: Utc::now(),
            filter: f.clone(),
            rows,
        },
        Uuid::from_u128(1),
        &f,
    )
}
#[test]
fn cash_offsets_advice_and_current_balances_stay_distinct() {
    let mut a = source("monthly", 10, 10001);
    a.cash_cents = 10001;
    a.received_on = Some("2026-10-05".parse().unwrap());
    a.payment_state = "received".into();
    a.outstanding_cents = Some(0);
    let mut b = source("cutoff", 11, 20002);
    b.offset_cents = 20002;
    b.received_on = Some("2026-10-10".parse().unwrap());
    b.payment_state = "final_offset".into();
    b.outstanding_cents = Some(0);
    b.reserved_case_id = Some(Uuid::from_u128(12));
    let mut f = row("final", 12);
    f.net_cents = Some(25003);
    f.contained_source_net_cents = Some(20002);
    f.cash_cents = 25003;
    f.status = "settled".into();
    f.payment_state = "settled".into();
    f.received_on = b.received_on;
    let mut t = row("annual_tax", 13);
    t.tax_adjustment_cents = Some(-1234);
    t.status = "settled".into();
    t.payment_state = "settled_adjustment".into();
    t.received_on = Some("2026-10-09".parse().unwrap());
    let mut pending = source("thirteenth", 14, 30000);
    pending.payment_state = "reserved".into();
    pending.reserved_cents = 30000;
    pending.reserved_case_id = Some(Uuid::from_u128(15));
    let r = report(vec![a, b, f, t, pending], filter()).unwrap();
    assert_eq!(r.totals.recorded_cash_cents, 35004);
    assert_eq!(r.totals.source_offset_cents, 20002);
    assert_eq!(r.totals.final_additional_net_cents, 5001);
    assert_eq!(r.totals.annual_settled_adjustment_cents, -1234);
    assert_eq!(r.totals.current_source_outstanding_cents, 30000);
    assert_eq!(r.totals.current_source_reserved_cents, 30000);
}
#[test]
fn payment_date_filter_uses_actual_cross_month_receipts_and_not_issue_date() {
    let mut f = filter();
    f.axis = "received".into();
    f.starts_on = "2026-11-01".parse().unwrap();
    f.ends_on = "2026-11-30".parse().unwrap();
    let mut r = source("monthly", 10, 1);
    r.payment_state = "received".into();
    r.outstanding_cents = Some(0);
    r.cash_cents = 1;
    r.received_on = Some("2026-11-05".parse().unwrap());
    assert!(f.matches(&r));
    assert_eq!(
        report(vec![r.clone()], f.clone())
            .unwrap()
            .totals
            .recorded_cash_cents,
        1
    );
    r.received_on = None;
    assert!(!f.matches(&r));
    assert!(report(vec![r], f).is_err());
}
#[test]
fn withdrawn_final_and_missing_legacy_receipt_never_invent_cash_or_balance() {
    let mut f = row("final", 10);
    f.net_cents = Some(17000);
    f.contained_source_net_cents = Some(20000);
    f.status = "withdrawn".into();
    f.payment_state = "withdrawn".into();
    let mut s = source("monthly", 11, 20000);
    s.status = "paid".into();
    s.payment_state = "receipt_missing".into();
    s.outstanding_cents = None;
    let r = report(vec![f, s], filter()).unwrap();
    assert_eq!(r.totals.recorded_cash_cents, 0);
    assert_eq!(r.totals.current_source_outstanding_cents, 0);
    assert_eq!(r.totals.missing_receipts, 1);
    assert_eq!(r.totals.final_additional_net_cents, -3000);
}
#[test]
fn rejects_duplicate_foreign_and_inconsistent_financial_rows() {
    let a = source("monthly", 10, 100);
    assert!(report(vec![a.clone(), a.clone()], filter()).is_err());
    let mut b = a.clone();
    b.company_id = Uuid::new_v4();
    assert!(report(vec![b], filter()).is_err());
    let mut b = a.clone();
    b.cash_cents = 100;
    assert!(report(vec![b], filter()).is_err());
    let mut b = a.clone();
    b.offset_cents = 100;
    b.payment_state = "final_offset".into();
    b.outstanding_cents = Some(0);
    b.received_on = Some("2026-10-01".parse().unwrap());
    assert!(report(vec![b], filter()).is_err());
    let mut b = row("annual_tax", 11);
    b.payment_state = "advice_only".into();
    b.tax_adjustment_cents = Some(100);
    b.cash_cents = 1;
    assert!(report(vec![b], filter()).is_err());
    let mut b = a;
    b.reserved_cents = 101;
    assert!(report(vec![b], filter()).is_err());
}
#[test]
fn safe_exact_csv_does_not_include_private_fields_or_float_rounding() {
    let mut a = source("monthly", 10, 123457);
    a.employee_name = "=1+1".into();
    a.department = "Comma, quote \"".into();
    let csv = String::from_utf8(report(vec![a], filter()).unwrap().csv().unwrap()).unwrap();
    assert!(csv.contains("1234.57"));
    assert!(csv.contains("'=1+1"));
    assert!(csv.contains("\"Comma, quote \"\"\""));
    for k in [
        "payment_reference",
        "source_basis",
        "worksheet",
        "tin",
        "PRIVATE_",
    ] {
        assert!(!csv.contains(k));
    }
}
#[test]
fn current_staff_scope_date_bounds_and_empty_results_are_explicit() {
    let mut d = DemoStore::new();
    assert!(
        p::demo(&d.data, &filter(), Utc::now())
            .unwrap()
            .wire
            .rows
            .is_empty()
    );
    d.data.access.role = Role::Manager;
    assert!(p::demo(&d.data, &filter(), Utc::now()).is_err());
    d.data.access.role = Role::Administrator;
    d.data.mode = Mode::SignedOut;
    assert!(p::demo(&d.data, &filter(), Utc::now()).is_err());
    let mut f = filter();
    f.ends_on = "2027-01-02".parse().unwrap();
    assert!(f.validate().is_err());
    f.ends_on = "2027-01-01".parse().unwrap();
    assert!(f.validate().is_ok());
    f.axis = "period".into();
    assert!(f.validate().is_err());
}
#[test]
fn exact_postgres_projection_decodes_without_private_or_misclassified_facts() {
    let path = std::path::Path::new("work/payroll-report40-vectors.json");
    if !path.exists() {
        return;
    }
    let vectors: Vec<Wire> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert!(vectors.len() >= 15);
    for v in vectors {
        let result = Report::from_wire(v.clone(), v.company_id, &v.filter).unwrap();
        let csv = String::from_utf8(result.csv().unwrap()).unwrap();
        assert!(!csv.contains("PRIVATE_"));
        assert_eq!(
            result.totals.recorded_cash_cents,
            v.rows.iter().map(|r| r.cash_cents).sum::<i64>()
        );
    }
}
