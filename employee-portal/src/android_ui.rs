//! Fixed, bundled Android artwork under the existing same-origin asset policy.
use axum::{Router, extract::Path, http::{StatusCode, header}, response::{IntoResponse, Response}, routing::get};
use crate::Portal;

pub(crate) fn routes() -> Router<Portal> {
    Router::new().route("/android-icons/{name}", get(icon))
}

async fn icon(Path(name): Path<String>) -> Response {
    let svg = match name.as_str() {
        "home.svg" => include_str!("../web/android-icons/home.svg"),
        "attendance.svg" => include_str!("../web/android-icons/attendance.svg"),
        "leave.svg" => include_str!("../web/android-icons/leave.svg"),
        "pay.svg" => include_str!("../web/android-icons/pay.svg"),
        "services.svg" => include_str!("../web/android-icons/services.svg"),
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    ([(header::CONTENT_TYPE, "image/svg+xml")], svg).into_response()
}
