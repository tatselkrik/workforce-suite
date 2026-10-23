use chrono::NaiveDate;
use hris_desktop::{
    access::Role,
    domain::{Action, DemoStore},
    payroll::{Pay, PayrollAction, Worksheet},
    payroll_cutoffs::{self as cutoff, Calculation},
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;
#[test]
fn rust_matches_postgres_for_cutoff_centavos_and_monthly_reconciliation() {
    #[derive(Deserialize)]
    struct Vector {
        pay: Pay,
        worksheet: Worksheet,
        month: NaiveDate,
        part: u8,
        previous: Option<Calculation>,
        result: Calculation,
    }
    let vectors: Vec<Vector> =
        serde_json::from_str(include_str!("fixtures/cutoff-rule-vectors.json")).unwrap();
    assert!(vectors.len() >= 10);
    for v in vectors {
        assert_eq!(
            cutoff::calculate(&v.pay, &v.worksheet, v.month, v.part, v.previous.as_ref()).unwrap(),
            v.result
        );
    }
}
fn execute(s: &mut DemoStore, op: &str, v: serde_json::Value) {
    s.execute(Action::Payroll(PayrollAction::new(op, v)))
        .unwrap();
}
#[test]
fn cutoff_overtime_without_authorization_requires_an_explicit_exception() {
    let mut s = DemoStore::new();
    let eid = s.data.employees[1].id;
    let run = Uuid::new_v4();
    let item = Uuid::new_v4();
    execute(
        &mut s,
        "profile_save",
        json!({"id":Uuid::new_v4(),"employee_id":eid,"effective_on":"2026-02-01","pay":Pay{rate_cents:3000000,..Default::default()},"reason":"Synthetic profile"}),
    );
    execute(
        &mut s,
        "cutoff_run_create",
        json!({"id":run,"month":"2026-09-01","part":1,"pay_date":"2026-09-15","title":"Synthetic overtime review","reason":"Synthetic cutoff"}),
    );
    execute(
        &mut s,
        "cutoff_item_add",
        json!({"id":item,"run_id":run,"employee_id":eid,"reason":"Synthetic employee"}),
    );
    let mut w = Worksheet {
        reviewed: true,
        review_note: "Reviewed actual time".into(),
        days: vec![hris_desktop::payroll::Day {
            date: NaiveDate::from_ymd_opt(2026, 9, 1).unwrap(),
            kind: "ordinary".into(),
            overtime_minutes: 60,
            ..Default::default()
        }],
        ..Default::default()
    };
    execute(
        &mut s,
        "cutoff_item_save",
        json!({"id":item,"expected_revision":0,"worksheet":w,"reason":"Unapproved overtime fixture"}),
    );
    let revision = s.data.payroll.cutoffs.runs[0].base.revision;
    assert!(s.execute(Action::Payroll(PayrollAction::new("cutoff_run_submit",json!({"id":run,"expected_revision":revision,"scope_reviewed":true,"reason":"Must require exception"})))).is_err());
    w.exception_note = "Reviewed correction awaiting historical authorization".into();
    execute(
        &mut s,
        "cutoff_item_save",
        json!({"id":item,"expected_revision":1,"worksheet":w,"reason":"Document explicit exception"}),
    );
    let revision = s.data.payroll.cutoffs.runs[0].base.revision;
    execute(
        &mut s,
        "cutoff_run_submit",
        json!({"id":run,"expected_revision":revision,"scope_reviewed":true,"reason":"Exception documented for independent approval"}),
    );
}
#[test]
fn issued_cutoff_preserves_its_period_redacts_hr_notes_and_limits_exports() {
    let mut s = DemoStore::new();
    let eid = s.data.employees[1].id;
    let p = Uuid::new_v4();
    let r = Uuid::new_v4();
    let i = Uuid::new_v4();
    execute(
        &mut s,
        "profile_save",
        json!({"id":p,"employee_id":eid,"effective_on":"2026-02-01","pay":Pay{rate_cents:3000000,coverage_note:"PRIVATE_CUTOFF_NOTE".into(),..Default::default()},"reason":"Synthetic profile"}),
    );
    execute(
        &mut s,
        "cutoff_run_create",
        json!({"id":r,"month":"2026-09-01","part":1,"pay_date":"2026-09-15","title":"Synthetic cutoff","reason":"Synthetic period"}),
    );
    execute(
        &mut s,
        "cutoff_item_add",
        json!({"id":i,"run_id":r,"employee_id":eid,"reason":"Synthetic employee"}),
    );
    let w = Worksheet {
        reviewed: true,
        review_note: "PRIVATE_CUTOFF_REVIEW".into(),
        ..Default::default()
    };
    execute(
        &mut s,
        "cutoff_item_save",
        json!({"id":i,"expected_revision":0,"worksheet":w,"reason":"Synthetic review"}),
    );
    let revision = s.data.payroll.cutoffs.runs[0].base.revision;
    execute(
        &mut s,
        "cutoff_run_submit",
        json!({"id":r,"expected_revision":revision,"scope_reviewed":true,"reason":"Synthetic submit"}),
    );
    let revision = s.data.payroll.cutoffs.runs[0].base.revision;
    assert!(
        s.execute(Action::Payroll(PayrollAction::new(
            "cutoff_run_approve",
            json!({"id":r,"expected_revision":revision,"reason":"Self approval"})
        )))
        .is_err()
    );
    s.data.access.user_id = Uuid::new_v4();
    execute(
        &mut s,
        "cutoff_run_approve",
        json!({"id":r,"expected_revision":revision,"reason":"Independent approval"}),
    );
    let slip = &s.data.payroll.cutoffs.slips[0];
    assert_eq!(slip.starts_on.to_string(), "2026-09-01");
    assert_eq!(slip.ends_on.to_string(), "2026-09-15");
    assert!(
        !serde_json::to_string(slip)
            .unwrap()
            .contains("PRIVATE_CUTOFF")
    );
    let pdf = hris_desktop::payroll_pdf::build_cutoff(slip).unwrap();
    assert!(pdf.starts_with(b"%PDF-1.4"));
    assert!(String::from_utf8_lossy(&pdf).contains("2026-09-01 to 2026-09-15"));
    s.data.access.role = Role::Manager;
    s.data.access.employee_id = Some(eid);
    assert!(cutoff::export_csv(&s.data, "cutoff_payslip", Some(i)).is_ok());
    assert!(cutoff::export_csv(&s.data, "cutoff_register", Some(r)).is_err());
    s.data.access.employee_id = Some(Uuid::new_v4());
    assert!(cutoff::export_csv(&s.data, "cutoff_payslip", Some(i)).is_err());
}
