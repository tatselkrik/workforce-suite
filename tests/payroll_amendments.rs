use hris_desktop::payroll_amendments as p;
use serde_json::json;
fn fixture() -> (p::Source, p::Worksheet) {
    let s=serde_json::from_value(json!({"kind":"cutoff","slip_id":uuid::Uuid::from_u128(1),"employee_id":uuid::Uuid::from_u128(2),"starts_on":"2026-09-01","ends_on":"2026-09-15","paid_on":"2026-09-15","cash_date":"2026-10-24","original_net_cents":4000000,"sss_refundable_cents":100000,"philhealth_refundable_cents":50000,"pagibig_refundable_cents":20000,"withholding_refundable_cents":250000,"shared_floor_cents":8900000})).unwrap();
    let w = p::Worksheet {
        purpose: "payout".into(),
        basic_cents: 100000,
        taxable_cents: 200000,
        shared_cents: 200000,
        other_benefits_cents: 8900000,
        exempt_cents: 30000,
        sss_cents: 10000,
        philhealth_cents: 5000,
        pagibig_cents: 1000,
        withholding_cents: 20000,
        lawful_deduction_cents: 4000,
        evidence: "PRIVATE_AMEND_EVIDENCE".into(),
        tax_note: "PRIVATE_AMEND_TAX".into(),
        contribution_note: "PRIVATE_AMEND_CONTRIBUTIONS".into(),
        review_note: "PRIVATE_AMEND_REVIEW".into(),
        reviewed: true,
        tax_reviewed: true,
        contributions_reviewed: true,
        ..Default::default()
    };
    (s, w)
}
#[test]
fn centavos_and_one_shared_exclusion() {
    let (s, w) = fixture();
    let c = p::ready(&s, &w).unwrap();
    assert_eq!(
        (
            c.gross_cents,
            c.taxable_cents,
            c.benefit_exempt_cents,
            c.contribution_cents,
            c.net_cents
        ),
        (430000, 284000, 100000, 16000, 390000)
    );
}
#[test]
fn only_remaining_same_year_charge_can_be_refunded() {
    let (mut s, mut w) = fixture();
    w.sss_cents = -100000;
    w.philhealth_cents = -50000;
    w.pagibig_cents = -20000;
    w.withholding_cents = -250000;
    let c = p::ready(&s, &w).unwrap();
    assert_eq!(
        (c.contribution_cents, c.taxable_cents, c.net_cents),
        (-170000, 470000, 846000)
    );
    w.withholding_cents = -250001;
    assert!(p::ready(&s, &w).is_err());
    w.withholding_cents = -1;
    s.paid_on = "2025-12-31".parse().unwrap();
    assert!(p::ready(&s, &w).is_err());
}
#[test]
fn reviews_create_no_employee_debt_or_tax_fact() {
    let (s, _) = fixture();
    let mut w = p::Worksheet {
        purpose: "review_only".into(),
        review_amount_cents: -300000,
        evidence: "PRIVATE_DISCREPANCY".into(),
        review_note: "No automatic recovery".into(),
        reviewed: true,
        ..Default::default()
    };
    let c = p::ready(&s, &w).unwrap();
    assert_eq!(
        (
            c.gross_cents,
            c.taxable_cents,
            c.net_cents,
            c.review_amount_cents
        ),
        (0, 0, 0, -300000)
    );
    w.withholding_cents = -1;
    assert!(p::ready(&s, &w).is_err());
}
#[test]
fn invalid_cash_year_overclaim_and_missing_reviews_are_denied() {
    let (mut s, mut w) = fixture();
    for flag in [0, 1, 2] {
        let mut b = w.clone();
        match flag {
            0 => b.reviewed = false,
            1 => b.tax_reviewed = false,
            _ => b.contributions_reviewed = false,
        };
        assert!(p::ready(&s, &b).is_err());
    }
    w.basic_cents = 200001;
    assert!(p::calculate(&s, &w).is_err());
    w.basic_cents = 100000;
    w.other_benefits_cents = 8899999;
    assert!(p::ready(&s, &w).is_err());
    w.other_benefits_cents = 8900000;
    s.cash_date = "2027-01-01".parse().unwrap();
    assert!(p::calculate(&s, &w).is_err());
}
#[test]
fn postgres_centavo_vectors_agree_with_rust() {
    let rows: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/amendment-rule-vectors.json")).unwrap();
    assert!(rows.as_array().unwrap().len() >= 14);
    for v in rows.as_array().unwrap() {
        let s: p::Source = serde_json::from_value(v["source"].clone()).unwrap();
        let w: p::Worksheet = serde_json::from_value(v["worksheet"].clone()).unwrap();
        assert_eq!(
            serde_json::to_value(p::ready(&s, &w).unwrap()).unwrap(),
            v["result"]
        );
    }
}
