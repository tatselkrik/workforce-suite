use super::government_export_tests::{access_json, client};
use super::staff_roles_tests::{data, server};
use super::*;
use crate::access::Role;
use crate::services::{self, Change, Context as ServiceContext, Operation, Request};
fn fixture() -> (Snapshot, ServiceContext, Request, services::Receipt) {
    let mut d = data(Role::Administrator);
    d.accounts[0].user_id = d.access.user_id;
    let mut repo = services::Demo::default();
    let c = repo.context(&d, services::Audience::Staff).unwrap();
    let r = Request::new(
        &d.access,
        &c,
        Uuid::new_v4(),
        Change::Submit {
            employee_id: d.employees[1].id,
            title: "Fictional service request for native adapter".into(),
            category: services::Category::GeneralSupport,
            priority: services::Priority::Normal,
            body: "Fictional shared service message".into(),
        },
    )
    .unwrap();
    let result = repo.save(&d, services::Audience::Staff, &r).unwrap();
    (d, result.context, r, result.receipt.unwrap())
}
#[test]
fn protected_context_read_rechecks_current_access() {
    let (d, c, _, _) = fixture();
    let a = access_json(&d.access);
    let (url, join) = server(vec![
        ("/rest/v1/rpc/my_hris_access", 200, a.clone()),
        ("/rest/v1/rpc/employee_service_context", 200, json!(c)),
        ("/rest/v1/rpc/my_hris_access", 200, a),
    ]);
    let result = client(url, &d).service_context(&d).unwrap();
    assert_eq!(result, c);
    assert_eq!(join.join().unwrap().len(), 3);
}
#[test]
fn successful_save_binds_the_exact_payload_and_receipt() {
    let (d, c, r, receipt) = fixture();
    let a = access_json(&d.access);
    let (url, join) = server(vec![
        ("/rest/v1/rpc/my_hris_access", 200, a.clone()),
        ("/rest/v1/rpc/save_employee_service", 200, json!(receipt)),
        ("/rest/v1/rpc/my_hris_access", 200, a.clone()),
        ("/rest/v1/rpc/my_hris_access", 200, a.clone()),
        ("/rest/v1/rpc/employee_service_context", 200, json!(c)),
        ("/rest/v1/rpc/my_hris_access", 200, a),
    ]);
    let response = client(url, &d)
        .execute_service(&d, Operation::Save(Box::new(r.clone())))
        .unwrap();
    assert_eq!(response.receipt, Some(receipt));
    let requests = join.join().unwrap();
    assert_eq!(requests[1].1, json!({"p_change":r}));
}
#[test]
fn definitive_rejection_is_distinct_from_uncertain_failure() {
    let (d, _, r, _) = fixture();
    for status in [400, 500] {
        let (url, join) = server(vec![
            ("/rest/v1/rpc/my_hris_access", 200, access_json(&d.access)),
            (
                "/rest/v1/rpc/save_employee_service",
                status,
                json!({"message":"Fictional rejection"}),
            ),
        ]);
        let error = client(url, &d)
            .execute_service(&d, Operation::Save(Box::new(r.clone())))
            .unwrap_err();
        assert_eq!(error.is::<services::Rejected>(), status == 400);
        join.join().unwrap();
    }
}
#[test]
fn foreign_context_and_mid_read_revocation_fail_closed() {
    let (d, c, _, _) = fixture();
    let mut foreign = c.clone();
    foreign.company_id = Uuid::new_v4();
    let (url, join) = server(vec![
        ("/rest/v1/rpc/my_hris_access", 200, access_json(&d.access)),
        ("/rest/v1/rpc/employee_service_context", 200, json!(foreign)),
    ]);
    assert!(
        client(url, &d)
            .service_context(&d)
            .unwrap_err()
            .is::<AccessRevoked>()
    );
    join.join().unwrap();
    let mut revoked = d.access.clone();
    revoked.role = Role::Manager;
    let (url, join) = server(vec![
        ("/rest/v1/rpc/my_hris_access", 200, access_json(&d.access)),
        ("/rest/v1/rpc/employee_service_context", 200, json!(c)),
        ("/rest/v1/rpc/my_hris_access", 200, access_json(&revoked)),
    ]);
    assert!(
        client(url, &d)
            .service_context(&d)
            .unwrap_err()
            .is::<AccessRevoked>()
    );
    join.join().unwrap();
}
#[test]
fn payroll_product_and_ordinary_employee_make_no_service_context_read() {
    let (mut d, _, _, _) = fixture();
    d.access.role = Role::Employee;
    let (url, join) = server(vec![
        ("/rest/v1/rpc/my_hris_access", 200, access_json(&d.access)),
        ("/auth/v1/logout?scope=local", 200, json!({})),
    ]);
    assert!(
        client(url, &d)
            .service_context(&d)
            .unwrap_err()
            .is::<PortalAccount>()
    );
    assert_eq!(join.join().unwrap().len(), 2);
    d.access.role = Role::Administrator;
    let (url, join) = server(vec![(
        "/rest/v1/rpc/my_hris_access",
        200,
        access_json(&d.access),
    )]);
    let mut c = client(url, &d);
    c.product = Some(crate::desktop::Product::Payroll);
    assert!(c.service_context(&d).is_err());
    assert_eq!(join.join().unwrap().len(), 1);
}
#[test]
fn native_adapter_rejects_a_portal_audience_even_for_administrator() {
    let (d, _, _, _) = fixture();
    let own = services::Demo::default()
        .context(&d, services::Audience::Own)
        .unwrap();
    let (url, join) = server(vec![
        ("/rest/v1/rpc/my_hris_access", 200, access_json(&d.access)),
        ("/rest/v1/rpc/employee_service_context", 200, json!(own)),
    ]);
    assert!(
        client(url, &d)
            .service_context(&d)
            .unwrap_err()
            .is::<AccessRevoked>()
    );
    join.join().unwrap();
}

#[test]
fn revocation_after_a_save_receipt_stops_before_loading_the_service_context() {
    let (d, _, r, receipt) = fixture();
    let mut revoked = d.access.clone();
    revoked.role = Role::Employee;
    let (url, join) = server(vec![
        ("/rest/v1/rpc/my_hris_access", 200, access_json(&d.access)),
        ("/rest/v1/rpc/save_employee_service", 200, json!(receipt)),
        ("/rest/v1/rpc/my_hris_access", 200, access_json(&revoked)),
        ("/auth/v1/logout?scope=local", 200, json!({})),
    ]);
    let error = client(url, &d)
        .execute_service(&d, Operation::Save(Box::new(r)))
        .unwrap_err();
    assert!(error.is::<PortalAccount>());
    assert_eq!(join.join().unwrap().len(), 4);
}
#[test]
fn wrong_receipt_never_acknowledges_or_replaces_an_uncertain_request() {
    let (d, _, r, mut receipt) = fixture();
    receipt.request_id = Uuid::new_v4();
    let (url, join) = server(vec![
        ("/rest/v1/rpc/my_hris_access", 200, access_json(&d.access)),
        ("/rest/v1/rpc/save_employee_service", 200, json!(receipt)),
    ]);
    let error = client(url, &d)
        .execute_service(&d, Operation::Save(Box::new(r.clone())))
        .unwrap_err();
    assert!(!error.is::<services::Rejected>());
    let sent = join.join().unwrap();
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[1].1, json!({"p_change": r}));
}
