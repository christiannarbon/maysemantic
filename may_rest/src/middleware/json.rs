use axum::{
    Json, async_trait,
    extract::{FromRequest, Request, rejection::JsonRejection},
    http::StatusCode,
};
use serde::de::DeserializeOwned;
use serde_json::json;

/// A drop-in replacement for [`axum::Json`] as an extractor.
///
/// `axum::Json` rejects a syntactically valid body with the wrong shape as `422`
/// and a missing content type as `415`, in both cases with a `text/plain` body.
/// Every other endpoint in this crate answers `{"error": "..."}`. This wrapper
/// normalises all deserialisation failures to `400 Bad Request` with that envelope,
/// so REST clients can parse errors the same way they parse successes.
pub struct ValidatedJson<T>(pub T);

#[async_trait]
impl<T, S> FromRequest<S> for ValidatedJson<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = (StatusCode, Json<serde_json::Value>);

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match Json::<T>::from_request(req, state).await {
            Ok(Json(value)) => Ok(Self(value)),
            Err(rejection) => Err(reject(&rejection)),
        }
    }
}

fn reject(rejection: &JsonRejection) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({ "error": rejection.body_text() })),
    )
}
