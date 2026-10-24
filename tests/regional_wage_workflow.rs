use hris_desktop::{
    access::Role,
    domain::{Action, DemoStore, EmployeeStatus},
    payroll::{Pay, PayrollAction as A, Worksheet},
    payroll_wages as w,
};
use serde_json::{Value, json};
use uuid::Uuid;
fn send(d: &mut DemoStore, op: &str, mut v: Value) -> anyhow::Result<()> {
    if op.starts_with("wage_") {
        v["catalog_version"] = json!(w::VERSION);
    }
    d.execute(Action::Payroll(A::new(op, v)))
}
struct F {
    d: DemoStore,
    eid: Uuid,
    admin: Uuid,
    hr: Uuid,
    id: Uuid,
}
fn fixture() -> F {
    let mut d = DemoStore::new();
    let admin = d.data.access.user_id;
    let hr = Uuid::new_v4();
    let eid = d.data.employees[1].id;
    let id = Uuid::new_v4();
    d.data.access.user_id = hr;
    d.data.access.role = Role::HrGeneralist;
    send(&mut d,"profile_save",json!({"id":Uuid::new_v4(),"employee_id":eid,"effective_on":"2026-02-01","pay":Pay{rate_cents:1_000_000,..Default::default()},"reason":"Synthetic wage salary"})).unwrap();
    send(&mut d,"wage_create",json!({"id":id,"employee_id":eid,"starts_on":"2026-01-01","ends_on":"2026-12-31","coverage":"covered","series_key":"vi-nonagri-large","workplace":"Fictional Bacolod workplace","evidence":"PRIVATE_SYNTHETIC_WAGE_EVIDENCE","note":"Actual industry and establishment headcount reviewed","reason":"Synthetic classification"})).unwrap();
    F {
        d,
        eid,
        admin,
        hr,
        id,
    }
}
fn approve(f: &mut F) {
    f.d.data.access.user_id = f.admin;
    f.d.data.access.role = Role::Administrator;
    send(
        &mut f.d,
        "wage_approve",
        json!({"id":f.id,"expected_revision":0,"reason":"Independent synthetic review"}),
    )
    .unwrap();
}
#[test]
fn classification_excludes_creator_and_most_recent_preparer() {
    let mut f = fixture();
    f.d.data.access.role = Role::Administrator;
    assert!(
        send(
            &mut f.d,
            "wage_approve",
            json!({"id":f.id,"expected_revision":0,"reason":"Forbidden self approval"})
        )
        .is_err()
    );
    approve(&mut f);
    assert_eq!(f.d.data.payroll.wages.classifications[0].status, "approved");
    assert_eq!(
        w::current(
            &f.d.data,
            f.eid,
            "2026-09-01".parse().unwrap(),
            "2026-09-30".parse().unwrap()
        )
        .unwrap()
        .blocking_days,
        30
    );
}
#[test]
fn changed_workplace_requires_review_but_archiving_alone_does_not() {
    let mut f = fixture();
    approve(&mut f);
    let lo = "2026-09-01".parse().unwrap();
    let hi = "2026-09-02".parse().unwrap();
    let before = w::current(&f.d.data, f.eid, lo, hi).unwrap();
    f.d.data
        .employees
        .iter_mut()
        .find(|e| e.id == f.eid)
        .unwrap()
        .status = EmployeeStatus::Archived;
    assert_eq!(before, w::current(&f.d.data, f.eid, lo, hi).unwrap());
    f.d.data
        .employees
        .iter_mut()
        .find(|e| e.id == f.eid)
        .unwrap()
        .work_location_id = Some(Uuid::new_v4());
    assert!(
        w::current(&f.d.data, f.eid, lo, hi)
            .unwrap()
            .days
            .iter()
            .all(|d| d.status == "workplace_or_employment_changed")
    );
}
#[test]
fn retire_retains_approval_facts_and_allows_a_reviewed_replacement() {
    let mut f = fixture();
    approve(&mut f);
    let original = f.d.data.payroll.wages.classifications[0].clone();
    send(
        &mut f.d,
        "wage_retire",
        json!({"id":f.id,"expected_revision":1,"reason":"Replace historical classification"}),
    )
    .unwrap();
    let retired = &f.d.data.payroll.wages.classifications[0];
    assert_eq!(retired.status, "retired");
    assert_eq!(retired.approved_at, original.approved_at);
    assert_eq!(retired.evidence, original.evidence);
    assert_eq!(
        w::current(
            &f.d.data,
            f.eid,
            "2026-09-01".parse().unwrap(),
            "2026-09-01".parse().unwrap()
        )
        .unwrap()
        .blocking_days,
        1
    );
}
#[test]
fn manager_and_disabled_context_cannot_manage_wage_reviews() {
    let mut f = fixture();
    let company_id = f.d.data.company.id;
    f.d.data.access.role = Role::Manager;
    assert!(send(&mut f.d,"wage_settings",json!({"id":company_id,"expected_revision":0,"enforced":true,"reason":"Forbidden manager setting"})).is_err());
    let mut f = fixture();
    f.d.data.mode = hris_desktop::domain::Mode::SignedOut;
    assert!(A::new("wage_approve",json!({"id":f.id,"expected_revision":0,"reason":"Signed out","catalog_version":w::VERSION})).payload(&f.d.data).is_err());
}
fn monthly(f: &mut F) -> Uuid {
    f.d.data.access.user_id = f.hr;
    f.d.data.access.role = Role::HrGeneralist;
    let run = Uuid::new_v4();
    let item = Uuid::new_v4();
    send(&mut f.d,"run_create",json!({"id":run,"month":"2026-09-01","title":"Synthetic wage payroll","reason":"Synthetic wage run"})).unwrap();
    send(
        &mut f.d,
        "item_add",
        json!({"id":item,"run_id":run,"employee_id":f.eid,"reason":"Synthetic recipient"}),
    )
    .unwrap();
    let w = Worksheet {
        reviewed: true,
        review_note: "Synthetic wage source reviewed".into(),
        ..Default::default()
    };
    send(
        &mut f.d,
        "item_save",
        json!({"id":item,"expected_revision":0,"worksheet":w,"reason":"Synthetic worksheet"}),
    )
    .unwrap();
    run
}
#[test]
fn optional_enforcement_is_atomic_and_snapshots_advisory_flags() {
    let mut f = fixture();
    approve(&mut f);
    let run = monthly(&mut f);
    let rev =
        f.d.data
            .payroll
            .runs
            .iter()
            .find(|r| r.id == run)
            .unwrap()
            .revision;
    send(&mut f.d,"run_submit",json!({"id":run,"expected_revision":rev,"scope_reviewed":true,"reason":"Advisory submission"})).unwrap();
    let check = &f.d.data.payroll.wages.checks[0];
    assert!(!check.enforced);
    assert_eq!(
        check.assessment["employees"][0]["result"]["blocking_days"],
        30
    );
    f.d.data.access.user_id = f.admin;
    f.d.data.access.role = Role::Administrator;
    let cid = f.d.data.company.id;
    send(&mut f.d,"wage_settings",json!({"id":cid,"expected_revision":0,"enforced":true,"reason":"Enable after explicit review"})).unwrap();
    let before = serde_json::to_value(&f.d.data.payroll.slips).unwrap();
    let rev =
        f.d.data
            .payroll
            .runs
            .iter()
            .find(|r| r.id == run)
            .unwrap()
            .revision;
    assert!(
        send(
            &mut f.d,
            "run_approve",
            json!({"id":run,"expected_revision":rev,"reason":"Underfloor cannot issue"})
        )
        .is_err()
    );
    assert_eq!(
        serde_json::to_value(&f.d.data.payroll.slips).unwrap(),
        before
    );
    assert_eq!(f.d.data.payroll.wages.checks.len(), 1);
}
#[test]
fn role_changes_and_tax_metadata_do_not_create_minimum_wage_exemptions() {
    let mut f = fixture();
    approve(&mut f);
    let old = w::current(
        &f.d.data,
        f.eid,
        "2026-09-01".parse().unwrap(),
        "2026-09-01".parse().unwrap(),
    )
    .unwrap();
    f.d.data.payroll.profiles[0].pay.minimum_wage_earner = true;
    f.d.data.payroll.profiles[0].pay.coverage_note = "PRIVATE_TAX_ONLY_REVIEW".into();
    f.d.data.access.role = Role::Manager;
    let result = w::current(
        &f.d.data,
        f.eid,
        "2026-09-01".parse().unwrap(),
        "2026-09-01".parse().unwrap(),
    )
    .unwrap();
    assert_eq!(old, result);
    assert_eq!(result.days[0].status, "below_recorded_floor");
}
