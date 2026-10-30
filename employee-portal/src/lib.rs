//! Own-record portal. No service key, token-bearing browser storage or arbitrary proxy.
#[cfg(feature = "android-ui")]
mod android_ui;
mod services;
mod account_security;
mod notifications;
mod device_alerts;
mod pictures;
mod requests;
mod self_service;
use anyhow::{Context, Result, ensure};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use chrono::Utc;
use reqwest::{Client, Method};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, Semaphore};
use uuid::Uuid;

#[derive(Clone)]
pub struct Config {
    url: String,
    key: String,
    origin: String,
    authority: String,
    secure: bool,
    branding: pictures::Branding,
}
impl Config {
    pub fn load(path: &std::path::Path, origin: &str, listen: &str) -> Result<Self> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Profile {
            url: String,
            publishable_key: String,
        }
        let source =
            std::fs::read_to_string(path).context("Cannot read the operator's company profile.")?;
        let p: Profile = serde_json::from_str(source.trim_start_matches('\u{feff}'))
            .context("Invalid company profile.")?;
        let url = reqwest::Url::parse(&p.url)?;
        ensure!(
            url.scheme() == "https"
                && url.host_str().is_some_and(|h| h.ends_with(".supabase.co"))
                && url.username().is_empty()
                && url.password().is_none()
                && url.path() == "/"
                && url.query().is_none()
                && url.fragment().is_none()
                && url.port().is_none(),
            "Use a dedicated Supabase HTTPS origin."
        );
        ensure!(
            p.publishable_key.starts_with("sb_publishable_")
                && p.publishable_key.len() <= 512
                && !p.publishable_key.contains(char::is_whitespace),
            "A publishable key is required; secret keys are forbidden."
        );
        let mut config = Self::origin(origin, listen)?;
        config.url = p.url.trim_end_matches('/').into();
        config.key = p.publishable_key;
        config.branding =
            pictures::load_bootstrap(&path.with_file_name("hris-branding.json"), &config)?;
        Ok(config)
    }
    fn origin(origin: &str, listen: &str) -> Result<Self> {
        let u = reqwest::Url::parse(origin)?;
        let bind: SocketAddr = listen.parse()?;
        ensure!(
            u.path() == "/"
                && u.query().is_none()
                && u.fragment().is_none()
                && u.username().is_empty()
                && u.password().is_none(),
            "Use a public origin without credentials, path or query."
        );
        let secure = u.scheme() == "https";
        ensure!(
            secure
                || (u.scheme() == "http"
                    && matches!(u.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"))
                    && bind.ip().is_loopback()),
            "HTTPS is required outside a loopback development server."
        );
        let origin = u.origin().ascii_serialization();
        let authority = origin.split_once("://").unwrap().1.to_string();
        Ok(Self {
            url: String::new(),
            key: String::new(),
            origin,
            authority,
            secure,
            branding: Default::default(),
        })
    }
    fn cookie_name(&self) -> &str {
        if self.secure {
            "__Host-hris_portal"
        } else {
            "hris_portal"
        }
    }
    fn cookie(&self, id: Option<Uuid>) -> String {
        format!(
            "{}={}; Path=/; HttpOnly; SameSite=Strict{}{}",
            self.cookie_name(),
            id.map(|x| x.to_string()).unwrap_or_default(),
            if self.secure { "; Secure" } else { "" },
            if id.is_none() { "; Max-Age=0" } else { "" }
        )
    }
}

