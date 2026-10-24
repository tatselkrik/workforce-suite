//! Own proposals only. The existing protected database functions decide scope,
//! freshness, time validity and approval; the browser has no review routes.
use super::*;
use axum::extract::Query;
use chrono::{DateTime, LocalResult, NaiveDate, NaiveDateTime};
use chrono_tz::Tz;

pub(super) fn routes() -> Router<Portal> {
    Router::new()
        .route("/api/time/context", get(context))
        .route("/api/time/requests", get(list))
        .route("/api/time/history/{kind}/{id}", get(history))
        .route("/api/time/correction", post(correction))
        .route("/api/time/overtime", post(overtime))
        .route("/api/time/cancel", post(cancel))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Day {
    day: Option<NaiveDate>,
}
fn note(s: &str) -> Reply<&str> {
    let n = s.trim();
    if n.is_empty() || s.chars().count() > 500 || s.chars().any(char::is_control) {
        return Err(invalid(
            "Enter a reason of 1–500 characters without control characters.",
        ));
    }
    Ok(n)
}
async fn context_for(p: &Portal, s: &Session, day: Option<NaiveDate>) -> Reply<Value> {
    let v = p
        .upstream(
            "/rest/v1/rpc/portal_attendance_context",
            Method::POST,
            Some(&s.access),
            Some(json!({"p_day":day})),
        )
        .await?;
    own(&v, s)?;
    if day.is_some_and(|d| v["work_date"].as_str() != Some(d.to_string().as_str())) {
        return Err(unavailable());
    }
    Ok(v)
}
async fn context(
    State(p): State<Portal>,
    h: HeaderMap,
    Query(q): Query<Day>,
) -> Reply<Json<Value>> {
    let e = p.session(&h, false).await?;
    let s = e.lock().await;
    let v = context_for(&p, &s, q.day).await?;
    let mut out = project(
        &v,
        &[
            "work_date",
            "timezone",
            "candidate_minutes",
            "correction_pending",
            "overtime_blocked",
            "current_approved_minutes",
        ],
    );
    out["attendance"] = if v["attendance"].is_null() {
        Value::Null
    } else {
        project(
            &v["attendance"],
            &[
                "id",
                "revision",
                "work_date",
                "checked_in_at",
                "checked_out_at",
            ],
        )
    };
    out["break_revisions"] = json!(
        v["break_revisions"]
            .as_array()
            .ok_or_else(unavailable)?
            .iter()
            .map(|r| project(r, &["id", "revision"]))
            .collect::<Vec<_>>()
    );
    Ok(Json(out))
}
fn tables(kind: &str) -> Reply<(&'static str, &'static str, &'static str)> {
    match kind {
        "correction" => Ok((
            "attendance_requests",
            "attendance_request_events",
            "attendance_request",
        )),
        "overtime" => Ok(("overtime_requests", "overtime_events", "overtime_request")),
        _ => Err(invalid("Choose a correction or overtime request.")),
    }
}
async fn own_request(p: &Portal, s: &Session, kind: &str, rid: Uuid) -> Reply<Value> {
    let (table, _, _) = tables(kind)?;
    let rows=p.upstream(&format!("/rest/v1/{table}?select=id,company_id,employee_id,work_date,revision,status&company_id=eq.{}&employee_id=eq.{}&id=eq.{rid}",s.company,s.employee),Method::GET,Some(&s.access),None).await?;
    let r = rows
        .as_array()
        .ok_or_else(unavailable)?
        .first()
        .ok_or_else(|| {
            Error(
                StatusCode::NOT_FOUND,
                "Your request is unavailable. Refresh first.",
            )
        })?;
    own(r, s)?;
    if id(r, "id")? != rid {
        return Err(unavailable());
    }
    Ok(r.clone())
}
async fn list(State(p): State<Portal>, h: HeaderMap) -> Reply<Json<Value>> {
    let e = p.session(&h, false).await?;
    let s = e.lock().await;
    let mut out = vec![];
    for kind in ["correction", "overtime"] {
        let (table, _, _) = tables(kind)?;
        let extra = if kind == "correction" {
            "timezone,proposed_in,proposed_out"
        } else {
            "requested_minutes,approved_minutes"
        };
        let rows=p.upstream(&format!("/rest/v1/{table}?select=id,company_id,employee_id,work_date,reason,status,revision,created_at,{extra}&company_id=eq.{}&employee_id=eq.{}&order=created_at.desc,id.desc&limit=200",s.company,s.employee),Method::GET,Some(&s.access),None).await?;
        for r in rows.as_array().ok_or_else(unavailable)? {
            own(r, &s)?;
            let mut v = project(
                r,
                &[
                    "id",
                    "work_date",
                    "reason",
                    "status",
                    "revision",
                    "created_at",
                    "timezone",
                    "proposed_in",
                    "proposed_out",
                    "requested_minutes",
                    "approved_minutes",
                ],
            );
            v["kind"] = json!(kind);
            out.push(v);
        }
    }
    out.sort_by(|a, b| b["created_at"].as_str().cmp(&a["created_at"].as_str()));
    Ok(Json(json!(out)))
}
async fn history(
    State(p): State<Portal>,
    h: HeaderMap,
    Path((kind, rid)): Path<(String, Uuid)>,
) -> Reply<Json<Value>> {
    let e = p.session(&h, false).await?;
    let s = e.lock().await;
    own_request(&p, &s, &kind, rid).await?;
    let (_, table, _) = tables(&kind)?;
    let extra = if kind == "overtime" {
        ",approved_minutes"
    } else {
        ""
    };
    let rows=p.upstream(&format!("/rest/v1/{table}?select=company_id,employee_id,request_id,status,reason,actor_name,actor_role,created_at{extra}&company_id=eq.{}&employee_id=eq.{}&request_id=eq.{rid}&order=created_at.asc,id.asc&limit=200",s.company,s.employee),Method::GET,Some(&s.access),None).await?;
    let mut out = vec![];
    for r in rows.as_array().ok_or_else(unavailable)? {
        own(r, &s)?;
        if id(r, "request_id")? != rid {
            return Err(unavailable());
        }
        out.push(project(
            r,
            &[
                "status",
                "reason",
                "actor_name",
                "actor_role",
                "created_at",
                "approved_minutes",
            ],
        ));
    }
    Ok(Json(json!(out)))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Correction {
    work_date: NaiveDate,
    timezone: String,
    checked_in_local: NaiveDateTime,
    checked_out_local: Option<NaiveDateTime>,
    attendance_id: Option<Uuid>,
    parent_revision: Option<i64>,
    reason: String,
}
fn stamp(zone: Tz, local: NaiveDateTime) -> Reply<DateTime<Utc>> {
    match local.and_local_timezone(zone) {
        LocalResult::Single(t) => Ok(t.with_timezone(&Utc)),
        _ => Err(invalid(
            "This company-local time is ambiguous or does not exist. Ask HR to review the exact timestamp.",
        )),
    }
}
async fn write(
    p: &Portal,
    s: &Session,
    rpc: &str,
    payload: Value,
    expected: Option<Uuid>,
) -> Reply<Json<Value>> {
    let v = p
        .upstream(
            &format!("/rest/v1/rpc/{rpc}"),
            Method::POST,
            Some(&s.access),
            Some(json!({"p_request":payload})),
        )
        .await
        .map_err(|e| {
            if e.0 == StatusCode::BAD_GATEWAY {
                Error(
                    e.0,
                    "The change may have been saved. Refresh your requests before trying again.",
                )
            } else {
                e
            }
        })?;
    let rid = v
        .as_str()
        .and_then(|v| v.parse::<Uuid>().ok())
        .ok_or_else(unavailable)?;
    if expected.is_some_and(|e| e != rid) {
        return Err(unavailable());
    }
    Ok(Json(json!({"saved":true,"request_id":rid})))
}
async fn correction(
    State(p): State<Portal>,
    h: HeaderMap,
    Json(d): Json<Correction>,
) -> Reply<Json<Value>> {
    let reason = note(&d.reason)?;
    if d.parent_revision.is_some_and(|r| r < 0)
        || d.attendance_id.is_some() != d.parent_revision.is_some()
    {
        return Err(invalid("Refresh the selected attendance record first."));
    }
    let zone: Tz = d
        .timezone
        .parse()
        .map_err(|_| invalid("Refresh the company time zone first."))?;
    let tin = stamp(zone, d.checked_in_local)?;
    let tout = d.checked_out_local.map(|t| stamp(zone, t)).transpose()?;
    if tout.is_some_and(|t| t < tin) {
        return Err(invalid("Check-out must follow check-in."));
    }
    let e = p.session(&h, true).await?;
    let s = e.lock().await;
    write(&p,&s,"attendance_request",json!({"operation":"submit","work_date":d.work_date,"expected_timezone":d.timezone,"attendance_id":d.attendance_id,"expected_revision":d.parent_revision,"checked_in_at":tin,"checked_out_at":tout,"reason":reason}),None).await
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct BreakRevision {
    id: Uuid,
    revision: i64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Overtime {
    attendance_id: Uuid,
    parent_revision: i64,
    expected_breaks: Vec<BreakRevision>,
    candidate_minutes: i64,
    minutes: u32,
    reason: String,
}
async fn overtime(
    State(p): State<Portal>,
    h: HeaderMap,
    Json(d): Json<Overtime>,
) -> Reply<Json<Value>> {
    let reason = note(&d.reason)?;
    if d.parent_revision < 0
        || d.candidate_minutes < 1
        || i64::from(d.minutes) > d.candidate_minutes
        || d.minutes == 0
        || d.expected_breaks.len() > 100
        || d.expected_breaks.iter().any(|r| r.revision < 0)
    {
        return Err(invalid(
            "Request whole minutes within the saved overtime candidate and refresh its revisions.",
        ));
    }
    let e = p.session(&h, true).await?;
    let s = e.lock().await;
    // Database submission resolves the employee from the current caller and
    // compares the exact parent, break revisions and computed candidate.
    write(&p,&s,"overtime_request",json!({"operation":"submit","attendance_id":d.attendance_id,"expected_revision":d.parent_revision,"expected_breaks":d.expected_breaks,"expected_candidate_minutes":d.candidate_minutes,"minutes":d.minutes,"reason":reason}),None).await
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Cancel {
    kind: String,
    id: Uuid,
    revision: i64,
    reason: String,
}
async fn cancel(
    State(p): State<Portal>,
    h: HeaderMap,
    Json(d): Json<Cancel>,
) -> Reply<Json<Value>> {
    let reason = note(&d.reason)?;
    if d.revision < 0 {
        return Err(invalid("Refresh the request first."));
    }
    let e = p.session(&h, true).await?;
    let s = e.lock().await;
    own_request(&p, &s, &d.kind, d.id).await?;
    let (_, _, rpc) = tables(&d.kind)?;
    write(&p,&s,rpc,json!({"operation":"cancel","request_id":d.id,"expected_revision":d.revision,"reason":reason}),Some(d.id)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn company_local_time_rejects_ambiguous_or_missing_dst() {
        let z: Tz = "America/New_York".parse().unwrap();
        for value in ["2026-11-01T01:30:00", "2026-03-08T02:30:00"] {
            assert!(stamp(z, value.parse().unwrap()).is_err());
        }
        assert_eq!(
            stamp(
                "Asia/Manila".parse().unwrap(),
                "2026-10-01T08:30:00".parse().unwrap()
            )
            .unwrap()
            .to_rfc3339(),
            "2026-10-01T00:30:00+00:00"
        );
    }
    #[test]
    fn reasons_require_actual_text_and_no_control_characters() {
        for value in ["", "  ", "bad\nreason"] {
            assert!(note(value).is_err());
        }
        assert_eq!(note(" Reason ").unwrap(), "Reason");
    }
}
