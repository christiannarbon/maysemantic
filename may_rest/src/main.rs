use may_auth::{repository::PgUserRepository, token::TokenService};
#[cfg(feature = "swagger")]
use may_rest::ApiDoc;
use may_rest::AppState;
use sqlx::postgres::PgPoolOptions;
use std::env;
use std::sync::Arc;
use tokio::net::TcpListener;
#[cfg(feature = "swagger")]
use utoipa::OpenApi;
#[cfg(feature = "swagger")]
use utoipa_swagger_ui::SwaggerUi;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let database_url = env::var("DATABASE_URL")
        .map_err(|_| anyhow::anyhow!("DATABASE_URL environment variable must be set"))?;

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    may_auth::seed::ensure_admin(&pool).await?;

    let token_service = Arc::new(TokenService::new()?);
    let user_repository = Arc::new(PgUserRepository::new(pool));

    let state_mgr = Arc::new(may_core::StateMgr::new());
    if let Ok(model_path) = env::var("MAY_MODEL_PATH") {
        match std::fs::read_to_string(&model_path) {
            Ok(yaml) => {
                if let Err(e) = state_mgr.load_from_yaml(&yaml) {
                    tracing::warn!("Failed to load semantic model from {model_path}: {e}");
                } else {
                    tracing::info!("Loaded semantic model from {model_path}");
                }
            }
            Err(e) => tracing::warn!("Could not read MAY_MODEL_PATH {model_path}: {e}"),
        }
    }

    let dialect_kind = env::var("MAY_DIALECT").unwrap_or_else(|_| "postgres".to_string());

    let state = AppState {
        user_repository,
        token_service,
        state_mgr,
        dialect_kind,
    };

    let app = may_rest::build_router(state);

    #[cfg(feature = "swagger")]
    let app =
        app.merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()));

    let port: u16 = env::var("MAY_REST_PORT")
        .as_deref()
        .unwrap_or("3000")
        .parse()
        .map_err(|_| anyhow::anyhow!("MAY_REST_PORT must be a valid port number (0–65535)"))?;

    let addr = format!("0.0.0.0:{port}");
    tracing::info!("May REST API listening on {}", addr);
    #[cfg(feature = "swagger")]
    tracing::info!("Swagger UI available at http://{}/swagger-ui", addr);

    let listener = TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
