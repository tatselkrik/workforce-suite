use super::government_export_tests::{access_json, client};
use super::staff_roles_tests::{data, server};
use super::*;
use crate::access::Role;
use crate::benefits::{self, Change, Context as BenefitContext, Operation, Request};
fn fixture() -> (Snapshot, BenefitContext, Request, benefits::Receipt) {
    let mut d = data(Role::Administrator);
    d.accounts[0].user_id = d.access.user_id;
    let mut repo = benefits::Demo::default();
    let c = repo.execute(Operation::Refresh, &d).unwrap().context;
    let plan = benefits::Plan {
        code: "LOCAL-01".into(),
        name: "Fictional local benefit".into(),
        category: benefits::Category::Wellness,
        description: "Reviewed company benefit".into(),
        eligibility_guidance: "Review current employment before enrollment".into(),
        dependent_limit: 2,
        available: true,
    };
    let r = Request::new(
        &d.access,
        &c,
        Uuid::new_v4(),
        Change::Plan(plan),
        "Fictional protected plan review".into(),
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
        ("/rest/v1/rpc/employee_benefit_context", 200, json!(c)),
        ("/rest/v1/rpc/my_hris_access", 200, a),
    ]);
    let result = client(url, &d).benefit_context(&d).unwrap();
    assert_eq!(result, c);
    assert_eq!(join.join().unwrap().len(), 3);
}
#[test]
fn successful_save_binds_the_exact_payload_and_receipt() {
    let (d, c, r, receipt) = fixture();
    let a = access_json(&d.access);
    let (url, join) = server(vec![
        ("/rest/v1/rpc/my_hris_access", 200, a.clone()),
        ("/rest/v1/rpc/save_employee_benefit", 200, json!(receipt)),
        ("/rest/v1/rpc/my_hris_access", 200, a.clone()),
        ("/rest/v1/rpc/my_hris_access", 200, a.clone()),
        ("/rest/v1/rpc/employee_benefit_context", 200, json!(c)),
        ("/rest/v1/rpc/my_hris_access", 200, a),
    ]);
    let response = client(url, &d)
        .execute_benefit(&d, Operation::Save(Box::new(r.clone())))
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
                "/rest/v1/rpc/save_employee_benefit",
                status,
                json!({"message":"Fictional rejection"}),
            ),
        ]);
        let error = client(url, &d)
            .execute_benefit(&d, Operation::Save(Box::new(r.clone())))
            .unwrap_err();
        assert_eq!(error.is::<benefits::Rejected>(), status == 400);
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
        ("/rest/v1/rpc/employee_benefit_context", 200, json!(foreign)),
    ]);
    assert!(
        client(url, &d)
            .benefit_context(&d)
            .unwrap_err()
            .is::<AccessRevoked>()
    );
    join.join().unwrap();
    let mut revoked = d.access.clone();
    revoked.role = Role::Manager;
    let (url, join) = server(vec![
        ("/rest/v1/rpc/my_hris_access", 200, access_json(&d.access)),
        ("/rest/v1/rpc/employee_benefit_context", 200, json!(c)),
        ("/rest/v1/rpc/my_hris_access", 200, access_json(&revoked)),
    ]);
    assert!(
        client(url, &d)
            .benefit_context(&d)
            .unwrap_err()
            .is::<AccessRevoked>()
    );
    join.join().unwrap();
}
#[test]
fn unrelated_manager_and_payroll_product_make_no_benefits_read() {
    let (mut d, _, _, _) = fixture();
    d.access.role = Role::Manager;
    let (url, join) = server(vec![(
        "/rest/v1/rpc/my_hris_access",
        200,
        access_json(&d.access),
    )]);
    assert!(client(url, &d).benefit_context(&d).is_err());
    assert_eq!(join.join().unwrap().len(), 1);
    d.access.role = Role::Administrator;
    let (url, join) = server(vec![(
        "/rest/v1/rpc/my_hris_access",
        200,
        access_json(&d.access),
    )]);
    let mut c = client(url, &d);
    c.product = Some(crate::desktop::Product::Payroll);
    assert!(c.benefit_context(&d).is_err());
    assert_eq!(join.join().unwrap().len(), 1);
}
#[test]
fn revocation_after_a_save_receipt_stops_before_loading_the_benefit_context() {
    let (d, _, r, receipt) = fixture();
    let mut revoked = d.access.clone();
    revoked.role = Role::Employee;
    let (url, join) = server(vec![
        ("/rest/v1/rpc/my_hris_access", 200, access_json(&d.access)),
        ("/rest/v1/rpc/save_employee_benefit", 200, json!(receipt)),
        ("/rest/v1/rpc/my_hris_access", 200, access_json(&revoked)),
        ("/auth/v1/logout?scope=local", 200, json!({})),
    ]);
    let error = client(url, &d)
        .execute_benefit(&d, Operation::Save(Box::new(r)))
        .unwrap_err();
    assert!(error.is::<PortalAccount>());
    assert_eq!(join.join().unwrap().len(), 4);
}
#[test]
fn wrong_receipt_never_acknowledges_or_replaces_an_uncertain_request() {
    let (d, _, r, mut receipt) = fixture();
    receipt.subject_id = Uuid::new_v4();
    let (url, join) = server(vec![
        ("/rest/v1/rpc/my_hris_access", 200, access_json(&d.access)),
        ("/rest/v1/rpc/save_employee_benefit", 200, json!(receipt)),
    ]);
    let error = client(url, &d)
        .execute_benefit(&d, Operation::Save(Box::new(r.clone())))
        .unwrap_err();
    assert!(!error.is::<benefits::Rejected>());
    let sent = join.join().unwrap();
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[1].1, json!({"p_change": r}));
}

