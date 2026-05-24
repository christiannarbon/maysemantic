use anyhow::{Context, Result};
use async_trait::async_trait;
use may_auth::error::AuthError;
use may_auth::models::{Role, User};
use may_auth::repository::UserRepository;
use may_core::StateMgr;
use pgwire::tokio::process_socket;
use sqlx::PgPool;
use std::env;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::signal;
use tracing::{error, info, warn};

mod handler;
use handler::{QueryProcessor, QueryProcessorFactory};

/// Fallback repository used when no `DATABASE_URL` is configured.
/// Every authentication attempt is rejected with a clear diagnostic error.
/// This allows the PGWire container to boot (e.g. in smoke-test environments)
/// without requiring a backing Postgres instance.
struct DenyAllRepository;

#[async_trait]
impl UserRepository for DenyAllRepository {
    async fn find_by_username(&self, _username: &str) -> Result<User, AuthError> {
        Err(AuthError::InvalidCredentials)
    }
    async fn create(
        &self,
        _username: &str,
        _password_hash: &str,
        _role: Role,
    ) -> Result<User, AuthError> {
        Err(AuthError::InvalidCredentials)
    }
    async fn list(&self) -> Result<Vec<User>, AuthError> {
        Err(AuthError::InvalidCredentials)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize structured logging
    tracing_subscriber::fmt::init();

    // Initialize semantic state
    info!("Loading semantic models from current directory...");
    let state_mgr = StateMgr::new();
    if let Err(e) = state_mgr.load_dir(".").await {
        error!("Failed to load semantic models: {:?}", e);
    } else if let Ok(stats) = state_mgr.get_stats() {
        info!(
            "Successfully loaded {} models ({} entities, {} metrics)",
            stats.model_count, stats.entity_count, stats.metric_count
        );
    }

    let shared_state = Arc::new(state_mgr);
    let processor = Arc::new(QueryProcessor::new(shared_state));

    let repository: Arc<dyn UserRepository + Send + Sync> = match env::var("DATABASE_URL") {
        Ok(database_url) => match PgPool::connect(&database_url).await {
            Ok(pool) => {
                info!("Connected to identity database.");
                Arc::new(may_auth::repository::PgUserRepository::new(pool))
            }
            Err(e) => {
                warn!(
                    "Failed to connect to identity database: {:?}. \
                         Authentication will be disabled — all logins will be rejected.",
                    e
                );
                Arc::new(DenyAllRepository)
            }
        },
        Err(_) => {
            warn!(
                "DATABASE_URL not set. \
                 Authentication is disabled — all logins will be rejected."
            );
            Arc::new(DenyAllRepository)
        }
    };
    let authenticator = Arc::new(handler::PgWireAuthenticator::new(repository));

    let factory = Arc::new(QueryProcessorFactory::new(processor, authenticator));

    let addr = "0.0.0.0:5432";
    let listener = TcpListener::bind(addr)
        .await
        .context("Failed to bind TCP listener. Is port 5432 already in use?")?;

    info!("PGWire Gateway Service listening on {}", addr);

    loop {
        tokio::select! {
            // 1. Wait for incoming TCP connections
            accept_result = listener.accept() => {
                match accept_result {
                    Ok((stream, peer_addr)) => {
                        info!("Accepted connection from {}", peer_addr);

                        // Disable Nagle's algorithm explicitly
                        if let Err(e) = stream.set_nodelay(true) {
                            error!("Failed to set TCP_NODELAY: {:?}", e);
                        }

                        let factory_ref = factory.clone();

                        // 2. Spawn a non-blocking task for each connection using pgwire
                        tokio::spawn(async move {
                            if let Err(e) = process_socket(stream, None, factory_ref).await {
                                error!("Connection error for {}: {:?}", peer_addr, e);
                            }
                            info!("Connection closed for {}", peer_addr);
                        });
                    }
                    Err(e) => {
                        error!("Failed to accept incoming connection: {:?}", e);
                        // Do not break the loop on a single accept error
                    }
                }
            }
            // 3. Graceful shutdown handler
            _ = signal::ctrl_c() => {
                info!("Shutdown signal received. Initiating graceful shutdown...");
                break;
            }
        }
    }

    info!("PGWire Gateway Service shut down successfully.");
    Ok(())
}
