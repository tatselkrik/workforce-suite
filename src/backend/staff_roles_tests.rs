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
fn data(role: Role) -> Snapshot {
    let mut d = DemoStore::new().data;
    d.access.role = role;
    d.access.user_id = Uuid::from_u128(200);
    d.access.company_id = d.company.id;
    d.access.employee_id = Some(d.employees[0].id);
    d
}
fn wire(d: &Snapshot, role: StaffRole, ready: bool) -> Value {
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
fn server(replies: Vec<(&str, u16, Value)>) -> (String, thread::JoinHandle<Vec<String>>) {
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
            paths.push(path);
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
