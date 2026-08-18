use axum::Router;

pub mod auth;
pub mod health;
pub mod query;
pub mod users;

pub fn router() -> Router<crate::AppState> {
    Router::new()
        .nest("/auth", auth::router())
        .nest("/users", users::router())
}
