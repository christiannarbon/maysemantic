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

/// The dialects `may_core::dialect_for` can actually resolve. Anything else falls through
/// to Postgres inside that function, so an unrecognised value must be rejected here rather
/// than silently producing SQL for the wrong warehouse.
const SUPPORTED_DIALECTS: [&str; 3] = ["postgres", "snowflake", "bigquery"];

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
        // `load_dir` accepts a file or a directory and loads every model it finds, matching
        // what may_pgwire does. Booting with empty state is allowed, but at error! level:
        // an unreadable model path is a deployment fault, and downstream it would otherwise
        // surface as a client-facing "metric not found".
        if let Err(e) = state_mgr.load_dir(&model_path).await {
            tracing::error!("Failed to load semantic models from {model_path}: {e}");
        } else if let Ok(stats) = state_mgr.get_stats() {
            tracing::info!(
                "Loaded {} models ({} entities, {} metrics) from {model_path}",
                stats.model_count,
                stats.entity_count,
                stats.metric_count
            );
        }
    }

    let dialect_kind = env::var("MAY_DIALECT")
        .unwrap_or_else(|_| "postgres".to_string())
        .to_ascii_lowercase();
    if !SUPPORTED_DIALECTS.contains(&dialect_kind.as_str()) {
        return Err(anyhow::anyhow!(
            "MAY_DIALECT must be one of {}; got `{dialect_kind}`",
            SUPPORTED_DIALECTS.join(", ")
        ));
    }

    let state = AppState {
        user_repository,
        token_service,
        state_mgr,
        dialect_kind: dialect_kind.into(),
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
