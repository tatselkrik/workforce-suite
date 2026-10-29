//! Android-only own-device adapter; browser builds never enable enrollment APIs.
use super::*;
pub(super) fn routes() -> Router<Portal> {
    let r = Router::new().route("/device-alerts.js", get(script));
    #[cfg(feature = "android-push")]
    let r = r.route("/api/device-alerts/{action}", post(update));
    r
}
async fn script() -> impl IntoResponse {
    ([(header::CONTENT_TYPE,"text/javascript; charset=utf-8")],include_str!("../web/device-alerts.js"))
}
#[cfg(any(feature = "android-push",test))]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input { token: String, request_id: Option<Uuid> }
#[cfg(any(feature = "android-push",test))]
fn validate(d: &Input, action: &str) -> Reply<()> {
    if !(40..=4096).contains(&d.token.len()) || !d.token.bytes().all(|v|v.is_ascii_alphanumeric() || b"_:.-".contains(&v))
        || !matches!(action,"status"|"register"|"disable"|"test")
        || (action=="test") != d.request_id.is_some()
        || d.request_id.is_some_and(|u|u.is_nil()) { return Err(invalid("Reopen device alerts and try again.")); }
    Ok(())
}
#[cfg(any(feature = "android-push",test))]
fn projection(v: &Value,s: &Session,action: &str) -> Reply<Value> {
    if action=="test" {
        if v["version"]!="device-push-test-v1" || !matches!(v["state"].as_str(),Some("sent"|"queued"|"failed"|"cancelled")) {return Err(unavailable());}
        return Ok(project(v,&["state"]));
    }
    own(v,s)?;
    if v["version"]!="portal-device-push-v1" || id(v,"user_id")?!=s.user || !v["enabled"].is_boolean() {return Err(unavailable());}
    if action=="disable" {return Ok(json!({"enabled":false}));}
    if !v["available"].is_boolean() || ["expires_at","last_sent_at"].iter().any(|k| !v[*k].is_null() && v[*k].as_str().is_none_or(|t|chrono::DateTime::parse_from_rfc3339(t).is_err()))
        || !matches!(v["last_error"].as_str(),None|Some("source_budget"|"source_unavailable"|"provider_auth"|"temporary"|"unregistered")) {return Err(unavailable());}
    Ok(project(v,&["available","enabled","expires_at","last_sent_at","last_error"]))
}
#[cfg(feature = "android-push")]
async fn update(State(p): State<Portal>,h: HeaderMap,Path(action): Path<String>,Json(d):Json<Input>) -> Reply<Json<Value>> {
    validate(&d,&action)?;
    let e=p.session(&h,true).await?;let s=e.lock().await;
    let (path,body)=if action=="test" {(String::from("/functions/v1/device-push"),json!({"operation":"test","token":d.token,"request_id":d.request_id}))}
        else {(format!("/rest/v1/rpc/portal_push_{action}"),json!({"p_token":d.token}))};
    let v=p.upstream(&path,Method::POST,Some(&s.access),Some(body)).await?;
    Ok(Json(projection(&v,&s,&action)?))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn no_user_target_or_ambiguous_test_identity() {
        assert!(serde_json::from_value::<Input>(json!({"token":"x".repeat(100),"employee_id":Uuid::new_v4()})).is_err());
        let mut d=Input{token:"sdk_:-123".repeat(10),request_id:None};assert!(validate(&d,"register").is_ok());
        assert!(validate(&d,"test").is_err());d.request_id=Some(Uuid::nil());assert!(validate(&d,"test").is_err());
        d.request_id=Some(Uuid::new_v4());assert!(validate(&d,"test").is_ok());assert!(validate(&d,"status").is_err());
        for token in ["x".repeat(39),"x".repeat(4097),"x ".repeat(50),"https://example.invalid/secret".repeat(3)] {d.token=token;assert!(validate(&d,"test").is_err());}
    }
    #[test] fn identity_checks_precede_minimal_projection() {
        let s=Session{access:String::new(),refresh:String::new(),expires:Instant::now(),last_seen:Instant::now(),csrf:Uuid::new_v4(),user:Uuid::new_v4(),company:Uuid::new_v4(),employee:Uuid::new_v4()};
        let v=json!({"version":"portal-device-push-v1","user_id":s.user,"company_id":s.company,"employee_id":s.employee,"enabled":true,"available":true,"expires_at":"2026-11-28T00:00:00Z","last_sent_at":null,"last_error":null,"token":"PRIVATE","worker_secret":"PRIVATE"});
        let out=projection(&v,&s,"status").unwrap();assert_eq!(out.as_object().unwrap().len(),5);assert!(!out.to_string().contains("PRIVATE"));
        for k in ["user_id","company_id","employee_id"] {let mut foreign=v.clone();foreign[k]=json!(Uuid::new_v4());assert!(projection(&foreign,&s,"status").is_err());}
        let mut invalid=v;invalid["expires_at"]=json!("not-a-date");assert!(projection(&invalid,&s,"status").is_err());
        assert!(projection(&json!({"version":"device-push-test-v1","state":"sent","token":"PRIVATE"}),&s,"test").unwrap().get("token").is_none());
        assert!(projection(&json!({"version":"device-push-test-v1","state":"arbitrary"}),&s,"test").is_err());
    }
    #[tokio::test] async fn enrollment_cannot_run_without_current_cookie_or_csrf() {
        use tower::ServiceExt;use axum::body::Body;
        let p=Portal::new(Config::origin("http://127.0.0.1:3030","127.0.0.1:3030").unwrap()).unwrap();
        let r=p.router().oneshot(Request::builder().uri("/api/device-alerts/register").method("POST").header("host","127.0.0.1:3030").header("origin","http://127.0.0.1:3030").header("content-type","application/json").body(Body::from(json!({"token":"x".repeat(100)}).to_string())).unwrap()).await.unwrap();
        #[cfg(feature="android-push")] assert_eq!(r.status(),StatusCode::UNAUTHORIZED);
        #[cfg(not(feature="android-push"))] assert_eq!(r.status(),StatusCode::NOT_FOUND);
    }
}
