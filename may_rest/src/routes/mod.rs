use axum::Router;
use axum::routing::{delete, get, post};

pub mod auth;
pub mod users;

pub fn router() -> Router<crate::AppState> {
    Router::new()
        .route("/auth/login", post(auth::login))
        .route("/users", post(users::create_user))
        .route("/users", get(users::list_users))
        .route("/users/:id", delete(users::deactivate_user))
}
