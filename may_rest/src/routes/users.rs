use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
};
use may_auth::models::{Role, User};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{
    AppState,
    error::{ApiError, api_error},
    middleware::{auth::AuthClaims, json::ValidatedJson},
};

const MAX_USERNAME_LEN: usize = 64;
const MAX_PASSWORD_LEN: usize = 128;

#[derive(Deserialize, ToSchema)]
pub struct CreateUserRequest {
    pub(crate) username: String,
    pub(crate) password: String,
    pub(crate) role: Role,
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateUserRequest {
    pub(crate) role: Option<Role>,
    pub(crate) password: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct UserResponse {
    pub id: uuid::Uuid,
    pub username: String,
    pub role: Role,
    pub is_active: bool,
    #[schema(value_type = String, format = DateTime)]
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[schema(value_type = String, format = DateTime)]
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<User> for UserResponse {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            username: user.username,
            role: user.role,
            is_active: user.is_active,
            created_at: user.created_at,
            updated_at: user.updated_at,
        }
    }
}

#[derive(Deserialize, IntoParams)]
pub struct PaginationQuery {
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_per_page")]
    pub per_page: u32,
}

fn default_page() -> u32 {
    1
}

fn default_per_page() -> u32 {
    20
}

/// Create a new user (admin only).
///
/// # Errors
///
/// Returns `403 Forbidden` if the caller's JWT role is `viewer`.
/// Returns `400 Bad Request` if username or password is empty or exceeds length limits.
/// Returns `500 Internal Server Error` if password hashing or database write fails.
#[utoipa::path(
    post,
    path = "/api/users",
    request_body = CreateUserRequest,
    responses(
        (status = 201, description = "User created successfully", body = UserResponse),
        (status = 400, description = "Bad request – invalid input"),
        (status = 401, description = "Unauthorized – missing or invalid JWT"),
        (status = 403, description = "Forbidden – requires admin role"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn create_user(
    State(state): State<AppState>,
    claims: AuthClaims,
    ValidatedJson(payload): ValidatedJson<CreateUserRequest>,
) -> Result<impl IntoResponse, ApiError> {
    if claims.0.role != Role::Admin {
        return Err(api_error(
            StatusCode::FORBIDDEN,
            "forbidden: requires admin role",
        ));
    }

    if payload.username.is_empty()
        || payload.password.is_empty()
        || payload.username.len() > MAX_USERNAME_LEN
        || payload.password.len() > MAX_PASSWORD_LEN
    {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "username/password must be non-empty and within length limits",
        ));
    }

    let password = payload.password;
    let password_hash =
        tokio::task::spawn_blocking(move || may_auth::password::hash_password(&password))
            .await
            .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "internal error"))?
            .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "failed to hash password"))?;

    let user = state
        .user_repository
        .create(&payload.username, &password_hash, payload.role)
        .await
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "failed to create user"))?;

    Ok((StatusCode::CREATED, Json(UserResponse::from(user))))
}

/// List all users with pagination (admin only).
///
/// # Errors
///
/// Returns `403 Forbidden` if the caller's JWT role is `viewer`.
/// Returns `500 Internal Server Error` if the database query fails.
#[utoipa::path(
    get,
    path = "/api/users",
    params(PaginationQuery),
    responses(
        (status = 200, description = "List of users", body = [UserResponse]),
        (status = 401, description = "Unauthorized – missing or invalid JWT"),
        (status = 403, description = "Forbidden – requires admin role"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn list_users(
    State(state): State<AppState>,
    claims: AuthClaims,
    Query(pagination): Query<PaginationQuery>,
) -> Result<impl IntoResponse, ApiError> {
    if claims.0.role != Role::Admin {
        return Err(api_error(
            StatusCode::FORBIDDEN,
            "forbidden: requires admin role",
        ));
    }

    let users = state
        .user_repository
        .list(pagination.page, pagination.per_page)
        .await
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "failed to list users"))?;

    let response: Vec<UserResponse> = users.into_iter().map(UserResponse::from).collect();

    Ok((StatusCode::OK, Json(response)))
}

/// Soft-deactivate a user by ID (admin only).
///
/// # Errors
///
/// Returns `403 Forbidden` if the caller's JWT role is `viewer`.
/// Returns `404 Not Found` if no active user with that ID exists.
/// Returns `500 Internal Server Error` if the database update fails.
#[utoipa::path(
    delete,
    path = "/api/users/{id}",
    params(
        ("id" = uuid::Uuid, Path, description = "User ID to deactivate")
    ),
    responses(
        (status = 204, description = "User deactivated successfully"),
        (status = 401, description = "Unauthorized – missing or invalid JWT"),
        (status = 403, description = "Forbidden – requires admin role"),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn deactivate_user(
    State(state): State<AppState>,
    claims: AuthClaims,
    Path(id): Path<uuid::Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    if claims.0.role != Role::Admin {
        return Err(api_error(
            StatusCode::FORBIDDEN,
            "forbidden: requires admin role",
        ));
    }

    match state.user_repository.deactivate(id).await {
        Ok(()) => Ok(StatusCode::NO_CONTENT.into_response()),
        Err(may_auth::error::AuthError::UserNotFound) => {
            Err(api_error(StatusCode::NOT_FOUND, "user not found"))
        }
        Err(_) => Err(api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "failed to deactivate user",
        )),
    }
}

/// Update a user's role or password (admin only).
///
/// # Errors
///
/// Returns `403 Forbidden` if the caller's JWT role is `viewer`.
/// Returns `404 Not Found` if the user is not found.
/// Returns `500 Internal Server Error` if hashing or updating fails.
#[utoipa::path(
    put,
    path = "/api/users/{id}",
    request_body = UpdateUserRequest,
    params(
        ("id" = uuid::Uuid, Path, description = "User ID to update")
    ),
    responses(
        (status = 200, description = "User updated successfully", body = UserResponse),
        (status = 401, description = "Unauthorized – missing or invalid JWT"),
        (status = 403, description = "Forbidden – requires admin role"),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn update_user(
    State(state): State<AppState>,
    claims: AuthClaims,
    Path(id): Path<uuid::Uuid>,
    ValidatedJson(payload): ValidatedJson<UpdateUserRequest>,
) -> Result<impl IntoResponse, ApiError> {
    if claims.0.role != Role::Admin {
        return Err(api_error(
            StatusCode::FORBIDDEN,
            "forbidden: requires admin role",
        ));
    }

    let password_hash = match payload.password {
        Some(pw) => {
            match tokio::task::spawn_blocking(move || may_auth::password::hash_password(&pw)).await
            {
                Ok(Ok(h)) => Some(h),
                _ => {
                    return Err(api_error(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "failed to hash password",
                    ));
                }
            }
        }
        None => None,
    };

    match state
        .user_repository
        .update(id, payload.role, password_hash)
        .await
    {
        Ok(user) => Ok((StatusCode::OK, Json(UserResponse::from(user))).into_response()),
        Err(may_auth::error::AuthError::UserNotFound) => {
            Err(api_error(StatusCode::NOT_FOUND, "user not found"))
        }
        Err(_) => Err(api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "failed to update user",
        )),
    }
}

pub fn router() -> Router<crate::AppState> {
    Router::new()
        .route("/", post(create_user))
        .route("/", get(list_users))
        .route("/:id", delete(deactivate_user).put(update_user))
}
