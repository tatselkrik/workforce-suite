use super::government_export_tests::client;
use super::staff_roles_tests::{base_wire, data, server, wire};
use super::*;
use crate::{
    access::Role,
    staff_roles::{
        StaffRole,
        provision::{self, Rejection, RejectionKind, Request},
    },
};
fn fixture() -> (Snapshot, Request) {
    let mut d = data(Role::Administrator);
    d.access.staff_contract_version = 1;
    d.access
        .bind_staff_context(
            serde_json::from_value(wire(&d, StaffRole::Administrator, true)).unwrap(),
        )
        .unwrap();
    let request = Request::new(provision::Input {
        id: Uuid::from_u128(333),
        email: "person@example.invalid".into(),
        roles: crate::staff_roles::StaffRoles::new(vec![
            StaffRole::Recruiter,
            StaffRole::LearningPerformanceOfficer,
        ])
        .unwrap(),
        active: true,
        reason: "Reviewed responsibilities".into(),
        employee: provision::Person::from_draft(
            &EmployeeDraft {
                employee_code: "000333".into(),
                full_name: "Fictional new staff".into(),
                annual_leave_days: 15,
                ..Default::default()
            },
            d.company.id,
        )
        .unwrap(),
        contacts: crate::contacts::EmployeeContacts::empty(d.company.id, Uuid::nil()),
        expected_source: None,
        supersedes: None,
    })
    .unwrap();
    (d, request)
}
fn receipt(d: &Snapshot, r: &Request) -> Value {
    json!({"schema_version":1,"id":r.data().id,"company_id":d.company.id,"actor_id":d.access.user_id,"user_id":Uuid::from_u128(334),"employee_id":Uuid::from_u128(335),"active":true,"roles":r.data().roles,"revision":1,"base_role":"employee","member_updated_at":"2026-10-27T12:00:00Z"})
}
fn replies(
    d: &Snapshot,
    path: &'static str,
    body: Value,
    success: bool,
) -> Vec<(&'static str, u16, Value)> {
    let mut result = vec![
        ("/rest/v1/rpc/my_hris_access", 200, base_wire(d)),
        (
            "/rest/v1/rpc/staff_role_context",
            200,
            wire(d, StaffRole::Administrator, true),
        ),
        (path, 200, body),
    ];
    if success {
        result.extend([
            ("/rest/v1/rpc/my_hris_access", 200, base_wire(d)),
            (
                "/rest/v1/rpc/staff_role_context",
                200,
                wire(d, StaffRole::Administrator, true),
            ),
        ]);
    }
    result
}
#[test]
fn native_provisioning_separates_the_in_memory_password_from_the_frozen_business_request() {
    let (d, r) = fixture();
    let original = serde_json::to_value(&r).unwrap();
    let complete = json!({"message":"Saved","receipt":receipt(&d,&r),"confirmation_sent":false,"replayed":false});
    let (url, h) = server(replies(&d, "/functions/v1/account-admin", complete, true));
    let done = client(url, &d)
        .create_staff_provision(&d.access, &r, Some("fictional initial password"))
        .unwrap();
    assert_eq!(done.receipt.id, r.data().id);
    assert_eq!(serde_json::to_value(&r).unwrap(), original);
    let calls = h.join().unwrap();
    assert_eq!(calls.len(), 5);
    assert_eq!(
        calls[2].1,
        json!({"operation":"create_staff_v2","request":original,"password":"fictional initial password"})
    );
    assert!(original.get("password").is_none());
}
#[test]
fn native_pending_recovery_does_not_require_or_resend_a_password() {
    let (d, r) = fixture();
    let complete = json!({"message":"Recovered","receipt":receipt(&d,&r),"confirmation_sent":false,"replayed":true});
    let (url, h) = server(replies(&d, "/functions/v1/account-admin", complete, true));
    assert!(
        client(url, &d)
            .create_staff_provision(&d.access, &r, None)
            .unwrap()
            .replayed
    );
    let calls = h.join().unwrap();
    assert!(calls[2].1.get("password").is_none());
    assert_eq!(calls.len(), 5);
}
#[test]
fn native_provisioning_distinguishes_confirmed_review_outcomes_from_uncertain_delivery() {
    for (kind, expected) in [
        ("not_started", RejectionKind::NotStarted),
        ("needs_review", RejectionKind::NeedsReview),
        ("closed", RejectionKind::Closed),
    ] {
        let (d, r) = fixture();
        let mut wire = replies(
            &d,
            "/functions/v1/account-admin",
            json!({"provision_state":kind,"intent_id":r.data().id}),
            false,
        );
        wire[2].1 = 409;
        let (url, h) = server(wire);
        let error = client(url, &d)
            .create_staff_provision(&d.access, &r, None)
            .err()
            .unwrap();
        assert_eq!(error.downcast_ref::<Rejection>().unwrap().kind, expected);
        assert_eq!(h.join().unwrap().len(), 3);
    }
    let (d, r) = fixture();
    let mut wire = replies(
        &d,
        "/functions/v1/account-admin",
        json!({"message":"Fictional lost delivery"}),
        false,
    );
    wire[2].1 = 502;
    let (url, h) = server(wire);
    let original = serde_json::to_value(&r).unwrap();
    let error = client(url, &d)
        .create_staff_provision(&d.access, &r, None)
        .err()
        .unwrap();
    assert!(!error.is::<Rejection>());
    assert!(!error.is::<AccessRevoked>());
    assert_eq!(serde_json::to_value(r).unwrap(), original);
    h.join().unwrap();
}
#[test]
fn foreign_receipt_or_late_revocation_never_returns_provisioning_data() {
    for (revoke, foreign) in [(false, true), (true, false)] {
        let (d, r) = fixture();
        let mut rec = receipt(&d, &r);
        if foreign {
            rec["actor_id"] = json!(Uuid::from_u128(999));
        }
        let complete =
            json!({"message":"Saved","receipt":rec,"confirmation_sent":false,"replayed":false});
        let mut wire = replies(&d, "/functions/v1/account-admin", complete, !foreign);
        if revoke {
            wire.truncate(4);
            wire[3].1 = 403;
            wire[3].2 = json!({"message":"Fictional revocation"});
        }
        let (url, h) = server(wire);
        assert!(
            client(url, &d)
                .create_staff_provision(&d.access, &r, None)
                .err()
                .unwrap()
                .is::<AccessRevoked>()
        );
        h.join().unwrap();
    }
}
#[test]
fn native_employee_source_is_same_employee_and_rechecks_current_access() {
    let (d, _) = fixture();
    let e = &d.employees[1];
    let source = json!({"schema_version":1,"company_id":d.company.id,"actor_id":d.access.user_id,"runtime_ready":true,"employee_id":e.id,"source_hash":"a".repeat(64),"employee":e,"contacts":null});
    let (url, h) = server(replies(
        &d,
        "/rest/v1/rpc/staff_employee_source",
        source,
        true,
    ));
    let source = client(url, &d)
        .staff_employee_source(&d.access, e.id)
        .unwrap();
    assert_eq!(source.employee.id, e.id);
    assert_eq!(h.join().unwrap().len(), 5);
}
#[test]
fn malformed_pending_count_cannot_silently_omit_a_pending_account() {
    let (d, _) = fixture();
    let body = json!({"schema_version":1,"company_id":d.company.id,"actor_id":d.access.user_id,"entry_count":1,"entries":[]});
    let (url, h) = server(replies(
        &d,
        "/rest/v1/rpc/pending_staff_provisions",
        body,
        false,
    ));
    assert!(
        client(url, &d)
            .pending_staff_provisions(&d.access)
            .err()
            .unwrap()
            .is::<AccessRevoked>()
    );
    assert_eq!(h.join().unwrap().len(), 3);
}
#[test]
fn cancellation_is_reason_bound_and_rechecked_before_local_draft_clearance() {
    let (d, r) = fixture();
    let (url, h) = server(replies(
        &d,
        "/rest/v1/rpc/cancel_staff_provision",
        json!({"id":r.data().id,"state":"cancelled"}),
        true,
    ));
    client(url, &d)
        .cancel_staff_provision(&d.access, &r, "Reviewed cancellation")
        .unwrap();
    let calls = h.join().unwrap();
    assert_eq!(
        calls[2].1,
        json!({"p_id":r.data().id,"p_reason":"Reviewed cancellation"})
    );
    assert_eq!(calls.len(), 5);
}
#[test]
fn unknown_own_status_is_explicit_null_after_current_access_rechecks() {
    let (d, r) = fixture();
    let (url, h) = server(replies(
        &d,
        "/rest/v1/rpc/staff_provision_status",
        Value::Null,
        true,
    ));
    assert!(
        client(url, &d)
            .staff_provision_status(&d.access, &r)
            .unwrap()
            .is_none()
    );
    assert_eq!(h.join().unwrap().len(), 5);
}
