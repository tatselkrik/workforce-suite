use hris_desktop::{
    access::{Access, Role},
    domain::{DemoStore, Snapshot},
    services::*,
};
use uuid::Uuid;
fn data(role: Role, user: u128, employee: usize) -> Snapshot {
    let mut d = DemoStore::new().data;
    d.access = Access {
        company_id: d.company.id,
        user_id: Uuid::from_u128(user),
        employee_id: Some(d.employees[employee].id),
        role,
        ..Default::default()
    };
    d.accounts[0].user_id = Uuid::from_u128(901);
    d
}
fn submit(a: &Access, c: &Context, employee: Uuid) -> Request {
    Request::new(
        a,
        c,
        Uuid::new_v4(),
        Change::Submit {
            employee_id: employee,
            title: "Fictional employment document request".into(),
            category: Category::EmploymentDocument,
            priority: Priority::Normal,
            body: "Fictional request; no official document is generated automatically.".into(),
        },
    )
    .unwrap()
}
#[test]
fn portal_own_scope_does_not_inherit_admin_queue_or_routing() {
    let d = data(Role::Administrator, 901, 0);
    let mut repo = Demo::default();
    let c = repo.context(&d, Audience::Staff).unwrap();
    let r = submit(&d.access, &c, d.employees[1].id);
    repo.save(&d, Audience::Staff, &r).unwrap();
    assert_eq!(repo.context(&d, Audience::Staff).unwrap().requests.len(), 1);
    let own = repo.context(&d, Audience::Own).unwrap();
    assert!(!own.can_route && own.requests.is_empty() && own.handlers.is_empty());
    assert!(
        Request::new(
            &d.access,
            &own,
            Uuid::new_v4(),
            Change::Submit {
                employee_id: d.employees[1].id,
                title: "An unauthorized on-behalf request".into(),
                category: Category::GeneralSupport,
                priority: Priority::Normal,
                body: "Own portal scope must not inherit Administrator rights".into()
            }
        )
        .is_err()
    );
}
#[test]
fn ordinary_employee_is_own_portal_only_and_can_cancel_own_active_work() {
    let d = data(Role::Employee, 902, 1);
    let mut repo = Demo::default();
    assert!(repo.context(&d, Audience::Staff).is_err());
    let c = repo.context(&d, Audience::Own).unwrap();
    let r = submit(&d.access, &c, d.access.employee_id.unwrap());
    let first = repo.save(&d, Audience::Own, &r).unwrap();
    let cancel = Request::new(
        &d.access,
        &first.context,
        r.request_id,
        Change::Cancel {
            body: "Fictional requester no longer needs this document".into(),
        },
    )
    .unwrap();
    let response = repo.save(&d, Audience::Own, &cancel).unwrap();
    assert_eq!(response.context.requests[0].status, Status::Cancelled);
    assert_eq!(response.context.events.len(), 2);
    assert!(
        Request::new(
            &d.access,
            &response.context,
            r.request_id,
            Change::Reply {
                body: "Cannot edit terminal work".into()
            }
        )
        .is_err()
    );
}
#[test]
fn assigned_manager_has_specific_service_access_without_team_permission() {
    let hr = data(Role::Administrator, 901, 0);
    let manager = data(Role::Manager, 12, 3);
    let mut repo = Demo::default();
    let c = repo.context(&hr, Audience::Staff).unwrap();
    let submitted = submit(&hr.access, &c, hr.employees[1].id);
    let first = repo.save(&hr, Audience::Staff, &submitted).unwrap();
    assert!(
        repo.context(&manager, Audience::Staff)
            .unwrap()
            .requests
            .is_empty()
    );
    let route = Request::new(
        &hr.access,
        &first.context,
        submitted.request_id,
        Change::Route {
            assignee_user_id: Some(manager.access.user_id),
            due_on: None,
            body: "Fictional routing to the responsible manager".into(),
        },
    )
    .unwrap();
    repo.save(&hr, Audience::Staff, &route).unwrap();
    let scoped = repo.context(&manager, Audience::Staff).unwrap();
    assert_eq!(scoped.requests.len(), 1);
    assert!(scoped.handlers.is_empty());
    let progress = Request::new(
        &manager.access,
        &scoped,
        submitted.request_id,
        Change::Progress {
            status: Status::InProgress,
            body: "Fictional handling started".into(),
        },
    )
    .unwrap();
    repo.save(&manager, Audience::Staff, &progress).unwrap();
    assert!(
        Request::new(
            &manager.access,
            &repo.context(&manager, Audience::Staff).unwrap(),
            submitted.request_id,
            Change::Cancel {
                body: "Assignee cannot cancel somebody else's request".into()
            }
        )
        .is_err()
    );
    assert!(
        repo.context(&manager, Audience::Own)
            .unwrap()
            .requests
            .is_empty()
    );
}
#[test]
fn reassigning_removes_old_handler_and_old_receipt_access() {
    let hr = data(Role::Administrator, 901, 0);
    let manager = data(Role::Manager, 12, 3);
    let mut repo = Demo::default();
    let c = repo.context(&hr, Audience::Staff).unwrap();
    let r = submit(&hr.access, &c, hr.employees[1].id);
    let response = repo.save(&hr, Audience::Staff, &r).unwrap();
    let route = Request::new(
        &hr.access,
        &response.context,
        r.request_id,
        Change::Route {
            assignee_user_id: Some(manager.access.user_id),
            due_on: None,
            body: "Fictional first routing".into(),
        },
    )
    .unwrap();
    repo.save(&hr, Audience::Staff, &route).unwrap();
    let ctx = repo.context(&manager, Audience::Staff).unwrap();
    let reply = Request::new(
        &manager.access,
        &ctx,
        r.request_id,
        Change::Reply {
            body: "Fictional handling note shared with requester".into(),
        },
    )
    .unwrap();
    repo.save(&manager, Audience::Staff, &reply).unwrap();
    let ctx = repo.context(&hr, Audience::Staff).unwrap();
    let unassign = Request::new(
        &hr.access,
        &ctx,
        r.request_id,
        Change::Route {
            assignee_user_id: None,
            due_on: None,
            body: "Fictional return to the HR queue".into(),
        },
    )
    .unwrap();
    repo.save(&hr, Audience::Staff, &unassign).unwrap();
    assert!(
        repo.context(&manager, Audience::Staff)
            .unwrap()
            .requests
            .is_empty()
    );
    assert!(repo.save(&manager, Audience::Staff, &reply).is_err());
}
#[test]
fn exact_retries_and_rejections_preserve_history_and_canonical_facts() {
    let d = data(Role::Administrator, 901, 0);
    let original = serde_json::to_value(&d.employees).unwrap();
    let mut repo = Demo::default();
    let c = repo.context(&d, Audience::Staff).unwrap();
    let r = submit(&d.access, &c, d.employees[1].id);
    let first = repo.save(&d, Audience::Staff, &r).unwrap();
    let retry = repo.save(&d, Audience::Staff, &r).unwrap();
    assert_eq!(first.context, retry.context);
    assert_eq!(first.receipt, retry.receipt);
    let mut changed = r.clone();
    changed.change = Change::Reply {
        body: "A different operation cannot reuse a save identity".into(),
    };
    assert!(repo.save(&d, Audience::Staff, &changed).is_err());
    assert_eq!(repo.context(&d, Audience::Staff).unwrap(), first.context);
    assert_eq!(serde_json::to_value(&d.employees).unwrap(), original);
    assert_eq!(d.payroll.profiles.len(), 0);
}
#[test]
fn context_forgery_and_truncation_fail_closed() {
    let d = data(Role::Administrator, 901, 0);
    let mut repo = Demo::default();
    let c = repo.context(&d, Audience::Staff).unwrap();
    let r = submit(&d.access, &c, d.employees[1].id);
    let c = repo.save(&d, Audience::Staff, &r).unwrap().context;
    let mut bad = c.clone();
    bad.user_id = Uuid::new_v4();
    assert!(bad.validate(&d.access).is_err());
    let mut bad = c.clone();
    bad.events.clear();
    assert!(bad.validate(&d.access).is_err());
    let mut bad = c.clone();
    bad.audience = Audience::Own;
    assert!(bad.validate(&d.access).is_err());
    let mut bad = c.clone();
    bad.requests[0].company_id = Uuid::new_v4();
    assert!(bad.validate(&d.access).is_err());
}
#[test]
fn terminal_reopening_requires_hr_and_cancellation_uses_its_own_operation() {
    assert!(!Status::Submitted.permits(Status::Cancelled, true));
    assert!(!Status::Completed.permits(Status::InProgress, false));
    assert!(Status::Completed.permits(Status::InProgress, true));
    assert!(Status::Cancelled.permits(Status::InProgress, true));
    assert!(!Status::InProgress.permits(Status::Submitted, true));
}
#[test]
fn payload_and_receipt_identity_are_exact() {
    let d = data(Role::Administrator, 901, 0);
    let mut repo = Demo::default();
    let c = repo.context(&d, Audience::Staff).unwrap();
    let r = submit(&d.access, &c, d.employees[1].id);
    let response = repo.save(&d, Audience::Staff, &r).unwrap();
    let mut receipt = response.receipt.unwrap();
    receipt.actor_id = Uuid::new_v4();
    assert!(receipt.validate(&r).is_err());
    let mut raw = serde_json::to_value(&r).unwrap();
    raw["change"]["salary"] = "untrusted".into();
    assert!(serde_json::from_value::<Request>(raw).is_err());
    let mut bad = r.clone();
    bad.change = Change::Reply {
        body: "龍".repeat(10000),
    };
    assert!(bad.binding(&d.access).is_err());
    bad = r;
    bad.actor_id = Uuid::new_v4();
    assert!(bad.binding(&d.access).is_err());
}

