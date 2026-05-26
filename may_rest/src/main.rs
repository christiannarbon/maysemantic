use axum::Router;
use may_auth::{repository::PgUserRepository, token::TokenService};
use may_rest::{ApiDoc, AppState, routes};
use sqlx::postgres::PgPoolOptions;
use std::env;
use std::sync::Arc;
use tokio::net::TcpListener;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:may_password@localhost:5433/pagila".to_string());

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    // In a real application, MAY_JWT_SECRET should be set in the environment.
    if env::var("MAY_JWT_SECRET").is_err() {
        tracing::warn!("MAY_JWT_SECRET not set, using a default secret for development");
        #[allow(unsafe_code, reason = "set_var required for testing default logic")]
        unsafe {
            env::set_var("MAY_JWT_SECRET", "super_secret_development_key_123");
        }
    }

    let token_service = Arc::new(TokenService::new()?);
    let user_repository = Arc::new(PgUserRepository::new(pool));

    let state = AppState {
        user_repository,
        token_service,
    };

    let app = Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .nest("/api", routes::router())
        .with_state(state);

    let port = env::var("MAY_REST_PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse::<u16>()
        .unwrap_or(3000);

    let addr = format!("0.0.0.0:{port}");
    tracing::info!("May REST API listening on {}", addr);
    tracing::info!("Swagger UI available at http://{}/swagger-ui", addr);

    let listener = TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
