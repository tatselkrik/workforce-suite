//! Native access negotiation over a real local HTTP fixture, never hosted Auth.
use super::government_export_tests::{access_json, client};
use super::*;
use crate::{
    access::Role,
    staff_roles::{Context as StaffContext, StaffRole, StaffRoles},
};
use std::{
    io::{BufRead, BufReader, Write},
    net::TcpListener,
};
pub(super) fn data(role: Role) -> Snapshot {
    let mut d = DemoStore::new().data;
    d.access.role = role;
    d.access.user_id = Uuid::from_u128(200);
    d.access.company_id = d.company.id;
    d.access.employee_id = Some(d.employees[0].id);
    d
}
pub(super) fn wire(d: &Snapshot, role: StaffRole, ready: bool) -> Value {
    serde_json::to_value(StaffContext {
        schema_version: 1,
        company_id: d.access.company_id,
        user_id: d.access.user_id,
        employee_id: d.access.employee_id.unwrap(),
        base_role: d.access.role,
        revision: 1,
        runtime_ready: ready,
        roles: StaffRoles::new(vec![role]).unwrap(),
    })
    .unwrap()
}
pub(super) fn server(
    replies: Vec<(&str, u16, Value)>,
) -> (String, thread::JoinHandle<Vec<(String, Value)>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let owned: Vec<_> = replies
        .into_iter()
        .map(|(p, s, b)| (p.to_owned(), s, b))
        .collect();
    let handle = thread::spawn(move || {
        let mut paths = Vec::new();
        let until = std::time::Instant::now() + Duration::from_secs(6);
        for (expected, status, body) in owned {
            let (mut socket, _) = loop {
                assert!(
                    std::time::Instant::now() < until,
                    "Staff HTTP fixture timed out"
                );
                match listener.accept() {
                    Ok(s) => break s,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2))
                    }
                    Err(e) => panic!("{e}"),
                }
            };
            // Windows can inherit the listener nonblocking mode on accept.
            socket.set_nonblocking(false).unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut reader = BufReader::new(socket.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let path = line.split_whitespace().nth(1).unwrap().to_owned();
            assert_eq!(path, expected);
            let mut length = 0;
            loop {
                line.clear();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
                if let Some((key, value)) = line.split_once(':') {
                    if key.eq_ignore_ascii_case("content-length") {
                        length = value.trim().parse::<usize>().unwrap();
                    }
                }
            }
            assert!(length < 4096);
            let mut bytes = vec![0; length];
            reader.read_exact(&mut bytes).unwrap();
            let text = body.to_string();
            let label = if status == 200 { "OK" } else { "Forbidden" };
            write!(socket,"HTTP/1.1 {status} {label}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",text.len()).unwrap();
            paths.push((
                path,
                if bytes.is_empty() {
                    Value::Null
                } else {
                    serde_json::from_slice(&bytes).unwrap()
                },
            ));
        }
        paths
    });
    (url, handle)
}
#[test]
fn old_servers_keep_the_original_one_request_access_contract() {
    let d = data(Role::HrGeneralist);
    let (url, handle) = server(vec![(
        "/rest/v1/rpc/my_hris_access",
        200,
        access_json(&d.access),
    )]);
    let a = client(url, &d).current_access().unwrap();
    assert!(a.staff_context.is_none());
    assert!(a.uses_desktop());
    assert_eq!(handle.join().unwrap().len(), 1);
}
#[test]
fn supported_specialists_are_admitted_only_after_protected_context_binding() {
    for role in [
        StaffRole::Recruiter,
        StaffRole::LearningPerformanceOfficer,
        StaffRole::PayrollOfficer,
    ] {
        let d = data(Role::Employee);
        let mut base = access_json(&d.access);
        base["staff_contract_version"] = json!(1);
        let (url, handle) = server(vec![
            ("/rest/v1/rpc/my_hris_access", 200, base),
            ("/rest/v1/rpc/staff_role_context", 200, wire(&d, role, true)),
        ]);
        let a = client(url, &d).current_access().unwrap();
        assert!(a.uses_desktop());
        assert!(a.staff_context.as_ref().unwrap().roles.contains(role));
        assert_eq!(handle.join().unwrap().len(), 2);
    }
}
#[test]
fn ordinary_employee_and_inactive_rollout_never_load_company_tables() {
    for ready in [false, true] {
        let d = data(Role::Employee);
        let mut base = access_json(&d.access);
        base["staff_contract_version"] = json!(1);
        let role = if ready {
            StaffRole::Employee
        } else {
            StaffRole::LearningPerformanceOfficer
        };
        let (url, handle) = server(vec![
            ("/rest/v1/rpc/my_hris_access", 200, base),
            (
                "/rest/v1/rpc/staff_role_context",
                200,
                wire(&d, role, ready),
            ),
            ("/auth/v1/logout?scope=local", 200, json!({})),
        ]);
        assert!(
            client(url, &d)
                .current_access()
                .unwrap_err()
                .is::<PortalAccount>()
        );
        assert_eq!(handle.join().unwrap().len(), 3);
    }
}
#[test]
fn wrong_identity_scope_version_revision_or_broad_base_fails_as_revoked() {
    let d = data(Role::Employee);
    let good = wire(&d, StaffRole::LearningPerformanceOfficer, true);
    let mut values = Vec::new();
    for key in ["company_id", "user_id", "employee_id"] {
        let mut c = good.clone();
        c[key] = json!(Uuid::from_u128(900));
        values.push(c);
    }
    let mut c = good.clone();
    c["schema_version"] = json!(2);
    values.push(c);
    let mut c = good.clone();
    c["revision"] = json!(0);
    values.push(c);
    let mut c = good.clone();
    c["roles"] = json!(["administrator"]);
    values.push(c);
    let mut c = good.clone();
    c["role_from_metadata"] = json!("administrator");
    values.push(c);
    for c in values {
        let mut base = access_json(&d.access);
        base["staff_contract_version"] = json!(1);
        let (url, handle) = server(vec![
            ("/rest/v1/rpc/my_hris_access", 200, base),
            ("/rest/v1/rpc/staff_role_context", 200, c),
        ]);
        assert!(
            client(url, &d)
                .current_access()
                .unwrap_err()
                .is::<AccessRevoked>()
        );
        assert_eq!(handle.join().unwrap().len(), 2);
    }
}
#[test]
fn denied_or_missing_advertised_context_does_not_fall_back_to_old_hr_rights() {
    for status in [401, 403, 404] {
        let d = data(Role::Administrator);
        let mut base = access_json(&d.access);
        base["staff_contract_version"] = json!(1);
        let (url, handle) = server(vec![
            ("/rest/v1/rpc/my_hris_access", 200, base),
            (
                "/rest/v1/rpc/staff_role_context",
                status,
                json!({"message":"denied"}),
            ),
        ]);
        assert!(
            client(url, &d)
                .current_access()
                .unwrap_err()
                .is::<AccessRevoked>()
        );
        assert_eq!(handle.join().unwrap().len(), 2);
    }
}
#[test]
fn unsupported_future_contract_is_refused_without_requesting_any_records() {
    let d = data(Role::Administrator);
    let mut base = access_json(&d.access);
    base["staff_contract_version"] = json!(2);
    let (url, handle) = server(vec![("/rest/v1/rpc/my_hris_access", 200, base)]);
    assert!(
        client(url, &d)
            .current_access()
            .unwrap_err()
            .is::<AccessRevoked>()
    );
    assert_eq!(handle.join().unwrap().len(), 1);
}

