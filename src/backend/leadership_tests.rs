use super::government_export_tests::{access_json, client};
use super::staff_roles_tests::{data, server};
use super::*;
use crate::{
    access::Role,
    leadership::{self, Report},
};
fn fixture() -> (Snapshot, Report) {
    let d = data(Role::Administrator);
    let r = leadership::preview(&d);
    r.validate(&d.access, &r.filter).unwrap();
    (d, r)
}
#[test]
fn report_read_checks_current_access_on_both_sides() {
    let (d, r) = fixture();
    let a = access_json(&d.access);
    let (url, join) = server(vec![
        ("/rest/v1/rpc/my_hris_access", 200, a.clone()),
        ("/rest/v1/rpc/hr_leadership_report", 200, json!(r)),
        ("/rest/v1/rpc/my_hris_access", 200, a),
    ]);
    let report = client(url, &d)
        .leadership_report(&d, &r.filter, None)
        .unwrap();
    assert_eq!(report, r);
    let sent = join.join().unwrap();
    assert_eq!(
        sent[1].1,
        json!({"p_start":r.filter.starts_on,"p_end":r.filter.ends_on})
    );
}
#[test]
fn hr_and_manager_are_denied_before_requesting_company_aggregates() {
    for role in [Role::HrGeneralist, Role::Manager] {
        let (d, _) = fixture();
        let mut current = d.access.clone();
        current.role = role;
        let (url, join) = server(vec![(
            "/rest/v1/rpc/my_hris_access",
            200,
            access_json(&current),
        )]);
        assert!(
            client(url, &d)
                .leadership_report(&d, &leadership::Filter::new(d.company.today()), None)
                .unwrap_err()
                .is::<AccessRevoked>()
        );
        assert_eq!(join.join().unwrap().len(), 1);
    }
}
#[test]
fn payroll_product_is_rejected_before_network_io() {
    let (d, r) = fixture();
    let mut c = client("http://127.0.0.1:1".into(), &d);
    c.product = Some(crate::desktop::Product::Payroll);
    assert!(c.leadership_report(&d, &r.filter, None).is_err());
}
#[test]
fn revocation_during_read_or_render_prevents_export() {
    let (d, r) = fixture();
    for after_render in [false, true] {
        let a = access_json(&d.access);
        let mut revoked = d.access.clone();
        revoked.role = Role::Manager;
        let mut replies = vec![
            ("/rest/v1/rpc/my_hris_access", 200, a.clone()),
            ("/rest/v1/rpc/hr_leadership_report", 200, json!(r)),
        ];
        if after_render {
            replies.push(("/rest/v1/rpc/my_hris_access", 200, a));
        }
        replies.push(("/rest/v1/rpc/my_hris_access", 200, access_json(&revoked)));
        let (url, join) = server(replies);
        let p = std::env::temp_dir().join(format!("hr-leadership-{}.csv", Uuid::new_v4()));
        assert!(
            client(url, &d)
                .leadership_report(&d, &r.filter, Some(&p))
                .unwrap_err()
                .is::<AccessRevoked>()
        );
        assert!(!p.exists());
        join.join().unwrap();
    }
}
#[test]
fn malformed_foreign_or_extra_private_projection_is_not_exported() {
    let (d, r) = fixture();
    for case in 0..4 {
        let mut v = json!(r);
        match case {
            0 => v["user_id"] = json!(Uuid::new_v4()),
            1 => v["company_id"] = json!(Uuid::new_v4()),
            2 => v["workforce"]["active"] = json!(0),
            _ => v["private_notes"] = json!("Never allowed"),
        };
        let (url, join) = server(vec![
            ("/rest/v1/rpc/my_hris_access", 200, access_json(&d.access)),
            ("/rest/v1/rpc/hr_leadership_report", 200, v),
        ]);
        assert!(
            client(url, &d)
                .leadership_report(&d, &r.filter, None)
                .is_err()
        );
        join.join().unwrap();
    }
}
#[test]
fn fresh_export_checks_authority_after_render_and_preserves_existing_file() {
    let (d, r) = fixture();
    let a = access_json(&d.access);
    let p = std::env::temp_dir().join(format!("hr-leadership-{}.xlsx", Uuid::new_v4()));
    std::fs::write(&p, b"keep existing bytes").unwrap();
    let (url, join) = server(vec![
        ("/rest/v1/rpc/my_hris_access", 200, a.clone()),
        ("/rest/v1/rpc/hr_leadership_report", 200, json!(r)),
        ("/rest/v1/rpc/my_hris_access", 200, a.clone()),
        ("/rest/v1/rpc/my_hris_access", 200, a),
    ]);
    assert!(
        client(url, &d)
            .leadership_report(&d, &r.filter, Some(&p))
            .is_err()
    );
    assert_eq!(std::fs::read(&p).unwrap(), b"keep existing bytes");
    std::fs::remove_file(&p).unwrap();
    assert_eq!(join.join().unwrap().len(), 4);
}
#[test]
fn server_forbidden_clears_access_instead_of_returning_cached_totals() {
    let (d, r) = fixture();
    let (url, join) = server(vec![
        ("/rest/v1/rpc/my_hris_access", 200, access_json(&d.access)),
        (
            "/rest/v1/rpc/hr_leadership_report",
            403,
            json!({"message":"Forbidden"}),
        ),
    ]);
    assert!(
        client(url, &d)
            .leadership_report(&d, &r.filter, None)
            .unwrap_err()
            .is::<AccessRevoked>()
    );
    join.join().unwrap();
}

