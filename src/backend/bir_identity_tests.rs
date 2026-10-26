//! Real local HTTP adapter; hosted evidence is a separate opt-in.
use super::government_export_tests::{access_json, client};
use super::*;
use crate::{
    access::Role,
    payroll_government::bir_identity::{self as bi, tests as fixture},
};
use std::{
    io::{BufRead, BufReader, Write},
    net::TcpListener,
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scenario {
    Good,
    RevokedAfter,
    ForeignBinding,
    NoEmployer,
    StaleHash,
}
fn server(data: &Snapshot, scenario: Scenario) -> (String, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let access = access_json(&data.access);
    let mut source = fixture::wire();
    if scenario == Scenario::ForeignBinding {
        source.people[0].registration.as_mut().unwrap().company_id = Uuid::new_v4();
    }
    if scenario == Scenario::NoEmployer {
        source.employer = None;
    }
    let source = serde_json::to_value(source).unwrap();
    let handle = thread::spawn(move || {
        let count = if matches!(scenario, Scenario::Good | Scenario::RevokedAfter) {
            3
        } else {
            2
        };
        let mut paths = vec![];
        let until = std::time::Instant::now() + Duration::from_secs(6);
        while paths.len() < count {
            assert!(
                std::time::Instant::now() < until,
                "Identity HTTP fixture timed out"
            );
            let mut socket = match listener.accept() {
                Ok((s, _)) => s,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(2));
                    continue;
                }
                Err(e) => panic!("{e}"),
            };
            socket.set_nonblocking(false).unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut reader = BufReader::new(socket.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let path = line.split_whitespace().nth(1).unwrap().to_owned();
            let mut length = 0usize;
            loop {
                line.clear();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
                if let Some((k, v)) = line.split_once(':') {
                    if k.eq_ignore_ascii_case("content-length") {
                        length = v.trim().parse().unwrap();
                    }
                }
            }
            assert!(length < 1024);
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            let n = paths.len();
            if n == 1 {
                assert_eq!(path, "/rest/v1/rpc/government_bir_identities");
                assert_eq!(
                    serde_json::from_slice::<Value>(&body).unwrap(),
                    json!({"p_year":2026})
                );
            } else {
                assert_eq!(path, "/rest/v1/rpc/my_hris_access");
            }
            paths.push(path);
            let denied = scenario == Scenario::RevokedAfter && n == 2;
            let payload = serde_json::to_vec(if denied {
                &Value::Null
            } else if n == 1 {
                &source
            } else {
                &access
            })
            .unwrap();
            write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",payload.len()).unwrap();
            socket.write_all(&payload).unwrap();
        }
        paths
    });
    (url, handle)
}
#[test]
fn declaration_adapter_uses_fresh_scoped_wire_and_preserves_new_file_guards() {
    use sha2::Digest;
    let mut data=DemoStore::new().data;data.access.user_id=Uuid::from_u128(4);
    let folder=std::env::temp_dir().join(format!("hris-declarations-http-{}",Uuid::new_v4()));
    std::fs::create_dir(&folder).unwrap();let source=folder.join(fixture::FILE_NAME);
    let raw=fixture::source(&["234567890"]);std::fs::write(&source,&raw).unwrap();
    let hash=format!("{:x}",sha2::Sha256::digest(&raw));
    for scenario in [Scenario::Good,Scenario::RevokedAfter,Scenario::ForeignBinding,Scenario::NoEmployer,Scenario::StaleHash] {
        for extension in ["csv","xlsx"] {
            let (url,server)=server(&data,scenario);let mut cloud=client(url,&data);
            let path=folder.join(format!("{scenario:?}.{extension}"));
            let result=cloud.government_bir_declarations(&data,2026,&source,
                Some(if scenario==Scenario::StaleHash{"stale"}else{&hash}),Some(&path));
            server.join().unwrap();
            if scenario==Scenario::Good {
                let report=result.unwrap();assert_eq!(report.rows[0].sources.len(),7);
                assert_eq!(report.rows[0].declared.len(),31);assert!(path.exists());
            } else {assert!(result.is_err());assert!(!path.exists());}
        }
    }
    let existing=folder.join("preserve.csv");std::fs::write(&existing,b"preserve existing export").unwrap();
    let (url,server)=server(&data,Scenario::Good);let mut cloud=client(url,&data);
    assert!(cloud.government_bir_declarations(&data,2026,&source,Some(&hash),Some(&existing)).is_err());
    server.join().unwrap();assert_eq!(std::fs::read(&existing).unwrap(),b"preserve existing export");
    for role in [Role::Manager,Role::Employee] {
        data.access.role=role;let mut cloud=client("http://127.0.0.1:1".into(),&data);
        assert!(cloud.government_bir_declarations(&data,2026,&source,Some(&hash),Some(&folder.join("denied.csv"))).is_err());
        assert!(!folder.join("denied.csv").exists());
    }
    assert_eq!(std::fs::read(&source).unwrap(),raw);
    std::fs::remove_dir_all(folder).unwrap();
}
#[test]
fn real_identity_adapter_protects_fresh_hash_role_roster_and_new_file_io() {
    use sha2::Digest;
    let mut data = DemoStore::new().data;
    data.access.user_id = Uuid::from_u128(4);
    let folder = std::env::temp_dir().join(format!("hris-identity-http-{}", Uuid::new_v4()));
    std::fs::create_dir(&folder).unwrap();
    let source = folder.join(fixture::FILE_NAME);
    let raw = fixture::source(&["234567890"]);
    std::fs::write(&source, &raw).unwrap();
    let hash = format!("{:x}", sha2::Sha256::digest(&raw));
    for scenario in [
        Scenario::Good,
        Scenario::RevokedAfter,
        Scenario::ForeignBinding,
        Scenario::NoEmployer,
        Scenario::StaleHash,
    ] {
        for extension in ["csv", "xlsx"] {
            let (url, server) = server(&data, scenario);
            let mut cloud = client(url, &data);
            let path = folder.join(format!("{scenario:?}.{extension}"));
            let result = cloud.government_bir_identities(
                &data,
                2026,
                &source,
                Some(if scenario == Scenario::StaleHash {
                    "stale"
                } else {
                    &hash
                }),
                Some(&path),
            );
            server.join().unwrap();
            if scenario == Scenario::Good {
                assert_eq!(
                    result.unwrap().rows[0].status,
                    bi::Status::CurrentPairMatched
                );
                assert!(path.exists());
            } else {
                assert!(result.is_err());
                assert!(!path.exists());
            }
            assert_eq!(std::fs::read(&source).unwrap(), raw);
        }
    }
    std::fs::remove_dir_all(folder).unwrap();
}
#[test]
fn identity_staff_and_unchecked_export_are_denied_before_network_or_file_read() {
    for role in [Role::Employee, Role::Manager, Role::Administrator] {
        let mut data = DemoStore::new().data;
        data.access.role = role;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let mut cloud = client(format!("http://{}", listener.local_addr().unwrap()), &data);
        assert!(
            cloud
                .government_bir_identities(
                    &data,
                    2026,
                    Path::new("absent.dat"),
                    None,
                    Some(Path::new("absent.xlsx"))
                )
                .is_err()
        );
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }
}
#[test]
fn identity_export_keeps_existing_destination_unchanged() {
    use sha2::Digest;
    let mut data = DemoStore::new().data;
    data.access.user_id = Uuid::from_u128(4);
    let folder = std::env::temp_dir().join(format!("hris-identity-existing-{}", Uuid::new_v4()));
    std::fs::create_dir(&folder).unwrap();
    let source = folder.join(fixture::FILE_NAME);
    let raw = fixture::source(&["234567890"]);
    std::fs::write(&source, &raw).unwrap();
    let hash = format!("{:x}", sha2::Sha256::digest(&raw));
    let destination = folder.join("existing.csv");
    std::fs::write(&destination, b"PRESERVE").unwrap();
    let (url, server) = server(&data, Scenario::Good);
    let mut cloud = client(url, &data);
    assert!(
        cloud
            .government_bir_identities(&data, 2026, &source, Some(&hash), Some(&destination))
            .is_err()
    );
    server.join().unwrap();
    assert_eq!(std::fs::read(destination).unwrap(), b"PRESERVE");
    assert_eq!(std::fs::read(source).unwrap(), raw);
    std::fs::remove_dir_all(folder).unwrap();
}

