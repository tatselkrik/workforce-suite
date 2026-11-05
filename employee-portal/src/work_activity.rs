//! Android owns only the signed-in employee's tasks, submissions and work policy.
use super::*;

pub(super) fn routes() -> Router<Portal> {
    Router::new()
        .route(
            "/api/work",
            get(read).post(save).layer(DefaultBodyLimit::max(60000)),
        )
        .route("/api/attendance-policy", get(attendance_policy))
}
async fn attendance_policy(State(p): State<Portal>, h: HeaderMap) -> Reply<Json<Value>> {
    let session = p.session(&h, false).await?;
    let s = session.lock().await;
    let value = p
        .upstream(
            "/rest/v1/rpc/work_attendance_status",
            Method::POST,
            Some(&s.access),
            Some(json!({})),
        )
        .await?;
    own(&value, &s)?;
    if id(&value, "user_id")? != s.user
        || value["version"] != 1
        || !matches!(
            value["method"].as_str(),
            Some("legacy" | "remote" | "kiosk")
        )
    {
        return Err(unavailable());
    }
    Ok(Json(project(
        &value,
        &["work_date", "method", "arrangement"],
    )))
}
fn projection(v: &Value, s: &Session) -> Reply<Value> {
    if id(v, "company_id")? != s.company
        || id(v, "user_id")? != s.user
        || id(v, "employee_id")? != s.employee
        || v["version"] != "workforce-attendance-v1"
        || v["own_only"] != true
        || v["can_administer"] != false
    {
        return Err(unavailable());
    }
    let mut output = json!({});
    for (key, fields) in [
        (
            "policies",
            vec![
                "id",
                "starts_on",
                "ends_on",
                "arrangement",
                "office_days",
                "enabled",
                "sequence",
            ],
        ),
        (
            "tasks",
            vec![
                "id",
                "title",
                "outcome",
                "due_on",
                "priority",
                "status",
                "progress",
                "review_note",
                "revision",
            ],
        ),
        (
            "reports",
            vec![
                "id",
                "work_date",
                "status",
                "revision",
                "submission",
                "body",
                "task_ids",
                "review_note",
            ],
        ),
        (
            "submissions",
            vec!["report_id", "version", "body", "submitted_at"],
        ),
    ] {
        let rows = v[key].as_array().ok_or_else(unavailable)?;
        if rows.len() > 10000 {
            return Err(unavailable());
        }
        let mut clean = vec![];
        for row in rows {
            own(row, s)?;
            if key == "submissions" {
                for task in row["task_snapshot"].as_array().ok_or_else(unavailable)? {
                    own(task, s)?;
                }
            }
            let mut entry = project(row, &fields);
            if key == "submissions" {
                entry["task_snapshot"] = json!(
                    row["task_snapshot"]
                        .as_array()
                        .ok_or_else(unavailable)?
                        .iter()
                        .map(|t| project(
                            t,
                            &[
                                "id", "title", "outcome", "due_on", "priority", "status",
                                "progress"
                            ]
                        ))
                        .collect::<Vec<_>>()
                );
            }
            clean.push(entry);
        }
        output[key] = json!(clean);
    }
    let events = v["events"].as_array().ok_or_else(unavailable)?;
    if events.len() > 1000 {
        return Err(unavailable());
    }
    let mut decisions = vec![];
    for event in events {
        own(event, s)?;
        if matches!(
            event["kind"].as_str(),
            Some("report_review" | "task_review")
        ) {
            decisions.push(json!({"record_id":event["record_id"],"kind":event["kind"],"created_at":event["created_at"],"status":event["body"]["request"]["status"],"note":event["body"]["request"]["note"]}));
        }
    }
    output["decisions"] = json!(decisions);
    Ok(output)
}
async fn read(State(p): State<Portal>, h: HeaderMap) -> Reply<Json<Value>> {
    let session = p.session(&h, false).await?;
    let s = session.lock().await;
    let value = p
        .upstream(
            "/rest/v1/rpc/work_activity_context",
            Method::POST,
            Some(&s.access),
            Some(json!({"p_own":true})),
        )
        .await?;
    Ok(Json(projection(&value, &s)?))
}
async fn save(
    State(p): State<Portal>,
    h: HeaderMap,
    Json(body): Json<Value>,
) -> Reply<Json<Value>> {
    let object = body
        .as_object()
        .ok_or_else(|| invalid("Use a valid own work request."))?;
    if !matches!(
        body["operation"].as_str(),
        Some("task_progress" | "report_save" | "report_submit")
    ) || object.keys().any(|k| {
        !matches!(
            k.as_str(),
            "request_id"
                | "id"
                | "operation"
                | "expected_revision"
                | "work_date"
                | "body"
                | "task_ids"
                | "status"
                | "note"
        )
    }) || id(&body, "request_id")?.is_nil()
        || id(&body, "id")?.is_nil()
    {
        return Err(invalid(
            "Only your own task progress and reports can be saved here.",
        ));
    }
    let session = p.session(&h, true).await?;
    let s = session.lock().await;
    let receipt = p
        .upstream(
            "/rest/v1/rpc/work_activity_change",
            Method::POST,
            Some(&s.access),
            Some(json!({"p_request":body})),
        )
        .await?;
    if receipt["id"] != body["id"] {
        return Err(unavailable());
    }
    let current = p
        .upstream(
            "/rest/v1/rpc/work_activity_context",
            Method::POST,
            Some(&s.access),
            Some(json!({"p_own":true})),
        )
        .await?;
    let mut output = projection(&current, &s)?;
    output["receipt"] = json!({"id":body["id"],"request_id":body["request_id"]});
    Ok(Json(output))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn session() -> Session {
        Session {
            access: String::new(),
            refresh: String::new(),
            expires: Instant::now(),
            last_seen: Instant::now(),
            csrf: Uuid::from_u128(1),
            user: Uuid::from_u128(2),
            company: Uuid::from_u128(3),
            employee: Uuid::from_u128(4),
        }
    }
    fn context(s: &Session) -> Value {
        json!({"version":"workforce-attendance-v1","company_id":s.company,"user_id":s.user,"employee_id":s.employee,"own_only":true,"can_administer":false,"policies":[],"tasks":[],"reports":[],"submissions":[],"events":[]})
    }
    #[test]
    fn rejects_company_staff_and_foreign_record_projection() {
        let s = session();
        let mut c = context(&s);
        assert!(projection(&c, &s).is_ok());
        c["can_administer"] = json!(true);
        assert!(projection(&c, &s).is_err());
        c["can_administer"] = json!(false);
        c["tasks"] =
            json!([{"id":Uuid::new_v4(),"company_id":s.company,"employee_id":Uuid::new_v4()}]);
        assert!(projection(&c, &s).is_err());
    }
    #[test]
    fn rejects_foreign_evidence_nested_in_own_submission() {
        let s = session();
        let mut c = context(&s);
        c["submissions"] = json!([{"company_id":s.company,"employee_id":s.employee,"task_snapshot":[{"company_id":s.company,"employee_id":Uuid::new_v4()}]}]);
        assert!(projection(&c, &s).is_err());
    }
    #[test]
    fn own_projection_excludes_staff_actor_and_internal_fields() {
        let s = session();
        let mut c = context(&s);
        c["tasks"] = json!([{"id":Uuid::new_v4(),"company_id":s.company,"employee_id":s.employee,"created_by":Uuid::new_v4(),"internal_secret":"fictional-private-field","title":"Own task"}]);
        let projected = projection(&c, &s).unwrap();
        assert_eq!(projected["tasks"][0]["title"], "Own task");
        assert!(projected["tasks"][0].get("created_by").is_none());
        assert!(projected["tasks"][0].get("internal_secret").is_none());
    }
}
