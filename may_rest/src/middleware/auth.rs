use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use may_auth::token::Claims;

use crate::{AppState, error::api_error};

pub struct AuthClaims(pub Claims);

#[async_trait]
impl FromRequestParts<AppState> for AuthClaims {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get("Authorization")
            .and_then(|value| value.to_str().ok())
            .filter(|value| value.starts_with("Bearer "))
            .map(|value| &value["Bearer ".len()..]);

        let Some(token) = auth_header else {
            return Err(api_error(
                StatusCode::UNAUTHORIZED,
                "missing or malformed authorization header",
            )
            .into_response());
        };

        if let Ok(claims) = state.token_service.verify(token) {
            Ok(Self(claims))
        } else {
            Err(api_error(StatusCode::UNAUTHORIZED, "invalid or expired token").into_response())
        }
    }
}
