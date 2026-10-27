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
    hosted_native_bir_projection("identities");
}
#[test]
#[ignore = "Explicit synthetic Auth session for the installed, target-guarded HRIS project only."]
fn hosted_native_declaration_projection_for_authorized_installation() {
    hosted_native_bir_projection("declarations");
}
#[test]
#[ignore = "Explicit synthetic Auth session for installed, target-guarded HRIS pay review only."]
fn hosted_native_annual_pay_for_authorized_installation() {
    hosted_native_bir_projection("paydetails");
}
#[test]
#[ignore = "Explicit synthetic session for fixed authorized installation and local arithmetic evidence only."]
fn hosted_native_compensation_arithmetic_for_authorized_installation() {
    hosted_native_bir_projection("arithmetic");
}
#[test]
#[ignore = "Explicit synthetic session for fixed authorized installation and local builder evidence only."]
fn hosted_native_bir_builder_for_authorized_installation() {
    hosted_native_bir_projection("builder");
}
fn hosted_native_bir_projection(mode: &str) {
    assert!(matches!(mode,"identities"|"declarations"|"paydetails"|"arithmetic"|"builder"));
    let declarations=mode=="declarations";
    let pay=mode=="paydetails";
    let arithmetic=mode=="arithmetic";
    let building=mode=="builder";
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
        &std::env::var(if building {"HRIS_BUILDER_AUTH_SESSION"}else if arithmetic {"HRIS_ARITHMETIC_AUTH_SESSION"}else if pay {"HRIS_PAYDETAIL_AUTH_SESSION"}else if declarations {"HRIS_DECLARATION_AUTH_SESSION"} else {"HRIS_IDENTITY_AUTH_SESSION"})
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
    let folder = work.join(format!("{}-live-{role}", if building {"builder062-native"}else if arithmetic {"arithmetic061-native"}else if pay {"paydetails060-verified"}else if declarations {"declarations059"} else {"identities058"}));
    std::fs::create_dir(&folder).unwrap();
    if building {
        use crate::payroll_government::bir_file::{self as bir,builder};
        let registration=cloud.government_registration(&data,None,None).unwrap();
        let ready=registration.record.as_ref().is_some_and(|r|
            bir::matchable_identity(r.details.get("bir_tin").map_or("",String::as_str),
                r.details.get("bir_branch").map_or("",String::as_str)));
        let mut cases=vec![];
        for (number,schedule) in [(1,bir::Schedule::One),(2,bir::Schedule::Two)] {
            for excel in [false,true] {
                let kind=if excel{"xlsx"}else{"csv"};
                let case=folder.join(format!("s{number}-{kind}"));std::fs::create_dir(&case).unwrap();
                let blank=case.join(format!("blank-template.{kind}"));
                assert!(cloud.government_bir_build(&data,&builder::Action::Template{schedule,destination:blank.clone()}).unwrap().is_none());
                let actual=std::fs::read(blank).unwrap();
                if excel {
                    // Workbook creation timestamps are not template declarations.
                    let rows=crate::tabular::read_xlsx(&actual,bir::MAX_DETAILS,builder::headers(schedule).len()).unwrap();
                    assert_eq!(rows.len(),1);
                    assert!(rows[0].cells.iter().map(String::as_str).eq(builder::headers(schedule).iter().copied()));
                }else{assert_eq!(actual,builder::template(schedule,false).unwrap());}
                let source=root.join(format!("work/bir062-vectors/schedule{number}-{kind}/declarations.{kind}"));
                let before=std::fs::read(&source).unwrap();
                let checked=cloud.government_bir_build(&data,&builder::Action::Prepare {
                    selection:builder::Selection{schedule,year:2026},source:source.clone(),
                    expected_basis:None,destination:None,reviewed:false,
                });
                if ready {
                    let preview=checked.unwrap().unwrap();
                    assert_eq!((preview.company_id,preview.user_id),(data.company.id,data.access.user_id));
                    assert_eq!(preview.source_sha256,format!("{:x}",sha2::Sha256::digest(&before)));
                    assert_eq!(preview.details,2);
                }else{
                    let error=checked.err().expect("Incomplete employer must not build a draft").to_string();
                    assert!(error.contains("BIR registration")||error.contains("protected BIR registration"));
                }
                assert_eq!(before,std::fs::read(source).unwrap());
                cases.push(json!({"schedule":number,"format":kind,"blank_template_matches":true,
                    "preview_built":ready,"incomplete_registration_blocked":!ready,"source_unchanged":true}));
            }
        }
        std::fs::write(folder.join("proof.json"),serde_json::to_vec_pretty(&json!({
            "all_passed":true,"cases":cases,"configured_employer_complete":ready,
            "full_positive_builder_hosted":ready,"fixed_authorized_project_and_local_evidence":true,
            "hosted_identity_data_persisted":false,"official_validation":false,"source_files_uploaded":false,
        })).unwrap()).unwrap();
        return;
    }
    if arithmetic {
        use crate::payroll_government::bir_file as bir;
        let mut cases=vec![];
        for (name,schedule,different) in [("s1-match",bir::Schedule::One,false),("s1-cent",bir::Schedule::One,true),
            ("s2-match",bir::Schedule::Two,false),("s2-cent",bir::Schedule::Two,true)] {
            let dir=folder.join(name);std::fs::create_dir(&dir).unwrap();
            let source=dir.join("0123456780000123120261604C.DAT");
            let raw=bir::arithmetic::fictional_source(schedule,different);std::fs::write(&source,&raw).unwrap();
            let hash=format!("{:x}",sha2::Sha256::digest(&raw));
            let r=cloud.government_bir_check(&data,&source,None,None).unwrap();
            assert_eq!(r.errors,0,"Configured employer mismatch must remain a blocking finding; never manufacture employer facts for QA.");
            assert_eq!(r.arithmetic.differences,usize::from(different));assert!(r.arithmetic.checked());
            for ext in ["csv","xlsx"] {
                let checked=cloud.government_bir_check(&data,&source,Some(&hash),Some(&dir.join(format!("findings.{ext}")))).unwrap();
                assert_eq!(checked.arithmetic,r.arithmetic);
            }
            assert_eq!(std::fs::read(&source).unwrap(),raw);
            cases.push(json!({"case":name,"schedule":schedule,"source_sha256":hash,"source_unchanged":true,
                "structural_errors":r.errors,"employer_match":r.employer_match,"arithmetic":r.arithmetic}));
        }
        assert_eq!(cloud.current_access().unwrap(),data.access);
        let proof=json!({"all_passed":true,"role":role,"cases":cases,
            "fixed_authorized_project_and_local_evidence":true,"hosted_identity_data_persisted":false,
            "official_acceptance":false});
        std::fs::write(folder.join("proof.json"),serde_json::to_vec(&proof).unwrap()).unwrap();
        return;
    }
    let mut summaries = vec![];
    let mut employer_ready = false;
    for year in [2025, 2026] {
        let before = cloud.current_access().unwrap();
        assert_eq!(before, data.access);
        if pay {
            let r=cloud.government_annual_pay_details(&data,year,None).unwrap();
            let mut exports=vec![];
            for ext in ["csv","xlsx"] {
                let bytes=r.export(std::path::Path::new(&format!("memory.{ext}"))).unwrap();
                exports.push(json!({"format":ext,"bytes":bytes.len(),"sha256":format!("{:x}",sha2::Sha256::digest(&bytes))}));
            }
            assert_eq!(cloud.current_access().unwrap(),before);
            summaries.push(json!({"year":year,"sources":r.ledger.wire.entries.len(),"details":r.details.len(),"in_memory_exports":exports}));
            continue;
        }
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
    if !employer_ready && !pay {
        let raw = fixture::source(&["234567890"]);
        let source = folder.join(fixture::FILE_NAME);
        std::fs::write(&source, &raw).unwrap();
        let hash = format!("{:x}", sha2::Sha256::digest(&raw));
        let destination = folder.join("must-not-be-created.csv");
        let result = if declarations {
            cloud.government_bir_declarations(&data, 2026, &source, Some(&hash), Some(&destination)).map(|_| ())
        } else {
            cloud.government_bir_identities(&data, 2026, &source, Some(&hash), Some(&destination)).map(|_| ())
        };
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
    let proof = json!({"all_passed":true,"role":role,"snapshots":summaries,"employer_ready":employer_ready,"declaration_adapter":declarations,"pay_details_adapter":pay,
        "missing_configuration_export_guard_exercised":!employer_ready&&!pay,"fixed_authorized_project_and_local_evidence":true,
        "hosted_identity_data_persisted":false,"hosted_full_file_matching_verified":false,"official_acceptance":false});
    std::fs::write(
        folder.join("proof.json"),
        serde_json::to_vec(&proof).unwrap(),
    )
    .unwrap();
}
