use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;

/// The crate's single error response shape: a status plus an `{"error": "..."}` body.
///
/// Every fallible endpoint returns this, so a client can parse failures the same way
/// it parses successes. Constructing it anywhere other than through `api_error` or a
/// `From` impl is a sign the envelope is being re-invented.
#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub message: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.message }))).into_response()
    }
}

/// Build an [`ApiError`]. Preferred over the struct literal at call sites.
pub fn api_error(status: StatusCode, message: impl Into<String>) -> ApiError {
    ApiError {
        status,
        message: message.into(),
    }
}
