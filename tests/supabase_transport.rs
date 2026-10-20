use hris_desktop::{
    backend::{CloudClient, ConnectionConfig},
    domain::Company,
};
use serde_json::json;
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

type ObservedRequests = Arc<Mutex<Vec<String>>>;

fn auth_fixture(
    companies: Vec<Company>,
) -> (ConnectionConfig, thread::JoinHandle<()>, ObservedRequests) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let observed = requests.clone();
    let expected_requests = if companies.is_empty() { 2 } else { 5 };
    let user_id = companies
        .first()
        .map(|company| company.owner_id)
        .unwrap_or_default();
    let company_json = serde_json::to_string(&companies).unwrap();
    let server = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut count = 0;
        while count < expected_requests && Instant::now() < deadline {
            let (mut stream, _) = match listener.accept() {
                Ok(s) => s,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(5));
                    continue;
                }
                Err(e) => panic!("{e}"),
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut buffer = [0u8; 2048];
            loop {
                let n = stream.read(&mut buffer).unwrap();
                if n == 0 {
                    break;
                }
                bytes.extend_from_slice(&buffer[..n]);
                if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    let header = String::from_utf8_lossy(&bytes[..end]);
                    let length = header
                        .lines()
                        .find_map(|l| {
                            l.to_lowercase()
                                .strip_prefix("content-length:")
                                .map(|s| s.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    if bytes.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            let request = String::from_utf8(bytes).unwrap();
            let first = request.lines().next().unwrap();
            let body = if first.starts_with("POST /auth/v1/token?") {
                json!({"access_token":"test-user-jwt","refresh_token":"test-refresh-token","expires_at":4102444800i64,"expires_in":3600,"user":{"id":user_id}}).to_string()
            } else if first.starts_with("GET /rest/v1/companies?") {
                company_json.clone()
            } else {
                "[]".into()
            };
            observed.lock().unwrap().push(request);
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
            count += 1;
        }
        assert_eq!(
            count, expected_requests,
            "All expected Auth and REST requests must complete"
        );
    });
    let config = ConnectionConfig {
        url: format!("http://{address}"),
        publishable_key: "sb_publishable_local_test_only".into(),
    };
    (config, server, requests)
}

#[test]
fn supabase_auth_uses_publishable_apikey_and_user_bearer_for_rls_requests() {
    let (config, server, requests) = auth_fixture(vec![Company::default()]);
    let result = CloudClient::connect(
        config,
        "demo@example.invalid".into(),
        "synthetic-test-password".into(),
    );
    server.join().unwrap();
    let (_, snapshot) = result.unwrap();
    assert_eq!(snapshot.company.name, "Northstar Company");
    let requests = requests.lock().unwrap();
    for request in requests.iter() {
        assert!(
            request
                .to_lowercase()
                .contains("apikey: sb_publishable_local_test_only")
        );
    }
    assert!(!requests[0].to_lowercase().contains("authorization:"));
    assert!(requests[0].starts_with("POST /auth/v1/token?grant_type=password "));
    for request in &requests[1..] {
        assert!(
            request
                .to_lowercase()
                .contains("authorization: bearer test-user-jwt")
        );
        assert!(!request.contains("Bearer sb_publishable_"));
    }
    assert!(
        requests
            .iter()
            .any(|r| r.starts_with("GET /rest/v1/employees?"))
    );
    assert!(
        requests
            .iter()
            .any(|r| r.starts_with("GET /rest/v1/attendance?"))
    );
    assert!(
        requests
            .iter()
            .any(|r| r.starts_with("GET /rest/v1/leave_requests?"))
    );
}

#[test]
fn an_unassigned_account_cannot_create_a_company_or_load_employee_modules() {
    let (config, server, requests) = auth_fixture(Vec::new());
    let result = CloudClient::connect(
        config,
        "unassigned@example.invalid".into(),
        "synthetic-test-password".into(),
    );
    server.join().unwrap();
    let error = match result {
        Err(error) => error.to_string(),
        Ok(_) => panic!("An unassigned account must not connect."),
    };
    assert!(error.contains("not assigned"));
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(requests[1].starts_with("GET /rest/v1/companies?"));
    assert!(
        !requests
            .iter()
            .any(|request| request.starts_with("POST /rest/v1/companies"))
    );
}