fn admin_wire(d: &Snapshot) -> Value {
    let stamp = "2026-10-27T09:00:00Z";
    json!({"schema_version":1,"company_id":d.access.company_id,"user_id":d.access.user_id,"runtime_ready":true,"account_count":2,
 "accounts":[{"user_id":d.access.user_id,"employee_id":d.access.employee_id.unwrap(),"employee_code":"EMP-000001","full_name":"Fictional Administrator","active":true,"base_role":"administrator","member_updated_at":stamp,"revision":1,"roles":["administrator"]},
 {"user_id":Uuid::from_u128(201),"employee_id":d.employees[1].id,"employee_code":"000002","full_name":"Fictional Employee","active":true,"base_role":"employee","member_updated_at":stamp,"revision":1,"roles":["employee"]}]})
}
pub(super) fn base_wire(d: &Snapshot) -> Value {
    let mut value = access_json(&d.access);
    value["staff_contract_version"] = json!(1);
    value
}
fn request(d: &Snapshot) -> crate::staff_roles::RoleChange {
    let mut actor = d.access.clone();
    actor.staff_contract_version = 1;
    actor
        .bind_staff_context(
            serde_json::from_value(wire(d, StaffRole::Administrator, true)).unwrap(),
        )
        .unwrap();
    let context: crate::staff_roles::admin::AdminContext =
        serde_json::from_value(admin_wire(d)).unwrap();
    context
        .prepare(
            &actor,
            Uuid::from_u128(201),
            StaffRoles::new(vec![StaffRole::LearningPerformanceOfficer]).unwrap(),
            true,
            "Assign reviewed development responsibilities".into(),
            Uuid::from_u128(202),
        )
        .unwrap()
}
#[test]
fn protected_admin_view_is_bound_and_rechecked_before_return() {
    let d = data(Role::Administrator);
    let base = base_wire(&d);
    let own = wire(&d, StaffRole::Administrator, true);
    let (url, handle) = server(vec![
        ("/rest/v1/rpc/my_hris_access", 200, base.clone()),
        ("/rest/v1/rpc/staff_role_context", 200, own.clone()),
        ("/rest/v1/rpc/staff_role_admin_context", 200, admin_wire(&d)),
        ("/rest/v1/rpc/my_hris_access", 200, base),
        ("/rest/v1/rpc/staff_role_context", 200, own),
    ]);
    let (_, view) = client(url, &d).staff_role_admin_context(false).unwrap();
    assert_eq!(view.account_count, 2);
    assert_eq!(view.accounts[1].employee_code, "000002");
    assert_eq!(handle.join().unwrap().len(), 5);
}
#[test]
fn malformed_admin_context_and_non_admin_actor_never_return_assignment_data() {
    let d = data(Role::Administrator);
    let mut values = Vec::new();
    let mut v = admin_wire(&d);
    v["company_id"] = json!(Uuid::from_u128(999));
    values.push(v);
    let mut v = admin_wire(&d);
    v["account_count"] = json!(3);
    values.push(v);
    let mut v = admin_wire(&d);
    v["accounts"][1]["employee_id"] = v["accounts"][0]["employee_id"].clone();
    values.push(v);
    let mut v = admin_wire(&d);
    v["runtime_ready"] = json!(false);
    values.push(v);
    for value in values {
        let (url, handle) = server(vec![
            ("/rest/v1/rpc/my_hris_access", 200, base_wire(&d)),
            (
                "/rest/v1/rpc/staff_role_context",
                200,
                wire(&d, StaffRole::Administrator, true),
            ),
            ("/rest/v1/rpc/staff_role_admin_context", 200, value),
        ]);
        assert!(
            client(url, &d)
                .staff_role_admin_context(false)
                .unwrap_err()
                .is::<AccessRevoked>()
        );
        assert_eq!(handle.join().unwrap().len(), 3);
    }
    let d = data(Role::Employee);
    let (url, handle) = server(vec![
        ("/rest/v1/rpc/my_hris_access", 200, base_wire(&d)),
        (
            "/rest/v1/rpc/staff_role_context",
            200,
            wire(&d, StaffRole::Recruiter, true),
        ),
    ]);
    assert!(client(url, &d).staff_role_admin_context(false).is_err());
    assert_eq!(handle.join().unwrap().len(), 2);
}
fn transaction_replies(
    d: &Snapshot,
    bad_receipt: bool,
    revoked_after: bool,
) -> Vec<(&'static str, u16, Value)> {
    let request = request(d);
    let base = base_wire(d);
    let own = wire(d, StaffRole::Administrator, true);
    let before = admin_wire(d);
    let mut after = before.clone();
    after["accounts"][1]["roles"] = json!(["learning_performance_officer"]);
    after["accounts"][1]["revision"] = json!(2);
    after["accounts"][1]["member_updated_at"] = json!("2026-10-27T09:00:01Z");
    let receipt = json!({"schema_version":1,"id":request.id,"company_id":d.access.company_id,
 "actor_id":if bad_receipt{Uuid::from_u128(999)}else{d.access.user_id},"user_id":request.user_id,"employee_id":d.employees[1].id,"active":request.active,"roles":request.roles,"revision":2,"base_role":"employee","member_updated_at":"2026-10-27T09:00:01Z"});
    let mut replies = vec![
        ("/rest/v1/rpc/my_hris_access", 200, base.clone()),
        ("/rest/v1/rpc/staff_role_context", 200, own.clone()),
        ("/rest/v1/rpc/staff_role_admin_context", 200, before),
        ("/rest/v1/rpc/set_staff_roles", 200, receipt),
    ];
    if bad_receipt {
        return replies;
    }
    if revoked_after {
        replies.push((
            "/rest/v1/rpc/staff_role_admin_context",
            403,
            json!({"message":"revoked"}),
        ));
        return replies;
    }
    replies.extend([
        ("/rest/v1/rpc/staff_role_admin_context", 200, after),
        ("/rest/v1/rpc/my_hris_access", 200, base),
        ("/rest/v1/rpc/staff_role_context", 200, own),
    ]);
    replies
}
#[test]
fn role_transaction_verifies_receipt_and_refreshes_current_assignments() {
    let d = data(Role::Administrator);
    let req = request(&d);
    let (url, handle) = server(transaction_replies(&d, false, false));
    let (_, receipt, current) = client(url, &d).set_staff_roles(&req).unwrap();
    assert_eq!(receipt.id, req.id);
    assert_eq!(current.accounts[1].roles, req.roles);
    assert_eq!(handle.join().unwrap().len(), 7);
}
#[test]
fn wrong_receipt_or_revoked_after_write_keeps_the_original_request_identity() {
    for (bad, revoked) in [(true, false), (false, true)] {
        let d = data(Role::Administrator);
        let req = request(&d);
        let original = req.clone();
        let (url, handle) = server(transaction_replies(&d, bad, revoked));
        assert!(
            client(url, &d)
                .set_staff_roles(&req)
                .unwrap_err()
                .is::<AccessRevoked>()
        );
        assert_eq!(req, original);
        assert_eq!(handle.join().unwrap().len(), if bad { 4 } else { 5 });
    }
}
#[test]
fn incomplete_rollout_cannot_send_an_assignment_transaction() {
    let d = data(Role::Administrator);
    let req = request(&d);
    let (url, handle) = server(vec![
        ("/rest/v1/rpc/my_hris_access", 200, base_wire(&d)),
        (
            "/rest/v1/rpc/staff_role_context",
            200,
            wire(&d, StaffRole::Administrator, false),
        ),
    ]);
    assert!(client(url, &d).set_staff_roles(&req).is_err());
    assert_eq!(handle.join().unwrap().len(), 2);
}

