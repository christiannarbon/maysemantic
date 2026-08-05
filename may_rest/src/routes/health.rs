use axum::{Json, http::StatusCode, response::IntoResponse};
use serde_json::json;

/// Liveness probe. Requires no auth and no application state.
///
/// Returns `200 OK` with `{"status":"ok"}` so load balancers and Kubernetes
/// readiness/liveness probes can verify the process is serving.
#[allow(clippy::unused_async, reason = "Axum async handler pattern")]
pub async fn health() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({ "status": "ok" })))
}
