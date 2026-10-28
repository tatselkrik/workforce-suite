//! Local HTTP fixtures exercise the real client path; these are not hosted Auth proofs.
use super::*;
use crate::access::Role;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;

pub(super) fn access_json(a: &Access) -> Value {
    json!({"company_id":a.company_id,"user_id":a.user_id,"role":a.role,
        "employee_id":a.employee_id,"direct_reports":a.direct_reports,"reports_to_name":a.reports_to_name})
}

pub(super) fn client(url: String, data: &Snapshot) -> CloudClient {
    CloudClient {
        product: None,
        config: ConnectionConfig {
            url,
            publishable_key: "sb_publishable_fictional_http_fixture".into(),
        },
        http: Client::builder()
            .timeout(Duration::from_secs(4))
            .build()
            .unwrap(),
        session: AuthSession {
            access_token: "fictional-memory-only".into(),
            refresh_token: "fictional-memory-only".into(),
            expires_at: Utc::now().timestamp() + 3600,
            expires_in: 3600,
            user: serde_json::from_value(json!({"id":data.access.user_id})).unwrap(),
        },
        account: AccountProfile::default(),
        logo_cache: None,
    }
}

fn projection(preparation: bool) -> Value {
    let report = crate::payroll_government::preview();
    if preparation {
        json!({"version":"government-preparation-v1","review":report.wire,"registrations":[]})
    } else {
        serde_json::to_value(report.wire).unwrap()
    }
}

fn server(
    data: &Snapshot,
    preparation: bool,
    revoked: bool,
) -> (String, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let access = access_json(&data.access);
    let source = projection(preparation);
    let handle = thread::spawn(move || {
        let mut paths = Vec::new();
        let until = std::time::Instant::now() + Duration::from_secs(5);
        while paths.len() < 3 {
            assert!(
                std::time::Instant::now() < until,
                "Local fixture request did not arrive"
            );
            let mut socket = match listener.accept() {
                Ok((s, _)) => s,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(2));
                    continue;
                }
                Err(e) => panic!("Fixture accept: {e}"),
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
            assert!(length < 16384);
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            let n = paths.len();
            let expected = if n == 1 {
                if preparation {
                    "/rest/v1/rpc/government_preparation"
                } else {
                    "/rest/v1/rpc/government_reconciliation"
                }
            } else {
                "/rest/v1/rpc/my_hris_access"
            };
            assert_eq!(path, expected);
            paths.push(path);
            let denied = revoked && n == 2;
            let denial = json!({"message":"Fixture membership revoked"});
            let payload = serde_json::to_vec(if denied {
                &denial
            } else if n == 1 {
                &source
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
fn both_real_export_adapters_check_current_access_and_block_files_after_revocation() {
    let data = DemoStore::new().data;
    let filter = crate::payroll_government::preview().filter();
    let folder =
        std::env::temp_dir().join(format!("hris-government-export-http-{}", Uuid::new_v4()));
    std::fs::create_dir(&folder).unwrap();
    for preparation in [false, true] {
        for extension in ["csv", "xlsx"] {
            for revoked in [false, true] {
                let (url, server) = server(&data, preparation, revoked);
                let mut cloud = client(url, &data);
                let path = folder.join(format!("{preparation}-{revoked}.{extension}"));
                let result = if preparation {
                    cloud.government_preparation(&data, filter, &path)
                } else {
                    cloud.government_reconciliation(&data, filter, None, Some(&path))
                };
                assert_eq!(server.join().unwrap().len(), 3);
                if revoked {
                    assert!(
                        result
                            .unwrap_err()
                            .downcast_ref::<AccessRevoked>()
                            .is_some()
                    );
                    assert!(!path.exists());
                } else {
                    result.unwrap();
                    let raw = std::fs::read(&path).unwrap();
                    assert!(raw.starts_with(if extension == "xlsx" {
                        b"PK"
                    } else {
                        b"\xef\xbb\xbf"
                    }));
                    std::fs::remove_file(path).unwrap();
                }
            }
        }
    }
    std::fs::remove_dir(folder).unwrap();
}

#[test]
fn managers_and_employees_cannot_fetch_any_government_export_projection() {
    let filter = crate::payroll_government::preview().filter();
    for role in [Role::Manager, Role::Employee] {
        let mut data = DemoStore::new().data;
        data.access.role = role;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let mut cloud = client(format!("http://{}", listener.local_addr().unwrap()), &data);
        let path = std::env::temp_dir().join(format!("hris-denied-{}.xlsx", Uuid::new_v4()));
        assert!(
            cloud
                .government_preparation(&data, filter, &path)
                .unwrap_err()
                .to_string()
                .contains("Only current HR/Admin")
        );
        assert!(
            cloud
                .government_reconciliation(&data, filter, None, Some(&path))
                .unwrap_err()
                .to_string()
                .contains("Only current HR/Admin")
        );
        assert!(!path.exists());
        assert!(matches!(listener.accept(),Err(e) if e.kind()==std::io::ErrorKind::WouldBlock));
    }
}