// No Debug implementation: secrets must not enter diagnostics.
struct Session {
    access: String,
    refresh: String,
    expires: Instant,
    last_seen: Instant,
    csrf: Uuid,
    user: Uuid,
    company: Uuid,
    employee: Uuid,
}
struct Attempt {
    first: Instant,
    count: u8,
}
#[derive(Clone)]
pub struct Portal {
    config: Arc<Config>,
    client: Client,
    sessions: Arc<Mutex<HashMap<Uuid, Arc<Mutex<Session>>>>>,
    attempts: Arc<Mutex<HashMap<String, Attempt>>>,
    auth_slots: Arc<Semaphore>,
    image_slots: Arc<Semaphore>,
    branding: Arc<Mutex<pictures::Branding>>,
}
#[derive(Debug)]
struct Error(StatusCode, &'static str);
type Reply<T> = std::result::Result<T, Error>;
impl IntoResponse for Error {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"error":self.1}))).into_response()
    }
}
fn invalid(message: &'static str) -> Error {
    Error(StatusCode::BAD_REQUEST, message)
}
fn expired() -> Error {
    Error(
        StatusCode::UNAUTHORIZED,
        "Your session ended. Sign in again.",
    )
}
fn unavailable() -> Error {
    Error(
        StatusCode::BAD_GATEWAY,
        "The company service is unavailable. Try again.",
    )
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Credentials {
    email: String,
    password: String,
}
#[derive(Deserialize, Serialize, Default)]
#[serde(deny_unknown_fields)]
struct Contacts {
    phone: String,
    personal_email: String,
    address: String,
    emergency_name: String,
    emergency_relationship: String,
    emergency_phone: String,
    emergency_alternate_phone: String,
}
impl Contacts {
    fn validate(&mut self) -> Reply<()> {
        for s in [
            &mut self.phone,
            &mut self.personal_email,
            &mut self.address,
            &mut self.emergency_name,
            &mut self.emergency_relationship,
            &mut self.emergency_phone,
            &mut self.emergency_alternate_phone,
        ] {
            *s = s.trim().into();
        }
        if self.address.chars().count() > 500
            || self.emergency_name.chars().count() > 120
            || self.emergency_relationship.chars().count() > 80
        {
            return Err(invalid("A contact field is too long."));
        }
        if !self.personal_email.is_empty() && !email_ok(&self.personal_email) {
            return Err(invalid("Enter a valid personal email."));
        }
        for p in [
            &self.phone,
            &self.emergency_phone,
            &self.emergency_alternate_phone,
        ] {
            if !p.is_empty()
                && (p.len() > 40
                    || p.chars().filter(char::is_ascii_digit).count() < 3
                    || !p
                        .chars()
                        .all(|c| c.is_ascii_digit() || " +()-./xX#".contains(c)))
            {
                return Err(invalid(
                    "Use a phone number up to 40 characters with at least three digits.",
                ));
            }
        }
        let any = [
            &self.emergency_name,
            &self.emergency_relationship,
            &self.emergency_phone,
            &self.emergency_alternate_phone,
        ]
        .iter()
        .any(|s| !s.is_empty());
        if any
            && (self.emergency_name.is_empty()
                || self.emergency_relationship.is_empty()
                || self.emergency_phone.is_empty())
        {
            return Err(invalid(
                "Enter the emergency contact name, relationship and primary phone, or leave all emergency fields blank.",
            ));
        }
        Ok(())
    }
}
fn email_ok(s: &str) -> bool {
    s.len() <= 254
        && !s.chars().any(|c| c.is_whitespace() || c.is_control())
        && s.split_once('@').is_some_and(|(a, b)| {
            !a.is_empty()
                && b.contains('.')
                && !b.starts_with('.')
                && !b.ends_with('.')
                && !b.contains('@')
        })
}
fn id(v: &Value, key: &str) -> Reply<Uuid> {
    v[key]
        .as_str()
        .and_then(|s| s.parse().ok())
        .ok_or_else(unavailable)
}
fn first(v: Value) -> Reply<Value> {
    v.as_array()
        .and_then(|a| a.first())
        .cloned()
        .ok_or_else(expired)
}
fn project(v: &Value, keys: &[&str]) -> Value {
    let mut out = serde_json::Map::new();
    for k in keys {
        out.insert((*k).into(), v.get(*k).cloned().unwrap_or(Value::Null));
    }
    Value::Object(out)
}
fn own(v: &Value, s: &Session) -> Reply<()> {
    if id(v, "company_id")? != s.company || id(v, "employee_id")? != s.employee {
        return Err(unavailable());
    }
    Ok(())
}
const CONTACT_FIELDS: &[&str] = &[
    "phone",
    "personal_email",
    "address",
    "emergency_name",
    "emergency_relationship",
    "emergency_phone",
    "emergency_alternate_phone",
];
const TOTAL_FIELDS: &[&str] = &[
    "basic",
    "paid_leave",
    "unworked_holiday",
    "premium",
    "overtime",
    "night",
    "allowances",
    "exempt_allowances",
    "reimbursement",
    "benefits",
    "voluntary_deductions",
    "advances",
    "gross",
    "taxable",
    "sss_basis",
    "sss_msc",
    "sss_employee",
    "sss_employer",
    "sss_regular_employee",
    "sss_regular_employer",
    "sss_mpf_employee",
    "sss_mpf_employer",
    "ec",
    "philhealth_employee",
    "philhealth_employer",
    "pagibig_employee",
    "pagibig_employer",
    "withholding",
    "net",
    "thirteenth_accrual",
];

impl Portal {
    pub fn new(config: Config) -> Result<Self> {
        let branding = Arc::new(Mutex::new(config.branding.clone()));
        let client = Client::builder()
            .timeout(Duration::from_secs(20))
            .redirect(reqwest::redirect::Policy::none());
        #[cfg(feature = "android-tls")]
        let client = {
            // Android uses reviewed Mozilla roots without a Java trust-manager bridge.
            // Certificate and hostname verification remain enabled.
            let roots = rustls::RootCertStore { roots: webpki_roots::TLS_SERVER_ROOTS.to_vec() };
            let tls = rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
                .with_safe_default_protocol_versions()?
                .with_root_certificates(roots)
                .with_no_client_auth();
            client.use_preconfigured_tls(tls)
        };
        Ok(Self {
            config: Arc::new(config),
            client: client.build()?,
            sessions: Default::default(),
            attempts: Default::default(),
            auth_slots: Arc::new(Semaphore::new(8)),
            image_slots: Arc::new(Semaphore::new(2)),
            branding,
        })
    }
    pub fn router(&self) -> Router {
        let router = Router::new()
            .route("/", get(index))
            .route("/app.css", get(styles))
            .route("/app.js", get(script))
            .route("/self-service.js", get(self_service_script))
            .route("/api/sign-in", post(sign_in))
            .route("/api/sign-out", post(sign_out))
            .route("/api/me", get(me))
            .route("/api/contacts", post(contacts))
            .route("/api/attendance", get(attendance))
            .route("/api/attendance/check-in", post(check_in))
            .route("/api/attendance/check-out", post(check_out))
            .route("/api/payslips", get(payslips))
            .route("/api/payslips/{id}", get(payslip))
            .merge(self_service::routes())
            .merge(requests::routes())
            .merge(services::routes())
            .merge(notifications::routes())
            .merge(device_alerts::routes())
            .merge(pictures::routes())
            .merge(account_security::routes())
            .route("/services.js", get(services_script))
            .route("/notifications.js", get(notification_script))
            .route("/time-requests.js", get(time_requests_script));
        #[cfg(feature = "android-ui")]
        let router = router.merge(android_ui::routes());
        router.layer(DefaultBodyLimit::max(8192))
            .layer(middleware::from_fn_with_state(self.clone(), security))
            .with_state(self.clone())
    }
    async fn upstream(
        &self,
        path: &str,
        method: Method,
        token: Option<&str>,
        body: Option<Value>,
    ) -> Reply<Value> {
        let mut r = self
            .client
            .request(method, format!("{}{path}", self.config.url))
            .header("apikey", &self.config.key)
            .header("Cache-Control", "no-cache")
            .header("Prefer", "return=representation");
        // Older desktop clients cannot decode custom/cancelled leave. Opt into
        // the existing read contract without changing its authorization rules.
        if path.starts_with("/rest/v1/leave_requests?") {
            r = r.header("x-hris-leave-contract", "2");
        }
        if let Some(t) = token {
            r = r.bearer_auth(t);
        }
        if let Some(b) = body {
            r = r.json(&b);
        }
        let response = r.send().await.map_err(|_| unavailable())?;
        let status = response.status();
        if !status.is_success() {
            return Err(match status.as_u16() {
                401 | 403 => Error(
                    StatusCode::FORBIDDEN,
                    "Access is unavailable. Refresh or contact HR.",
                ),
                409 => Error(
                    StatusCode::CONFLICT,
                    "This record changed or already exists. Refresh before trying again.",
                ),
                400 | 422 => {
                    invalid("The company service rejected the fields. Check them and refresh.")
                }
                429 => Error(
                    StatusCode::TOO_MANY_REQUESTS,
                    "Too many attempts. Wait a few minutes.",
                ),
                _ => unavailable(),
            });
        }
        if response.content_length().is_some_and(|n| n > 5_000_000) {
            return Err(unavailable());
        }
        let bytes = response.bytes().await.map_err(|_| unavailable())?;
        if bytes.len() > 5_000_000 {
            return Err(unavailable());
        }
        if bytes.is_empty() {
            Ok(Value::Null)
        } else {
            serde_json::from_slice(&bytes).map_err(|_| unavailable())
        }
    }
    async fn access(&self, s: &Session) -> Reply<Value> {
        let user = self
            .upstream("/auth/v1/user", Method::GET, Some(&s.access), None)
            .await
            .map_err(|e| {
                if e.0 == StatusCode::FORBIDDEN {
                    expired()
                } else {
                    e
                }
            })?;
        let a = self
            .upstream(
                "/rest/v1/rpc/my_hris_access",
                Method::POST,
                Some(&s.access),
                Some(json!({})),
            )
            .await
            .map_err(|e| {
                if e.0 == StatusCode::FORBIDDEN {
                    expired()
                } else {
                    e
                }
            })?;
        if id(&user, "id")? != s.user
            || id(&a, "user_id")? != s.user
            || id(&a, "company_id")? != s.company
            || id(&a, "employee_id")? != s.employee
        {
            return Err(expired());
        }
        Ok(
            json!({"email":user["email"],"pending_email":user["new_email"],"reports_to_name":a["reports_to_name"]}),
        )
    }
    async fn session(&self, h: &HeaderMap, write: bool) -> Reply<Arc<Mutex<Session>>> {
        self.session_activity(h, write, true).await
    }
    // Passive inbox polling must still validate live access and expiry, but must
    // not keep an unattended shared-device session alive indefinitely.
    async fn session_activity(
        &self,
        h: &HeaderMap,
        write: bool,
        activity: bool,
    ) -> Reply<Arc<Mutex<Session>>> {
        let sid = cookie_id(h, self.config.cookie_name()).ok_or_else(expired)?;
        let session = self
            .sessions
            .lock()
            .await
            .get(&sid)
            .cloned()
            .ok_or_else(expired)?;
        let mut s = session.lock().await;
        if s.last_seen.elapsed() > Duration::from_secs(1800) {
            drop(s);
            self.sessions.lock().await.remove(&sid);
            return Err(expired());
        }
        if write
            && h.get("x-csrf-token")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<Uuid>().ok())
                != Some(s.csrf)
        {
            return Err(Error(
                StatusCode::FORBIDDEN,
                "Refresh this page before submitting.",
            ));
        }
        if s.expires <= Instant::now() + Duration::from_secs(60) {
            let v = self
                .upstream(
                    "/auth/v1/token?grant_type=refresh_token",
                    Method::POST,
                    None,
                    Some(json!({"refresh_token":s.refresh})),
                )
                .await
                .map_err(|_| expired())?;
            set_tokens(&mut s, &v)?;
        }
        self.access(&s).await?;
        if activity {
            s.last_seen = Instant::now();
        }
        drop(s);
        Ok(session)
    }
    async fn revoke(&self, s: &Session) {
        let _ = self
            .upstream(
                "/auth/v1/logout?scope=local",
                Method::POST,
                Some(&s.access),
                Some(json!({})),
            )
            .await;
    }
    pub async fn close(&self) {
        let sessions = std::mem::take(&mut *self.sessions.lock().await);
        for (_, s) in sessions {
            self.revoke(&*s.lock().await).await;
        }
    }
}
fn cookie_id(h: &HeaderMap, name: &str) -> Option<Uuid> {
    let mut ids = vec![];
    for h in h.get_all(header::COOKIE) {
        for p in h.to_str().ok()?.split(';') {
            if let Some((k, v)) = p.trim().split_once('=') {
                if k == name {
                    ids.push(v.parse().ok()?);
                }
            }
        }
    }
    if ids.len() == 1 { Some(ids[0]) } else { None }
}
fn set_tokens(s: &mut Session, v: &Value) -> Reply<()> {
    s.access = v["access_token"].as_str().ok_or_else(unavailable)?.into();
    s.refresh = v["refresh_token"].as_str().ok_or_else(unavailable)?.into();
    let secs = v["expires_in"]
        .as_u64()
        .filter(|n| *n >= 60 && *n <= 86400)
        .ok_or_else(unavailable)?;
    s.expires = Instant::now() + Duration::from_secs(secs);
    Ok(())
}
async fn security(State(p): State<Portal>, request: Request, next: Next) -> Response {
    let h = request.headers();
    let host_ok =
        h.get(header::HOST).and_then(|x| x.to_str().ok()) == Some(p.config.authority.as_str());
    let origin_ok = request.method() == Method::GET
        || h.get(header::ORIGIN).and_then(|x| x.to_str().ok()) == Some(p.config.origin.as_str());
    let mut r = if !host_ok || !origin_ok {
        Error(StatusCode::FORBIDDEN, "This request origin is not allowed.").into_response()
    } else {
        next.run(request).await
    };
    for (k, v) in [
        ("cache-control", "no-store, private"),
        ("pragma", "no-cache"),
        ("x-content-type-options", "nosniff"),
        ("referrer-policy", "no-referrer"),
        (
            "permissions-policy",
            "camera=(), microphone=(), geolocation=()",
        ),
        (
            "content-security-policy",
            "default-src 'self'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self' blob:; frame-ancestors 'none'; base-uri 'none'; form-action 'self'; object-src 'none'",
        ),
    ] {
        r.headers_mut().insert(k, HeaderValue::from_static(v));
    }
    if p.config.secure {
        r.headers_mut().insert(
            "strict-transport-security",
            HeaderValue::from_static("max-age=31536000"),
        );
    }
    if r.status() == StatusCode::UNAUTHORIZED {
        r.headers_mut().insert(
            header::SET_COOKIE,
            HeaderValue::from_str(&p.config.cookie(None)).unwrap(),
        );
    }
    r
}
async fn index() -> impl IntoResponse {
    #[cfg(not(feature = "android-ui"))]
    let html = include_str!("../web/index.html");
    #[cfg(feature = "android-ui")]
    let html = include_str!("../web/index.html")
        .replace("data-page=\"attendance\">Attendance", "data-page=\"attendance\" aria-label=\"Attendance\">Time")
        .replace("data-page=\"pay\">Payslips", "data-page=\"pay\" aria-label=\"Payslips\">Pay")
        .replace("data-page=\"services\">Services", "data-page=\"services\" aria-label=\"Employee services\">Help")
        .replace("class=\"profile-link\">", "class=\"profile-link\" aria-label=\"My profile\">")
        .replace("<span>My profile</span>", "<span>Profile</span>");
    (
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        html,
    )
}
#[cfg(not(feature = "android-ui"))]
const APP_STYLES: &str = include_str!("../web/app.css");
#[cfg(feature = "android-ui")]
const APP_STYLES: &str = concat!(include_str!("../web/app.css"), "\n", include_str!("../web/android.css"));
async fn styles() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        APP_STYLES,
    )
}
async fn script() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("../web/app.js"),
    )
}
async fn self_service_script() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("../web/self-service.js"),
    )
}
async fn time_requests_script() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("../web/time-requests.js"),
    )
}
async fn services_script() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/javascript; charset=utf-8")], include_str!("../web/services.js"))
}
async fn notification_script() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("../web/notifications.js"),
    )
}
async fn sign_in(
    State(p): State<Portal>,
    h: HeaderMap,
    Json(c): Json<Credentials>,
) -> Reply<Response> {
    if !email_ok(c.email.trim()) || c.password.is_empty() || c.password.len() > 512 {
        return Err(invalid("Enter your company account email and password."));
    }
    let _permit = p.auth_slots.try_acquire().map_err(|_| {
        Error(
            StatusCode::TOO_MANY_REQUESTS,
            "Sign-in is busy. Try again shortly.",
        )
    })?;
    {
        let mut attempts = p.attempts.lock().await;
        attempts.retain(|_, v| v.first.elapsed() < Duration::from_secs(300));
        let key = c.email.trim().to_lowercase();
        if !attempts.contains_key(&key) && attempts.len() >= 2048 {
            return Err(Error(
                StatusCode::TOO_MANY_REQUESTS,
                "Sign-in is busy. Try again later.",
            ));
        }
        let a = attempts.entry(key).or_insert(Attempt {
            first: Instant::now(),
            count: 0,
        });
        if a.count >= 10 {
            return Err(Error(
                StatusCode::TOO_MANY_REQUESTS,
                "Too many sign-in attempts. Wait five minutes.",
            ));
        }
        a.count += 1;
    }
    let v = p
        .upstream(
            "/auth/v1/token?grant_type=password",
            Method::POST,
            None,
            Some(json!({"email":c.email.trim(),"password":c.password})),
        )
        .await
        .map_err(|_| {
            Error(
                StatusCode::UNAUTHORIZED,
                "Sign-in failed. Check your credentials and account access.",
            )
        })?;
    let mut s = Session {
        access: String::new(),
        refresh: String::new(),
        expires: Instant::now(),
        last_seen: Instant::now(),
        csrf: Uuid::new_v4(),
        user: id(&v["user"], "id")?,
        company: Uuid::nil(),
        employee: Uuid::nil(),
    };
    set_tokens(&mut s, &v)?;
    let access = p
        .upstream(
            "/rest/v1/rpc/my_hris_access",
            Method::POST,
            Some(&s.access),
            Some(json!({})),
        )
        .await;
    let result = async {
        let a = access?;
        if id(&a, "user_id")? != s.user {
            return Err(expired());
        }
        s.company = id(&a, "company_id")?;
        s.employee = id(&a, "employee_id")?;
        let person = first(
            p.upstream(
                &format!(
                    "/rest/v1/employees?select=id,status&company_id=eq.{}&id=eq.{}",
                    s.company, s.employee
                ),
                Method::GET,
                Some(&s.access),
                None,
            )
            .await?,
        )?;
        if person["status"] != "active" {
            return Err(expired());
        }
        Ok(())
    }
    .await;
    if let Err(e) = result {
        p.revoke(&s).await;
        return Err(e);
    }
    let sid = Uuid::new_v4();
    let csrf = s.csrf;
    if let Some(old) = cookie_id(&h, p.config.cookie_name()) {
        let prior = p.sessions.lock().await.remove(&old);
        if let Some(prior) = prior {
            p.revoke(&*prior.lock().await).await;
        }
    }
    {
        let mut sessions = p.sessions.lock().await;
        sessions.retain(|_, v| {
            v.try_lock()
                .map(|s| s.last_seen.elapsed() < Duration::from_secs(1800))
                .unwrap_or(true)
        });
        if sessions.len() >= 512 {
            drop(sessions);
            p.revoke(&s).await;
            return Err(Error(
                StatusCode::TOO_MANY_REQUESTS,
                "The portal is busy. Try again later.",
            ));
        }
        sessions.insert(sid, Arc::new(Mutex::new(s)));
    }
    let mut r = Json(json!({"csrf":csrf})).into_response();
    r.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&p.config.cookie(Some(sid))).unwrap(),
    );
    Ok(r)
}
async fn sign_out(State(p): State<Portal>, h: HeaderMap) -> Reply<Response> {
    if let Some(sid) = cookie_id(&h, p.config.cookie_name()) {
        let entry = p.sessions.lock().await.get(&sid).cloned();
        if let Some(entry) = entry {
            let s = entry.lock().await;
            if h.get("x-csrf-token")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<Uuid>().ok())
                != Some(s.csrf)
            {
                return Err(Error(StatusCode::FORBIDDEN, "Refresh before signing out."));
            }
            p.sessions.lock().await.remove(&sid);
            p.revoke(&s).await;
        }
    }
    let mut r = Json(json!({"signed_out":true})).into_response();
    r.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&p.config.cookie(None)).unwrap(),
    );
    Ok(r)
}
async fn me(State(p): State<Portal>, h: HeaderMap) -> Reply<Json<Value>> {
    let session = p.session(&h, false).await?;
    let s = session.lock().await;
    let access = p.access(&s).await?;
    let person=first(p.upstream(&format!("/rest/v1/employees?select=id,company_id,employee_code,full_name,department,job_title,joined_on,status,annual_leave_days,employment_type_id,work_location_id&company_id=eq.{}&id=eq.{}",s.company,s.employee),Method::GET,Some(&s.access),None).await?)?;
    if person["status"] != "active" {
        return Err(expired());
    }
    if id(&person, "id")? != s.employee || id(&person, "company_id")? != s.company {
        return Err(unavailable());
    }
    let company = first(
        p.upstream(
            &format!(
                "/rest/v1/companies?select=id,name,timezone&id=eq.{}",
                s.company
            ),
            Method::GET,
            Some(&s.access),
            None,
        )
        .await?,
    )?;
    if id(&company, "id")? != s.company {
        return Err(unavailable());
    }
    let contacts=p.upstream(&format!("/rest/v1/employee_contacts?select=company_id,employee_id,phone,personal_email,address,emergency_name,emergency_relationship,emergency_phone,emergency_alternate_phone&company_id=eq.{}&employee_id=eq.{}",s.company,s.employee),Method::GET,Some(&s.access),None).await?;
    let rows = contacts.as_array().ok_or_else(unavailable)?;
    if rows.len() > 1 {
        return Err(unavailable());
    }
    let contact = if let Some(c) = rows.first() {
        own(c, &s)?;
        project(c, CONTACT_FIELDS)
    } else {
        serde_json::to_value(Contacts::default()).unwrap()
    };
    let mut employment = project(
        &person,
        &[
            "employee_code",
            "full_name",
            "department",
            "job_title",
            "joined_on",
            "annual_leave_days",
        ],
    );
    employment["reports_to"] = access["reports_to_name"].clone();
    for (field, table, target) in [
        ("employment_type_id", "employment_types", "employment_type"),
        ("work_location_id", "work_locations", "work_location"),
    ] {
        employment[target] = Value::Null;
        if let Some(id) = person[field].as_str().and_then(|v| v.parse::<Uuid>().ok()) {
            let rows = p
                .upstream(
                    &format!(
                        "/rest/v1/{table}?select=name&company_id=eq.{}&id=eq.{id}",
                        s.company
                    ),
                    Method::GET,
                    Some(&s.access),
                    None,
                )
                .await?;
            employment[target] = rows
                .as_array()
                .and_then(|a| a.first())
                .map(|x| x["name"].clone())
                .unwrap_or(Value::Null);
        }
    }
    Ok(Json(
        json!({"csrf":s.csrf,"company":project(&company,&["name","timezone"]),"employment":employment,"contacts":contact,"email":access["email"]}),
    ))
}
async fn contacts(
    State(p): State<Portal>,
    h: HeaderMap,
    Json(mut c): Json<Contacts>,
) -> Reply<Json<Value>> {
    c.validate()?;
    let session = p.session(&h, true).await?;
    let s = session.lock().await;
    let mut body = serde_json::to_value(c).unwrap();
    body["company_id"] = json!(s.company);
    body["employee_id"] = json!(s.employee);
    let path = format!(
        "/rest/v1/employee_contacts?company_id=eq.{}&employee_id=eq.{}",
        s.company, s.employee
    );
    let existing = p
        .upstream(
            &(path.clone() + "&select=id"),
            Method::GET,
            Some(&s.access),
            None,
        )
        .await?;
    if existing.as_array().is_some_and(|a| !a.is_empty()) {
        body.as_object_mut().unwrap().remove("company_id");
        body.as_object_mut().unwrap().remove("employee_id");
        p.upstream(&path, Method::PATCH, Some(&s.access), Some(body))
            .await?;
    } else {
        p.upstream(
            "/rest/v1/employee_contacts",
            Method::POST,
            Some(&s.access),
            Some(body),
        )
        .await?;
    }
    Ok(Json(json!({"saved":true})))
}
async fn attendance(State(p): State<Portal>, h: HeaderMap) -> Reply<Json<Value>> {
    let session = p.session(&h, false).await?;
    let s = session.lock().await;
    let path = format!(
        "/rest/v1/attendance?select=id,company_id,employee_id,work_date,checked_in_at,checked_out_at&company_id=eq.{}&employee_id=eq.{}",
        s.company, s.employee
    );
    let rows = p
        .upstream(
            &(path.clone() + "&order=work_date.desc&limit=93"),
            Method::GET,
            Some(&s.access),
            None,
        )
        .await?;
    let open = p
        .upstream(
            &(path + "&checked_out_at=is.null&limit=1"),
            Method::GET,
            Some(&s.access),
            None,
        )
        .await?;
    let mut records = rows.as_array().ok_or_else(unavailable)?.clone();
    for a in open.as_array().ok_or_else(unavailable)? {
        if !records.iter().any(|r| r["id"] == a["id"]) {
            records.push(a.clone());
        }
    }
    let mut output = vec![];
    for a in records {
        own(&a, &s)?;
        output.push(project(
            &a,
            &["id", "work_date", "checked_in_at", "checked_out_at"],
        ));
    }
    Ok(Json(json!(output)))
}
async fn check_in(State(p): State<Portal>, h: HeaderMap) -> Reply<Json<Value>> {
    let session = p.session(&h, true).await?;
    let s = session.lock().await;
    let v = first(
        p.upstream(
            "/rest/v1/attendance",
            Method::POST,
            Some(&s.access),
            Some(json!({"company_id":s.company,"employee_id":s.employee})),
        )
        .await?,
    )?;
    own(&v, &s)?;
    Ok(Json(json!({"saved":true})))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ClockOut {
    id: Uuid,
}
async fn check_out(
    State(p): State<Portal>,
    h: HeaderMap,
    Json(body): Json<ClockOut>,
) -> Reply<Json<Value>> {
    let session = p.session(&h, true).await?;
    let s = session.lock().await;
    let v=p.upstream(&format!("/rest/v1/attendance?id=eq.{}&company_id=eq.{}&employee_id=eq.{}&checked_out_at=is.null",body.id,s.company,s.employee),Method::PATCH,Some(&s.access),Some(json!({"checked_out_at":Utc::now()}))).await?;
    let rows = v.as_array().ok_or_else(unavailable)?;
    if rows.is_empty() {
        return Err(invalid(
            "No matching open attendance record. Refresh first.",
        ));
    }
    for r in rows {
        own(r, &s)?;
    }
    Ok(Json(json!({"saved":true})))
}
async fn payslips(State(p): State<Portal>, h: HeaderMap) -> Reply<Json<Value>> {
    let session = p.session(&h, false).await?;
    let s = session.lock().await;
    let rows=p.upstream(&format!("/rest/v1/payroll_slips?select=id,company_id,employee_id,month,company_name,approved_at,item&company_id=eq.{}&employee_id=eq.{}&order=month.desc&limit=60",s.company,s.employee),Method::GET,Some(&s.access),None).await?;
    let mut out = vec![];
    for r in rows.as_array().ok_or_else(unavailable)? {
        own(r, &s)?;
        own(&r["item"], &s)?;
        out.push(json!({"id":r["id"],"month":r["month"],"company_name":r["company_name"],"approved_at":r["approved_at"],"net":r["item"]["totals"]["net"]}));
    }
    Ok(Json(json!(out)))
}
async fn payslip(
    State(p): State<Portal>,
    h: HeaderMap,
    Path(slip): Path<Uuid>,
) -> Reply<Json<Value>> {
    let session = p.session(&h, false).await?;
    let s = session.lock().await;
    let v=first(p.upstream(&format!("/rest/v1/payroll_slips?select=id,company_id,employee_id,month,company_name,approved_at,rule_version,item&company_id=eq.{}&employee_id=eq.{}&id=eq.{slip}",s.company,s.employee),Method::GET,Some(&s.access),None).await?).map_err(|_|Error(StatusCode::NOT_FOUND,"Payslip not available."))?;
    own(&v, &s)?;
    own(&v["item"], &s)?;
    if id(&v, "id")? != slip {
        return Err(unavailable());
    }
    // A second explicit projection protects the browser even if the upstream
    // published schema later grows HR-only fields.
    Ok(Json(
        json!({"id":v["id"],"month":v["month"],"company_name":v["company_name"],"approved_at":v["approved_at"],"rule_version":v["rule_version"],"employee":project(&v["item"],&["employee_code","employee_name","department"]),"totals":project(&v["item"]["totals"],TOTAL_FIELDS),"components":v["item"]["profile"]["pay"]["components"].as_array().map(|a|a.iter().map(|c|project(c,&["label","kind","amount_cents"])).collect::<Vec<_>>()).unwrap_or_default(),"adjustments":v["item"]["worksheet"]["adjustments"].as_array().map(|a|a.iter().map(|c|project(c,&["label","kind","amount_cents"])).collect::<Vec<_>>()).unwrap_or_default()}),
    ))
}

#[cfg(test)]
mod tests;
