use super::government_export_tests::{access_json, client};
use super::staff_roles_tests::{data, server};
use super::*;
use crate::access::Role;
use crate::relations::{self, Change, Context as CaseContext, Operation, Request};
fn fixture() -> (Snapshot, CaseContext, Request, relations::Receipt) {
    let mut d = data(Role::Administrator);
    d.accounts[0].user_id = d.access.user_id;
    let mut repo = relations::Demo::default();
    let c = repo.execute(Operation::Refresh, &d).unwrap().context;
    let r = Request::new(
        &d.access,
        &c,
        Uuid::new_v4(),
        Change::Create {
            employee_id: d.employees[1].id,
            owner_id: d.access.user_id,
            title: "Fictional case for native adapter".into(),
            category: relations::Category::Other,
            priority: relations::Priority::Normal,
            body: "Fictional initial account".into(),
        },
    )
    .unwrap();
    let result = repo
        .execute(Operation::Save(Box::new(r.clone())), &d)
        .unwrap();
    (d, result.context, r, result.receipt.unwrap())
}
#[test]
fn protected_context_read_rechecks_current_access() {
    let (d, c, _, _) = fixture();
    let a = access_json(&d.access);
    let (url, join) = server(vec![
        ("/rest/v1/rpc/my_hris_access", 200, a.clone()),
        ("/rest/v1/rpc/employee_relation_context", 200, json!(c)),
        ("/rest/v1/rpc/my_hris_access", 200, a),
    ]);
    let result = client(url, &d).relation_context(&d).unwrap();
    assert_eq!(result, c);
    assert_eq!(join.join().unwrap().len(), 3);
}
#[test]
fn successful_save_binds_the_exact_payload_and_receipt() {
    let (d, c, r, receipt) = fixture();
    let a = access_json(&d.access);
    let (url, join) = server(vec![
        ("/rest/v1/rpc/my_hris_access", 200, a.clone()),
        ("/rest/v1/rpc/save_employee_relation", 200, json!(receipt)),
        ("/rest/v1/rpc/my_hris_access", 200, a.clone()),
        ("/rest/v1/rpc/my_hris_access", 200, a.clone()),
        ("/rest/v1/rpc/employee_relation_context", 200, json!(c)),
        ("/rest/v1/rpc/my_hris_access", 200, a),
    ]);
    let response = client(url, &d)
        .execute_relation(&d, Operation::Save(Box::new(r.clone())))
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
                "/rest/v1/rpc/save_employee_relation",
                status,
                json!({"message":"Fictional rejection"}),
            ),
        ]);
        let error = client(url, &d)
            .execute_relation(&d, Operation::Save(Box::new(r.clone())))
            .unwrap_err();
        assert_eq!(error.is::<relations::Rejected>(), status == 400);
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
        (
            "/rest/v1/rpc/employee_relation_context",
            200,
            json!(foreign),
        ),
    ]);
    assert!(
        client(url, &d)
            .relation_context(&d)
            .unwrap_err()
            .is::<AccessRevoked>()
    );
    join.join().unwrap();
    let mut revoked = d.access.clone();
    revoked.role = Role::Manager;
    let (url, join) = server(vec![
        ("/rest/v1/rpc/my_hris_access", 200, access_json(&d.access)),
        ("/rest/v1/rpc/employee_relation_context", 200, json!(c)),
        ("/rest/v1/rpc/my_hris_access", 200, access_json(&revoked)),
    ]);
    assert!(
        client(url, &d)
            .relation_context(&d)
            .unwrap_err()
            .is::<AccessRevoked>()
    );
    join.join().unwrap();
}
#[test]
fn unrelated_manager_and_payroll_product_make_no_case_read() {
    let (mut d, _, _, _) = fixture();
    d.access.role = Role::Manager;
    let (url, join) = server(vec![(
        "/rest/v1/rpc/my_hris_access",
        200,
        access_json(&d.access),
    )]);
    assert!(client(url, &d).relation_context(&d).is_err());
    assert_eq!(join.join().unwrap().len(), 1);
    d.access.role = Role::Administrator;
    let (url, join) = server(vec![(
        "/rest/v1/rpc/my_hris_access",
        200,
        access_json(&d.access),
    )]);
    let mut c = client(url, &d);
    c.product = Some(crate::desktop::Product::Payroll);
    assert!(c.relation_context(&d).is_err());
    assert_eq!(join.join().unwrap().len(), 1);
}
#[test]
#[ignore = "synthetic managed Auth on the exact installed prototype only"]
fn hosted_native_relations_context_and_save() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target = std::fs::read_to_string(root.join("supabase/.temp/project-ref")).unwrap();
    let target = target.trim();
    let guard =
        std::fs::read_to_string(root.join("deployment-private/SQL-AUTHORIZATION.md")).unwrap();
    assert!(guard.contains(&format!("{}{}{}", char::from(96), target, char::from(96))));
    let installed = std::path::PathBuf::from(std::env::var("LOCALAPPDATA").unwrap())
        .join("Programs/HRIS Desktop/hris-connection.json");
    let config = ConnectionConfig::load_profile(&installed, true).unwrap();
    assert!(config.url.trim_end_matches('/') == format!("https://{target}.supabase.co"));
    let session: AuthSession =
        serde_json::from_str(&std::env::var("HRIS_RELATIONS_SESSION").unwrap()).unwrap();
    assert!(
        session.user.email.starts_with("qa-gov042-relations066-")
            && session.user.email.ends_with("@example.invalid")
    );
    let roles: crate::staff_roles::StaffRoles =
        serde_json::from_str(&std::env::var("HRIS_RELATIONS_ROLES").unwrap()).unwrap();
    let case = std::env::var("HRIS_RELATIONS_CASE").unwrap();
    assert!(case.len() < 40 && case.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'));
    let mut c = client(config.url.clone(), &data(Role::Administrator));
    c.config = config;
    c.session = session;
    c.product = Some(crate::desktop::Product::Hris);
    c.http = Client::builder()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let allowed = roles.allows(crate::staff_roles::Capability::Personnel)
        && std::env::var("HRIS_RELATIONS_ACTIVE").as_deref() != Ok("false");
    let access = c.current_access();
    let proof = if let Ok(access) = access {
        assert!(access.staff_roles().is_ok_and(|r| r == roles));
        let mut d = Snapshot::signed_out();
        d.company.id = access.company_id;
        d.access = access;
        let mut request_proof = false;
        if let Ok(raw) = std::env::var("HRIS_RELATIONS_REQUEST") {
            assert!(allowed);
            let r: Request = serde_json::from_str(&raw).unwrap();
            let ctx = c.relation_context(&d);
            assert!(ctx.is_ok());
            let ctx = ctx.unwrap();
            let subject = match &r.change {
                Change::Create { employee_id, .. } => *employee_id,
                _ => ctx.case(r.case_id).unwrap().employee_id,
            };
            let roster: Vec<crate::domain::Employee> = c
                .scoped_rows("employees", Some(d.access.company_id), Some(subject))
                .unwrap();
            assert!(roster.len() == 1 && roster[0].employee_code.starts_with("QA-ER066-"));
            let first = c.execute_relation(&d, Operation::Save(Box::new(r.clone())));
            assert!(first.is_ok(), "Native synthetic case write failed");
            let first = first.unwrap();
            assert!(
                first
                    .receipt
                    .as_ref()
                    .is_some_and(|v| v.validate(&r).is_ok())
            );
            let retry = c.execute_relation(&d, Operation::Save(Box::new(r)));
            assert!(retry.is_ok(), "Native exact case retry failed");
            let retry = retry.unwrap();
            assert!(retry.receipt == first.receipt && retry.context == first.context);
            request_proof = true;
        }
        let result = c.relation_context(&d);
        assert!(
            result.is_ok() == allowed,
            "Protected native case eligibility mismatch"
        );
        if let Ok(ctx) = result {
            let expected: std::collections::BTreeSet<Uuid> =
                serde_json::from_str(&std::env::var("HRIS_RELATIONS_EXPECT_IDS").unwrap()).unwrap();
            let actual: std::collections::BTreeSet<_> = ctx.cases.iter().map(|v| v.id).collect();
            assert!(expected == actual, "Native case assignment scope mismatch");
            json!({"allowed":true,"cases":ctx.cases.len(),"events":ctx.events.len(),"follow_ups":ctx.follow_ups.len(),"typed_source_and_history_validated":true,"native_save_and_exact_retry":request_proof})
        } else {
            json!({"allowed":false})
        }
    } else {
        assert!(!allowed, "Expected active HR admission");
        json!({"allowed":false})
    };
    std::fs::write(
        root.join("work")
            .join(format!("relations-native-{case}.json")),
        serde_json::to_vec_pretty(&proof).unwrap(),
    )
    .unwrap();
}