#[test]
#[ignore = "managed synthetic Auth on the exact installed prototype only"]
fn hosted_native_leadership_projection() {
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
        serde_json::from_str(&std::env::var("HRIS_LEADERSHIP_SESSION").unwrap()).unwrap();
    assert!(
        session.user.email.starts_with("qa-gov042-leadership069-")
            && session.user.email.ends_with("@example.invalid")
    );
    let roles: crate::staff_roles::StaffRoles =
        serde_json::from_str(&std::env::var("HRIS_LEADERSHIP_ROLES").unwrap()).unwrap();
    let case = std::env::var("HRIS_LEADERSHIP_CASE").unwrap();
    assert!(case.len() < 40 && case.bytes().all(|v| v.is_ascii_alphanumeric() || v == b'-'));
    let payroll = std::env::var("HRIS_LEADERSHIP_PAYROLL").as_deref() == Ok("true");
    let active = std::env::var("HRIS_LEADERSHIP_ACTIVE").as_deref() != Ok("false");
    let allowed =
        roles.allows(crate::staff_roles::Capability::AccountAdministration) && active && !payroll;
    let mut c = client(config.url.clone(), &data(Role::Administrator));
    c.config = config;
    c.session = session;
    c.product = Some(if payroll {
        crate::desktop::Product::Payroll
    } else {
        crate::desktop::Product::Hris
    });
    c.http = Client::builder()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let mut exported = false;
    let proof = match c.current_access() {
        Ok(access) => {
            assert!(access.staff_roles().is_ok_and(|v| v == roles));
            let mut d = Snapshot::signed_out();
            d.company.id = access.company_id;
            d.company.timezone = "Asia/Manila".into();
            d.access = access;
            d.mode = Mode::Supabase;
            let filter = leadership::Filter::new(d.company.today());
            let report = c.leadership_report(&d, &filter, None);
            assert_eq!(
                report.is_ok(),
                allowed,
                "Native protected leadership admission mismatch"
            );
            if let Ok(report) = report {
                let metadata = serde_json::to_value(&report).unwrap();
                assert!(!metadata.to_string().contains("PRIVATE_"));
                for ext in ["csv", "xlsx"] {
                    let path = root
                        .join("work")
                        .join(format!("leadership-native-{case}.{ext}"));
                    let r = c.leadership_report(&d, &filter, Some(&path)).unwrap();
                    assert_eq!(r.workforce, report.workforce);
                    assert!(path.metadata().unwrap().len() > 100);
                }
                exported = true;
                json!({"allowed":true,"current_roles_verified":true,"native_csv_and_xlsx":true,"schema_version":report.schema_version,"months":report.months.len(),"groups":report.groups.len(),"raw_personal_data_projected":false})
            } else {
                json!({"allowed":false,"current_roles_verified":true})
            }
        }
        Err(_) => {
            assert!(!allowed);
            json!({"allowed":false,"current_roles_verified":true})
        }
    };
    assert_eq!(exported, allowed);
    std::fs::write(
        root.join("work")
            .join(format!("leadership-native-{case}.json")),
        serde_json::to_vec(&proof).unwrap(),
    )
    .unwrap();
    println!("Verified protected native aggregate leadership boundary.");
}