#[test]
fn staff_editor_worker_returns_bound_context_and_exact_request_receipt() {
    use crate::staff_roles::admin::Operation;
    let d = data(Role::Administrator);
    let req = request(&d);
    let mut current = d.clone();
    current.access.staff_contract_version = 1;
    current
        .access
        .bind_staff_context(
            serde_json::from_value(wire(&d, StaffRole::Administrator, true)).unwrap(),
        )
        .unwrap();
    let mut replies = vec![
        ("/rest/v1/rpc/my_hris_access", 200, base_wire(&d)),
        (
            "/rest/v1/rpc/staff_role_context",
            200,
            wire(&d, StaffRole::Administrator, true),
        ),
    ];
    replies.extend(transaction_replies(&d, false, false));
    let (url, handle) = server(replies);
    let (snapshot, update) = client(url, &d)
        .execute_staff_roles(Operation::Save(req.clone()), &current)
        .unwrap();
    assert_eq!(snapshot.access, current.access);
    assert_eq!(update.receipt.unwrap().id, req.id);
    assert_eq!(update.context.accounts[1].roles, req.roles);
    let requests = handle.join().unwrap();
    assert_eq!(requests.len(), 9);
    assert_eq!(requests[5].0, "/rest/v1/rpc/set_staff_roles");
    assert_eq!(requests[5].1, json!({"p_change":req}));
}
#[test]
fn staff_editor_worker_refuses_changed_actor_before_sending_a_save() {
    use crate::staff_roles::admin::Operation;
    let d = data(Role::Administrator);
    let req = request(&d);
    let (url, handle) = server(vec![
        ("/rest/v1/rpc/my_hris_access", 200, base_wire(&d)),
        (
            "/rest/v1/rpc/staff_role_context",
            200,
            wire(&d, StaffRole::Administrator, true),
        ),
    ]);
    assert!(
        client(url, &d)
            .execute_staff_roles(Operation::Save(req), &d)
            .err()
            .unwrap()
            .is::<AccessRevoked>()
    );
    assert_eq!(handle.join().unwrap().len(), 2);
}