#[test]
#[ignore = "synthetic managed Auth on the exact installed prototype only"]
fn hosted_native_benefits_context_and_save() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target = std::fs::read_to_string(root.join("supabase/.temp/project-ref")).unwrap();
    let target = target.trim();
    let guard =
        std::fs::read_to_string(root.join("deployment-private/SQL-AUTHORIZATION.md")).unwrap();
    assert!(guard.contains(&format!("{}{}{}", char::from(96), target, char::from(96))));
    let installed = std::path::PathBuf::from(std::env::var("LOCALAPPDATA").unwrap())
        .join("Programs/HRIS Desktop");
    let config: ConnectionConfig =
        serde_json::from_slice(&std::fs::read(installed.join("hris-connection.json")).unwrap())
            .unwrap();
    assert_eq!(
        config.url.trim_end_matches('/'),
        format!("https://{target}.supabase.co")
    );
    let session: AuthSession =
        serde_json::from_str(&std::env::var("HRIS_BENEFITS_SESSION").unwrap()).unwrap();
    assert!(
        session.user.email.starts_with("qa-gov042-benefits068-")
            && session.user.email.ends_with("@example.invalid")
    );
    let roles: crate::staff_roles::StaffRoles =
        serde_json::from_str(&std::env::var("HRIS_BENEFITS_ROLES").unwrap()).unwrap();
    let case = std::env::var("HRIS_BENEFITS_CASE").unwrap();
    assert!(case.len() < 40 && case.bytes().all(|v| v.is_ascii_alphanumeric() || v == b'-'));
    let payroll = std::env::var("HRIS_BENEFITS_PAYROLL").as_deref() == Ok("true");
    let active = std::env::var("HRIS_BENEFITS_ACTIVE").as_deref() != Ok("false");
    let allowed = roles.allows(crate::staff_roles::Capability::Personnel) && active && !payroll;
    let mut client = client(config.url.clone(), &data(Role::Administrator));
    client.config = config;
    client.session = session;
    client.product = Some(if payroll {
        crate::desktop::Product::Payroll
    } else {
        crate::desktop::Product::Hris
    });
    client.http = Client::builder()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let proof = match client.current_access() {
        Ok(access) => {
            assert!(access.staff_roles().is_ok_and(|v| v == roles));
            let mut d = Snapshot::signed_out();
            d.company.id = access.company_id;
            d.access = access;
            d.mode = Mode::Supabase;
            let mut replay = false;
            if let Ok(raw) = std::env::var("HRIS_BENEFITS_REQUEST") {
                assert!(allowed);
                let r: Request = serde_json::from_str(&raw).unwrap();
                let context = client.benefit_context(&d).unwrap();
                r.validate(&context, &d.access).unwrap();
                match &r.change {
                    Change::Plan(p) => {
                        assert!(p.code.starts_with("QA-BN068-"));
                        if r.expected_revision > 0 {
                            assert!(
                                context
                                    .plan(r.subject_id)
                                    .unwrap()
                                    .data
                                    .code
                                    .starts_with("QA-BN068-")
                            );
                        }
                    }
                    Change::Enrollment(e) => {
                        assert!(
                            e.employee_source.employee_code.starts_with("QA-BN068-")
                                && e.plan_snapshot.code.starts_with("QA-BN068-")
                        );
                        if r.expected_revision > 0 {
                            assert!(
                                context
                                    .enrollment(r.subject_id)
                                    .unwrap()
                                    .data
                                    .employee_source
                                    .employee_code
                                    .starts_with("QA-BN068-")
                            );
                        }
                    }
                }
                let first = client
                    .execute_benefit(&d, Operation::Save(Box::new(r.clone())))
                    .unwrap();
                let next = client
                    .execute_benefit(&d, Operation::Save(Box::new(r)))
                    .unwrap();
                assert_eq!(first.receipt, next.receipt);
                assert_eq!(first.context, next.context);
                replay = true;
            }
            let result = client.benefit_context(&d);
            assert_eq!(
                result.is_ok(),
                allowed,
                "Native protected benefits admission mismatch"
            );
            if let Ok(c) = result {
                json!({"allowed":true,"current_roles_verified":true,"plans":c.plans.len(),"enrollments":c.enrollments.len(),"events":c.events.len(),"native_save_exact_retry":replay})
            } else {
                json!({"allowed":false,"current_roles_verified":true})
            }
        }
        Err(_) => {
            assert!(!allowed);
            json!({"allowed":false,"current_roles_verified":true})
        }
    };
    std::fs::write(
        root.join("work")
            .join(format!("benefits-native-{case}.json")),
        serde_json::to_vec(&proof).unwrap(),
    )
    .unwrap();
    println!("Verified protected synthetic native benefits context/save boundary.");
}
