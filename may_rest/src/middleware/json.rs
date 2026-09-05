use axum::{
    Json, async_trait,
    extract::{FromRequest, Request, rejection::JsonRejection},
    http::StatusCode,
};
use serde::de::DeserializeOwned;

use crate::error::{ApiError, api_error};

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
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match Json::<T>::from_request(req, state).await {
            Ok(Json(value)) => Ok(Self(value)),
            Err(rejection) => Err(reject(&rejection)),
        }
    }
}

/// Normalises a rejection to the crate's `{"error": ...}` envelope.
///
/// `JsonDataError` (422) and `MissingJsonContentType` (415) are deliberately reported as
/// `400` so that every "your body is wrong" outcome has one status. `BytesRejection` is
/// NOT: a body over the size limit is a `413`, and telling the caller `400` would send it
/// off to fix syntax that is fine.
fn reject(rejection: &JsonRejection) -> ApiError {
    let status = match rejection {
        JsonRejection::JsonDataError(_) | JsonRejection::MissingJsonContentType(_) => {
            StatusCode::BAD_REQUEST
        }
        other => other.status(),
    };
    api_error(status, rejection.body_text())
}
