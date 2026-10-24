//! Own credentials only. Never expose upstream sessions or accept a target user.
use super::*;
use axum::extract::Query;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use sha2::{Digest, Sha256};
pub(super) fn routes() -> Router<Portal> {
    Router::new()
        .route("/account.js", get(script))
        .route("/api/account", get(account))
        .route("/api/account/password", post(password))
        .route("/api/account/email", post(email))
        .route("/api/account/sign-out-others", post(sign_out_others))
        .route("/account/confirm-email", get(confirmation_return))
}
// Confirmation is not a sign-in. GoTrue commits an email change before its PKCE
// code exchange (auth/internal/api/verify.go). Request S256 so the emailed link
// never redirects bearer tokens into browser fragments. We deliberately discard
// the verifier: this portal uses password sign-in and does not redeem that code.
// No callback may create, restore or replace a portal session.
fn confirmation_challenge() -> String {
    let verifier = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}
async fn confirmation_return() -> impl IntoResponse {
    // Ignore all untrusted confirmation parameters. Only a fresh authenticated
    // account-status request can establish the resulting email or pending state.
    axum::response::Redirect::to("/")
}
async fn script() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("../web/account.js"),
    )
}
async fn details(p: &Portal, s: &Session) -> Reply<Value> {
    let a = p.access(s).await?;
    let email = a["email"]
        .as_str()
        .filter(|v| email_ok(v))
        .ok_or_else(unavailable)?;
    let pending = a["pending_email"].as_str().unwrap_or("");
    if !pending.is_empty() && !email_ok(pending) {
        return Err(unavailable());
    }
    Ok(
        json!({"email":email,"pending_email":pending,"confirmation_pending":!pending.is_empty(),"idle_timeout_minutes":30}),
    )
}
async fn account(
    State(p): State<Portal>,
    h: HeaderMap,
    Query(_): Query<Empty>,
) -> Reply<Json<Value>> {
    let e = p.session(&h, false).await?;
    let s = e.lock().await;
    Ok(Json(details(&p, &s).await?))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Password {
    current_password: String,
    new_password: String,
    confirmation: String,
}
fn current_valid(value: &str) -> Reply<()> {
    if value.is_empty() || value.len() > 1024 {
        return Err(invalid("Enter your current password."));
    }
    Ok(())
}
fn password_valid(d: &Password) -> Reply<()> {
    current_valid(&d.current_password)?;
    if !(12..=128).contains(&d.new_password.chars().count()) {
        return Err(invalid("Use a new password between 12 and 128 characters."));
    }
    if d.new_password != d.confirmation {
        return Err(invalid("The new passwords do not match."));
    }
    if d.current_password == d.new_password {
        return Err(invalid("Choose a different new password."));
    }
    Ok(())
}
async fn throttle(p: &Portal, user: Uuid) -> Reply<()> {
    let mut attempts = p.attempts.lock().await;
    attempts.retain(|_, v| v.first.elapsed() < Duration::from_secs(300));
    let key = format!("security:{user}");
    if !attempts.contains_key(&key) && attempts.len() >= 2048 {
        return Err(Error(
            StatusCode::TOO_MANY_REQUESTS,
            "Account security is busy. Try again later.",
        ));
    }
    let a = attempts.entry(key).or_insert(Attempt {
        first: Instant::now(),
        count: 0,
    });
    if a.count >= 10 {
        return Err(Error(
            StatusCode::TOO_MANY_REQUESTS,
            "Too many password verification attempts. Wait five minutes.",
        ));
    }
    a.count += 1;
    Ok(())
}
async fn reconfirm(p: &Portal, s: &mut Session, current: &str) -> Reply<()> {
    current_valid(current)?;
    let a = details(p, s).await?;
    throttle(p, s.user).await?;
    let _slot = p.auth_slots.try_acquire().map_err(|_| {
        Error(
            StatusCode::TOO_MANY_REQUESTS,
            "Account security is busy. Try again later.",
        )
    })?;
    let verified = p
        .upstream(
            "/auth/v1/token?grant_type=password",
            Method::POST,
            None,
            Some(json!({"email":a["email"],"password":current})),
        )
        .await
        .map_err(|e| {
            if matches!(e.0, StatusCode::BAD_REQUEST | StatusCode::FORBIDDEN) {
                invalid("Your current password could not be verified.")
            } else {
                e
            }
        })?;
    let mut fresh = Session {
        access: String::new(),
        refresh: String::new(),
        expires: Instant::now(),
        last_seen: Instant::now(),
        csrf: s.csrf,
        user: id(&verified["user"], "id")?,
        company: s.company,
        employee: s.employee,
    };
    set_tokens(&mut fresh, &verified)?;
    if fresh.user != s.user {
        p.revoke(&fresh).await;
        return Err(unavailable());
    }
    if let Err(e) = p.access(&fresh).await {
        p.revoke(&fresh).await;
        return Err(e);
    }
    // Use the newly verified upstream session, keep this opaque browser cookie,
    // and revoke only the superseded upstream session (never all other devices).
    std::mem::swap(s, &mut fresh);
    p.revoke(&fresh).await;
    Ok(())
}
fn uncertain(e: Error) -> Error {
    if e.0 == StatusCode::BAD_GATEWAY {
        Error(
            e.0,
            "The account change may have been saved. Refresh account status before retrying.",
        )
    } else {
        e
    }
}
async fn update(p: &Portal, s: &Session, path: &str, body: Value) -> Reply<()> {
    let user = p
        .upstream(path, Method::PUT, Some(&s.access), Some(body))
        .await
        .map_err(uncertain)?;
    if id(&user, "id").ok() != Some(s.user) {
        return Err(Error(
            StatusCode::BAD_GATEWAY,
            "The account change may have been saved. Refresh account status before retrying.",
        ));
    }
    Ok(())
}
async fn password(
    State(p): State<Portal>,
    h: HeaderMap,
    Query(_): Query<Empty>,
    Json(d): Json<Password>,
) -> Reply<Json<Value>> {
    password_valid(&d)?;
    let e = p.session(&h, true).await?;
    let mut s = e.lock().await;
    reconfirm(&p, &mut s, &d.current_password).await?;
    update(
        &p,
        &s,
        "/auth/v1/user",
        json!({"password":d.new_password,"current_password":d.current_password}),
    )
    .await?;
    Ok(Json(json!({"saved":true,"password_changed":true})))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Email {
    current_password: String,
    new_email: String,
}
async fn email(
    State(p): State<Portal>,
    h: HeaderMap,
    Query(_): Query<Empty>,
    Json(d): Json<Email>,
) -> Reply<Json<Value>> {
    current_valid(&d.current_password)?;
    let email = d.new_email.trim();
    if !email_ok(email) {
        return Err(invalid("Enter a valid account email address."));
    }
    let e = p.session(&h, true).await?;
    let mut s = e.lock().await;
    let old = details(&p, &s).await?;
    if old["email"]
        .as_str()
        .is_some_and(|v| v.eq_ignore_ascii_case(email))
    {
        return Err(invalid("That is already your account email."));
    }
    reconfirm(&p, &mut s, &d.current_password).await?;
    let redirect = format!("{}/account/confirm-email", p.config.origin);
    let mut url = reqwest::Url::parse(&format!("{}/auth/v1/user", p.config.url))
        .map_err(|_| unavailable())?;
    url.query_pairs_mut().append_pair("redirect_to", &redirect);
    let path = format!("{}?{}", url.path(), url.query().unwrap_or(""));
    update(&p, &s, &path, json!({"email":email,"code_challenge":confirmation_challenge(),"code_challenge_method":"s256"})).await?;
    let account = details(&p, &s).await?;
    Ok(Json(
        json!({"saved":true,"confirmation_pending":account["confirmation_pending"]}),
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}
async fn sign_out_others(
    State(p): State<Portal>,
    h: HeaderMap,
    Query(_): Query<Empty>,
    Json(_): Json<Empty>,
) -> Reply<Json<Value>> {
    let sid = cookie_id(&h, p.config.cookie_name()).ok_or_else(expired)?;
    let e = p.session(&h, true).await?;
    let s = e.lock().await;
    let user = s.user;
    p.upstream(
        "/auth/v1/logout?scope=others",
        Method::POST,
        Some(&s.access),
        Some(json!({})),
    )
    .await
    .map_err(uncertain)?;
    drop(s);
    // Do not hold the current session lock while inspecting another session:
    // simultaneous own-device sign-outs must not deadlock each other.
    let snapshot: Vec<_> = p
        .sessions
        .lock()
        .await
        .iter()
        .filter(|(id, _)| **id != sid)
        .map(|(id, s)| (*id, s.clone()))
        .collect();
    let mut own = vec![];
    for (id, s) in snapshot {
        if s.lock().await.user == user {
            own.push(id);
        }
    }
    let mut sessions = p.sessions.lock().await;
    let removed = own
        .into_iter()
        .filter(|id| sessions.remove(id).is_some())
        .count();
    Ok(Json(
        json!({"signed_out_others":true,"local_sessions_removed":removed}),
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn password_changes_match_native_bounds_and_require_confirmation() {
        let mut d = Password {
            current_password: "original".into(),
            new_password: "long new password".into(),
            confirmation: "long new password".into(),
        };
        assert!(password_valid(&d).is_ok());
        d.confirmation = "different".into();
        assert!(password_valid(&d).is_err());
        d.new_password = "short".into();
        d.confirmation = d.new_password.clone();
        assert!(password_valid(&d).is_err());
    }
}
