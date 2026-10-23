use super::*;
use axum::extract::Query;
use chrono::{Datelike, NaiveDate};

pub(super) fn routes() -> Router<Portal> {
    Router::new()
        .route("/api/leave/context", get(context))
        .route("/api/leave", get(leave))
        .route("/api/leave/request", post(submit))
        .route("/api/leave/cancel", post(cancel))
        .route("/api/breaks/{id}", get(breaks))
        .route("/api/breaks/start", post(start_break))
        .route("/api/breaks/end", post(end_break))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AsOf {
    asof: Option<NaiveDate>,
}
async fn context(
    State(p): State<Portal>,
    h: HeaderMap,
    Query(q): Query<AsOf>,
) -> Reply<Json<Value>> {
    let entry = p.session(&h, false).await?;
    let s = entry.lock().await;
    let c = p
        .upstream(
            "/rest/v1/rpc/portal_leave_context",
            Method::POST,
            Some(&s.access),
            Some(json!({"p_asof":q.asof})),
        )
        .await?;
    own(&c, &s)?;
    let mut out = project(&c, &["policy_revision", "asof", "workweek"]);
    let mut types = vec![];
    for t in c["types"].as_array().ok_or_else(unavailable)? {
        let mut v = project(t, &["code", "name", "paid", "unlimited"]);
        v["balance"] = if t["balance"].is_null() {
            Value::Null
        } else {
            project(
                &t["balance"],
                &[
                    "earned_units",
                    "carryover_units",
                    "used_units",
                    "reserved_units",
                    "available_units",
                ],
            )
        };
        types.push(v);
    }
    out["types"] = json!(types);
    out["holidays"] = json!(
        c["holidays"]
            .as_array()
            .ok_or_else(unavailable)?
            .iter()
            .map(|h| project(h, &["date", "name"]))
            .collect::<Vec<_>>()
    );
    Ok(Json(out))
}
async fn leave(State(p): State<Portal>, h: HeaderMap) -> Reply<Json<Value>> {
    let entry = p.session(&h, false).await?;
    let s = entry.lock().await;
    let rows=p.upstream(&format!("/rest/v1/leave_requests?select=id,company_id,employee_id,type_code,type_name,starts_on,ends_on,day_part,reason,status,charged_units,charged_dates,revision,policy_revision,approval_steps&company_id=eq.{}&employee_id=eq.{}&order=created_at.desc&limit=200",s.company,s.employee),Method::GET,Some(&s.access),None).await?;
    let mut out = vec![];
    for r in rows.as_array().ok_or_else(unavailable)? {
        own(r, &s)?;
        let mut v = project(
            r,
            &[
                "id",
                "type_code",
                "type_name",
                "starts_on",
                "ends_on",
                "day_part",
                "reason",
                "status",
                "charged_units",
                "charged_dates",
                "revision",
                "policy_revision",
            ],
        );
        v["approvals"] = json!(
            r["approval_steps"]
                .as_array()
                .ok_or_else(unavailable)?
                .iter()
                .map(|a| project(a, &["kind", "approved_by", "approved_at"]))
                .collect::<Vec<_>>()
        );
        out.push(v);
    }
    Ok(Json(json!(out)))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LeaveDraft {
    type_code: String,
    starts_on: NaiveDate,
    ends_on: NaiveDate,
    day_part: String,
    reason: String,
    policy_revision: i64,
}
impl LeaveDraft {
    fn validate(&self) -> Reply<()> {
        if self.type_code.is_empty()
            || self.type_code.len() > 40
            || self.starts_on > self.ends_on
            || self.starts_on.year() != self.ends_on.year()
            || self.starts_on.year() < 1900
            || self.ends_on.year() > 2200
            || !matches!(self.day_part.as_str(), "full" | "am" | "pm")
            || (self.day_part != "full" && self.starts_on != self.ends_on)
            || self.reason.chars().count() > 500
            || self.policy_revision < 0
        {
            return Err(invalid(
                "Choose a leave type and valid dates in one year; half-days use one date. Keep the reason to 500 characters.",
            ));
        }
        Ok(())
    }
}
fn uncertain(e: Error) -> Error {
    if e.0 == StatusCode::BAD_GATEWAY {
        Error(
            e.0,
            "The change may have been saved. Refresh your records before trying again.",
        )
    } else {
        e
    }
}
async fn submit(
    State(p): State<Portal>,
    h: HeaderMap,
    Json(d): Json<LeaveDraft>,
) -> Reply<Json<Value>> {
    d.validate()?;
    let entry = p.session(&h, true).await?;
    let s = entry.lock().await;
    let saved=p.upstream("/rest/v1/rpc/portal_submit_leave",Method::POST,Some(&s.access),Some(json!({"p_type_code":d.type_code,"p_starts":d.starts_on,"p_ends":d.ends_on,"p_part":d.day_part,"p_reason":d.reason.trim(),"p_policy_revision":d.policy_revision}))).await.map_err(uncertain)?;
    let request_id = saved
        .as_str()
        .and_then(|x| x.parse::<Uuid>().ok())
        .ok_or_else(unavailable)?;
    Ok(Json(json!({"saved":true,"request_id":request_id})))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Cancel {
    id: Uuid,
    revision: i64,
    reason: String,
}
async fn own_record(p: &Portal, s: &Session, table: &str, record: Uuid) -> Reply<Value> {
    let v = p
        .upstream(
            &format!(
                "/rest/v1/{table}?select=*&company_id=eq.{}&employee_id=eq.{}&id=eq.{record}",
                s.company, s.employee
            ),
            Method::GET,
            Some(&s.access),
            None,
        )
        .await?;
    let row = v
        .as_array()
        .ok_or_else(unavailable)?
        .first()
        .cloned()
        .ok_or_else(|| {
            Error(
                StatusCode::NOT_FOUND,
                "Your record is unavailable. Refresh first.",
            )
        })?;
    own(&row, s)?;
    if id(&row, "id")? != record {
        return Err(unavailable());
    }
    Ok(row)
}
async fn cancel(
    State(p): State<Portal>,
    h: HeaderMap,
    Json(d): Json<Cancel>,
) -> Reply<Json<Value>> {
    if d.revision < 0 || d.reason.trim().is_empty() || d.reason.chars().count() > 500 {
        return Err(invalid("Enter a cancellation reason up to 500 characters."));
    }
    let entry = p.session(&h, true).await?;
    let s = entry.lock().await;
    own_record(&p, &s, "leave_requests", d.id).await?;
    let saved = p
        .upstream(
            "/rest/v1/rpc/portal_cancel_leave",
            Method::POST,
            Some(&s.access),
            Some(json!({"p_id":d.id,"p_revision":d.revision,"p_reason":d.reason.trim()})),
        )
        .await
        .map_err(uncertain)?;
    if saved.as_str().and_then(|x| x.parse::<Uuid>().ok()) != Some(d.id) {
        return Err(unavailable());
    }
    Ok(Json(json!({"saved":true})))
}
async fn breaks(
    State(p): State<Portal>,
    h: HeaderMap,
    Path(aid): Path<Uuid>,
) -> Reply<Json<Value>> {
    let entry = p.session(&h, false).await?;
    let s = entry.lock().await;
    let parent = own_record(&p, &s, "attendance", aid).await?;
    let rows=p.upstream(&format!("/rest/v1/attendance_breaks?select=id,company_id,employee_id,attendance_id,rule_id,name,paid,required,expected_minutes,started_at,ended_at,revision&company_id=eq.{}&employee_id=eq.{}&attendance_id=eq.{aid}&voided=eq.false&order=started_at.asc",s.company,s.employee),Method::GET,Some(&s.access),None).await?;
    let mut records = vec![];
    for r in rows.as_array().ok_or_else(unavailable)? {
        own(r, &s)?;
        if id(r, "attendance_id")? != aid {
            return Err(unavailable());
        }
        records.push(project(
            r,
            &[
                "id",
                "rule_id",
                "name",
                "paid",
                "required",
                "expected_minutes",
                "started_at",
                "ended_at",
                "revision",
            ],
        ));
    }
    let rules = parent["planned_shift"]["break_rules"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|r| project(r, &["id", "name", "minutes", "paid", "required"]))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(Json(
        json!({"attendance":project(&parent,&["id","revision","work_date","checked_in_at","checked_out_at"]),"rules":rules,"records":records}),
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Start {
    attendance_id: Uuid,
    rule_id: Uuid,
    parent_revision: i64,
}
async fn start_break(
    State(p): State<Portal>,
    h: HeaderMap,
    Json(d): Json<Start>,
) -> Reply<Json<Value>> {
    if d.parent_revision < 0 {
        return Err(invalid("Refresh the attendance record first."));
    }
    let entry = p.session(&h, true).await?;
    let s = entry.lock().await;
    own_record(&p, &s, "attendance", d.attendance_id).await?;
    let v=p.upstream("/rest/v1/rpc/manage_attendance_break",Method::POST,Some(&s.access),Some(json!({"p_operation":"start","p_break":{"attendance_id":d.attendance_id,"rule_id":d.rule_id,"expected_parent_revision":d.parent_revision}}))).await.map_err(uncertain)?;
    let break_id = v
        .as_str()
        .and_then(|x| x.parse::<Uuid>().ok())
        .ok_or_else(unavailable)?;
    Ok(Json(json!({"saved":true,"break_id":break_id})))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct End {
    attendance_id: Uuid,
    id: Uuid,
    parent_revision: i64,
    revision: i64,
}
async fn end_break(
    State(p): State<Portal>,
    h: HeaderMap,
    Json(d): Json<End>,
) -> Reply<Json<Value>> {
    if d.parent_revision < 0 || d.revision < 0 {
        return Err(invalid("Refresh the attendance and break records first."));
    }
    let entry = p.session(&h, true).await?;
    let s = entry.lock().await;
    own_record(&p, &s, "attendance", d.attendance_id).await?;
    let b = own_record(&p, &s, "attendance_breaks", d.id).await?;
    if id(&b, "attendance_id")? != d.attendance_id {
        return Err(Error(StatusCode::NOT_FOUND, "Your break is unavailable."));
    }
    let v=p.upstream("/rest/v1/rpc/manage_attendance_break",Method::POST,Some(&s.access),Some(json!({"p_operation":"end","p_break":{"attendance_id":d.attendance_id,"id":d.id,"expected_parent_revision":d.parent_revision,"expected_revision":d.revision}}))).await.map_err(uncertain)?;
    if v.as_str().and_then(|x| x.parse::<Uuid>().ok()) != Some(d.id) {
        return Err(unavailable());
    }
    Ok(Json(json!({"saved":true})))
}
