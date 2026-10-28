use chrono::NaiveDate;
use hris_desktop::{
    access::Role, backend::Command, desktop::Product, domain::DemoStore, relations::*,
};
use uuid::Uuid;
fn initial() -> (DemoStore, Demo, Context) {
    let d = DemoStore::new();
    let mut repo = Demo::default();
    let c = repo.execute(Operation::Refresh, &d.data).unwrap().context;
    (d, repo, c)
}
fn create(d: &DemoStore, c: &Context) -> Request {
    Request::new(
        &d.data.access,
        c,
        Uuid::new_v4(),
        Change::Create {
            employee_id: d.data.employees[1].id,
            owner_id: d.data.access.user_id,
            title: "Fictional workplace concern".into(),
            category: Category::WorkplaceConcern,
            priority: Priority::High,
            body: "Fictional initial account, retained without changing employment".into(),
        },
    )
    .unwrap()
}
fn save(repo: &mut Demo, d: &DemoStore, r: &Request) -> Response {
    repo.execute(Operation::Save(Box::new(r.clone())), &d.data)
        .unwrap()
}
#[test]
fn case_follow_up_and_resolution_preserve_history() {
    let (d, mut repo, c) = initial();
    let first = create(&d, &c);
    let mut c = save(&mut repo, &d, &first).context;
    let case = first.case_id;
    let task = Uuid::new_v4();
    let r = Request::new(
        &d.data.access,
        &c,
        case,
        Change::FollowUp {
            follow_up_id: task,
            due_on: NaiveDate::from_ymd_opt(2026, 11, 2).unwrap(),
            body: "Review the fictional duty notes".into(),
        },
    )
    .unwrap();
    c = save(&mut repo, &d, &r).context;
    assert!(
        Request::new(
            &d.data.access,
            &c,
            case,
            Change::Status {
                status: Status::Resolved,
                body: "Cannot skip this task".into()
            }
        )
        .is_err()
    );
    let r = Request::new(
        &d.data.access,
        &c,
        case,
        Change::Complete {
            follow_up_id: task,
            body: "Fictional follow-up completed".into(),
        },
    )
    .unwrap();
    c = save(&mut repo, &d, &r).context;
    for status in [Status::Resolved, Status::Closed, Status::InReview] {
        let r = Request::new(
            &d.data.access,
            &c,
            case,
            Change::Status {
                status,
                body: "Reviewed fictional status change".into(),
            },
        )
        .unwrap();
        c = save(&mut repo, &d, &r).context;
    }
    assert_eq!(c.cases[0].revision, 6);
    assert_eq!(c.events.len(), 6);
    assert_eq!(
        c.follow_ups[0].completion_note.as_deref(),
        Some("Fictional follow-up completed")
    );
    c.validate(&d.data.access).unwrap();
    assert_eq!(
        d.data.attendance.len(),
        DemoStore::new().data.attendance.len()
    );
    assert_eq!(d.data.employees.len(), 8);
}
#[test]
fn retries_keep_one_identity_and_immutable_receipt() {
    let (d, mut repo, c) = initial();
    let first = create(&d, &c);
    let response = save(&mut repo, &d, &first);
    let retry = save(&mut repo, &d, &first);
    assert_eq!(retry.context, response.context);
    assert_eq!(retry.receipt, response.receipt);
    let mut changed = first.clone();
    changed.change = Change::Note {
        body: "A different payload must fail".into(),
    };
    assert!(
        repo.execute(Operation::Save(Box::new(changed)), &d.data)
            .is_err()
    );
}
#[test]
fn stale_revision_and_unknown_employee_are_rejected() {
    let (d, mut repo, c) = initial();
    let first = create(&d, &c);
    let c = save(&mut repo, &d, &first).context;
    let r = Request::new(
        &d.data.access,
        &c,
        first.case_id,
        Change::Note {
            body: "First reviewed note".into(),
        },
    )
    .unwrap();
    let c2 = save(&mut repo, &d, &r).context;
    let mut stale = r.clone();
    stale.id = Uuid::new_v4();
    assert!(
        repo.execute(Operation::Save(Box::new(stale)), &d.data)
            .is_err()
    );
    let mut unknown = create(&d, &c2);
    if let Change::Create { employee_id, .. } = &mut unknown.change {
        *employee_id = Uuid::new_v4();
    }
    assert!(
        repo.execute(Operation::Save(Box::new(unknown)), &d.data)
            .is_err()
    );
}
#[test]
fn confidential_context_rejects_foreign_actor_scope_and_truncated_history() {
    let (d, mut repo, c) = initial();
    let r = create(&d, &c);
    let c = save(&mut repo, &d, &r).context;
    let mut bad = c.clone();
    bad.user_id = Uuid::new_v4();
    assert!(bad.validate(&d.data.access).is_err());
    let mut bad = c.clone();
    bad.cases[0].company_id = Uuid::new_v4();
    assert!(bad.validate(&d.data.access).is_err());
    let mut bad = c.clone();
    bad.events.clear();
    assert!(bad.validate(&d.data.access).is_err());
    let mut bad = c.clone();
    bad.events.push(bad.events[0].clone());
    assert!(bad.validate(&d.data.access).is_err());
    let mut bad = c.clone();
    bad.events[0].current_state.title = "Conflicting case title".into();
    assert!(bad.validate(&d.data.access).is_err());
    let mut a = d.data.access.clone();
    a.role = Role::HrGeneralist;
    let mut assigned = c.clone();
    assigned.can_assign = false;
    assigned.validate(&a).unwrap();
    assigned.cases[0].owner_id = Uuid::new_v4();
    assert!(assigned.validate(&a).is_err());
}
#[test]
fn request_receipt_is_bound_to_actor_company_case_revision() {
    let (d, mut repo, c) = initial();
    let r = create(&d, &c);
    let receipt = save(&mut repo, &d, &r).receipt.unwrap();
    receipt.validate(&r).unwrap();
    for field in 0..5 {
        let mut bad = receipt.clone();
        match field {
            0 => bad.id = Uuid::new_v4(),
            1 => bad.company_id = Uuid::new_v4(),
            2 => bad.actor_id = Uuid::new_v4(),
            3 => bad.case_id = Uuid::new_v4(),
            _ => bad.revision += 1,
        };
        assert!(bad.validate(&r).is_err());
    }
}
#[test]
fn fields_are_typed_and_confidential_payloads_are_bounded() {
    let (d, _, c) = initial();
    let r = create(&d, &c);
    let mut value = serde_json::to_value(&r).unwrap();
    value["unexpected"] = true.into();
    assert!(serde_json::from_value::<Request>(value).is_err());
    let mut value = serde_json::to_value(&r).unwrap();
    value["change"]["body"] = 1234.into();
    assert!(serde_json::from_value::<Request>(value).is_err());
    let mut value = serde_json::to_value(&r).unwrap();
    value["change"]["unknown"] = true.into();
    assert!(serde_json::from_value::<Request>(value).is_err());
    let mut bad = r.clone();
    bad.change = Change::Note {
        body: "z".repeat(12001),
    };
    assert!(bad.binding(&d.data.access).is_err());
    bad.change = Change::Note {
        body: "龍".repeat(10000),
    };
    assert!(bad.binding(&d.data.access).is_err());
    bad.change = Change::Note {
        body: "padded note ".into(),
    };
    assert!(bad.binding(&d.data.access).is_err());
}
#[test]
fn payroll_and_non_hr_access_cannot_load_cases() {
    let (d, _, _) = initial();
    for role in [Role::Manager, Role::Employee] {
        let mut a = d.data.access.clone();
        a.role = role;
        assert!(!eligible(&a));
    }
    assert!(Product::Hris.command_allowed(&Command::Relations(Operation::Refresh)));
    assert!(!Product::Payroll.command_allowed(&Command::Relations(Operation::Refresh)));
}
#[test]
fn transition_contract_requires_reopening_and_admin_for_closed() {
    assert!(!Status::Open.permits(Status::Closed, true));
    assert!(!Status::Closed.permits(Status::InReview, false));
    assert!(Status::Closed.permits(Status::InReview, true));
    assert!(!Status::Resolved.permits(Status::Open, true));
}
#[test]
fn protected_hr_generalist_demo_uses_capability_instead_of_employee_base_role() {
    use hris_desktop::staff_roles::{Context as StaffContext, StaffRole, StaffRoles};
    let mut d = DemoStore::new();
    let roles = StaffRoles::new(vec![StaffRole::HrGeneralist]).unwrap();
    d.data.access.user_id = Uuid::from_u128(640);
    d.data.accounts[0].user_id = d.data.access.user_id;
    d.data.access.role = roles.compatible_base_role();
    d.data.access.staff_contract_version = 1;
    d.data
        .access
        .bind_staff_context(StaffContext {
            schema_version: 1,
            company_id: d.data.company.id,
            user_id: d.data.access.user_id,
            employee_id: d.data.access.employee_id.unwrap(),
            base_role: d.data.access.role,
            revision: 1,
            runtime_ready: true,
            roles,
        })
        .unwrap();
    d.data.accounts[0].role = d.data.access.role;
    assert_eq!(d.data.access.role, Role::Employee);
    assert!(eligible(&d.data.access));
    let mut repo = Demo::default();
    let c = repo.execute(Operation::Refresh, &d.data).unwrap().context;
    let r = create(&d, &c);
    let response = save(&mut repo, &d, &r);
    assert_eq!(response.context.cases.len(), 1);
    assert!(!response.context.can_assign);
}
