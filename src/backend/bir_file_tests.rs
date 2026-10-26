//! Real local HTTP client tests. Fictional registration responses are not hosted Auth evidence.
use super::government_export_tests::{access_json, client};
use super::*;
use crate::{access::Role, payroll_government::bir_file as bir};
use sha2::Digest;
use std::{
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    path::PathBuf,
};

fn source() -> Vec<u8> {
    let mut d = vec![String::new(); 49];
    for (i, s) in [
        (0, "D1"),
        (1, "1604C"),
        (2, "012345678"),
        (3, "0000"),
        (4, "12/31/2026"),
        (5, "1"),
        (6, "234567890"),
        (7, "0000"),
        (8, "PRIVATE FAMILY"),
        (9, "PRIVATE GIVEN"),
        (11, "NCR"),
        (23, "01/01/2026"),
        (24, "12/31/2026"),
        (44, "FILIPINO"),
        (45, "R"),
        (46, "NA"),
        (47, "N"),
    ] {
        d[i] = s.into();
    }
    for &i in &bir::MONEY_COLUMNS {
        d[i] = "0.00".into();
    }
    let mut c = vec!["C1", "1604C", "012345678", "0000", "12/31/2026"];
    c.extend(["0.00"; 31]);
    format!(
        "H1604C,012345678,0000,12/31/2026\r\n{}\r\n{}\r\n",
        d.join(","),
        c.join(",")
    )
    .into_bytes()
}
fn folder() -> (PathBuf, PathBuf, Vec<u8>) {
    let root = std::env::temp_dir().join(format!("hris-bir-http-{}", Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let p = root.join("0123456780000123120261604C.DAT");
    let raw = source();
    std::fs::write(&p, &raw).unwrap();
    (root, p, raw)
}
fn server(
    data: &Snapshot,
    count: usize,
    revoked: Option<usize>,
    foreign: bool,
) -> (String, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let access = access_json(&data.access);
    let registration = json!({"version":"government-registration-v1","company_id":if foreign{Uuid::new_v4()}else{data.company.id},
        "employee_id":null,"record":null,"history":[]});
    let handle = thread::spawn(move || {
        let mut paths = Vec::new();
        let until = std::time::Instant::now() + Duration::from_secs(6);
        while paths.len() < count {
            assert!(
                std::time::Instant::now() < until,
                "BIR client request did not arrive"
            );
            let mut socket = match listener.accept() {
                Ok((s, _)) => s,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(2));
                    continue;
                }
                Err(e) => panic!("Accept: {e}"),
            };
            socket.set_nonblocking(false).unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut reader = BufReader::new(socket.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let path = line.split_whitespace().nth(1).unwrap().to_owned();
            let mut length = 0;
            loop {
                line.clear();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
                if let Some((k, v)) = line.split_once(':') {
                    if k.eq_ignore_ascii_case("content-length") {
                        length = v.trim().parse::<usize>().unwrap();
                    }
                }
            }
            assert!(length < 1024);
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            let n = paths.len();
            assert_eq!(
                path,
                if n == 1 {
                    "/rest/v1/rpc/government_registration"
                } else {
                    "/rest/v1/rpc/my_hris_access"
                }
            );
            assert_eq!(
                serde_json::from_slice::<Value>(&body).unwrap(),
                if n == 1 {
                    json!({"p_employee_id":null})
                } else {
                    json!({})
                },
                "No source rows may be uploaded"
            );
            paths.push(path);
            let denied = revoked == Some(n);
            let denial = json!({"message":"Fixture membership revoked"});
            let payload = serde_json::to_vec(if denied {
                &denial
            } else if n == 1 {
                &registration
            } else {
                &access
            })
            .unwrap();
            let status = if denied { "403 Forbidden" } else { "200 OK" };
            write!(socket,"HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",payload.len()).unwrap();
            socket.write_all(&payload).unwrap();
        }
        paths
    });
    (url, handle)
}

#[test]
fn bir_actual_client_exports_private_minimal_findings_without_uploading_source_or_changing_it() {
    let mut data = DemoStore::new().data;
    data.mode = Mode::Supabase;
    let (root, p, raw) = folder();
    let hash = format!("{:x}", sha2::Sha256::digest(&raw));
    for extension in ["csv", "xlsx"] {
        let (url, handle) = server(&data, 4, None, false);
        let mut c = client(url, &data);
        let destination = root.join(format!("findings.{extension}"));
        let report = c
            .government_bir_check(&data, &p, Some(&hash), Some(&destination))
            .unwrap();
        assert_eq!(handle.join().unwrap().len(), 4);
        assert_eq!(
            (report.errors, report.reviews, report.employer_match),
            (0, 1, None)
        );
        let content = std::fs::read(&destination).unwrap();
        assert!(content.starts_with(if extension == "xlsx" {
            b"PK"
        } else {
            b"\xef\xbb\xbf"
        }));
        if extension == "csv" {
            let s = String::from_utf8(content).unwrap();
            for private in ["PRIVATE FAMILY", "PRIVATE GIVEN", "234567890", "012345678"] {
                assert!(!s.contains(private));
            }
        }
        assert_eq!(std::fs::read(&p).unwrap(), raw);
        std::fs::remove_file(destination).unwrap();
    }
    std::fs::remove_file(p).unwrap();
    std::fs::remove_dir(root).unwrap();
}
#[test]
fn bir_actual_client_revocation_before_read_after_registration_or_before_file_io_blocks_exports() {
    let data = DemoStore::new().data;
    let (root, p, raw) = folder();
    let hash = format!("{:x}", sha2::Sha256::digest(&raw));
    for n in [0, 2, 3] {
        let (url, handle) = server(&data, n + 1, Some(n), false);
        let mut c = client(url, &data);
        let out = root.join(format!("denied-{n}.xlsx"));
        assert!(
            c.government_bir_check(&data, &p, Some(&hash), Some(&out))
                .is_err()
        );
        assert_eq!(handle.join().unwrap().len(), n + 1);
        assert!(!out.exists());
        assert_eq!(std::fs::read(&p).unwrap(), raw);
    }
    std::fs::remove_file(p).unwrap();
    std::fs::remove_dir(root).unwrap();
}
#[test]
fn bir_actual_client_rejects_changed_source_foreign_registration_and_existing_output() {
    let data = DemoStore::new().data;
    let (root, p, raw) = folder();
    let hash = format!("{:x}", sha2::Sha256::digest(&raw));
    let out = root.join("findings.csv");
    let (url, handle) = server(&data, 3, None, false);
    let mut c = client(url, &data);
    assert!(
        c.government_bir_check(&data, &p, Some(&"0".repeat(64)), Some(&out))
            .unwrap_err()
            .to_string()
            .contains("source file changed")
    );
    handle.join().unwrap();
    assert!(!out.exists());
    let (url, handle) = server(&data, 2, None, true);
    let mut c = client(url, &data);
    assert!(
        c.government_bir_check(&data, &p, Some(&hash), Some(&out))
            .unwrap_err()
            .to_string()
            .contains("protected scope")
    );
    handle.join().unwrap();
    assert!(!out.exists());
    std::fs::write(&out, b"existing work").unwrap();
    let (url, handle) = server(&data, 4, None, false);
    let mut c = client(url, &data);
    assert!(
        c.government_bir_check(&data, &p, Some(&hash), Some(&out))
            .is_err()
    );
    handle.join().unwrap();
    assert_eq!(std::fs::read(&out).unwrap(), b"existing work");
    assert_eq!(std::fs::read(&p).unwrap(), raw);
    std::fs::remove_file(out).unwrap();
    std::fs::remove_file(p).unwrap();
    std::fs::remove_dir(root).unwrap();
}
#[test]
fn bir_actual_client_denies_non_hr_and_unchecked_export_before_network_or_source_read() {
    for role in [Role::Manager, Role::Employee, Role::Administrator] {
        let mut data = DemoStore::new().data;
        data.access.role = role;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let mut c = client(format!("http://{}", listener.local_addr().unwrap()), &data);
        assert!(
            c.government_bir_check(
                &data,
                Path::new("absent.dat"),
                None,
                Some(Path::new("absent-findings.xlsx"))
            )
            .is_err()
        );
        assert!(matches!(listener.accept(),Err(e)if e.kind()==std::io::ErrorKind::WouldBlock));
    }
}

#[test]
#[ignore = "Explicit normal hosted synthetic session supplied only through child environment."]
fn bir_hosted_native_check_and_findings() {
    let input: Value = serde_json::from_str(
        &std::env::var("HRIS_BIR_LIVE_SESSION")
            .expect("Provide an explicit ephemeral session fixture"),
    )
    .expect("Decode session fixture");
    let mut data = DemoStore::new().data;
    data.mode = Mode::Supabase;
    let mut c = client(input["url"].as_str().unwrap().into(), &data);
    c.config.publishable_key = input["publishable_key"].as_str().unwrap().into();
    c.session =
        serde_json::from_value(input["session"].clone()).expect("Decode normal session response");
    data.access = c.current_access().expect("Current protected access");
    data.company.id = data.access.company_id;
    let role = input["role"].as_str().unwrap();
    assert!(data.access.role.manages_hr());
    let root = PathBuf::from(std::env::var("HRIS_BIR_LIVE_OUT").unwrap());
    std::fs::create_dir(&root).unwrap();
    let p = root.join("0123456780000123120261604C.DAT");
    let raw = source();
    std::fs::write(&p, &raw).unwrap();
    let first = c
        .government_bir_check(&data, &p, None, None)
        .expect("Native protected BIR check");
    for ext in ["csv", "xlsx"] {
        c.government_bir_check(
            &data,
            &p,
            Some(&first.sha256),
            Some(&root.join(format!("findings.{ext}"))),
        )
        .expect("Native protected findings export");
    }
    assert_eq!(std::fs::read(&p).unwrap(), raw);
    std::fs::write(&root.join("proof.json"),&serde_json::to_vec_pretty(&json!({"role":role,"source_unchanged":true,"errors":first.errors,"reviews":first.reviews,"details":first.details,"employer_match":first.employer_match,"sha256":first.sha256})).unwrap()).unwrap();
}
