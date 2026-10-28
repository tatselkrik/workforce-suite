//! The real HTTP adapter with local fictional responses; hosted evidence is separate.
use super::government_export_tests::{access_json, client};
use super::*;
use crate::{access::Role, payroll_government::annual};
use std::{
    io::{BufRead, BufReader, Write},
    net::TcpListener,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Scenario {
    Good,
    RevokedBefore,
    RevokedAfter,
    ForeignCompany,
    WrongYear,
}
fn server(data: &Snapshot, scenario: Scenario) -> (String, thread::JoinHandle<Vec<String>>) {
    source_server(data, scenario, false)
}
fn source_server(
    data: &Snapshot,
    scenario: Scenario,
    pay: bool,
) -> (String, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let access = access_json(&data.access);
    let mut wire = annual::fixture().wire;
    if scenario == Scenario::ForeignCompany {
        wire.company_id = Uuid::new_v4();
    }
    if scenario == Scenario::WrongYear {
        wire.year = 2025;
    }
    let source = if pay {
        let r = crate::payroll_government::annual_pay::fixture();
        let mut v = serde_json::to_value(crate::payroll_government::annual_pay::Wire {
            version: "government-annual-pay-v1".into(),
            ledger: r.ledger.wire,
            details: r.details,
        })
        .unwrap();
        if scenario == Scenario::ForeignCompany {
            v["ledger"]["company_id"] = json!(Uuid::new_v4());
        }
        if scenario == Scenario::WrongYear {
            v["ledger"]["year"] = json!(2025);
        }
        v
    } else {
        serde_json::to_value(wire).unwrap()
    };
    let handle = thread::spawn(move || {
        let count = match scenario {
            Scenario::RevokedBefore => 1,
            Scenario::ForeignCompany | Scenario::WrongYear => 2,
            _ => 3,
        };
        let until = std::time::Instant::now() + Duration::from_secs(5);
        let mut paths = vec![];
        while paths.len() < count {
            assert!(
                std::time::Instant::now() < until,
                "Annual HTTP fixture timed out"
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
            let mut length = 0;
            loop {
                line.clear();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
                if let Some((key, value)) = line.split_once(':') {
                    if key.eq_ignore_ascii_case("content-length") {
                        length = value.trim().parse().unwrap();
                    }
                }
            }
            assert!(length < 1024);
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            let n = paths.len();
            if n == 1 {
                assert_eq!(
                    path,
                    if pay {
                        "/rest/v1/rpc/government_annual_pay_details"
                    } else {
                        "/rest/v1/rpc/government_annual_ledger"
                    }
                );
                assert_eq!(
                    serde_json::from_slice::<Value>(&body).unwrap(),
                    json!({"p_year":2026})
                );
            } else {
                assert_eq!(path, "/rest/v1/rpc/my_hris_access");
            }
            paths.push(path);
            let denied = scenario == Scenario::RevokedBefore
                || (scenario == Scenario::RevokedAfter && n == 2);
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
fn annual_pay_fresh_read_and_export_obey_all_access_and_file_guards() {
    let data = DemoStore::new().data;
    let folder = std::env::temp_dir().join(format!("hris-pay-detail-http-{}", Uuid::new_v4()));
    std::fs::create_dir(&folder).unwrap();
    for scenario in [
        Scenario::Good,
        Scenario::RevokedBefore,
        Scenario::RevokedAfter,
        Scenario::ForeignCompany,
        Scenario::WrongYear,
    ] {
        for extension in ["csv", "xlsx"] {
            let (url, server) = source_server(&data, scenario, true);
            let mut c = client(url, &data);
            let path = folder.join(format!("{scenario:?}.{extension}"));
            let result = c.government_annual_pay_details(&data, 2026, Some(&path));
            assert_eq!(
                result.is_ok(),
                scenario == Scenario::Good,
                "{scenario:?}: {result:?}"
            );
            assert_eq!(path.exists(), scenario == Scenario::Good);
            server.join().unwrap();
        }
    }
    let (url, server) = source_server(&data, Scenario::Good, true);
    let mut c = client(url, &data);
    let path = folder.join("existing.csv");
    std::fs::write(&path, b"KEEP").unwrap();
    assert!(
        c.government_annual_pay_details(&data, 2026, Some(&path))
            .is_err()
    );
    server.join().unwrap();
    assert_eq!(std::fs::read(path).unwrap(), b"KEEP");
    let mut data = data;
    data.access.role = Role::Manager;
    let mut c = client("http://127.0.0.1:1".into(), &data);
    assert!(c.government_annual_pay_details(&data, 2026, None).is_err());
    std::fs::remove_dir_all(folder).unwrap();
}
#[test]
fn annual_real_export_fetches_fresh_source_and_checks_access_before_new_file_io() {
    let data = DemoStore::new().data;
    let folder = std::env::temp_dir().join(format!("hris-annual-http-{}", Uuid::new_v4()));
    std::fs::create_dir(&folder).unwrap();
    for scenario in [
        Scenario::Good,
        Scenario::RevokedBefore,
        Scenario::RevokedAfter,
        Scenario::ForeignCompany,
        Scenario::WrongYear,
    ] {
        for extension in ["csv", "xlsx"] {
            let (url, server) = server(&data, scenario);
            let mut cloud = client(url, &data);
            let path = folder.join(format!("{scenario:?}.{extension}"));
            let result = cloud.government_annual_ledger(&data, 2026, Some(&path));
            let paths = server.join().unwrap();
            if scenario == Scenario::Good {
                let r = result.unwrap();
                assert_eq!(paths.len(), 3);
                assert_eq!(r.summary.complete_received_cents, Some(111611));
                assert!(path.exists());
            } else {
                assert!(result.is_err());
                assert!(!path.exists());
            }
        }
    }
    std::fs::remove_dir_all(folder).unwrap();
}
#[test]
fn annual_staff_denial_and_unsupported_year_make_no_source_request() {
    for role in [Role::Employee, Role::Manager, Role::Administrator] {
        let mut data = DemoStore::new().data;
        data.access.role = role;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let mut cloud = client(format!("http://{}", listener.local_addr().unwrap()), &data);
        assert!(
            cloud
                .government_annual_ledger(
                    &data,
                    if role == Role::Administrator {
                        2024
                    } else {
                        2026
                    },
                    None
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
fn annual_export_does_not_replace_an_existing_file() {
    let data = DemoStore::new().data;
    let (url, server) = server(&data, Scenario::Good);
    let mut cloud = client(url, &data);
    let path = std::env::temp_dir().join(format!("hris-existing-{}.csv", Uuid::new_v4()));
    std::fs::write(&path, b"KEEP ORIGINAL").unwrap();
    assert!(
        cloud
            .government_annual_ledger(&data, 2026, Some(&path))
            .is_err()
    );
    server.join().unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"KEEP ORIGINAL");
    std::fs::remove_file(path).unwrap();
}

#[test]
#[ignore = "Normal hosted synthetic Auth session supplied only in child environment."]
fn hosted_native_annual_read_and_exports() {
    let input: Value =
        serde_json::from_str(&std::env::var("HRIS_ANNUAL_LIVE_SESSION").unwrap()).unwrap();
    let mut data = DemoStore::new().data;
    data.mode = Mode::Supabase;
    let mut c = client(input["url"].as_str().unwrap().into(), &data);
    c.config.publishable_key = input["publishable_key"].as_str().unwrap().into();
    c.http = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap();
    c.session = serde_json::from_value(input["session"].clone()).unwrap();
    data.access = c.current_access().expect("Normal protected access");
    data.company.id = data.access.company_id;
    assert!(data.access.role.manages_hr());
    let folder = std::path::PathBuf::from(std::env::var("HRIS_ANNUAL_LIVE_OUT").unwrap());
    std::fs::create_dir(&folder).unwrap();
    let mut wires = vec![];
    for year in [2025, 2026] {
        let report = c
            .government_annual_ledger(&data, year, None)
            .expect("Protected annual read");
        for extension in ["csv", "xlsx"] {
            let path = folder.join(format!("annual-{year}.{extension}"));
            let exported = c
                .government_annual_ledger(&data, year, Some(&path))
                .expect("Fresh native export");
            assert_eq!(exported.summary, report.summary);
            assert!(path.exists());
        }
        wires.push(report.wire);
    }
    std::fs::write(
        folder.join("snapshots.json"),
        serde_json::to_vec(&wires).unwrap(),
    )
    .unwrap();
    std::fs::write(folder.join("proof.json"),serde_json::to_vec(&json!({"all_passed":true,"years":2,"fresh_exports":4,"role":input["role"],"official_acceptance":false})).unwrap()).unwrap();
}