#[test]
fn portal_projection_has_only_public_thread_fields_and_no_staff_identity_metadata() {
    use std::collections::BTreeSet;
    fn keys(v: &serde_json::Value) -> BTreeSet<&str> {
        v.as_object().unwrap().keys().map(String::as_str).collect()
    }
    let hr = data(Role::Administrator, 901, 0);
    let owner = data(Role::Employee, 902, 1);
    let manager = data(Role::Manager, 12, 3);
    let mut repo = Demo::default();
    let c = repo.context(&hr, Audience::Staff).unwrap();
    let request = submit(&hr.access, &c, owner.access.employee_id.unwrap());
    let c = repo.save(&hr, Audience::Staff, &request).unwrap().context;
    let route = Request::new(
        &hr.access,
        &c,
        request.request_id,
        Change::Route {
            assignee_user_id: Some(manager.access.user_id),
            due_on: None,
            body: "Shared fictional routing reason".into(),
        },
    )
    .unwrap();
    let c = repo.save(&hr, Audience::Staff, &route).unwrap().context;
    assert!(OwnView::from_context(&c, &hr.access).is_err());
    let own = repo.context(&owner, Audience::Own).unwrap();
    let view = OwnView::from_context(&own, &owner.access).unwrap();
    assert_eq!(
        view.requests[0].handler_name.as_deref(),
        Some(manager.own_employee().unwrap().full_name.as_str())
    );
    assert!(view.requests[0].can_reply && view.requests[0].can_cancel);
    assert_eq!(
        view.requests[0]
            .messages
            .iter()
            .map(|m| m.revision)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    let raw = serde_json::to_value(&view).unwrap();
    assert_eq!(keys(&raw), BTreeSet::from(["schema_version", "requests"]));
    assert_eq!(
        keys(&raw["requests"][0]),
        BTreeSet::from([
            "id",
            "code",
            "title",
            "category",
            "priority",
            "status",
            "handler_name",
            "due_on",
            "revision",
            "created_at",
            "updated_at",
            "can_reply",
            "can_cancel",
            "messages",
        ])
    );
    for m in raw["requests"][0]["messages"].as_array().unwrap() {
        assert_eq!(
            keys(m),
            BTreeSet::from([
                "id",
                "revision",
                "kind",
                "actor_name",
                "body",
                "status",
                "created_at"
            ])
        );
    }
    let encoded = raw.to_string();
    for secret_id in [
        hr.access.user_id,
        manager.access.user_id,
        owner.access.user_id,
        owner.access.company_id,
        owner.access.employee_id.unwrap(),
    ] {
        assert!(!encoded.contains(&secret_id.to_string()));
    }
    let cancel = Request::new(
        &owner.access,
        &own,
        request.request_id,
        Change::Cancel {
            body: "Fictional request is no longer needed".into(),
        },
    )
    .unwrap();
    let closed = repo.save(&owner, Audience::Own, &cancel).unwrap();
    let view = OwnView::from_context(&closed.context, &owner.access).unwrap();
    assert!(!view.requests[0].can_reply && !view.requests[0].can_cancel);
    assert_eq!(view.requests[0].messages.len(), 3);
    assert!(OwnView::from_context(&closed.context, &hr.access).is_err());
}
#[test]
fn portal_admin_stays_own_and_assignment_names_are_paired_snapshots() {
    let hr = data(Role::Administrator, 901, 0);
    let mut repo = Demo::default();
    let c = repo.context(&hr, Audience::Staff).unwrap();
    let foreign = submit(&hr.access, &c, hr.employees[1].id);
    repo.save(&hr, Audience::Staff, &foreign).unwrap();
    let own = repo.context(&hr, Audience::Own).unwrap();
    assert!(
        OwnView::from_context(&own, &hr.access)
            .unwrap()
            .requests
            .is_empty()
    );
    let submitted = submit(&hr.access, &own, hr.access.employee_id.unwrap());
    let c = repo.save(&hr, Audience::Own, &submitted).unwrap().context;
    assert_eq!(
        OwnView::from_context(&c, &hr.access)
            .unwrap()
            .requests
            .len(),
        1
    );
    let mut bad = c;
    bad.requests[0].assignee_name = Some("Unbound handler name".into());
    assert!(OwnView::from_context(&bad, &hr.access).is_err());
}

#[test]
fn protected_specialist_roles_use_capabilities_despite_employee_compatibility_base() {
    use hris_desktop::staff_roles::{Context as StaffContext, StaffRole, StaffRoles};
    fn protected(mut d: Snapshot, role: StaffRole) -> Snapshot {
        let roles = StaffRoles::new(vec![role]).unwrap();
        d.access.role = roles.compatible_base_role();
        d.access
            .bind_staff_context(StaffContext {
                schema_version: 1,
                company_id: d.access.company_id,
                user_id: d.access.user_id,
                employee_id: d.access.employee_id.unwrap(),
                base_role: d.access.role,
                revision: 1,
                runtime_ready: true,
                roles,
            })
            .unwrap();
        d
    }
    let hr = protected(data(Role::Employee, 901, 0), StaffRole::HrGeneralist);
    let specialist = protected(data(Role::Employee, 12, 3), StaffRole::Recruiter);
    assert_eq!(hr.access.role, Role::Employee);
    assert_eq!(specialist.access.role, Role::Employee);
    let mut repo = Demo::default();
    let c = repo.context(&hr, Audience::Staff).unwrap();
    assert!(c.can_route);
    let r = submit(&hr.access, &c, hr.employees[1].id);
    let c = repo.save(&hr, Audience::Staff, &r).unwrap().context;
    let route = Request::new(
        &hr.access,
        &c,
        r.request_id,
        Change::Route {
            assignee_user_id: Some(specialist.access.user_id),
            due_on: None,
            body: "Fictional specific assignment without broad personnel access".into(),
        },
    )
    .unwrap();
    repo.save(&hr, Audience::Staff, &route).unwrap();
    let c = repo.context(&specialist, Audience::Staff).unwrap();
    assert!(!c.can_route && c.requests.len() == 1 && c.handlers.is_empty());
    let reply = Request::new(
        &specialist.access,
        &c,
        r.request_id,
        Change::Reply {
            body: "Fictional specialist shared response".into(),
        },
    )
    .unwrap();
    repo.save(&specialist, Audience::Staff, &reply).unwrap();
    let revoked = protected(data(Role::Employee, 12, 3), StaffRole::Employee);
    assert!(repo.context(&revoked, Audience::Staff).is_err());
    assert!(repo.save(&revoked, Audience::Own, &reply).is_err());
    assert!(
        repo.context(&revoked, Audience::Own)
            .unwrap()
            .requests
            .is_empty()
    );
}
#[test]
fn invalid_actor_or_changed_historical_requester_is_rejected_before_projection() {
    let hr = data(Role::Administrator, 901, 0);
    let mut repo = Demo::default();
    let c = repo.context(&hr, Audience::Own).unwrap();
    let r = submit(&hr.access, &c, hr.access.employee_id.unwrap());
    let c = repo.save(&hr, Audience::Own, &r).unwrap().context;
    let reply = Request::new(
        &hr.access,
        &c,
        r.request_id,
        Change::Reply {
            body: "Fictional retained shared thread".into(),
        },
    )
    .unwrap();
    let c = repo.save(&hr, Audience::Own, &reply).unwrap().context;
    let mut bad = c.clone();
    bad.events[0].current_state.employee_id = hr.employees[1].id;
    bad.events[1].previous_state = Some(bad.events[0].current_state.clone());
    assert!(OwnView::from_context(&bad, &hr.access).is_err());
    let mut bad = c.clone();
    bad.events[1].kind = "submit".into();
    assert!(OwnView::from_context(&bad, &hr.access).is_err());
    let mut unsigned = hr.clone();
    unsigned.access.user_id = Uuid::nil();
    assert!(repo.context(&unsigned, Audience::Own).is_err());
    let mut bad = c;
    bad.events[1].actor_id = Uuid::nil();
    assert!(OwnView::from_context(&bad, &hr.access).is_err());
}
