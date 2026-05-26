use axum::{Json, Router, extract::State, http::StatusCode, response::IntoResponse, routing::post};
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::ToSchema;

const MAX_USERNAME_LEN: usize = 64;
const MAX_PASSWORD_LEN: usize = 128;

#[derive(Deserialize, ToSchema)]
pub struct LoginRequest {
    pub(crate) username: String,
    pub(crate) password: String,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct LoginResponse {
    pub token: String,
}

pub fn router() -> Router<crate::AppState> {
    Router::new().route("/login", post(login))
}

/// Handle login requests.
///
/// # Errors
///
/// Returns HTTP 401 if credentials are invalid or if an error occurs while verifying.
/// Returns HTTP 500 if the server fails to issue a token.
#[utoipa::path(
    post,
    path = "/auth/login",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful", body = LoginResponse),
        (status = 401, description = "Invalid credentials", body = serde_json::Value)
    )
)]
pub async fn login(
    State(state): State<crate::AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, impl IntoResponse> {
    let invalid_credentials = (
        StatusCode::UNAUTHORIZED,
        Json(json!({"error": "invalid credentials"})),
    );

    if payload.username.is_empty()
        || payload.password.is_empty()
        || payload.username.len() > MAX_USERNAME_LEN
        || payload.password.len() > MAX_PASSWORD_LEN
    {
        return Err(invalid_credentials);
    }

    let Ok(user) = state
        .user_repository
        .find_by_username(&payload.username)
        .await
    else {
        return Err(invalid_credentials);
    };

    let password = payload.password;
    let password_hash = user.password_hash.clone();

    let join_result = tokio::task::spawn_blocking(move || {
        may_auth::password::verify_password(&password, &password_hash)
    })
    .await;

    match join_result {
        Ok(Ok(true)) => {} // password correct, continue
        Ok(Ok(false)) => return Err(invalid_credentials),
        Ok(Err(_)) | Err(_) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "internal error"})),
            ));
        }
    }

    let token = state.token_service.issue(&user).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "internal error"})),
        )
    })?;

    Ok(Json(LoginResponse { token }))
}