#[test]
fn only_a_confirmed_transaction_rejection_releases_the_editor_retry() {
    for (status, code, rejected) in [
        (400, "23514", true),
        (400, "PGRST000", false),
        (500, "XX000", false),
    ] {
        let d = data(Role::Administrator);
        let req = request(&d);
        let original = req.clone();
        let mut replies = transaction_replies(&d, false, false);
        replies.truncate(4);
        replies[3].1 = status;
        replies[3].2 = json!({"code":code,"message":"Fictional rejection"});
        let (url, handle) = server(replies);
        let error = client(url, &d).set_staff_roles(&req).unwrap_err();
        assert_eq!(error.is::<super::staff_roles_adapter::Rejected>(), rejected);
        assert_eq!(req, original);
        assert_eq!(handle.join().unwrap().len(), 4);
    }
}

#[test]
fn temporary_post_save_refresh_failure_preserves_the_pending_retry() {
    let d = data(Role::Administrator);
    let req = request(&d);
    let original = req.clone();
    let mut replies = transaction_replies(&d, false, false);
    replies.truncate(5);
    replies[4].1 = 503;
    replies[4].2 = json!({"message":"Fictional temporary refresh failure"});
    let (url, handle) = server(replies);
    let error = client(url, &d).set_staff_roles(&req).unwrap_err();
    assert!(!error.is::<AccessRevoked>());
    assert!(!error.is::<super::staff_roles_adapter::Rejected>());
    assert!(error.to_string().contains("recorded"));
    assert_eq!(req, original);
    assert_eq!(handle.join().unwrap().len(), 5);
}
