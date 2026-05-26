#![allow(clippy::needless_for_each, reason = "utoipa macro generates this")]

use std::sync::Arc;
use utoipa::OpenApi;

pub mod middleware;
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
        routes::users::create_user,
        routes::users::list_users,
        routes::users::deactivate_user,
        routes::users::update_user
    ),
    components(
        schemas(
            routes::auth::LoginRequest,
            routes::auth::LoginResponse,
            routes::users::CreateUserRequest,
            routes::users::UpdateUserRequest,
            routes::users::UserResponse
        )
    ),
    modifiers(&SecurityAddon),
    tags(
        (name = "auth", description = "Authentication endpoints"),
        (name = "users", description = "User management endpoints")
    )
)]
pub struct ApiDoc;

struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        // SAFETY: utoipa always initialises `components` before calling modifiers.
        #[allow(clippy::unwrap_used, reason = "utoipa guarantees components is Some")]
        let components = openapi.components.as_mut().unwrap();
        components.add_security_scheme(
            "bearerAuth",
            utoipa::openapi::security::SecurityScheme::Http(
                utoipa::openapi::security::HttpBuilder::new()
                    .scheme(utoipa::openapi::security::HttpAuthScheme::Bearer)
                    .bearer_format("JWT")
                    .build(),
            ),
        );
    }
}
