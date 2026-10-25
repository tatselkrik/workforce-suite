//! Personal inbox only; desktop staff tasks and private document reminders stay
//! in their existing scopes. Read markers confer no record permissions.
use super::*;
use axum::extract::Query;

pub(super) fn routes() -> Router<Portal> {
    Router::new()
        .route("/api/notifications", get(inbox))
        .route("/api/notifications/read", post(reads))
}
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Page {
    #[serde(default)]
    offset: u32,
    #[serde(default)]
    unread: bool,
}
fn key_valid(key: &str) -> bool {
    if lifecycle_key(key) {
        return true;
    }
    let p: Vec<_> = key.split(':').collect();
    key.len() <= 130
        && matches!(p.len(), 3 | 4)
        && matches!(
            p[0],
            "leave-status"
                | "leave-upcoming"
                | "correction-status"
                | "overtime-status"
                | "checkout"
        )
        && p[1]
            .parse::<Uuid>()
            .is_ok_and(|u| !u.is_nil() && u.to_string() == p[1])
        && !p[2].is_empty()
        && p[2].bytes().all(|v| v.is_ascii_digit())
        && p[2].parse::<i64>().is_ok_and(|n| n >= 0)
        && if p[0] == "leave-upcoming" {
            p.len() == 4 && matches!(p[3], "soon" | "today")
        } else {
            p.len() == 3
        }
}
fn lifecycle_key(key: &str) -> bool {
    let p: Vec<_> = key.split(':').collect();
    if !(p.len() == 3 || p.len() == 4) || key.len() > 130 {
        return false;
    }
    let staged = matches!(
        p[0],
        "training-start" | "training-due" | "certificate-expiry"
    );
    let plain = matches!(
        p[0],
        "payroll-monthly-issued"
            | "payroll-cutoff-issued"
            | "payroll-thirteenth-issued"
            | "payroll-tax-issued"
            | "payroll-final-issued"
            | "payroll-amendment-issued"
            | "payroll-receipt"
            | "payroll-final-settled"
            | "payroll-amendment-paid"
            | "appraisal-published"
    );
    (staged || plain)
        && p[1]
            .parse::<Uuid>()
            .is_ok_and(|u| !u.is_nil() && u.to_string() == p[1])
        && p[2]
            .parse::<i64>()
            .is_ok_and(|n| n >= 0 && n.to_string() == p[2])
        && if staged {
            p.len() == 4 && matches!(p[3], "soon" | "today" | "expired")
        } else {
            p.len() == 3
        }
}
fn identity(v: &Value, s: &Session) -> Reply<()> {
    own(v, s)?;
    if id(v, "user_id")? != s.user {
        return Err(unavailable());
    }
    Ok(())
}
async fn inbox(State(p): State<Portal>, h: HeaderMap, Query(q): Query<Page>) -> Reply<Json<Value>> {
    if q.offset > 50000 {
        return Err(invalid("Choose a bounded inbox page."));
    }
    let entry = p.session_activity(&h, false, false).await?;
    let s = entry.lock().await;
    let v = p
        .upstream(
            "/rest/v1/rpc/portal_notifications_v2",
            Method::POST,
            Some(&s.access),
            Some(json!({"p_offset":q.offset,"p_limit":25,"p_unread_only":q.unread})),
        )
        .await?;
    identity(&v, &s)?;
    if v["version"] != "portal-notifications-v2" {
        return Err(unavailable());
    }
    if v["offset"].as_u64() != Some(u64::from(q.offset))
        || v["limit"].as_u64() != Some(25)
        || v["unread_only"].as_bool() != Some(q.unread)
        || v["total_items"].as_u64().is_none()
        || v["unread_count"].as_u64().is_none()
    {
        return Err(unavailable());
    }
    let total = v["total_items"].as_u64().ok_or_else(unavailable)?;
    let unread = v["unread_count"].as_u64().ok_or_else(unavailable)?;
    if (q.unread && unread != total) || (!q.unread && unread > total) {
        return Err(unavailable());
    }
    let mut out = project(
        &v,
        &[
            "asof",
            "generated_at",
            "offset",
            "limit",
            "unread_only",
            "total_items",
            "unread_count",
        ],
    );
    let mut items = vec![];
    let rows = v["items"].as_array().ok_or_else(unavailable)?;
    if rows.len() > 25 {
        return Err(unavailable());
    }
    if rows.len() as u64 != total.saturating_sub(u64::from(q.offset)).min(25) {
        return Err(unavailable());
    }
    for r in rows {
        let key = r["key"].as_str().ok_or_else(unavailable)?;
        let kind = r["kind"].as_str().ok_or_else(unavailable)?;
        if !key_valid(key)
            || key.split(':').next() != Some(kind)
            || id(r, "target_id")?.is_nil()
            || r["read"].as_bool().is_none()
            || r["urgent"].as_bool().is_none()
        {
            return Err(unavailable());
        }
        let expected_page = if lifecycle_key(key) {
            match kind {
                "payroll-monthly-issued" => "payroll_monthly",
                "payroll-cutoff-issued" => "payroll_cutoff",
                "payroll-thirteenth-issued" => "payroll_thirteenth",
                "payroll-tax-issued" => "payroll_tax",
                "payroll-final-issued" | "payroll-final-settled" => "payroll_final",
                "payroll-amendment-issued" | "payroll-amendment-paid" => "payroll_amendment",
                "payroll-receipt" => r["target_page"]
                    .as_str()
                    .filter(|s| {
                        matches!(
                            *s,
                            "payroll_monthly" | "payroll_cutoff" | "payroll_thirteenth"
                        )
                    })
                    .ok_or_else(unavailable)?,
                "appraisal-published" => "appraisal",
                "training-start" | "training-due" => "training",
                "certificate-expiry" => "certificate",
                _ => return Err(unavailable()),
            }
        } else if matches!(kind, "leave-status" | "leave-upcoming") {
            "leave"
        } else {
            "attendance"
        };
        if kind != "payroll-receipt"
            && key.split(':').nth(1).and_then(|k| k.parse::<Uuid>().ok())
                != Some(id(r, "target_id")?)
        {
            return Err(unavailable());
        }
        if r["target_page"].as_str() != Some(expected_page)
            || r["title"].as_str().is_none_or(|s| s.chars().count() > 200)
            || r["detail"]
                .as_str()
                .is_none_or(|s| s.chars().count() > 600 || s.chars().any(char::is_control))
        {
            return Err(unavailable());
        }
        let mut item = project(
            r,
            &[
                "key",
                "kind",
                "title",
                "detail",
                "date",
                "urgent",
                "read",
                "target_id",
                "target_page",
            ],
        );
        if lifecycle_key(key) {
            item["target_page"] = json!(if expected_page == "payroll_monthly" {
                "pay"
            } else {
                "notifications"
            });
        }
        items.push(item);
    }
    out["items"] = json!(items);
    Ok(Json(out))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reads {
    keys: Vec<String>,
    read: bool,
    #[serde(default)]
    reset: bool,
}
async fn reads(State(p): State<Portal>, h: HeaderMap, Json(d): Json<Reads>) -> Reply<Json<Value>> {
    if d.keys.len() > 100
        || d.keys.iter().any(|s| !key_valid(s))
        || d.keys
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len()
            != d.keys.len()
        || if d.reset {
            !d.keys.is_empty() || d.read
        } else {
            d.keys.is_empty()
        }
    {
        return Err(invalid(
            "Choose1–100 own notifications, or reset only your read history.",
        ));
    }
    let e = p.session(&h, true).await?;
    let s = e.lock().await;
    let v = p
        .upstream(
            "/rest/v1/rpc/portal_notification_reads_v2",
            Method::POST,
            Some(&s.access),
            Some(json!({"p_keys":d.keys,"p_read":d.read,"p_reset":d.reset})),
        )
        .await
        .map_err(|e| {
            if e.0 == StatusCode::BAD_GATEWAY {
                Error(e.0, "Read history may have been saved. Refresh your inbox.")
            } else {
                e
            }
        })?;
    identity(&v, &s)?;
    if v["saved"] != true {
        return Err(unavailable());
    }
    Ok(Json(json!({"saved":true})))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keys_are_bounded_own_inbox_categories_not_staff_tasks() {
        let id = "00000000-0000-0000-0000-00000000006e";
        for value in [
            format!("leave-status:{id}:1"),
            format!("leave-upcoming:{id}:1:soon"),
            format!("checkout:{id}:0"),
            format!("payroll-monthly-issued:{id}:0"),
            format!("training-due:{id}:1:expired"),
            format!("certificate-expiry:{id}:1:soon"),
        ] {
            assert!(key_valid(&value));
        }
        for value in [
            format!("leave-review:{id}:1"),
            format!("document-expiry:{id}:1:soon"),
            format!("checkout:{id}:1:today"),
            format!("leave-upcoming:{id}:1"),
            format!("leave-status:{id}:-1"),
            format!("appraisal-review:{id}:1:soon"),
            format!("payroll-monthly-issued:{id}:0:soon"),
            format!("training-due:{id}:01:soon"),
            "checkout:00000000-0000-0000-0000-000000000000:0".into(),
        ] {
            assert!(!key_valid(&value));
        }
    }
}
