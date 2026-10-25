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
    for path in [
        "/api/me",
        "/api/attendance",
        "/api/payslips",
        "/api/leave",
        "/api/leave/context",
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
    break_state: Arc<std::sync::atomic::AtomicU8>,
    leave_cancelled: Arc<std::sync::atomic::AtomicBool>,
    notification_read: Arc<std::sync::atomic::AtomicBool>,
    lifecycle_kind: Arc<std::sync::atomic::AtomicU8>,
    photo_png: Arc<Mutex<Option<Vec<u8>>>>,
    logo_png: Arc<Mutex<Option<Vec<u8>>>>,
    logo_revision: Arc<std::sync::atomic::AtomicU64>,
    auth_password: Arc<Mutex<Option<String>>>,
    pending_email: Arc<Mutex<String>>,
    foreign_proof: Arc<std::sync::atomic::AtomicBool>,
    foreign_update: Arc<std::sync::atomic::AtomicBool>,
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
fn lid() -> Uuid {
    Uuid::from_u128(107)
}
fn bid() -> Uuid {
    Uuid::from_u128(108)
}
fn rule_id() -> Uuid {
    Uuid::from_u128(109)
}
async fn fake(State(m): State<Mock>, request: Request) -> Response {
    use std::sync::atomic::Ordering;
    let method = request.method().to_string();
    let uri = request.uri().to_string();
    let path = request.uri().path().to_string();
    let modern_leave = request
        .headers()
        .get("x-hris-leave-contract")
        .and_then(|v| v.to_str().ok())
        == Some("2");
    let body = to_bytes(request.into_body(), 5 * 1024 * 1024)
        .await
        .unwrap();
    let b: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    m.requests
        .lock()
        .await
        .push((method.clone(), uri.clone(), b.clone()));
    if path == format!("/storage/v1/object/profile-photos/{}/avatar.png", uid()) && method == "POST"
    {
        assert!(body.starts_with(b"\x89PNG\r\n\x1a\n"));
        *m.photo_png.lock().await = Some(body.to_vec());
        return Json(json!({"Key":"PRIVATE_SENTINEL"})).into_response();
    }
    if path == "/storage/v1/object/profile-photos" && method == "DELETE" {
        assert_eq!(b["prefixes"], json!([format!("{}/avatar.png", uid())]));
        *m.photo_png.lock().await = None;
        return Json(json!([])).into_response();
    }
    if path
        == format!(
            "/storage/v1/object/authenticated/profile-photos/{}/avatar.png",
            uid()
        )
        || path
            == format!(
                "/storage/v1/object/authenticated/company-branding/{}/logo.png",
                cid()
            )
    {
        let raw = if path.contains("profile-photos") {
            m.photo_png.lock().await.clone()
        } else {
            m.logo_png.lock().await.clone()
        };
        return match raw {
            Some(bytes) => ([(header::CONTENT_TYPE, "image/png")], bytes).into_response(),
            None => (
                StatusCode::NOT_FOUND,
                Json(json!({"error":"not_found","message":"Object not found"})),
            )
                .into_response(),
        };
    }
    let person = if m.foreign.load(Ordering::SeqCst) {
        Uuid::from_u128(999)
    } else {
        eid()
    };
    let mut row = json!({"id":aid(),"company_id":cid(),"employee_id":person,"work_date":"2026-09-01","checked_in_at":"2026-09-01T01:00:00Z","checked_out_at":null,"hr_note":"PRIVATE_SENTINEL"});
    row["revision"] = json!(0);
    row["planned_shift"] = json!({"break_rules":[{"id":rule_id(),"name":"Standard break","minutes":15,"paid":false,"required":false,"private":"PRIVATE_SENTINEL"}]});
    let data = match path.as_str() {
        "/auth/v1/token" => {
            if uri.contains("grant_type=password") {
                let password = m
                    .auth_password
                    .lock()
                    .await
                    .clone()
                    .unwrap_or("fictional-test-password".into());
                if b["password"] != password {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(json!({"error":"PRIVATE_SENTINEL"})),
                    )
                        .into_response();
                }
            }
            json!({"user":{"id":if m.foreign_proof.load(Ordering::SeqCst){Uuid::from_u128(999)}else{uid()}},"access_token":"upstream-only-access","refresh_token":"upstream-only-refresh","expires_in":3600})
        }
        "/auth/v1/user" => {
            if method == "PUT" {
                if let Some(password) = b["password"].as_str() {
                    assert_eq!(
                        b["current_password"],
                        m.auth_password
                            .lock()
                            .await
                            .clone()
                            .unwrap_or("fictional-test-password".into())
                    );
                    *m.auth_password.lock().await = Some(password.into());
                }
                if let Some(email) = b["email"].as_str() {
                    assert_eq!(b["code_challenge_method"], "s256");
                    assert_eq!(b["code_challenge"].as_str().unwrap().len(), 43);
                    *m.pending_email.lock().await = email.into();
                }
            }
            json!({"id":if method=="PUT"&&m.foreign_update.load(Ordering::SeqCst){Uuid::from_u128(999)}else{uid()},"email":"portal-staff@example.invalid","new_email":*m.pending_email.lock().await,"user_metadata":{"role":"administrator","secret":"PRIVATE_SENTINEL"}})
        }
        "/auth/v1/logout" => json!({}),
        "/rest/v1/rpc/portal_notifications_v2" => {
            let read = m.notification_read.load(Ordering::SeqCst);
            let mode=m.lifecycle_kind.load(Ordering::SeqCst);let source=Uuid::from_u128(110);
            let(kind,page,key)=match mode{
                1=>("payroll-monthly-issued","payroll_monthly",format!("payroll-monthly-issued:{source}:0")),
                2=>("training-due","training",format!("training-due:{source}:1:soon")),
                3=>("payroll-receipt","payroll_monthly",format!("payroll-receipt:{}:0",Uuid::from_u128(111))),
                4=>("appraisal-review","appraisal",format!("appraisal-review:{source}:1:soon")),
                5=>("payroll-monthly-issued","private_files",format!("payroll-monthly-issued:{source}:0")),
                _=>("correction-status","attendance",format!("correction-status:{source}:1")),
            };
            json!({"version":"portal-notifications-v2","company_id":cid(),"employee_id":person,"user_id":uid(),"asof":"2026-10-25","generated_at":"2026-10-25T00:00:00Z","offset":b["p_offset"],"limit":25,"unread_only":b["p_unread_only"],"total_items":if b["p_unread_only"]==true&&read{0}else{1},"unread_count":if read{0}else{1},"items":if (b["p_unread_only"]==true&&read)||b["p_offset"].as_u64().unwrap_or(0)>0{json!([])}else{json!([{"key":key,"kind":kind,"title":"Your current personal update","detail":"A fictional scoped update","date":"2026-10-25","urgent":false,"read":read,"target_id":source,"target_page":page,"private":"PRIVATE_SENTINEL"}])},"private":"PRIVATE_SENTINEL"})
        }
        "/rest/v1/rpc/portal_notification_reads_v2" => {
            m.notification_read
                .store(b["p_read"].as_bool().unwrap_or(false), Ordering::SeqCst);
            json!({"company_id":cid(),"employee_id":person,"user_id":uid(),"saved":true,"private":"PRIVATE_SENTINEL"})
        }
        "/rest/v1/rpc/portal_attendance_context" => {
            json!({"company_id":cid(),"employee_id":person,"work_date":b["p_day"].as_str().unwrap_or("2026-09-01"),"timezone":"Asia/Manila","attendance":{"id":aid(),"revision":0,"work_date":"2026-09-01","checked_in_at":"2026-09-01T00:00:00Z","checked_out_at":"2026-09-01T10:30:00Z","secret":"PRIVATE_SENTINEL"},"candidate_minutes":90,"break_revisions":[{"id":bid(),"revision":0,"secret":"PRIVATE_SENTINEL"}],"correction_pending":false,"overtime_blocked":false,"current_approved_minutes":0,"secret":"PRIVATE_SENTINEL"})
        }
        "/rest/v1/rpc/attendance_request" | "/rest/v1/rpc/overtime_request" => {
            if b["p_request"]["expected_revision"]
                .as_i64()
                .is_some_and(|r| r != 0)
            {
                return (
                    StatusCode::CONFLICT,
                    Json(json!({"secret":"PRIVATE_SENTINEL"})),
                )
                    .into_response();
            }
            json!(Uuid::from_u128(110))
        }
        "/rest/v1/attendance_requests" => {
            json!([{"id":Uuid::from_u128(110),"company_id":cid(),"employee_id":person,"work_date":"2026-09-01","timezone":"Asia/Manila","proposed_in":"2026-09-01T00:30:00Z","proposed_out":"2026-09-01T10:30:00Z","revision":0,"status":"pending","reason":"Synthetic corrected punch","created_at":"2026-09-02T00:00:00Z","base_record":{"secret":"PRIVATE_SENTINEL"}}])
        }
        "/rest/v1/overtime_requests" => {
            json!([{"id":Uuid::from_u128(110),"company_id":cid(),"employee_id":person,"work_date":"2026-09-01","requested_minutes":60,"approved_minutes":null,"revision":0,"status":"pending","reason":"Synthetic overtime","created_at":"2026-09-02T00:00:00Z","basis":{"secret":"PRIVATE_SENTINEL"}}])
        }
        "/rest/v1/attendance_request_events" | "/rest/v1/overtime_events" => {
            json!([{"company_id":cid(),"employee_id":person,"request_id":Uuid::from_u128(110),"status":"pending","actor_name":"Synthetic staff","actor_role":"employee","reason":"Synthetic submission","created_at":"2026-09-02T00:00:00Z","before_record":{"secret":"PRIVATE_SENTINEL"}}])
        }
        "/rest/v1/rpc/portal_leave_context" => {
            json!({"company_id":cid(),"employee_id":person,"policy_revision":7,"asof":b["p_asof"].as_str().unwrap_or("2026-11-02"),"workweek":[1,2,3,4,5],"holidays":[],"types":[{"code":"annual","name":"Annual leave","paid":true,"unlimited":false,"approval_chain":"PRIVATE_SENTINEL","balance":{"earned_units":30,"carryover_units":0,"used_units":2,"reserved_units":1,"available_units":27,"private":"PRIVATE_SENTINEL"}}]})
        }
        "/rest/v1/rpc/portal_submit_leave" => json!(lid()),
        "/rest/v1/rpc/portal_cancel_leave" => {
            m.leave_cancelled.store(true, Ordering::SeqCst);
            json!(lid())
        }
        "/rest/v1/leave_requests" => {
            if m.leave_cancelled.load(Ordering::SeqCst) && !modern_leave {
                return Json(json!([])).into_response();
            }
            json!([{"id":lid(),"company_id":cid(),"employee_id":person,"type_code":"annual","type_name":"Annual leave","starts_on":"2026-11-02","ends_on":"2026-11-02","day_part":"am","reason":"Fictional leave","status":if m.leave_cancelled.load(Ordering::SeqCst){"cancelled"}else{"pending"},"charged_units":1,"charged_dates":[{"date":"2026-11-02","units":1}],"revision":2,"policy_revision":7,"approval_steps":[{"kind":"hr","employee_id":Uuid::from_u128(999),"approved_by":null,"approved_at":null,"private":"PRIVATE_SENTINEL"}],"private":"PRIVATE_SENTINEL"}])
        }
        "/rest/v1/rpc/manage_attendance_break" => {
            if b["p_break"]["expected_parent_revision"] != 0 {
                return (StatusCode::CONFLICT, Json(json!({"error":"stale"}))).into_response();
            }
            m.break_state.store(
                if b["p_operation"] == "start" { 1 } else { 2 },
                Ordering::SeqCst,
            );
            json!(bid())
        }
        "/rest/v1/attendance_breaks" => {
            let n = m.break_state.load(Ordering::SeqCst);
            if n == 0 {
                json!([])
            } else {
                json!([{"id":bid(),"company_id":cid(),"employee_id":person,"attendance_id":aid(),"rule_id":rule_id(),"name":"Standard break","paid":false,"required":false,"expected_minutes":15,"started_at":"2026-09-01T01:30:00Z","ended_at":if n==2{Some("2026-09-01T01:45:00Z")}else{None},"revision":0,"private":"PRIVATE_SENTINEL"}])
            }
        }
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
            json!([{"id":cid(),"name":"Sample Company","timezone":"Asia/Manila","logo_version":if m.logo_png.lock().await.is_some(){json!(Uuid::from_u128(501+u128::from(m.logo_revision.load(Ordering::SeqCst))))}else{Value::Null},"secret":"PRIVATE_SENTINEL"}])
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
            json!([{"id":pid(),"company_id":cid(),"employee_id":person,"month":"2026-09-01","company_name":"Sample Company","approved_at":"2026-10-01T00:00:00Z","rule_version":"TEST","item":{"company_id":cid(),"employee_id":person,"employee_name":"Sample Employee","employee_code":"EMP-TEST","department":"Operations","source_basis":{"secret":"PRIVATE_SENTINEL"},"profile":{"pay":{"coverage_note":"PRIVATE_SENTINEL","components":[{"label":"Loan","kind":"deduction","amount_cents":6544,"reference":"PRIVATE_SENTINEL"}]}},"worksheet":{"review_note":"PRIVATE_SENTINEL","adjustments":[]},"totals":{"net":123456,"basic":130000,"gross":130000,"voluntary_deductions":6544,"private_hr":"PRIVATE_SENTINEL"}}}])
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

fn password_change() -> Value {
    json!({"current_password":"fictional-test-password","new_password":"a different fictional password","confirmation":"a different fictional password"})
}
#[tokio::test]
async fn security_status_is_own_projected_live_and_strictly_scoped() {
    use std::sync::atomic::Ordering;
    let f = fixture().await;
    assert_eq!(
        request(&f.portal, "/api/account", None, None, None).await.0,
        StatusCode::UNAUTHORIZED
    );
    let (cookie, csrf) = login(&f).await;
    *f.mock.pending_email.lock().await = "new-account@example.invalid".into();
    let (status, _, value) = request(&f.portal, "/api/account", None, Some(&cookie), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        value,
        json!({"email":"portal-staff@example.invalid","pending_email":"new-account@example.invalid","confirmation_pending":true,"idle_timeout_minutes":30})
    );
    assert_eq!(
        request(
            &f.portal,
            "/api/account?employee_id=other",
            None,
            Some(&cookie),
            None
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        request(
            &f.portal,
            "/api/account/password?user_id=other",
            Some(password_change()),
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    f.mock.disabled.store(true, Ordering::SeqCst);
    assert_eq!(
        request(&f.portal, "/api/account", None, Some(&cookie), None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}
#[tokio::test]
async fn password_requires_csrf_matching_fields_and_fresh_current_proof() {
    let f = fixture().await;
    let (cookie, csrf) = login(&f).await;
    assert_eq!(
        request(
            &f.portal,
            "/api/account/password",
            Some(password_change()),
            Some(&cookie),
            None
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    for (key, value) in [
        ("current_password", "incorrect"),
        ("confirmation", "incorrect"),
        ("new_password", "short"),
    ] {
        let mut body = password_change();
        body[key] = json!(value);
        let r = request(
            &f.portal,
            "/api/account/password",
            Some(body),
            Some(&cookie),
            Some(&csrf),
        )
        .await;
        assert_eq!(r.0, StatusCode::BAD_REQUEST);
        assert!(!r.2.to_string().contains("PRIVATE_SENTINEL"));
    }
    let mut body = password_change();
    body["employee_id"] = json!(eid());
    assert_eq!(
        request(
            &f.portal,
            "/api/account/password",
            Some(body),
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert!(
        !f.mock
            .requests
            .lock()
            .await
            .iter()
            .any(|(m, _, _)| m == "PUT")
    );
    assert_eq!(
        request(&f.portal, "/api/me", None, Some(&cookie), None)
            .await
            .0,
        StatusCode::OK
    );
}
#[tokio::test]
async fn password_update_keeps_opaque_cookie_and_never_returns_password_or_tokens() {
    let f = fixture().await;
    let (cookie, csrf) = login(&f).await;
    let (status, h, v) = request(
        &f.portal,
        "/api/account/password",
        Some(password_change()),
        Some(&cookie),
        Some(&csrf),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v, json!({"saved":true,"password_changed":true}));
    assert!(!h.contains_key(header::SET_COOKIE));
    assert_eq!(
        *f.mock.auth_password.lock().await,
        Some("a different fictional password".into())
    );
    assert_eq!(
        request(&f.portal, "/api/account", None, Some(&cookie), None)
            .await
            .0,
        StatusCode::OK
    );
    let calls = f.mock.requests.lock().await;
    let put = calls
        .iter()
        .position(|(m, p, _)| m == "PUT" && p == "/auth/v1/user")
        .unwrap();
    assert!(
        calls[..put]
            .iter()
            .any(|(_, p, b)| p == "/auth/v1/token?grant_type=password"
                && b["email"] == "portal-staff@example.invalid")
    );
    assert!(
        calls[..put]
            .iter()
            .any(|(_, p, _)| p == "/auth/v1/logout?scope=local")
    );
}
#[tokio::test]
async fn password_reconfirmation_rejects_foreign_identity_and_uncertain_save_is_explicit() {
    use std::sync::atomic::Ordering;
    let f = fixture().await;
    let (cookie, csrf) = login(&f).await;
    f.mock.foreign_proof.store(true, Ordering::SeqCst);
    assert_eq!(
        request(
            &f.portal,
            "/api/account/password",
            Some(password_change()),
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .0,
        StatusCode::BAD_GATEWAY
    );
    assert!(
        !f.mock
            .requests
            .lock()
            .await
            .iter()
            .any(|(m, _, _)| m == "PUT")
    );
    f.mock.foreign_proof.store(false, Ordering::SeqCst);
    f.mock.foreign_update.store(true, Ordering::SeqCst);
    let r = request(
        &f.portal,
        "/api/account/password",
        Some(password_change()),
        Some(&cookie),
        Some(&csrf),
    )
    .await;
    assert_eq!(r.0, StatusCode::BAD_GATEWAY);
    assert!(
        r.2["error"]
            .as_str()
            .unwrap()
            .contains("may have been saved")
    );
}
#[tokio::test]
async fn email_requests_use_current_password_and_pkce_without_browser_session_exchange() {
    let f = fixture().await;
    let (cookie, csrf) = login(&f).await;
    let b = json!({"current_password":"fictional-test-password","new_email":" new-account@example.invalid "});
    let r = request(
        &f.portal,
        "/api/account/email",
        Some(b),
        Some(&cookie),
        Some(&csrf),
    )
    .await;
    assert_eq!(r.0, StatusCode::OK);
    assert_eq!(r.2, json!({"saved":true,"confirmation_pending":true}));
    let calls = f.mock.requests.lock().await;
    let (_, path, b) = calls
        .iter()
        .find(|(m, p, _)| m == "PUT" && p.starts_with("/auth/v1/user?"))
        .unwrap();
    let url = reqwest::Url::parse(&format!("http://unused{path}")).unwrap();
    assert_eq!(
        url.query_pairs()
            .find(|(k, _)| k == "redirect_to")
            .unwrap()
            .1,
        "http://127.0.0.1:3030/account/confirm-email"
    );
    assert_eq!(b["email"], "new-account@example.invalid");
    assert_eq!(b.as_object().unwrap().len(), 3);
    assert!(!b.to_string().contains("current_password"));
    drop(calls);
    for query in [
        "code=attacker-code",
        "error=bad&error_description=PRIVATE_SENTINEL",
        "access_token=attacker&state=attacker",
    ] {
        let r = request(
            &f.portal,
            &format!("/account/confirm-email?{query}"),
            None,
            None,
            None,
        )
        .await;
        assert_eq!(r.0, StatusCode::SEE_OTHER);
        assert_eq!(r.1[header::LOCATION], "/");
        assert!(!r.1.contains_key(header::SET_COOKIE));
    }
    assert_eq!(f.portal.sessions.lock().await.len(), 1);
    assert!(
        !f.mock
            .requests
            .lock()
            .await
            .iter()
            .any(|(_, p, _)| p.contains("grant_type=pkce"))
    );
    assert_eq!(
        request(&f.portal, "/api/account", None, Some(&cookie), None)
            .await
            .2["pending_email"],
        "new-account@example.invalid"
    );
}
#[tokio::test]
async fn email_validation_never_accepts_targets_or_replaces_current_email_before_confirmation() {
    let f = fixture().await;
    let (cookie, csrf) = login(&f).await;
    for email in ["portal-staff@example.invalid", "broken-email", ""] {
        let b = json!({"current_password":"fictional-test-password","new_email":email});
        assert_eq!(
            request(
                &f.portal,
                "/api/account/email",
                Some(b),
                Some(&cookie),
                Some(&csrf)
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
    }
    let b = json!({"current_password":"incorrect","new_email":"new@example.invalid"});
    assert_eq!(
        request(
            &f.portal,
            "/api/account/email",
            Some(b),
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert!(
        !f.mock
            .requests
            .lock()
            .await
            .iter()
            .any(|(m, _, _)| m == "PUT")
    );
}
#[tokio::test]
async fn sign_out_others_removes_only_other_own_portal_sessions() {
    let f = fixture().await;
    let (first, _) = login(&f).await;
    let (current, csrf) = login(&f).await;
    let foreign = Uuid::new_v4();
    f.portal.sessions.lock().await.insert(
        foreign,
        Arc::new(Mutex::new(Session {
            access: "foreign".into(),
            refresh: "foreign".into(),
            expires: Instant::now() + Duration::from_secs(3600),
            last_seen: Instant::now(),
            csrf: Uuid::new_v4(),
            user: Uuid::from_u128(999),
            company: cid(),
            employee: Uuid::from_u128(999),
        })),
    );
    assert_eq!(
        request(
            &f.portal,
            "/api/account/sign-out-others",
            Some(json!({})),
            Some(&current),
            None
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(
            &f.portal,
            "/api/account/sign-out-others",
            Some(json!({"user_id":uid()})),
            Some(&current),
            Some(&csrf)
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let r = request(
        &f.portal,
        "/api/account/sign-out-others",
        Some(json!({})),
        Some(&current),
        Some(&csrf),
    )
    .await;
    assert_eq!(r.0, StatusCode::OK);
    assert_eq!(
        r.2,
        json!({"signed_out_others":true,"local_sessions_removed":1})
    );
    assert!(f.portal.sessions.lock().await.contains_key(&foreign));
    assert_eq!(
        request(&f.portal, "/api/account", None, Some(&first), None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        request(&f.portal, "/api/account", None, Some(&current), None)
            .await
            .0,
        StatusCode::OK
    );
}
#[tokio::test]
async fn security_throttle_does_not_lock_out_existing_own_read_session() {
    let f = fixture().await;
    let (cookie, csrf) = login(&f).await;
    for n in 0..11 {
        let b = json!({"current_password":"incorrect","new_email":"new@example.invalid"});
        assert_eq!(
            request(
                &f.portal,
                "/api/account/email",
                Some(b),
                Some(&cookie),
                Some(&csrf)
            )
            .await
            .0,
            if n < 10 {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::TOO_MANY_REQUESTS
            }
        );
    }
    assert_eq!(
        request(&f.portal, "/api/account", None, Some(&cookie), None)
            .await
            .0,
        StatusCode::OK
    );
    assert!(
        !f.mock
            .requests
            .lock()
            .await
            .iter()
            .any(|(m, _, _)| m == "PUT")
    );
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

#[tokio::test]
async fn leave_context_and_history_are_own_only_and_project_staff_targets() {
    let f = fixture().await;
    let (cookie, _) = login(&f).await;
    for path in ["/api/leave/context?asof=2026-11-02", "/api/leave"] {
        let (s, _, v) = request(&f.portal, path, None, Some(&cookie), None).await;
        assert_eq!(s, StatusCode::OK);
        let text = v.to_string();
        assert!(!text.contains("PRIVATE_SENTINEL"));
        assert!(!text.contains("employee_id"));
    }
    f.mock
        .foreign
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let (s, _, _) = request(&f.portal, "/api/leave/context", None, Some(&cookie), None).await;
    assert_eq!(s, StatusCode::BAD_GATEWAY);
}
#[tokio::test]
async fn leave_submit_has_no_client_identity_and_cancellation_keeps_revision() {
    let f = fixture().await;
    let (cookie, csrf) = login(&f).await;
    let draft = json!({"type_code":"annual","starts_on":"2026-11-02","ends_on":"2026-11-02","day_part":"am","reason":"Fictional filing","policy_revision":7});
    let mut forged = draft.clone();
    forged["employee_id"] = json!(Uuid::from_u128(999));
    assert_eq!(
        request(
            &f.portal,
            "/api/leave/request",
            Some(forged),
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        request(
            &f.portal,
            "/api/leave/request",
            Some(draft),
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        request(
            &f.portal,
            "/api/leave/cancel",
            Some(json!({"id":lid(),"revision":2,"reason":"Fictional cancellation"})),
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, _, history) = request(&f.portal, "/api/leave", None, Some(&cookie), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(history[0]["status"], "cancelled");
    let seen = f.mock.requests.lock().await;
    let (_, _, body) = seen
        .iter()
        .find(|(_, p, _)| p == "/rest/v1/rpc/portal_submit_leave")
        .unwrap();
    assert!(body.get("p_employee").is_none());
    assert_eq!(body["p_policy_revision"], 7);
    assert_eq!(body["p_part"], "am");
    let (_, _, body) = seen
        .iter()
        .find(|(_, p, _)| p == "/rest/v1/rpc/portal_cancel_leave")
        .unwrap();
    assert_eq!(body["p_revision"], 2);
}
#[tokio::test]
async fn break_controls_use_saved_rules_preserve_revisions_and_reject_time_overrides() {
    let f = fixture().await;
    let (cookie, csrf) = login(&f).await;
    let path = format!("/api/breaks/{}", aid());
    let (s, _, v) = request(&f.portal, &path, None, Some(&cookie), None).await;
    assert_eq!(s, StatusCode::OK);
    assert!(!v.to_string().contains("PRIVATE_SENTINEL"));
    assert_eq!(v["rules"][0]["id"], json!(rule_id()));
    let body = json!({"attendance_id":aid(),"rule_id":rule_id(),"parent_revision":0});
    let mut bad = body.clone();
    bad["started_at"] = json!("2000-01-01");
    assert_eq!(
        request(
            &f.portal,
            "/api/breaks/start",
            Some(bad),
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let mut stale = body.clone();
    stale["parent_revision"] = json!(9);
    assert_eq!(
        request(
            &f.portal,
            "/api/breaks/start",
            Some(stale),
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        request(
            &f.portal,
            "/api/breaks/start",
            Some(body),
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        request(
            &f.portal,
            "/api/breaks/end",
            Some(json!({"attendance_id":aid(),"id":bid(),"parent_revision":0,"revision":0})),
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, _, v) = request(&f.portal, &path, None, Some(&cookie), None).await;
    assert!(v["records"][0]["ended_at"].is_string());
    let seen = f.mock.requests.lock().await;
    for (_, _, body) in seen
        .iter()
        .filter(|(_, p, _)| p == "/rest/v1/rpc/manage_attendance_break")
    {
        assert!(body["p_break"].get("started_at").is_none());
        assert!(body["p_break"].get("paid").is_none());
    }
}

#[tokio::test]
async fn time_context_lists_and_history_project_only_own_safe_fields() {
    let f = fixture().await;
    let (cookie, _) = login(&f).await;
    for path in [
        "/api/time/context?day=2026-09-01",
        "/api/time/requests",
        "/api/time/history/correction/00000000-0000-0000-0000-00000000006e",
        "/api/time/history/overtime/00000000-0000-0000-0000-00000000006e",
    ] {
        let (s, _, v) = request(&f.portal, path, None, Some(&cookie), None).await;
        assert_eq!(s, StatusCode::OK, "{path}: {v}");
        assert!(!v.to_string().contains("PRIVATE_SENTINEL"));
        assert!(!v.to_string().contains("company_id"));
        assert!(!v.to_string().contains("employee_id"));
        assert!(!v.to_string().contains("actor_id"));
    }
    let seen = f.mock.requests.lock().await;
    for (_, path, _) in seen.iter().filter(|(_, p, _)| {
        p.starts_with("/rest/v1/attendance_requests?")
            || p.starts_with("/rest/v1/overtime_requests?")
    }) {
        assert!(path.contains(&format!("employee_id=eq.{}", eid())));
        assert!(path.contains(&format!("company_id=eq.{}", cid())));
    }
}
fn correction_draft() -> Value {
    json!({"work_date":"2026-09-01","timezone":"Asia/Manila","checked_in_local":"2026-09-01T08:30:00","checked_out_local":"2026-09-01T18:30:00","attendance_id":aid(),"parent_revision":0,"reason":" Synthetic correction "})
}
#[tokio::test]
async fn correction_uses_company_zone_parent_revision_and_no_client_identity() {
    let f = fixture().await;
    let (cookie, csrf) = login(&f).await;
    let (s, _, v) = request(
        &f.portal,
        "/api/time/correction",
        Some(correction_draft()),
        Some(&cookie),
        Some(&csrf),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let seen = f.mock.requests.lock().await;
    let (_, _, b) = seen
        .iter()
        .find(|(_, p, _)| p == "/rest/v1/rpc/attendance_request")
        .unwrap();
    let b = &b["p_request"];
    assert_eq!(b["checked_in_at"], "2026-09-01T00:30:00Z");
    assert_eq!(b["checked_out_at"], "2026-09-01T10:30:00Z");
    assert_eq!(b["expected_revision"], 0);
    assert_eq!(b["operation"], "submit");
    assert_eq!(b["reason"], "Synthetic correction");
    assert!(b.get("employee_id").is_none());
    assert!(b.get("status").is_none());
}
#[tokio::test]
async fn correction_rejects_forged_fields_missing_csrf_and_stale_or_invalid_times() {
    let f = fixture().await;
    let (cookie, csrf) = login(&f).await;
    for key in ["employee_id", "operation", "status", "actor_id"] {
        let mut b = correction_draft();
        b[key] = json!("forged");
        assert_eq!(
            request(
                &f.portal,
                "/api/time/correction",
                Some(b),
                Some(&cookie),
                Some(&csrf)
            )
            .await
            .0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    assert_eq!(
        request(
            &f.portal,
            "/api/time/correction",
            Some(correction_draft()),
            Some(&cookie),
            None
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let mut b = correction_draft();
    b["parent_revision"] = json!(9);
    assert_eq!(
        request(
            &f.portal,
            "/api/time/correction",
            Some(b),
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let mut b = correction_draft();
    b["checked_out_local"] = json!("2026-09-01T07:30:00");
    assert_eq!(
        request(
            &f.portal,
            "/api/time/correction",
            Some(b),
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let mut b = correction_draft();
    b["attendance_id"] = Value::Null;
    assert_eq!(
        request(
            &f.portal,
            "/api/time/correction",
            Some(b),
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
}
#[tokio::test]
async fn overtime_retains_break_candidate_revisions_and_rejects_arbitrary_review() {
    let f = fixture().await;
    let (cookie, csrf) = login(&f).await;
    let b = json!({"attendance_id":aid(),"parent_revision":0,"expected_breaks":[{"id":bid(),"revision":0}],"candidate_minutes":90,"minutes":60,"reason":"Synthetic overtime"});
    for minutes in [json!(0), json!(91), json!(1.5)] {
        let mut bad = b.clone();
        bad["minutes"] = minutes;
        assert!(matches!(
            request(
                &f.portal,
                "/api/time/overtime",
                Some(bad),
                Some(&cookie),
                Some(&csrf)
            )
            .await
            .0,
            StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY
        ));
    }
    assert_eq!(
        request(
            &f.portal,
            "/api/time/overtime",
            Some(b),
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        request(
            &f.portal,
            "/api/time/approve",
            Some(json!({})),
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let seen = f.mock.requests.lock().await;
    let (_, _, b) = seen
        .iter()
        .find(|(_, p, _)| p == "/rest/v1/rpc/overtime_request")
        .unwrap();
    let b = &b["p_request"];
    assert_eq!(b["expected_candidate_minutes"], 90);
    assert_eq!(b["expected_breaks"][0]["id"], json!(bid()));
    assert_eq!(b["operation"], "submit");
    assert!(b.get("employee_id").is_none());
}
#[tokio::test]
async fn time_cancellation_requires_owned_parent_and_saved_revision() {
    let f = fixture().await;
    let (cookie, csrf) = login(&f).await;
    let b = json!({"kind":"correction","id":Uuid::from_u128(110),"revision":0,"reason":"Synthetic cancellation"});
    assert_eq!(
        request(
            &f.portal,
            "/api/time/cancel",
            Some(b.clone()),
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .0,
        StatusCode::OK
    );
    let mut stale = b.clone();
    stale["revision"] = json!(9);
    assert_eq!(
        request(
            &f.portal,
            "/api/time/cancel",
            Some(stale),
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let mut foreign = b;
    foreign["id"] = json!(Uuid::from_u128(999));
    assert_eq!(
        request(
            &f.portal,
            "/api/time/cancel",
            Some(foreign),
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .0,
        StatusCode::BAD_GATEWAY
    );
}
#[tokio::test]
async fn foreign_context_or_revoked_membership_never_reaches_browser() {
    let f = fixture().await;
    let (cookie, _) = login(&f).await;
    f.mock
        .foreign
        .store(true, std::sync::atomic::Ordering::SeqCst);
    for path in ["/api/time/context", "/api/time/requests"] {
        assert_eq!(
            request(&f.portal, path, None, Some(&cookie), None).await.0,
            StatusCode::BAD_GATEWAY
        );
    }
    f.mock
        .foreign
        .store(false, std::sync::atomic::Ordering::SeqCst);
    f.mock
        .disabled
        .store(true, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(
        request(&f.portal, "/api/time/context", None, Some(&cookie), None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn personal_inbox_projects_metadata_and_uses_saved_page_filter() {
    let f = fixture().await;
    let (cookie, _) = login(&f).await;
    let (s, _, v) = request(&f.portal, "/api/notifications", None, Some(&cookie), None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["unread_count"], 1);
    assert_eq!(v["items"][0]["target_page"], "attendance");
    assert!(!v.to_string().contains("PRIVATE_SENTINEL"));
    for k in ["company_id", "employee_id", "user_id"] {
        assert!(v.get(k).is_none());
    }
    let (s, _, v) = request(
        &f.portal,
        "/api/notifications?offset=25&unread=true",
        None,
        Some(&cookie),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v["items"], json!([]));
    let seen = f.mock.requests.lock().await;
    let (_, _, b) = seen
        .iter()
        .rev()
        .find(|(_, p, _)| p == "/rest/v1/rpc/portal_notifications_v2")
        .unwrap();
    assert_eq!(b["p_offset"], 25);
    assert_eq!(b["p_unread_only"], true);
    assert!(b.get("p_employee_id").is_none());
}
#[tokio::test]
async fn lifecycle_notices_keep_pay_links_valid_and_unimplemented_records_informational(){
    use std::sync::atomic::Ordering;
    let f=fixture().await;let(cookie,_)=login(&f).await;
    for(mode,page)in[(1,"pay"),(2,"notifications"),(3,"pay")]{
        f.mock.lifecycle_kind.store(mode,Ordering::SeqCst);
        let(status,_,value)=request(&f.portal,"/api/notifications",None,Some(&cookie),None).await;
        assert_eq!(status,StatusCode::OK,"{value}");assert_eq!(value["items"][0]["target_page"],page);
        assert!(!value.to_string().contains("PRIVATE_SENTINEL"));
    }
    for mode in[4,5]{f.mock.lifecycle_kind.store(mode,Ordering::SeqCst);let(status,_,_)=request(&f.portal,"/api/notifications",None,Some(&cookie),None).await;assert_eq!(status,StatusCode::BAD_GATEWAY);}
}
#[tokio::test]
async fn own_read_unread_reset_persist_without_browser_identity_fields() {
    let f = fixture().await;
    let (cookie, csrf) = login(&f).await;
    let key = "correction-status:00000000-0000-0000-0000-00000000006e:1";
    let (s, _, v) = request(
        &f.portal,
        "/api/notifications/read",
        Some(json!({"keys":[key],"read":true})),
        Some(&cookie),
        Some(&csrf),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v, json!({"saved":true}));
    assert_eq!(
        request(
            &f.portal,
            "/api/notifications?unread=true",
            None,
            Some(&cookie),
            None
        )
        .await
        .2["total_items"],
        0
    );
    assert_eq!(
        request(
            &f.portal,
            "/api/notifications/read",
            Some(json!({"keys":[],"read":false,"reset":true})),
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        request(&f.portal, "/api/notifications", None, Some(&cookie), None)
            .await
            .2["unread_count"],
        1
    );
    let seen = f.mock.requests.lock().await;
    for (_, _, b) in seen
        .iter()
        .filter(|(_, p, _)| p == "/rest/v1/rpc/portal_notification_reads_v2")
    {
        assert!(b.get("p_user_id").is_none());
        assert!(b.get("p_company_id").is_none());
    }
}
#[tokio::test]
async fn read_history_rejects_foreign_fields_review_keys_duplicates_and_missing_csrf() {
    let f = fixture().await;
    let (cookie, csrf) = login(&f).await;
    let key = "correction-status:00000000-0000-0000-0000-00000000006e:1";
    let b = json!({"keys":[key],"read":true});
    let mut forged = b.clone();
    forged["user_id"] = json!(Uuid::from_u128(999));
    assert_eq!(
        request(
            &f.portal,
            "/api/notifications/read",
            Some(forged),
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    for keys in [
        json!([]),
        json!([key, key]),
        json!(["leave-review:00000000-0000-0000-0000-00000000006e:1"]),
    ] {
        assert_eq!(
            request(
                &f.portal,
                "/api/notifications/read",
                Some(json!({"keys":keys,"read":true})),
                Some(&cookie),
                Some(&csrf)
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
    }
    assert_eq!(
        request(
            &f.portal,
            "/api/notifications/read",
            Some(b),
            Some(&cookie),
            None
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert!(
        !f.mock
            .requests
            .lock()
            .await
            .iter()
            .any(|(_, p, _)| p == "/rest/v1/rpc/portal_notification_reads_v2")
    );
}
#[tokio::test]
async fn notification_queries_reject_spoofed_scope_and_excessive_offsets() {
    let f = fixture().await;
    let (cookie, _) = login(&f).await;
    assert_eq!(
        request(
            &f.portal,
            "/api/notifications?employee_id=forged",
            None,
            Some(&cookie),
            None
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        request(
            &f.portal,
            "/api/notifications?offset=50001",
            None,
            Some(&cookie),
            None
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        request(&f.portal, "/api/notifications", None, None, None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}
#[tokio::test]
async fn foreign_or_disabled_notification_identity_never_reaches_browser() {
    let f = fixture().await;
    let (cookie, _) = login(&f).await;
    f.mock
        .foreign
        .store(true, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(
        request(&f.portal, "/api/notifications", None, Some(&cookie), None)
            .await
            .0,
        StatusCode::BAD_GATEWAY
    );
    f.mock
        .foreign
        .store(false, std::sync::atomic::Ordering::SeqCst);
    f.mock
        .disabled
        .store(true, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(
        request(&f.portal, "/api/notifications", None, Some(&cookie), None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn passive_notification_poll_does_not_extend_idle_session() {
    let f = fixture().await;
    let (cookie, _) = login(&f).await;
    let sid: Uuid = cookie.split_once('=').unwrap().1.parse().unwrap();
    let session = f.portal.sessions.lock().await[&sid].clone();
    let inactive = Instant::now() - Duration::from_secs(1200);
    session.lock().await.last_seen = inactive;
    assert_eq!(
        request(&f.portal, "/api/notifications", None, Some(&cookie), None)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(session.lock().await.last_seen, inactive);
    session.lock().await.last_seen = Instant::now() - Duration::from_secs(1801);
    assert_eq!(
        request(&f.portal, "/api/notifications", None, Some(&cookie), None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert!(!f.portal.sessions.lock().await.contains_key(&sid));
}

fn sample_photo() -> Vec<u8> {
    let image = image::RgbaImage::from_fn(160, 80, |x, y| {
        let n = (x.wrapping_mul(1597334677) ^ y.wrapping_mul(3812015801)).wrapping_mul(2654435761);
        image::Rgba([n as u8, (n >> 8) as u8, (n >> 16) as u8, 180])
    });
    let mut c = std::io::Cursor::new(vec![]);
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut c, image::ImageFormat::Png)
        .unwrap();
    c.into_inner()
}
async fn binary_request(
    p: &Portal,
    path: &str,
    body: Option<Vec<u8>>,
    cookie: Option<&str>,
    csrf: Option<&str>,
) -> (StatusCode, HeaderMap, Vec<u8>) {
    let mut b = axum::http::Request::builder()
        .uri(path)
        .method(if body.is_some() { "POST" } else { "GET" })
        .header("host", "127.0.0.1:3030")
        .header("origin", "http://127.0.0.1:3030");
    if let Some(c) = cookie {
        b = b.header("cookie", c);
    }
    if let Some(c) = csrf {
        b = b.header("x-csrf-token", c);
    }
    let r = p
        .router()
        .oneshot(b.body(Body::from(body.unwrap_or_default())).unwrap())
        .await
        .unwrap();
    let status = r.status();
    let headers = r.headers().clone();
    let bytes = to_bytes(r.into_body(), 3 * 1024 * 1024)
        .await
        .unwrap()
        .to_vec();
    (status, headers, bytes)
}
#[tokio::test]
async fn photo_upload_download_and_removal_use_only_current_account() {
    let f = fixture().await;
    let (cookie, csrf) = login(&f).await;
    let raw = sample_photo();
    assert!(raw.len() > 8192);
    assert_eq!(
        binary_request(
            &f.portal,
            "/api/photo",
            Some(raw.clone()),
            Some(&cookie),
            None
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert!(f.mock.photo_png.lock().await.is_none());
    let (status, _, saved) = binary_request(
        &f.portal,
        "/api/photo",
        Some(raw),
        Some(&cookie),
        Some(&csrf),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        serde_json::from_slice::<Value>(&saved).unwrap(),
        json!({"saved":true})
    );
    let (status, h, png) = binary_request(&f.portal, "/api/photo", None, Some(&cookie), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(h["content-type"], "image/png");
    assert!(h["cache-control"].to_str().unwrap().contains("no-store"));
    let img = image::load_from_memory(&png).unwrap();
    assert_eq!((img.width(), img.height()), (256, 256));
    let (_, _, brand) = request(&f.portal, "/api/branding", None, None, None).await;
    assert_eq!(brand["has_logo"], false);
    assert_eq!(
        binary_request(&f.portal, "/api/branding/logo", None, None, None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        request(
            &f.portal,
            "/api/photo/remove",
            Some(json!({"user_id":Uuid::from_u128(999)})),
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        request(
            &f.portal,
            "/api/photo/remove",
            Some(json!({})),
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        binary_request(&f.portal, "/api/photo", None, Some(&cookie), None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let seen = f.mock.requests.lock().await;
    assert!(
        seen.iter()
            .filter(|(_, p, _)| p.contains("profile-photos/"))
            .all(|(_, p, _)| p
                .split('?')
                .next()
                .unwrap()
                .ends_with(&format!("{}/avatar.png", uid())))
    );
    assert!(
        seen.iter()
            .filter(|(method, p, _)| method == "GET" && p.contains("authenticated/profile-photos/"))
            .all(|(_, p, _)| p.contains("cacheNonce="))
    );
}
#[tokio::test]
async fn pictures_reject_bad_images_spoofed_scope_and_revoked_sessions() {
    let f = fixture().await;
    let (cookie, csrf) = login(&f).await;
    assert_eq!(
        binary_request(
            &f.portal,
            "/api/photo",
            Some(b"<svg></svg>".to_vec()),
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        binary_request(
            &f.portal,
            "/api/photo",
            Some(vec![0; 5 * 1024 * 1024 + 1]),
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .0,
        StatusCode::PAYLOAD_TOO_LARGE
    );
    assert_eq!(
        binary_request(
            &f.portal,
            "/api/photo?user_id=other",
            None,
            Some(&cookie),
            None
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        binary_request(&f.portal, "/api/photo", None, None, None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    f.mock
        .disabled
        .store(true, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(
        binary_request(
            &f.portal,
            "/api/photo",
            Some(sample_photo()),
            Some(&cookie),
            Some(&csrf)
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert!(f.mock.photo_png.lock().await.is_none());
}
#[tokio::test]
async fn only_verified_company_branding_survives_sign_out() {
    let f = fixture().await;
    let (cookie, csrf) = login(&f).await;
    *f.mock.logo_png.lock().await = Some(sample_photo());
    let (status, _, v) = request(&f.portal, "/api/pictures", None, Some(&cookie), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v, json!({"company_name":"Sample Company","has_logo":true}));
    request(
        &f.portal,
        "/api/sign-out",
        Some(json!({})),
        Some(&cookie),
        Some(&csrf),
    )
    .await;
    let (status, _, v) = request(&f.portal, "/api/branding", None, None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v, json!({"company_name":"Sample Company","has_logo":true}));
    assert!(!v.to_string().contains("PRIVATE_SENTINEL"));
    assert_eq!(
        binary_request(&f.portal, "/api/branding/logo", None, None, None)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        binary_request(&f.portal, "/api/company-logo", None, Some(&cookie), None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn company_logo_revision_refresh_replaces_cached_image_and_removal_clears_it() {
    let f = fixture().await;
    let (cookie, _) = login(&f).await;
    *f.mock.logo_png.lock().await = Some(sample_photo());
    assert_eq!(
        request(&f.portal, "/api/pictures", None, Some(&cookie), None)
            .await
            .0,
        StatusCode::OK
    );
    let (_, _, first) = binary_request(&f.portal, "/api/branding/logo", None, None, None).await;
    let mut c = std::io::Cursor::new(vec![]);
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        80,
        40,
        image::Rgba([250, 30, 100, 100]),
    ))
    .write_to(&mut c, image::ImageFormat::Png)
    .unwrap();
    *f.mock.logo_png.lock().await = Some(c.into_inner());
    f.mock
        .logo_revision
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(
        request(&f.portal, "/api/pictures", None, Some(&cookie), None)
            .await
            .0,
        StatusCode::OK
    );
    let (status, _, second) =
        binary_request(&f.portal, "/api/branding/logo", None, None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_ne!(first, second);
    *f.mock.logo_png.lock().await = None;
    let (_, _, context) = request(&f.portal, "/api/pictures", None, Some(&cookie), None).await;
    assert_eq!(context["has_logo"], false);
    assert_eq!(
        binary_request(&f.portal, "/api/branding/logo", None, None, None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}
