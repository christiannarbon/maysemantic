#![allow(clippy::needless_for_each, reason = "utoipa macro generates this")]

use std::sync::Arc;
use utoipa::OpenApi;

pub mod routes;

#[derive(Clone)]
pub struct AppState {
    pub user_repository: Arc<dyn may_auth::repository::UserRepository + Send + Sync>,
    pub token_service: Arc<may_auth::token::TokenService>,
}

#[derive(OpenApi)]
#[openapi(
    paths(
        routes::auth::login,
    ),
    components(
        schemas(routes::auth::LoginRequest, routes::auth::LoginResponse)
    ),
    tags(
        (name = "auth", description = "Authentication endpoints")
    )
)]
pub struct ApiDoc;
