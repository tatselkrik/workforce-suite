use chrono::{Days, NaiveDate};
use hris_desktop::{
    access::Role,
    benefits::*,
    domain::{DemoStore, EmployeeStatus},
};
use uuid::Uuid;
fn setup() -> (DemoStore, Demo, Context) {
    let mut d = DemoStore::new();
    if d.data.access.user_id.is_nil() {
        d.data.access.user_id = Uuid::from_u128(10);
    }
    let mut repo = Demo::default();
    let c = repo.execute(Operation::Refresh, &d.data).unwrap().context;
    (d, repo, c)
}
fn day(n: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, n).unwrap()
}
fn plan() -> Plan {
    Plan {
        code: "HEALTH-01".into(),
        name: "Fictional health coverage".into(),
        category: Category::HealthInsurance,
        description: "Company-reviewed fictional benefit; no insurer connection".into(),
        eligibility_guidance: "HR records reviewed company eligibility separately".into(),
        dependent_limit: 2,
        available: true,
    }
}
fn save(repo: &mut Demo, d: &DemoStore, r: &Request) -> Response {
    repo.execute(Operation::Save(Box::new(r.clone())), &d.data)
        .unwrap()
}
fn add_plan(repo: &mut Demo, d: &DemoStore, c: &Context) -> Context {
    let r = Request::new(
        &d.data.access,
        c,
        Uuid::new_v4(),
        Change::Plan(plan()),
        "Initial fictional plan review".into(),
    )
    .unwrap();
    save(repo, d, &r).context
}
fn enrollment(c: &Context) -> Enrollment {
    let p = &c.plans[0];
    Enrollment {
        plan_id: p.id,
        plan_revision: p.revision,
        plan_snapshot: p.data.clone(),
        employee_source: c.employees.iter().find(|e| e.active).unwrap().clone(),
        eligibility: Eligibility::Eligible,
        eligibility_basis: "Reviewed company conditions against current employment".into(),
        status: Status::Active,
        coverage_from: day(1),
        coverage_to: None,
        dependents: vec![],
    }
}
fn request(d: &DemoStore, c: &Context, id: Uuid, e: Enrollment) -> Request {
    Request::new(
        &d.data.access,
        c,
        id,
        Change::Enrollment(e),
        "Fictional reviewed enrollment change".into(),
    )
    .unwrap()
}
fn with_enrollment() -> (DemoStore, Demo, Context, Uuid) {
    let (d, mut repo, c) = setup();
    let c = add_plan(&mut repo, &d, &c);
    let id = Uuid::new_v4();
    let r = request(&d, &c, id, enrollment(&c));
    let c = save(&mut repo, &d, &r).context;
    (d, repo, c, id)
}
#[test]
fn reviewed_enrollment_and_plan_history_preserve_captured_sources() {
    let (d, mut repo, c, id) = with_enrollment();
    let original = c.enrollment(id).unwrap().clone();
    let mut p = c.plans[0].data.clone();
    p.name = "Fictional revised plan".into();
    let r = Request::new(
        &d.data.access,
        &c,
        c.plans[0].id,
        Change::Plan(p),
        "Explicit later plan change".into(),
    )
    .unwrap();
    let c = save(&mut repo, &d, &r).context;
    assert_eq!(c.enrollment(id).unwrap(), &original);
    assert_eq!(c.plans[0].revision, 2);
    assert_eq!(c.events.len(), 3);
    c.validate(&d.data.access).unwrap();
}
#[test]
fn activation_requires_recorded_eligibility() {
    let (d, mut repo, c) = setup();
    let c = add_plan(&mut repo, &d, &c);
    for eligibility in [Eligibility::Pending, Eligibility::Ineligible] {
        let mut e = enrollment(&c);
        e.eligibility = eligibility;
        assert!(
            Request::new(
                &d.data.access,
                &c,
                Uuid::new_v4(),
                Change::Enrollment(e.clone()),
                "Explicit eligibility reason".into()
            )
            .is_err()
        );
        e.status = Status::Pending;
        assert!(
            Request::new(
                &d.data.access,
                &c,
                Uuid::new_v4(),
                Change::Enrollment(e),
                "Explicit eligibility reason".into()
            )
            .is_ok()
        );
    }
}
#[test]
fn unavailable_or_changed_plan_requires_fresh_review() {
    let (d, mut repo, c) = setup();
    let c = add_plan(&mut repo, &d, &c);
    let first = request(&d, &c, Uuid::new_v4(), enrollment(&c));
    let mut p = c.plans[0].data.clone();
    p.available = false;
    let r = Request::new(
        &d.data.access,
        &c,
        c.plans[0].id,
        Change::Plan(p),
        "Stop new enrollment for review".into(),
    )
    .unwrap();
    save(&mut repo, &d, &r);
    assert!(
        repo.execute(Operation::Save(Box::new(first)), &d.data)
            .is_err()
    );
}
#[test]
fn changed_employment_blocks_a_prepared_new_review() {
    let (mut d, mut repo, c) = setup();
    let c = add_plan(&mut repo, &d, &c);
    let e = enrollment(&c);
    let id = e.employee_source.id;
    let r = request(&d, &c, Uuid::new_v4(), e);
    d.data
        .employees
        .iter_mut()
        .find(|e| e.id == id)
        .unwrap()
        .department = "Changed department".into();
    assert!(repo.execute(Operation::Save(Box::new(r)), &d.data).is_err());
}
#[test]
fn archive_does_not_erase_coverage_and_hr_can_end_original_source() {
    let (mut d, mut repo, c, id) = with_enrollment();
    let mut e = c.enrollment(id).unwrap().data.clone();
    d.data
        .employees
        .iter_mut()
        .find(|p| p.id == e.employee_source.id)
        .unwrap()
        .status = EmployeeStatus::Archived;
    let fresh = repo.execute(Operation::Refresh, &d.data).unwrap().context;
    assert_eq!(fresh.enrollments[0].data, e);
    e.status = Status::Ended;
    e.coverage_to = Some(day(28));
    let r = request(&d, &fresh, id, e);
    let fresh = save(&mut repo, &d, &r).context;
    assert_eq!(fresh.enrollments[0].data.status, Status::Ended);
    assert_eq!(fresh.events.len(), 3);
    let mut ended = fresh.enrollments[0].data.clone();
    ended.status = Status::Active;
    assert!(
        Request::new(
            &d.data.access,
            &fresh,
            id,
            Change::Enrollment(ended),
            "Must retain ended source".into()
        )
        .is_err()
    );
}
#[test]
fn duplicate_or_overlapping_coverage_is_rejected() {
    let (d, _, c, _) = with_enrollment();
    let e = enrollment(&c);
    assert!(
        Request::new(
            &d.data.access,
            &c,
            Uuid::new_v4(),
            Change::Enrollment(e),
            "Prevent overlap".into()
        )
        .is_err()
    );
}
#[test]
fn consecutive_coverage_after_an_ended_interval_is_allowed() {
    let (d, mut repo, c, id) = with_enrollment();
    let mut e = c.enrollment(id).unwrap().data.clone();
    e.status = Status::Ended;
    e.coverage_to = Some(day(20));
    let r = request(&d, &c, id, e);
    let c = save(&mut repo, &d, &r).context;
    let mut e = enrollment(&c);
    e.coverage_from = day(21);
    assert!(
        Request::new(
            &d.data.access,
            &c,
            Uuid::new_v4(),
            Change::Enrollment(e.clone()),
            "Next explicit coverage interval".into()
        )
        .is_ok()
    );
    e.coverage_from = day(20);
    assert!(
        Request::new(
            &d.data.access,
            &c,
            Uuid::new_v4(),
            Change::Enrollment(e),
            "Inclusive boundary overlap".into()
        )
        .is_err()
    );
}
#[test]
fn namesakes_are_allowed_but_dependent_ids_and_limits_are_checked() {
    let (d, mut repo, c) = setup();
    let c = add_plan(&mut repo, &d, &c);
    let mut e = enrollment(&c);
    let dep = Dependent {
        id: Uuid::new_v4(),
        full_name: "Fictional namesake".into(),
        relationship: Relationship::Child,
        birth_date: None,
        coverage_from: day(1),
        coverage_to: None,
    };
    e.dependents = vec![
        dep.clone(),
        Dependent {
            id: Uuid::new_v4(),
            ..dep.clone()
        },
    ];
    assert!(
        Request::new(
            &d.data.access,
            &c,
            Uuid::new_v4(),
            Change::Enrollment(e.clone()),
            "Reviewed two people with same name".into()
        )
        .is_ok()
    );
    e.dependents[1].id = e.dependents[0].id;
    assert!(
        Request::new(
            &d.data.access,
            &c,
            Uuid::new_v4(),
            Change::Enrollment(e.clone()),
            "Duplicate identity denied".into()
        )
        .is_err()
    );
    e.dependents[1].id = Uuid::new_v4();
    e.dependents.push(Dependent {
        id: Uuid::new_v4(),
        ..dep
    });
    assert!(
        Request::new(
            &d.data.access,
            &c,
            Uuid::new_v4(),
            Change::Enrollment(e),
            "Cannot exceed plan limit".into()
        )
        .is_err()
    );
}
#[test]
fn dependent_dates_stay_inside_parent_and_birth_is_not_future() {
    let (d, mut repo, c) = setup();
    let c = add_plan(&mut repo, &d, &c);
    let mut e = enrollment(&c);
    e.dependents.push(Dependent {
        id: Uuid::new_v4(),
        full_name: "Fictional dependent".into(),
        relationship: Relationship::Spouse,
        birth_date: None,
        coverage_from: day(1),
        coverage_to: None,
    });
    e.coverage_to = Some(day(20));
    assert!(
        Request::new(
            &d.data.access,
            &c,
            Uuid::new_v4(),
            Change::Enrollment(e.clone()),
            "Child end required within parent".into()
        )
        .is_err()
    );
    e.dependents[0].coverage_to = Some(day(20));
    assert!(
        Request::new(
            &d.data.access,
            &c,
            Uuid::new_v4(),
            Change::Enrollment(e.clone()),
            "Reviewed bounded coverage".into()
        )
        .is_ok()
    );
    e.dependents[0].birth_date = Some(c.today.checked_add_days(Days::new(1)).unwrap());
    assert!(
        Request::new(
            &d.data.access,
            &c,
            Uuid::new_v4(),
            Change::Enrollment(e),
            "Future birth denied".into()
        )
        .is_err()
    );
}
#[test]
fn exact_retry_survives_later_revision_but_changed_payload_is_denied() {
    let (d, mut repo, c) = setup();
    let r = Request::new(
        &d.data.access,
        &c,
        Uuid::new_v4(),
        Change::Plan(plan()),
        "Initial reviewed plan".into(),
    )
    .unwrap();
    let first = save(&mut repo, &d, &r);
    let mut p = plan();
    p.description = "Later explicit plan description".into();
    let second = Request::new(
        &d.data.access,
        &first.context,
        r.subject_id,
        Change::Plan(p),
        "Another plan review".into(),
    )
    .unwrap();
    save(&mut repo, &d, &second);
    let retry = save(&mut repo, &d, &r);
    assert_eq!(retry.receipt, first.receipt);
    assert_eq!(retry.context.plans[0].revision, 2);
    assert_eq!(retry.context.events.len(), 2);
    let mut changed = r;
    changed.body = "Different full payload".into();
    assert!(
        repo.execute(Operation::Save(Box::new(changed)), &d.data)
            .is_err()
    );
}
#[test]
fn current_access_denies_old_receipts_and_forged_actor() {
    let (mut d, mut repo, c) = setup();
    let r = Request::new(
        &d.data.access,
        &c,
        Uuid::new_v4(),
        Change::Plan(plan()),
        "Initial reviewed plan".into(),
    )
    .unwrap();
    save(&mut repo, &d, &r);
    let mut forged = r.clone();
    forged.actor_id = Uuid::new_v4();
    assert!(
        repo.execute(Operation::Save(Box::new(forged)), &d.data)
            .is_err()
    );
    d.data.access.role = Role::Manager;
    assert!(repo.execute(Operation::Save(Box::new(r)), &d.data).is_err());
}
#[test]
fn history_or_snapshot_tampering_is_rejected() {
    let (d, _, c, _) = with_enrollment();
    let mut missing = c.clone();
    missing.events.remove(0);
    assert!(missing.validate(&d.data.access).is_err());
    let mut changed = c.clone();
    changed.enrollments[0].data.plan_snapshot.name = "Forged captured plan".into();
    assert!(changed.validate(&d.data.access).is_err());
    let mut changed = c;
    changed.events[0].company_id = Uuid::new_v4();
    assert!(changed.validate(&d.data.access).is_err());
}
#[test]
fn stale_revisions_do_not_mutate_the_repository() {
    let (d, mut repo, c) = setup();
    let c = add_plan(&mut repo, &d, &c);
    let mut p = plan();
    p.description = "Revision one candidate".into();
    let first = Request::new(
        &d.data.access,
        &c,
        c.plans[0].id,
        Change::Plan(p.clone()),
        "First concurrent review".into(),
    )
    .unwrap();
    p.description = "Revision two candidate".into();
    let second = Request::new(
        &d.data.access,
        &c,
        c.plans[0].id,
        Change::Plan(p),
        "Second concurrent review".into(),
    )
    .unwrap();
    let saved = save(&mut repo, &d, &first);
    assert!(
        repo.execute(Operation::Save(Box::new(second)), &d.data)
            .is_err()
    );
    assert_eq!(
        repo.execute(Operation::Refresh, &d.data).unwrap().context,
        saved.context
    );
}
#[test]
fn full_requests_reject_unknown_fields_and_private_employee_json_is_not_snapshot() {
    let (d, mut repo, c) = setup();
    let c = add_plan(&mut repo, &d, &c);
    let r = request(&d, &c, Uuid::new_v4(), enrollment(&c));
    let mut json = serde_json::to_value(r).unwrap();
    json["salary_cents"] = serde_json::json!(10000);
    assert!(serde_json::from_value::<Request>(json).is_err());
    let snapshot = serde_json::to_string(&d.data.employees).unwrap();
    assert!(!snapshot.contains("eligibility_basis") && !snapshot.contains("dependents"));
}
#[test]
fn duplicate_codes_and_invalid_terms_are_denied() {
    let (d, mut repo, c) = setup();
    let c = add_plan(&mut repo, &d, &c);
    assert!(
        Request::new(
            &d.data.access,
            &c,
            Uuid::new_v4(),
            Change::Plan(plan()),
            "Do not duplicate codes".into()
        )
        .is_err()
    );
    let mut p = plan();
    p.code = "lower case".into();
    assert!(p.validate().is_err());
    p.code = "VALID-01".into();
    p.dependent_limit = 26;
    assert!(p.validate().is_err());
}

