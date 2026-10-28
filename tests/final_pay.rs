use hris_desktop::{payroll_final as f, payroll_tax as t};
fn date(s: &str) -> chrono::NaiveDate {
    s.parse().unwrap()
}
fn income(id: &str, lo: &str, hi: &str, taxable: i64, withheld: i64) -> t::Income {
    t::Income {
        id: id.into(),
        starts_on: Some(date(lo)),
        ends_on: Some(date(hi)),
        taxable_cents: taxable,
        withholding_cents: withheld,
        benefit_gross_cents: 0,
        benefit_exempt_cents: 0,
        contribution_cents: 0,
    }
}
fn fixture() -> (f::Source, f::Worksheet) {
    let row = income(
        "cutoff:owed",
        "2026-09-01",
        "2026-09-30",
        10_000_000,
        1_000_000,
    );
    let s = f::Source {
        employment_start: date("2020-01-01"),
        service_end: date("2026-09-30"),
        cash_date: date("2026-10-15"),
        annual: t::Source {
            starts_on: date("2026-01-01"),
            service_end: date("2026-09-30"),
            as_of: date("2026-10-15"),
            income: vec![income(
                "monthly:paid",
                "2026-01-01",
                "2026-08-31",
                20_000_000,
                1_000_000,
            )],
            prior_adjustment_cents: 0,
            pending_payments: 1,
            pending_adjustments: 0,
            external_thirteenth_floor: 0,
            other_benefits_floor: 0,
        },
        outstanding: vec![f::Pending {
            id: row.id.clone(),
            income: row,
            net_cents: 9_000_000,
        }],
        previous_vacation_days_hundredths: 0,
    };
    let w = f::Worksheet {
        entitlements_reviewed: true,
        source_scope_reviewed: true,
        contributions_reviewed: true,
        review_note: "PRIVATE_FINAL_REVIEW".into(),
        contribution_note: "PRIVATE_FINAL_CONTRIBUTIONS".into(),
        tax: t::Worksheet {
            final_compensation_reviewed: true,
            previous_employers_reviewed: true,
            benefits_reviewed: true,
            review_note: "PRIVATE_FINAL_TAX".into(),
            settlement_note: "PRIVATE_FINAL_HANDLING".into(),
            ..Default::default()
        },
        ..Default::default()
    };
    (s, w)
}
#[test]
fn prior_year_separation_uses_receipt_year_without_fabricating_service() {
    let (mut s, mut w) = fixture();
    s.service_end = date("2025-12-31");
    s.annual.service_end = s.service_end;
    s.annual.income.clear();
    s.annual.pending_payments = 0;
    s.outstanding.clear();
    w.lines.push(f::Line {
        label: "Delayed earned pay".into(),
        kind: "taxable".into(),
        amount_cents: 30_000_000,
        reference: "Fictional amount, not available or received in2025".into(),
    });
    assert!(f::ready(&s, &w).is_err());
    w.cash_year_reviewed = true;
    w.cash_year_note = "Fictional evidence: actual/constructive receipt only in2026; no previous withholding or other employer".into();
    let c = f::ready(&s, &w).unwrap();
    assert!(c.tax.coverage_gaps.is_empty());
    assert_eq!(
        (c.tax.taxable_cents, c.tax.annual_tax_cents, c.net_cents),
        (30_000_000, 750_000, 29_250_000)
    );
    assert_eq!(c.due_on, Some(date("2026-01-30")));
    assert!(c.planned_late);
    w.tax.openings.push(t::Opening {
        starts_on: date("2026-01-01"),
        ends_on: date("2026-01-01"),
        taxable_cents: 0,
        withholding_cents: 0,
        benefit_gross_cents: 0,
        benefit_exempt_cents: 0,
        reference: "Invented service is rejected".into(),
    });
    assert!(f::calculate(&s, &w).is_err());
    w.tax.openings.clear();
    w.tax.previous_employers.push(t::PreviousEmployer {
        starts_on: date("2026-01-01"),
        ends_on: date("2026-03-31"),
        employer: "Fictional later employer".into(),
        taxable_cents: 20_000_000,
        withholding_cents: 1_000_000,
        benefit_gross_cents: 0,
        benefit_exempt_cents: 0,
        reference: "Fictional certified income net of refund".into(),
    });
    let c = f::ready(&s, &w).unwrap();
    assert_eq!(
        (c.tax.taxable_cents, c.tax.annual_tax_cents, c.net_cents),
        (50_000_000, 4_250_000, 26_750_000)
    );
    w.tax.previous_employers[0].ends_on = date("2026-12-31");
    assert!(f::calculate(&s, &w).is_err());
    w.tax.previous_employers[0].ends_on = date("2026-03-31");
    s.service_end = date("2024-12-31");
    s.annual.service_end = s.service_end;
    assert!(
        f::calculate(&s, &w)
            .unwrap_err()
            .to_string()
            .contains("historical")
    );
}
#[test]
fn unpaid_source_enters_annual_once_and_refund_is_in_final_cash() {
    let (s, w) = fixture();
    let c = f::ready(&s, &w).unwrap();
    assert_eq!(
        (
            c.source_net_cents,
            c.tax.taxable_cents,
            c.tax.current_withheld_cents,
            c.tax.refund_cents,
            c.net_cents
        ),
        (9_000_000, 30_000_000, 2_000_000, 1_250_000, 10_250_000)
    );
    assert_eq!(c.due_on, Some(date("2026-10-30")));
    assert!(!c.planned_late);
}
#[test]
fn vacation_12_day_exclusion_then_shared_cap_not_second_exclusion() {
    let (mut s, mut w) = fixture();
    w.vacation = f::Vacation {
        days_hundredths: 1300,
        daily_cents: 100_000,
        previous_days_hundredths: 0,
        reference: "PRIVATE_VACATION".into(),
    };
    let c = f::ready(&s, &w).unwrap();
    assert_eq!(
        (
            c.vacation_gross_cents,
            c.vacation_exempt_cents,
            c.vacation_shared_benefit_cents,
            c.tax.taxable_cents,
            c.net_cents
        ),
        (1_300_000, 1_200_000, 100_000, 30_000_000, 11_550_000)
    );
    s.annual.income.push(t::Income {
        id: "benefit:paid".into(),
        starts_on: None,
        ends_on: None,
        taxable_cents: 0,
        withholding_cents: 0,
        benefit_gross_cents: 9_000_000,
        benefit_exempt_cents: 9_000_000,
        contribution_cents: 0,
    });
    let c = f::ready(&s, &w).unwrap();
    assert_eq!(
        (c.tax.benefit_exempt_cents, c.tax.taxable_cents, c.net_cents),
        (9_000_000, 30_100_000, 11_535_000)
    );
    w.vacation.previous_days_hundredths = 1250;
    let c = f::ready(&s, &w).unwrap();
    assert_eq!(
        (c.vacation_exempt_cents, c.tax.taxable_cents, c.net_cents),
        (0, 31_300_000, 11_355_000)
    );
}
#[test]
fn separation_includes_regular_allowances_and_subtracts_prior_award_payment() {
    let (s, mut w) = fixture();
    w.separation = f::Separation {
        cause: "retrenchment".into(),
        monthly_basic_cents: 3_000_000,
        regular_allowances_cents: 500_000,
        total_award_cents: 12_250_000,
        previously_paid_cents: 2_250_000,
        tax_exempt: true,
        reference: "PRIVATE_SEPARATION".into(),
    };
    let c = f::ready(&s, &w).unwrap();
    assert_eq!(
        (
            c.credited_service_years,
            c.separation_floor_cents,
            c.separation_cents,
            c.net_cents
        ),
        (7, 12_250_000, 10_000_000, 20_250_000)
    );
    w.separation.total_award_cents -= 1;
    assert!(f::calculate(&s, &w).is_err());
    w.separation.cause = "resignation".into();
    assert!(f::calculate(&s, &w).is_err());
}
#[test]
fn short_service_separation_has_one_month_floor_and_exact_six_month_rounding() {
    let (mut s, mut w) = fixture();
    s.employment_start = date("2026-08-01");
    s.annual.starts_on = s.employment_start;
    s.annual.income[0].starts_on = Some(s.employment_start);
    w.separation = f::Separation {
        cause: "retrenchment".into(),
        monthly_basic_cents: 3_000_000,
        regular_allowances_cents: 500_000,
        total_award_cents: 3_500_000,
        reference: "PRIVATE_SEPARATION".into(),
        ..Default::default()
    };
    assert_eq!(
        f::calculate(&s, &w).unwrap().separation_floor_cents,
        3_500_000
    );
    assert_eq!(
        f::service_years(date("2026-01-01"), date("2026-06-29")).unwrap(),
        (0, 0)
    );
    assert_eq!(
        f::service_years(date("2026-01-01"), date("2026-06-30")).unwrap(),
        (0, 1)
    );
    assert_eq!(
        f::service_years(date("2020-02-29"), date("2026-02-27")).unwrap(),
        (6, 6)
    );
}
#[test]
fn actual_five_year_retirement_eligibility_is_distinct_from_rounding() {
    let (mut s, mut w) = fixture();
    s.employment_start = date("2021-01-01");
    w.retirement = f::Retirement {
        method: "statutory".into(),
        daily_basic_cents: 100_000,
        plan_award_cents: 14_000_000,
        previously_paid_cents: 500_000,
        birth_date: Some(date("1966-01-01")),
        ordinary_statutory_coverage: true,
        tax_treatment: "statutory_no_plan".into(),
        reference: "PRIVATE_RETIREMENT".into(),
        ..Default::default()
    };
    let c = f::ready(&s, &w).unwrap();
    assert_eq!(
        (c.retirement_floor_cents, c.retirement_cents, c.net_cents),
        (13_500_000, 13_500_000, 23_750_000)
    );
    s.employment_start = date("2022-01-01");
    assert_eq!(
        f::service_years(s.employment_start, s.service_end).unwrap(),
        (4, 5)
    );
    assert!(f::calculate(&s, &w).is_err());
}
#[test]
fn employee_covered_but_ineligible_under_plan_cannot_fallback_to_statutory_tax_exemption() {
    let (s, mut w) = fixture();
    w.retirement = f::Retirement {
        method: "statutory".into(),
        daily_basic_cents: 100_000,
        birth_date: Some(date("1960-01-01")),
        ordinary_statutory_coverage: true,
        employee_plan_covered: true,
        tax_treatment: "statutory_no_plan".into(),
        reference: "PRIVATE_COVERAGE_REVIEW".into(),
        ..Default::default()
    };
    assert!(f::calculate(&s, &w).is_err());
    w.retirement.tax_treatment = "taxable".into();
    assert!(f::ready(&s, &w).unwrap().taxable_extras_cents > 0);
    w.retirement.tax_treatment = "qualified_plan".into();
    w.retirement.qualified_plan_reviewed = true;
    w.retirement.first_qualified_availment = true;
    assert!(f::calculate(&s, &w).is_err());
}
#[test]
fn deductions_and_contributions_once_with_explicit_employer_deficiency() {
    let (mut s, mut w) = fixture();
    s.annual.income[0].taxable_cents = 40_000_000;
    s.annual.income[0].withholding_cents = 0;
    s.outstanding[0].income.withholding_cents = 0;
    s.outstanding[0].net_cents = 100_000;
    let c = f::ready(&s, &w).unwrap();
    assert_eq!(
        (
            c.tax.annual_tax_cents,
            c.tax.available_deduction_cents,
            c.tax.employer_advance_cents,
            c.net_cents
        ),
        (4_250_000, 100_000, 4_150_000, 0)
    );
    w.lines.push(f::Line {
        label: "Deposit return".into(),
        kind: "deposit_return".into(),
        amount_cents: 1_000_000,
        reference: "PRIVATE_DEPOSIT".into(),
    });
    w.lines.push(f::Line {
        label: "Reviewed deduction".into(),
        kind: "lawful_deduction".into(),
        amount_cents: 100_000,
        reference: "PRIVATE_LAWFUL_DEDUCTION".into(),
    });
    w.sss_adjustment_cents = 50_000;
    let c = f::ready(&s, &w).unwrap();
    assert_eq!(
        (
            c.tax.taxable_cents,
            c.cash_before_tax_cents,
            c.tax.employer_advance_cents
        ),
        (49_950_000, 950_000, 3_290_000)
    );
    w.lines[1].amount_cents = 2_000_000;
    assert!(f::calculate(&s, &w).is_err());
}
#[test]
fn pending_advice_duplicate_source_omitted_pay_and_unreviewed_scope_block_issue() {
    let (s, w) = fixture();
    let mut x = s.clone();
    x.annual.pending_adjustments = 1;
    assert!(f::ready(&x, &w).is_err());
    x = s.clone();
    x.outstanding.push(x.outstanding[0].clone());
    x.annual.pending_payments = 2;
    assert!(f::calculate(&x, &w).is_err());
    x = s.clone();
    x.outstanding.clear();
    assert!(f::calculate(&x, &w).is_err());
    let mut a = w.clone();
    a.source_scope_reviewed = false;
    assert!(f::ready(&s, &a).is_err());
    a = w.clone();
    a.tax.previous_employers_reviewed = false;
    assert!(f::ready(&s, &a).is_err());
    x = s.clone();
    x.previous_vacation_days_hundredths = 100;
    assert!(f::calculate(&x, &w).is_err());
}
#[test]
fn late_plan_is_flagged_and_cross_year_rule_pack_cannot_be_assumed() {
    let (mut s, mut w) = fixture();
    s.cash_date = date("2026-11-01");
    s.annual.as_of = s.cash_date;
    assert!(f::ready(&s, &w).unwrap().planned_late);
    w.release_within_days = 31;
    assert!(f::calculate(&s, &w).is_err());
    w.release_within_days = 15;
    assert!(f::ready(&s, &w).unwrap().planned_late);
    s.cash_date = date("2027-01-01");
    s.annual.as_of = s.cash_date;
    assert!(f::calculate(&s, &w).is_err());
}

#[test]
fn final_pay_centavos_match_postgres_for_service_floors_vacation_and_prospective_tax() {
    #[derive(serde::Deserialize)]
    struct Vector {
        source: f::Source,
        worksheet: f::Worksheet,
        result: f::Calculation,
    }
    let cases: Vec<Vector> =
        serde_json::from_str(include_str!("fixtures/final-pay-rule-vectors.json")).unwrap();
    assert_eq!(cases.len(), 28);
    for c in cases {
        assert_eq!(f::calculate(&c.source, &c.worksheet).unwrap(), c.result);
    }
}
