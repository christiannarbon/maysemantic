use axum::Router;

pub mod auth;

pub fn router() -> Router<crate::AppState> {
    Router::new().nest("/auth", auth::router())
}