#[test]
fn ending_existing_dependents_cannot_drop_or_rewrite_them() {
    let (d, mut repo, c) = setup();
    let c = add_plan(&mut repo, &d, &c);
    let mut e = enrollment(&c);
    e.dependents.push(Dependent {
        id: Uuid::new_v4(),
        full_name: "Fictional retained dependent".into(),
        relationship: Relationship::Child,
        birth_date: None,
        coverage_from: day(1),
        coverage_to: None,
    });
    let id = Uuid::new_v4();
    let r = request(&d, &c, id, e);
    let c = save(&mut repo, &d, &r).context;
    let mut ended = c.enrollment(id).unwrap().data.clone();
    ended.status = Status::Ended;
    ended.coverage_to = Some(day(28));
    ended.dependents[0].coverage_to = Some(day(28));
    assert!(
        Request::new(
            &d.data.access,
            &c,
            id,
            Change::Enrollment(ended.clone()),
            "End existing reviewed coverage".into()
        )
        .is_ok()
    );
    ended.dependents.clear();
    assert!(
        Request::new(
            &d.data.access,
            &c,
            id,
            Change::Enrollment(ended),
            "Cannot drop prior dependent at ending".into()
        )
        .is_err()
    );
}
#[test]
fn record_identities_cannot_change_entity_kind() {
    let (d, mut repo, c) = setup();
    let c = add_plan(&mut repo, &d, &c);
    assert!(
        Request::new(
            &d.data.access,
            &c,
            c.plans[0].id,
            Change::Enrollment(enrollment(&c)),
            "Cannot reuse a plan identity".into()
        )
        .is_err()
    );
}

#[test]
#[ignore = "fresh fictional PostgreSQL projection vectors from local database tests"]
fn postgres_benefits_contexts_decode_and_validate() {
    let path = std::env::var_os("HRIS_BENEFITS_VECTORS")
        .expect("Use the freshly generated fictional vector folder");
    let entries = std::fs::read_dir(path)
        .unwrap()
        .map(|v| v.unwrap().path())
        .filter(|p| p.extension().is_some_and(|v| v == "json"))
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 4);
    for path in entries {
        let c: Context = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let mut a = hris_desktop::access::Access::default();
        a.role = Role::Administrator;
        a.company_id = c.company_id;
        a.user_id = c.user_id;
        c.validate(&a).unwrap();
        a.role = Role::Manager;
        assert!(c.validate(&a).is_err());
    }
}
