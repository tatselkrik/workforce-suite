use super::*;
use axum::body::{Body, to_bytes};
use tower::ServiceExt;
fn config() -> Config {
    let mut c = Config::origin("http://127.0.0.1:3030", "127.0.0.1:3030").unwrap();
    c.url = "http://127.0.0.1:9".into();
    c.key = "sb_publishable_test".into();
    c
}
#[test]
fn external_plain_http_and_secret_origin_are_rejected() {
    for (o, l) in [
        ("http://example.invalid", "0.0.0.0:3030"),
        ("http://127.0.0.1:3030", "0.0.0.0:3030"),
        ("https://user:secret@example.invalid", "127.0.0.1:3030"),
        ("https://example.invalid/path", "127.0.0.1:3030"),
    ] {
        assert!(Config::origin(o, l).is_err());
    }
    assert!(Config::origin("https://hr.example.invalid", "0.0.0.0:3030").is_ok());
}
#[test]
fn secure_cookie_has_host_scope_and_no_auth_token() {
    let c = Config::origin("https://hr.example.invalid", "127.0.0.1:3030").unwrap();
    let cookie = c.cookie(Some(Uuid::new_v4()));
    for s in [
        "__Host-hris_portal=",
        "HttpOnly",
        "Secure",
        "SameSite=Strict",
        "Path=/",
    ] {
        assert!(cookie.contains(s));
    }
    assert!(!cookie.contains("Domain="));
    assert!(c.cookie(None).contains("Max-Age=0"));
}
#[test]
fn duplicate_or_malformed_cookies_do_not_select_a_session() {
    let mut h = HeaderMap::new();
    let id = Uuid::new_v4();
    h.insert(
        header::COOKIE,
        format!("hris_portal={id}; other=ok").parse().unwrap(),
    );
    assert_eq!(cookie_id(&h, "hris_portal"), Some(id));
    h.insert(
        header::COOKIE,
        format!("hris_portal={id}; hris_portal={id}")
            .parse()
            .unwrap(),
    );
    assert!(cookie_id(&h, "hris_portal").is_none());
}
#[test]
fn contact_identity_fields_and_employment_updates_are_rejected() {
    let mut c = serde_json::to_value(Contacts::default()).unwrap();
    c["employee_id"] = json!(Uuid::new_v4());
    assert!(serde_json::from_value::<Contacts>(c).is_err());
    let mut c = Contacts {
        emergency_name: "Person".into(),
        ..Default::default()
    };
    assert!(c.validate().is_err());
    c.emergency_name.clear();
    c.phone = "+63 912 345 6789".into();
    assert!(c.validate().is_ok());
    c.phone = "invalid".into();
    assert!(c.validate().is_err());
}
#[test]
fn email_validation_is_bounded_and_rejects_control_text() {
    for e in [
        "no-at",
        "x@y",
        "x@y..",
        "x@y@example.com",
        "x\n@example.com",
        "@example.com",
    ] {
        assert!(!email_ok(e));
    }
    assert!(email_ok("person@example.invalid"));
}
#[tokio::test]
async fn host_origin_csrf_and_private_cache_are_enforced() {
    let p = Portal::new(config()).unwrap();
    let app = p.router();
    let req = || {
        Request::builder()
            .uri("/api/sign-in")
            .method("POST")
            .header("host", "127.0.0.1:3030")
            .header("origin", "https://evil.example.invalid")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"email":"p@example.invalid","password":"test"}"#,
            ))
            .unwrap()
    };
    let r = app.clone().oneshot(req()).await.unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN);
    assert_eq!(r.headers()["cache-control"], "no-store, private");
    assert!(
        r.headers()["content-security-policy"]
            .to_str()
            .unwrap()
            .contains("frame-ancestors 'none'")
    );
    let r = app
        .oneshot(
            Request::builder()
                .uri("/")
                .header("host", "evil.example.invalid")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN);
}
#[tokio::test]
async fn unauthenticated_record_reads_clear_cookie_without_hr_requests() {
    let p = Portal::new(config()).unwrap();
    for path in ["/api/me", "/api/attendance", "/api/payslips"] {
        let r = p
            .router()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .header("host", "127.0.0.1:3030")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
        assert!(
            r.headers()[header::SET_COOKIE]
                .to_str()
                .unwrap()
                .contains("Max-Age=0")
        );
        let b = to_bytes(r.into_body(), 4096).await.unwrap();
        assert!(!String::from_utf8_lossy(&b).contains("sb_publishable"));
    }
}
#[tokio::test]
async fn there_is_no_arbitrary_proxy_or_administrator_route() {
    let p = Portal::new(config()).unwrap();
    for path in [
        "/api/account-admin",
        "/api/proxy?table=payroll_profiles",
        "/api/employees",
    ] {
        let r = p
            .router()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .header("host", "127.0.0.1:3030")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::NOT_FOUND);
    }
}