#[test]
#[ignore = "Explicit synthetic Auth session for the installed, target-guarded HRIS project only."]
fn hosted_native_identity_projection_for_authorized_installation() {
    use sha2::Digest;
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .canonicalize()
        .unwrap();
    let guard =
        std::fs::read_to_string(root.join("deployment-private/SQL-AUTHORIZATION.md")).unwrap();
    let reference = guard
        .split("Authorized Supabase project reference: `")
        .nth(1)
        .and_then(|s| s.split('`').next())
        .expect("Existing private target authorization");
    assert!(
        (16..=32).contains(&reference.len()) && reference.bytes().all(|b| b.is_ascii_lowercase())
    );
    let installed = std::path::PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap())
        .join("Programs/HRIS Desktop");
    let config =
        ConnectionConfig::load_profile(&installed.join("hris-connection.json"), true).unwrap();
    assert_eq!(
        config.url.trim_end_matches('/'),
        format!("https://{reference}.supabase.co")
    );
    assert_eq!(
        std::fs::read_to_string(root.join("supabase/.temp/project-ref"))
            .unwrap()
            .trim(),
        reference
    );
    let session: AuthSession = serde_json::from_str(
        &std::env::var("HRIS_IDENTITY_AUTH_SESSION")
            .expect("Explicit short-lived synthetic session"),
    )
    .unwrap();
    assert!(
        session.user.email.starts_with("qa-gov042-")
            && session.user.email.ends_with("@example.invalid")
    );
    let mut data = DemoStore::new().data;
    data.mode = Mode::Supabase;
    let mut cloud = client(config.url.clone(), &data);
    cloud.config = config;
    cloud.session = session;
    cloud.http = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap();
    data.access = cloud.current_access().unwrap();
    assert_eq!(data.access.user_id, cloud.session.user.id);
    assert!(data.access.role.manages_hr());
    data.company.id = data.access.company_id;
    let work = root.join("work").canonicalize().unwrap();
    assert!(work.starts_with(&root));
    let role = if data.access.role == Role::Administrator {
        "administrator"
    } else {
        "hr_generalist"
    };
    let folder = work.join(format!("identities058-live-{role}"));
    std::fs::create_dir(&folder).unwrap();
    let mut summaries = vec![];
    let mut employer_ready = false;
    for year in [2025, 2026] {
        let before = cloud.current_access().unwrap();
        assert_eq!(before, data.access);
        let wire: bi::Wire = decode(
            cloud
                .request(Method::POST, "rpc/government_bir_identities")
                .unwrap()
                .json(&json!({"p_year":year}))
                .send()
                .unwrap(),
        )
        .unwrap();
        let basis = bi::Basis::from_wire(wire, before.company_id, year).unwrap();
        assert_eq!(cloud.current_access().unwrap(), before);
        if year == 2026 {
            employer_ready = basis.employer.as_ref().is_some_and(|e| {
                crate::payroll_government::bir_file::matchable_identity(&e.tin, &e.branch)
            });
        }
        summaries.push(json!({"year":year,"roster_people":basis.people.len(),"source_people":basis.ledger.wire.employees.len()}));
        // Never persist the hosted roster, tax IDs, names or registration history.
    }
    if !employer_ready {
        let raw = fixture::source(&["234567890"]);
        let source = folder.join(fixture::FILE_NAME);
        std::fs::write(&source, &raw).unwrap();
        let hash = format!("{:x}", sha2::Sha256::digest(&raw));
        let destination = folder.join("must-not-be-created.csv");
        let result =
            cloud.government_bir_identities(&data, 2026, &source, Some(&hash), Some(&destination));
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Complete the company's current BIR TIN")
        );
        assert!(!destination.exists());
        assert_eq!(std::fs::read(&source).unwrap(), raw);
        std::fs::remove_file(source).unwrap();
    }
    let proof = json!({"all_passed":true,"role":role,"snapshots":summaries,"employer_ready":employer_ready,
        "missing_configuration_export_guard_exercised":!employer_ready,"fixed_authorized_project_and_local_evidence":true,
        "hosted_identity_data_persisted":false,"hosted_full_file_matching_verified":false,"official_acceptance":false});
    std::fs::write(
        folder.join("proof.json"),
        serde_json::to_vec(&proof).unwrap(),
    )
    .unwrap();
}
