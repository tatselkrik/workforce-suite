//! Own shared service requests. This module offers no staff queue or routing proxy.
use super::*;
use std::collections::BTreeSet;

pub(super) fn routes() -> Router<Portal> {
    Router::new().route("/api/services", get(list)).route(
        "/api/services/save",
        post(save).layer(DefaultBodyLimit::max(24576)),
    )
}
fn text(s: &str, min: usize, max: usize) -> bool {
    s == s.trim()
        && (min..=max).contains(&s.chars().count())
        && !s
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
}
fn identity(v: &Value, s: &Session) -> Reply<()> {
    own(v, s)?;
    if id(v, "user_id")? != s.user || v["schema_version"] != 1 {
        return Err(unavailable());
    }
    Ok(())
}
fn status(v: &Value) -> Reply<&str> {
    v.as_str()
        .filter(|v| {
            matches!(
                *v,
                "submitted" | "in_progress" | "waiting_on_employee" | "completed" | "cancelled"
            )
        })
        .ok_or_else(unavailable)
}
fn stamp(v: &Value) -> bool {
    v.as_str()
        .is_some_and(|v| v.len() < 80 && chrono::DateTime::parse_from_rfc3339(v).is_ok())
}
fn message(v: &Value, revision: i64) -> Reply<Value> {
    if id(v, "id")?.is_nil()
        || v["revision"].as_i64() != Some(revision)
        || !v["body"].as_str().is_some_and(|v| text(v, 3, 12000))
        || !v["actor_name"].as_str().is_some_and(|v| text(v, 1, 200))
        || !stamp(&v["created_at"])
        || !v["kind"].as_str().is_some_and(|v| {
            if revision == 1 {
                v == "submit"
            } else {
                matches!(v, "reply" | "route" | "progress" | "cancel")
            }
        })
    {
        return Err(unavailable());
    }
    status(&v["status"])?;
    Ok(project(
        v,
        &[
            "id",
            "revision",
            "kind",
            "actor_name",
            "body",
            "status",
            "created_at",
        ],
    ))
}
fn own_projection(v: &Value, s: &Session) -> Reply<Value> {
    identity(v, s)?;
    let rows = v["requests"].as_array().ok_or_else(unavailable)?;
    if rows.len() > 1000 {
        return Err(unavailable());
    }
    let mut out = vec![];
    let mut ids = BTreeSet::new();
    let mut event_ids = BTreeSet::new();
    let mut total = 0usize;
    for r in rows {
        let rid = id(r, "id")?;
        let current_status = status(&r["status"])?;
        let revision = r["revision"]
            .as_i64()
            .filter(|n| *n > 0 && *n <= 10000)
            .ok_or_else(unavailable)?;
        if rid.is_nil()
            || !ids.insert(rid)
            || r["code"].as_str()
                != Some(format!("SR-{}", rid.simple().to_string().to_uppercase()).as_str())
            || !r["title"].as_str().is_some_and(|s| text(s, 3, 160))
            || !r["category"].as_str().is_some_and(|v| {
                matches!(
                    v,
                    "employment_document"
                        | "information_update"
                        | "benefits_question"
                        | "general_support"
                )
            })
            || !r["priority"]
                .as_str()
                .is_some_and(|v| matches!(v, "normal" | "high"))
            || (!r["handler_name"].is_null()
                && !r["handler_name"].as_str().is_some_and(|v| text(v, 1, 200)))
            || (!r["due_on"].is_null()
                && !r["due_on"].as_str().is_some_and(|v| {
                    v.len() == 10 && chrono::NaiveDate::parse_from_str(v, "%Y-%m-%d").is_ok()
                }))
            || !stamp(&r["created_at"])
            || !stamp(&r["updated_at"])
        {
            return Err(unavailable());
        }
        let messages = r["messages"].as_array().ok_or_else(unavailable)?;
        total = total.checked_add(messages.len()).ok_or_else(unavailable)?;
        if total > 10000 || messages.len() as i64 != revision {
            return Err(unavailable());
        }
        let mut thread = vec![];
        for (index, e) in messages.iter().enumerate() {
            if !event_ids.insert(id(e, "id")?) {
                return Err(unavailable());
            }
            thread.push(message(e, index as i64 + 1)?);
        }
        if messages.last().and_then(|v| v["status"].as_str()) != Some(current_status) {
            return Err(unavailable());
        }
        let active = !matches!(current_status, "completed" | "cancelled");
        if r["can_reply"] != active || r["can_cancel"] != active {
            return Err(unavailable());
        }
        let mut safe = project(
            r,
            &[
                "id",
                "code",
                "title",
                "category",
                "priority",
                "status",
                "handler_name",
                "due_on",
                "revision",
                "created_at",
                "updated_at",
            ],
        );
        safe["can_reply"] = json!(active);
        safe["can_cancel"] = json!(active);
        safe["messages"] = json!(thread);
        out.push(safe);
    }
    Ok(json!({"schema_version":1,"requests":out}))
}
async fn list(State(p): State<Portal>, h: HeaderMap) -> Reply<Json<Value>> {
    let e = p.session(&h, false).await?;
    let s = e.lock().await;
    let v = p
        .upstream(
            "/rest/v1/rpc/portal_service_context",
            Method::POST,
            Some(&s.access),
            Some(json!({})),
        )
        .await?;
    Ok(Json(own_projection(&v, &s)?))
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Change {
    Submit {
        title: String,
        category: String,
        priority: String,
        body: String,
    },
    Reply {
        body: String,
    },
    Cancel {
        body: String,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Save {
    id: Uuid,
    request_id: Uuid,
    expected_revision: i64,
    change: Change,
}
impl Save {
    fn validate(&self) -> Reply<()> {
        if self.id.is_nil()
            || self.request_id.is_nil()
            || !(0..=10000).contains(&self.expected_revision)
            || serde_json::to_vec(self).map_err(|_| unavailable())?.len() > 24000
        {
            return Err(invalid(
                "Review the service request identity, version and message length.",
            ));
        }
        let body = match &self.change {
            Change::Submit {
                title,
                category,
                priority,
                body,
            } => {
                if self.expected_revision != 0
                    || !text(title, 3, 160)
                    || !matches!(
                        category.as_str(),
                        "employment_document"
                            | "information_update"
                            | "benefits_question"
                            | "general_support"
                    )
                    || !matches!(priority.as_str(), "normal" | "high")
                {
                    return Err(invalid("Review the request title, category and priority."));
                }
                body
            }
            Change::Reply { body } | Change::Cancel { body } => {
                if self.expected_revision == 0 {
                    return Err(invalid(
                        "Open a saved request before replying or cancelling.",
                    ));
                }
                body
            }
        };
        if !text(body, 3, 12000) {
            return Err(invalid(
                "Enter a shared message of 3–12,000 characters without unsupported control characters.",
            ));
        }
        Ok(())
    }
}
fn receipt(v: &Value, s: &Session, r: &Save) -> Reply<Value> {
    identity(v, s)?;
    if id(v, "id")? != r.id
        || id(v, "request_id")? != r.request_id
        || v["revision"].as_i64() != r.expected_revision.checked_add(1)
        || !stamp(&v["applied_at"])
    {
        return Err(unavailable());
    }
    Ok(project(
        v,
        &[
            "schema_version",
            "id",
            "request_id",
            "revision",
            "applied_at",
        ],
    ))
}
async fn save(State(p): State<Portal>, h: HeaderMap, Json(r): Json<Save>) -> Reply<Json<Value>> {
    r.validate()?;
    let e = p.session(&h, true).await?;
    let s = e.lock().await;
    let v=p.upstream("/rest/v1/rpc/portal_save_service",Method::POST,Some(&s.access),Some(json!({"p_request":r}))).await.map_err(|e|{
        if e.0==StatusCode::BAD_GATEWAY{Error(e.0,"The save may have completed. Retry the same saved request; do not prepare a replacement.")}else{e}
    })?;
    Ok(Json(receipt(&v, &s, &r)?))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn staff_identity_fields_and_staff_operations_are_rejected() {
        let raw = json!({"id":Uuid::new_v4(),"request_id":Uuid::new_v4(),"expected_revision":0,"change":{"kind":"submit","title":"Fictional service","category":"general_support","priority":"normal","body":"Fictional shared message"}});
        let r: Save = serde_json::from_value(raw.clone()).unwrap();
        r.validate().unwrap();
        for key in ["employee_id", "company_id", "actor_id", "audience"] {
            let mut bad = raw.clone();
            bad[key] = json!(Uuid::new_v4());
            assert!(serde_json::from_value::<Save>(bad).is_err());
        }
        for key in ["employee_id", "assignee_user_id", "due_on"] {
            let mut bad = raw.clone();
            bad["change"][key] = json!("untrusted");
            assert!(serde_json::from_value::<Save>(bad).is_err());
        }
        for kind in ["route", "progress", "case"] {
            let mut bad = raw.clone();
            bad["change"]["kind"] = json!(kind);
            assert!(serde_json::from_value::<Save>(bad).is_err());
        }
    }
}
