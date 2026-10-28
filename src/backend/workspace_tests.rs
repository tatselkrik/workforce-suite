use super::government_export_tests::client;
use super::staff_roles_tests::{data, server};
use super::*;
use crate::{
    access::Role,
    workspaces::{Context as WorkspaceContext, Module, Receipt, Rejected, Save},
};
fn context(d: &Snapshot) -> Value {
    json!({"schema_version":1,"company_id":d.company.id,"user_id":d.access.user_id,"revision":0,"enabled":Module::ALL,"updated_at":null})
}
#[test]
fn workspace_read_accepts_only_current_bound_context_or_exact_old_server_absence() {
    let d = data(Role::Administrator);
    for case in 0..7 {
        let mut wire = context(&d);
        let status = match case {
            1 => {
                wire = json!({"code":"PGRST202","message":"Could not find public.desktop_workspace_context without parameters"});
                404
            }
            2 => {
                wire = json!({"code":"PGRST202","message":"Could not find another_function"});
                404
            }
            3 => {
                wire["company_id"] = json!(Uuid::new_v4());
                200
            }
            4 => {
                wire["schema_version"] = json!(99);
                200
            }
            5 => {
                wire["unexpected_authority"] = json!(true);
                200
            }
            6 => {
                wire = json!({"code":"42501"});
                403
            }
            _ => 200,
        };
        let (url, handle) = server(vec![(
            "/rest/v1/rpc/desktop_workspace_context",
            status,
            wire,
        )]);
        let result = client(url, &d).workspace_context(&d.access);
        if case <= 1 {
            let c = result.unwrap();
            assert_eq!(c.schema_version, if case == 0 { 1 } else { 0 });
            assert_eq!(c.enabled, Module::ALL);
        } else {
            assert!(result.is_err());
            if case == 6 {
                assert!(result.err().unwrap().is::<AccessRevoked>());
            }
        }
        assert_eq!(handle.join().unwrap().len(), 1);
    }
}
#[test]
fn exact_workspace_saves_distinguish_confirmed_rejection_from_uncertain_delivery() {
    let mut d = data(Role::Administrator);
    d.workspaces = serde_json::from_value::<WorkspaceContext>(context(&d)).unwrap();
    let request = Save::new(&d.workspaces, &d.access, &[]).unwrap();
    let receipt = Receipt {
        schema_version: 1,
        id: request.id,
        company_id: request.company_id,
        actor_id: request.actor_id,
        revision: 1,
        enabled: vec![],
        applied_at: Utc::now(),
    };
    for case in 0..5 {
        let (status, wire) = match case {
            1 => (400, json!({"code":"23514","message":"source changed"})),
            2 => (500, json!({"code":"P0001","message":"uncertain response"})),
            3 => (403, json!({"code":"42501"})),
            4 => {
                let mut v = serde_json::to_value(&receipt).unwrap();
                v["id"] = json!(Uuid::new_v4());
                (200, v)
            }
            _ => (200, serde_json::to_value(&receipt).unwrap()),
        };
        let (url, handle) = server(vec![("/rest/v1/rpc/save_desktop_workspaces", status, wire)]);
        let result = client(url, &d).save_workspaces(&request, &d);
        if case == 0 {
            assert_eq!(result.unwrap(), receipt)
        } else {
            let e = result.unwrap_err();
            assert_eq!(e.is::<Rejected>(), case == 1);
            if case == 3 {
                assert!(e.is::<AccessRevoked>())
            }
        }
        let calls = handle.join().unwrap();
        assert_eq!(calls[0].1, json!({"p_change":request}));
    }
}

#[test]
#[ignore = "requires synthetic managed Auth on the exact authorized installed project"]
fn hosted_native_workspace_save_and_retry() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target = std::fs::read_to_string(root.join("supabase/.temp/project-ref")).unwrap();
    let target = target.trim();
    let guard =
        std::fs::read_to_string(root.join("deployment-private/SQL-AUTHORIZATION.md")).unwrap();
    assert!(guard.contains(&format!("{}{}{}", char::from(96), target, char::from(96))));
    let profile = std::path::PathBuf::from(std::env::var("LOCALAPPDATA").unwrap())
        .join("Programs/HRIS Desktop/hris-connection.json");
    let config = ConnectionConfig::load_profile(&profile, true).unwrap();
    assert_eq!(
        config.url.trim_end_matches('/'),
        format!("https://{target}.supabase.co")
    );
    let session: AuthSession =
        serde_json::from_str(&std::env::var("HRIS_WORKSPACE_AUTH_SESSION").unwrap()).unwrap();
    assert!(
        session.user.email.starts_with("qa-gov042-")
            && session.user.email.ends_with("@example.invalid")
    );
    let expected: i64 = std::env::var("HRIS_WORKSPACE_EXPECTED_REVISION")
        .unwrap()
        .parse()
        .unwrap();
    let seed = data(Role::Administrator);
    let mut cloud = client(config.url.clone(), &seed);
    cloud.config = config;
    cloud.session = session;
    cloud.http = Client::builder()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let before = cloud.snapshot().unwrap();
    assert_eq!(before.workspaces.schema_version, 1);
    assert_eq!(before.workspaces.revision, expected);
    let request = Save::new(
        &before.workspaces,
        &before.access,
        &before.workspaces.enabled,
    )
    .unwrap();
    let receipt = cloud.save_workspaces(&request, &before).unwrap();
    receipt.validate(&request).unwrap();
    let after = cloud.snapshot().unwrap();
    assert_eq!(after.workspaces.revision, receipt.revision);
    assert_eq!(after.workspaces.enabled, request.enabled);
    let replay = cloud.save_workspaces(&request, &after).unwrap();
    assert_eq!(receipt, replay);
    let proof = json!({"saved_and_replayed":true,"previous_revision":expected,"revision":receipt.revision,"enabled_count":request.enabled.len()});
    std::fs::write(
        root.join("work/workspaces064-native-save-proof.json"),
        serde_json::to_vec_pretty(&proof).unwrap(),
    )
    .unwrap();
}
