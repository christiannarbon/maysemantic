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

/// The versioned API tree, mounted at `/api/v1`. New v1 routes are registered here,
/// not in `build_router`.
pub fn v1_router() -> Router<crate::AppState> {
    Router::new().merge(query::router())
}