#[derive(Clone, Default)]
struct Mock {
    requests: Arc<Mutex<Vec<(String, String, Value)>>>,
    disabled: Arc<std::sync::atomic::AtomicBool>,
    foreign: Arc<std::sync::atomic::AtomicBool>,
}
fn uid() -> Uuid {
    Uuid::from_u128(101)
}
fn cid() -> Uuid {
    Uuid::from_u128(102)
}
fn eid() -> Uuid {
    Uuid::from_u128(103)
}
fn aid() -> Uuid {
    Uuid::from_u128(104)
}
fn pid() -> Uuid {
    Uuid::from_u128(105)
}
async fn fake(State(m): State<Mock>, request: Request) -> Response {
    use std::sync::atomic::Ordering;
    let method = request.method().to_string();
    let uri = request.uri().to_string();
    let path = request.uri().path().to_string();
    let body = to_bytes(request.into_body(), 8192).await.unwrap();
    let b: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    m.requests
        .lock()
        .await
        .push((method.clone(), uri.clone(), b));
    let person = if m.foreign.load(Ordering::SeqCst) {
        Uuid::from_u128(999)
    } else {
        eid()
    };
    let mut row = json!({"id":aid(),"company_id":cid(),"employee_id":person,"work_date":"2026-09-01","checked_in_at":"2026-09-01T01:00:00Z","checked_out_at":null,"hr_note":"PRIVATE_SENTINEL"});
    let data = match path.as_str() {
        "/auth/v1/token" => {
            json!({"user":{"id":uid()},"access_token":"upstream-only-access","refresh_token":"upstream-only-refresh","expires_in":3600})
        }
        "/auth/v1/user" => {
            json!({"id":uid(),"email":"portal-staff@example.invalid","user_metadata":{"role":"administrator","secret":"PRIVATE_SENTINEL"}})
        }
        "/auth/v1/logout" => json!({}),
        "/rest/v1/rpc/my_hris_access" => {
            if m.disabled.load(Ordering::SeqCst) {
                return (StatusCode::FORBIDDEN, Json(json!({"error":"disabled"}))).into_response();
            }
            json!({"user_id":uid(),"company_id":cid(),"employee_id":eid(),"role":"manager","reports_to_name":"Sample supervisor","direct_reports":[Uuid::from_u128(999)]})
        }
        "/rest/v1/employees" => {
            json!([{"id":eid(),"company_id":cid(),"employee_code":"EMP-TEST","full_name":"Sample Employee","department":"Operations","job_title":"Coordinator","joined_on":"2025-01-01","annual_leave_days":15,"status":"active","private_hr":"PRIVATE_SENTINEL"}])
        }
        "/rest/v1/companies" => {
            json!([{"id":cid(),"name":"Sample Company","timezone":"Asia/Manila","secret":"PRIVATE_SENTINEL"}])
        }
        "/rest/v1/employee_contacts" => {
            let mut c = serde_json::to_value(Contacts::default()).unwrap();
            c["company_id"] = json!(cid());
            c["employee_id"] = json!(eid());
            c["id"] = json!(Uuid::from_u128(106));
            c["hr_note"] = json!("PRIVATE_SENTINEL");
            json!([c])
        }
        "/rest/v1/attendance" => {
            if method == "PATCH" {
                row["checked_out_at"] = json!("2026-09-01T02:00:00Z");
            }
            json!([row])
        }
        "/rest/v1/payroll_slips" => {
            json!([{"id":pid(),"company_id":cid(),"employee_id":person,"month":"2026-09-01","company_name":"Sample Company","approved_at":"2026-10-01T00:00:00Z","rule_version":"TEST","item":{"company_id":cid(),"employee_id":person,"employee_name":"Sample Employee","employee_code":"EMP-TEST","department":"Operations","source_basis":{"secret":"PRIVATE_SENTINEL"},"profile":{"pay":{"coverage_note":"PRIVATE_SENTINEL","components":[{"label":"Loan","kind":"deduction","amount_cents":10000,"reference":"PRIVATE_SENTINEL"}]}},"worksheet":{"review_note":"PRIVATE_SENTINEL","adjustments":[]},"totals":{"net":123456,"basic":130000,"private_hr":"PRIVATE_SENTINEL"}}}])
        }
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    Json(data).into_response()
}
struct Fixture {
    portal: Portal,
    mock: Mock,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn fixture() -> Fixture {
    let mock = Mock::default();
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut c = config();
    c.url = format!("http://{}", l.local_addr().unwrap());
    let service = Router::new()
        .fallback(axum::routing::any(fake))
        .with_state(mock.clone());
    let task = tokio::spawn(async move {
        axum::serve(l, service).await.unwrap();
    });
    Fixture {
        portal: Portal::new(c).unwrap(),
        mock,
        task,
    }
}
async fn request(
    p: &Portal,
    path: &str,
    body: Option<Value>,
    cookie: Option<&str>,
    csrf: Option<&str>,
) -> (StatusCode, HeaderMap, Value) {
    let mut b = Request::builder()
        .uri(path)
        .method(if body.is_some() { "POST" } else { "GET" })
        .header("host", "127.0.0.1:3030")
        .header("origin", "http://127.0.0.1:3030");
    if let Some(c) = cookie {
        b = b.header(header::COOKIE, c);
    }
    if let Some(t) = csrf {
        b = b.header("x-csrf-token", t);
    }
    if body.is_some() {
        b = b.header("content-type", "application/json");
    }
    let r = p
        .router()
        .oneshot(
            b.body(
                body.map(|v| Body::from(v.to_string()))
                    .unwrap_or_else(Body::empty),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let status = r.status();
    let headers = r.headers().clone();
    let b = to_bytes(r.into_body(), 100_000).await.unwrap();
    let data: Value = serde_json::from_slice(&b).unwrap_or(Value::Null);
    (status, headers, data)
}
async fn login(f: &Fixture) -> (String, String) {
    let (status, h, b) = request(
        &f.portal,
        "/api/sign-in",
        Some(json!({"email":"portal-staff@example.invalid","password":"fictional-test-password"})),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let text = b.to_string();
    assert!(!text.contains("upstream-only"));
    assert!(!text.contains("refresh"));
    (
        h[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .into(),
        b["csrf"].as_str().unwrap().into(),
    )
}

#[tokio::test]
async fn authenticated_manager_portal_projects_only_own_records_and_no_hr_notes() {
    let f = fixture().await;
    let (cookie, _) = login(&f).await;
    for path in [
        "/api/me",
        "/api/attendance",
        "/api/payslips",
        &format!("/api/payslips/{}", pid()),
    ] {
        let (s, _, b) = request(&f.portal, path, None, Some(&cookie), None).await;
        assert_eq!(s, StatusCode::OK, "{path}");
        let text = b.to_string();
        assert!(!text.contains("PRIVATE_SENTINEL"), "{path}");
        assert!(!text.contains("upstream-only"));
        assert!(!text.contains("direct_reports"));
    }
    let seen = f.mock.requests.lock().await;
    for (_, path, _) in seen.iter().filter(|(_, p, _)| {
        p.starts_with("/rest/v1/attendance")
            || p.starts_with("/rest/v1/payroll_slips")
            || p.starts_with("/rest/v1/employee_contacts")
    }) {
        assert!(
            path.contains(&format!("company_id=eq.{}", cid()))
                && path.contains(&format!("employee_id=eq.{}", eid()))
        );
    }
    assert!(
        !seen
            .iter()
            .any(|(_, p, _)| p.contains("payroll_profiles") || p.contains("list_company_accounts"))
    );
}
#[tokio::test]
async fn csrf_failure_writes_nothing_and_clock_ignores_client_employee_and_time() {
    let f = fixture().await;
    let (cookie, csrf) = login(&f).await;
    f.mock.requests.lock().await.clear();
    let (s, _, _) = request(
        &f.portal,
        "/api/attendance/check-in",
        Some(json!({})),
        Some(&cookie),
        Some(&Uuid::new_v4().to_string()),
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    assert!(f.mock.requests.lock().await.is_empty());
    let (s, _, _) = request(
        &f.portal,
        "/api/attendance/check-in",
        Some(json!({"employee_id":Uuid::from_u128(999),"checked_in_at":"2000-01-01"})),
        Some(&cookie),
        Some(&csrf),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let seen = f.mock.requests.lock().await;
    let (_, _, body) = seen
        .iter()
        .find(|(m, p, _)| m == "POST" && p == "/rest/v1/attendance")
        .unwrap();
    assert_eq!(body, &json!({"company_id":cid(),"employee_id":eid()}));
}
#[tokio::test]
async fn contact_updates_bind_identity_and_logout_revokes_and_rejects_old_cookie() {
    let f = fixture().await;
    let (cookie, csrf) = login(&f).await;
    let mut c = serde_json::to_value(Contacts::default()).unwrap();
    c["phone"] = json!("1234567");
    let (s, _, _) = request(
        &f.portal,
        "/api/contacts",
        Some(c),
        Some(&cookie),
        Some(&csrf),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, h, b) = request(
        &f.portal,
        "/api/sign-out",
        Some(json!({})),
        Some(&cookie),
        Some(&csrf),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(b["signed_out"], true);
    assert!(
        h[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .contains("Max-Age=0")
    );
    let (s, _, _) = request(&f.portal, "/api/me", None, Some(&cookie), None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let seen = f.mock.requests.lock().await;
    assert!(
        seen.iter()
            .any(|(_, p, _)| p == "/auth/v1/logout?scope=local")
    );
    let (_, path, body) = seen
        .iter()
        .find(|(m, p, _)| m == "PATCH" && p.starts_with("/rest/v1/employee_contacts"))
        .unwrap();
    assert!(path.contains(&format!("employee_id=eq.{}", eid())));
    assert_eq!(body["phone"], "1234567");
    assert!(body.get("employee_id").is_none());
}
#[tokio::test]
async fn current_membership_revocation_blocks_unexpired_portal_session() {
    let f = fixture().await;
    let (cookie, _) = login(&f).await;
    f.mock
        .disabled
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let (s, _, b) = request(&f.portal, "/api/attendance", None, Some(&cookie), None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    assert!(!b.to_string().contains("PRIVATE_SENTINEL"));
}
#[tokio::test]
async fn unexpected_foreign_upstream_row_is_rejected_before_browser_delivery() {
    let f = fixture().await;
    let (cookie, _) = login(&f).await;
    f.mock
        .foreign
        .store(true, std::sync::atomic::Ordering::SeqCst);
    for path in ["/api/attendance", "/api/payslips"] {
        let (s, _, b) = request(&f.portal, path, None, Some(&cookie), None).await;
        assert_eq!(s, StatusCode::BAD_GATEWAY);
        assert!(!b.to_string().contains("Sample Employee"));
    }
}
#[tokio::test]
async fn refresh_serializes_rotated_tokens_and_expired_session_is_discarded() {
    let f = fixture().await;
    let (cookie, _) = login(&f).await;
    let sid: Uuid = cookie.split_once('=').unwrap().1.parse().unwrap();
    let arc = f.portal.sessions.lock().await[&sid].clone();
    arc.lock().await.expires = Instant::now();
    let (s, _, _) = request(&f.portal, "/api/me", None, Some(&cookie), None).await;
    assert_eq!(s, StatusCode::OK);
    assert!(
        f.mock
            .requests
            .lock()
            .await
            .iter()
            .any(|(_, p, _)| p == "/auth/v1/token?grant_type=refresh_token")
    );
    arc.lock().await.last_seen = Instant::now() - Duration::from_secs(1801);
    let (s, _, _) = request(&f.portal, "/api/me", None, Some(&cookie), None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    assert!(!f.portal.sessions.lock().await.contains_key(&sid));
}

#[tokio::test]
#[ignore = "Isolated fictional browser preview; stop with POST /qa/stop"]
async fn visual_preview_server() {
    let f = fixture().await;
    let mut config = Config::origin("http://127.0.0.1:3031", "127.0.0.1:3031").unwrap();
    config.url = f.portal.config.url.clone();
    config.key = f.portal.config.key.clone();
    let portal = Portal::new(config).unwrap();
    let done = Arc::new(tokio::sync::Notify::new());
    let wake = done.clone();
    let app = portal.router().route(
        "/qa/stop",
        post(move || {
            let wake = wake.clone();
            async move {
                wake.notify_one();
                StatusCode::OK
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3031")
        .await
        .unwrap();
    println!("Fictional portal preview: http://127.0.0.1:3031");
    axum::serve(listener, app)
        .with_graceful_shutdown(async move { done.notified().await })
        .await
        .unwrap();
    portal.close().await;
}
