use chrono::NaiveDate;
use hris_desktop::{
    access::Role,
    domain::{Action, DemoStore},
    payroll::*,
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;
fn mon() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 9, 1).unwrap()
}
fn pay() -> Pay {
    Pay {
        rate_cents: 3_000_000,
        ..Default::default()
    }
}
fn day(kind: &str, rest: bool) -> Day {
    Day {
        date: mon(),
        kind: kind.into(),
        rest,
        regular_minutes: 480,
        overtime_minutes: 60,
        night_regular: 60,
        night_overtime: 60,
        ..Default::default()
    }
}
#[test]
fn published_dole_multipliers_compound_night_and_overtime() {
    let p = Pay {
        basis: "daily".into(),
        rate_cents: 80_000,
        monthly_reference_cents: 2_080_000,
        ..Default::default()
    };
    for (kind, rest, factor) in [
        ("ordinary", false, 100),
        ("ordinary", true, 130),
        ("special", false, 130),
        ("special", true, 150),
        ("double_special", true, 195),
        ("regular", false, 200),
        ("regular", true, 260),
        ("double_regular", false, 300),
        ("double_regular", true, 390),
    ] {
        let w = Worksheet {
            days: vec![day(kind, rest)],
            ..Default::default()
        };
        let t = calculate(&p, &w, mon()).unwrap();
        let ot = if factor == 100 { 125 } else { 130 };
        assert_eq!(t.basic, 80_000);
        assert_eq!(t.premium, 800 * (factor - 100));
        assert_eq!(t.overtime, factor * ot);
        assert_eq!(t.night, 10 * factor + (factor * ot + 5) / 10);
    }
}
#[test]
fn statutory_monthly_30000_and_no_automatic_absence_deduction() {
    let t = calculate(&pay(), &Worksheet::default(), mon()).unwrap();
    assert_eq!(
        (t.basic, t.sss_employee, t.sss_employer, t.ec),
        (3_000_000, 150_000, 300_000, 3_000)
    );
    assert_eq!(
        (t.sss_regular_employee, t.sss_mpf_employee),
        (100_000, 50_000)
    );
    assert_eq!(
        (
            t.philhealth_employee,
            t.pagibig_employee,
            t.withholding,
            t.net
        ),
        (75_000, 20_000, 100_755, 2_654_245)
    );
}
#[test]
fn monthly_unworked_double_holiday_adds_only_the_second_day() {
    let p = pay();
    let mut w = Worksheet {
        days: vec![Day {
            date: mon(),
            kind: "double_regular".into(),
            unworked_holiday_minutes: 480,
            ..Default::default()
        }],
        ..Default::default()
    };
    let t = calculate(&p, &w, mon()).unwrap();
    assert_eq!(t.basic, p.rate_cents);
    assert_eq!(t.unworked_holiday, (p.rate_cents * 12 + 182) / 365);
    w.days[0].kind = "regular".into();
    assert_eq!(calculate(&p, &w, mon()).unwrap().unworked_holiday, 0);
    w.days[0].kind = "double_regular".into();
    let exempt = Pay {
        premium_eligible: false,
        coverage_note: "Reviewed coverage exception".into(),
        ..p
    };
    assert_eq!(calculate(&exempt, &w, mon()).unwrap().unworked_holiday, 0);
    w.days[0].paid_leave_minutes = 60;
    assert!(calculate(&pay(), &w, mon()).is_err());
    w.days[0].paid_leave_minutes = 0;
    w.days[0].unpaid_minutes = 60;
    assert!(calculate(&pay(), &w, mon()).is_err());
}
#[test]
fn sss_brackets_round_at_250_and_split_provident() {
    for (basis, msc, ec) in [
        (524_999, 500_000, 1000),
        (525_000, 550_000, 1000),
        (1_474_999, 1_450_000, 1000),
        (1_475_000, 1_500_000, 3000),
        (3_475_000, 3_500_000, 3000),
    ] {
        let w = Worksheet {
            sss_basis_override: Some(basis),
            exception_note: "Reviewed fixture".into(),
            ..Default::default()
        };
        let t = calculate(&pay(), &w, mon()).unwrap();
        assert_eq!((t.sss_msc, t.ec), (msc, ec));
        assert_eq!(t.sss_regular_employee + t.sss_mpf_employee, t.sss_employee);
    }
}
#[test]
fn philhealth_uses_fixed_basic_despite_unpaid_time() {
    let w = Worksheet {
        days: vec![Day {
            regular_minutes: 420,
            unpaid_minutes: 60,
            overtime_minutes: 0,
            night_overtime: 0,
            ..day("ordinary", false)
        }],
        ..Default::default()
    };
    let t = calculate(&pay(), &w, mon()).unwrap();
    assert!(t.basic < 3_000_000);
    assert_eq!(t.philhealth_employee, 75_000);
}
#[test]
fn de_minimis_caps_aggregate_across_recurring_and_adjustments() {
    let p = Pay {
        components: vec![Component {
            label: "Rice".into(),
            kind: "rice".into(),
            amount_cents: 200_000,
            reference: String::new(),
        }],
        ..pay()
    };
    let w = Worksheet {
        adjustments: vec![Component {
            label: "More rice".into(),
            kind: "rice".into(),
            amount_cents: 100_000,
            reference: String::new(),
        }],
        ..Default::default()
    };
    let t = calculate(&p, &w, mon()).unwrap();
    assert_eq!((t.allowances, t.exempt_allowances), (300_000, 250_000));
}
#[test]
fn voluntary_deductions_and_advances_do_not_reduce_taxable_income() {
    let p = pay();
    let plain = calculate(&p, &Worksheet::default(), mon()).unwrap();
    let w = Worksheet {
        adjustments: vec![
            Component {
                label: "Loan".into(),
                kind: "deduction".into(),
                amount_cents: 100_000,
                reference: "Consent".into(),
            },
            Component {
                label: "Prior salary".into(),
                kind: "advance".into(),
                amount_cents: 200_000,
                reference: "Payment reference".into(),
            },
        ],
        ..Default::default()
    };
    let t = calculate(&p, &w, mon()).unwrap();
    assert_eq!(t.taxable, plain.taxable);
    assert_eq!(t.withholding, plain.withholding);
    assert_eq!(t.net, plain.net - 300_000);
}
#[test]
fn coverage_exceptions_require_review_not_an_account_role() {
    let mut p = pay();
    p.premium_eligible = false;
    assert!(p.validate().is_err());
    p.coverage_note = "Independent labor-law coverage review".into();
    let t = calculate(
        &p,
        &Worksheet {
            days: vec![day("regular", true)],
            ..Default::default()
        },
        mon(),
    )
    .unwrap();
    assert_eq!((t.premium, t.overtime, t.night), (0, 0, 0));
}
#[test]
fn daily_unworked_holiday_is_subject_to_explicit_eligibility() {
    let p = Pay {
        basis: "daily".into(),
        rate_cents: 80_000,
        monthly_reference_cents: 2_080_000,
        ..Default::default()
    };
    let w = Worksheet {
        days: vec![Day {
            date: mon(),
            kind: "double_regular".into(),
            unworked_holiday_minutes: 480,
            ..Default::default()
        }],
        ..Default::default()
    };
    assert_eq!(calculate(&p, &w, mon()).unwrap().unworked_holiday, 160_000);
    let mut invalid = w;
    invalid.days[0].regular_minutes = 480;
    assert!(calculate(&p, &invalid, mon()).is_err());
}
#[test]
fn worksheet_rejects_duplicate_days_invalid_subsets_and_undocumented_overrides() {
    let mut w = Worksheet {
        days: vec![day("ordinary", false)],
        ..Default::default()
    };
    w.days[0].night_overtime = 61;
    assert!(calculate(&pay(), &w, mon()).is_err());
    w.days[0].night_overtime = 60;
    w.days.push(w.days[0].clone());
    assert!(calculate(&pay(), &w, mon()).is_err());
    w.days.clear();
    w.basic_override_cents = Some(10);
    assert!(calculate(&pay(), &w, mon()).is_err());
}
#[test]
fn monthly_tax_official_bracket_edges() {
    assert_eq!(monthly_tax(2_083_300), 0);
    assert_eq!(monthly_tax(3_333_300), 187_500);
    assert_eq!(monthly_tax(6_666_700), 854_180);
    assert_eq!(monthly_tax(16_666_700), 3_354_180);
    assert_eq!(monthly_tax(66_666_700), 18_354_180);
}
#[test]
fn action_and_exports_reject_manager_team_payroll() {
    let mut data = DemoStore::new().data;
    data.access.role = Role::Manager;
    let a = PayrollAction::new(
        "run_create",
        json!({"id":Uuid::new_v4(),"month":mon(),"title":"Synthetic","reason":"Synthetic"}),
    );
    assert!(data.access.authorize(&Action::Payroll(a), &data).is_err());
    assert!(export_csv(&data, "annual", None, 2026).is_err());
}
#[test]
fn overnight_proposal_splits_midnight_holiday_and_night() {
    let mut data = DemoStore::new().data;
    let eid = data.employees[1].id;
    data.attendance.clear();
    let a = hris_desktop::domain::Attendance {
        id: Uuid::new_v4(),
        company_id: data.company.id,
        employee_id: eid,
        work_date: mon(),
        checked_in_at: "2026-09-01T14:00:00Z".parse().unwrap(),
        checked_out_at: Some("2026-09-02T00:00:00Z".parse().unwrap()),
        revision: 0,
        planned_shift: None,
        calculation_snapshot: None,
    };
    data.company.timezone = "Asia/Manila".into();
    data.payroll.holidays.push(Holiday {
        id: Uuid::new_v4(),
        company_id: data.company.id,
        date: mon().succ_opt().unwrap(),
        name: "Fictional holiday".into(),
        kind: "regular".into(),
        source: "Synthetic".into(),
        revision: 0,
    });
    data.attendance.push(a);
    let (ds, _) = attendance_proposal(&data, eid, mon());
    assert_eq!(ds.len(), 2);
    assert_eq!((ds[0].regular_minutes, ds[0].night_regular), (120, 120));
    assert_eq!(
        (
            ds[1].regular_minutes,
            ds[1].overtime_minutes,
            ds[1].night_regular,
            ds[1].night_overtime
        ),
        (360, 120, 360, 0)
    );
    assert_eq!(ds[1].kind, "regular");
}
#[test]
fn rust_and_postgres_centavo_parity_for_statutory_vectors() {
    #[derive(Deserialize)]
    struct Vector {
        pay: Pay,
        worksheet: Worksheet,
        month: NaiveDate,
        totals: Totals,
    }
    let vectors: Vec<Vector> =
        serde_json::from_str(include_str!("fixtures/payroll-rule-vectors.json")).unwrap();
    assert!(vectors.len() >= 34);
    for v in vectors {
        assert_eq!(calculate(&v.pay, &v.worksheet, v.month).unwrap(), v.totals);
    }
}
