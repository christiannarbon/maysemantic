use anyhow::Result;
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
use tokio::sync::broadcast;
use tracing::{error, info, warn};

use crate::handler::{PgWireAuthenticator, QueryProcessor, QueryProcessorFactory};

pub struct DenyAllRepository;

#[async_trait]
impl UserRepository for DenyAllRepository {
    async fn find_by_username(&self, _username: &str) -> Result<User, AuthError> {
        Err(AuthError::InvalidCredentials)
    }
    async fn create(&self, _u: &str, _p: &str, _r: Role) -> Result<User, AuthError> {
        Err(AuthError::InvalidCredentials)
    }
    async fn list(&self, _page: u32, _per_page: u32) -> Result<Vec<User>, AuthError> {
        Err(AuthError::UserNotFound)
    }

    async fn deactivate(&self, _id: uuid::Uuid) -> Result<(), AuthError> {
        Err(AuthError::UserNotFound)
    }
    async fn update(
        &self,
        _id: uuid::Uuid,
        _role: Option<Role>,
        _password_hash: Option<String>,
    ) -> Result<User, AuthError> {
        Err(AuthError::UserNotFound)
    }
}

pub async fn run_server(
    listener: TcpListener,
    database_url: Option<String>,
    mut shutdown_rx: broadcast::Receiver<()>,
) -> Result<()> {
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

    let db_url_opt = database_url.or_else(|| env::var("DATABASE_URL").ok());
    let repository: Arc<dyn UserRepository + Send + Sync> = match db_url_opt {
        Some(db_url) => match PgPool::connect(&db_url).await {
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
        None => {
            warn!(
                "DATABASE_URL not set. \
                 Authentication is disabled — all logins will be rejected."
            );
            Arc::new(DenyAllRepository)
        }
    };
    let authenticator = Arc::new(PgWireAuthenticator::new(repository));
    let factory = Arc::new(QueryProcessorFactory::new(processor, authenticator));

    info!(
        "PGWire Gateway Service listening on {}",
        listener.local_addr()?
    );

    loop {
        tokio::select! {
            accept_result = listener.accept() => {
                match accept_result {
                    Ok((stream, peer_addr)) => {
                        info!("Accepted connection from {}", peer_addr);
                        if let Err(e) = stream.set_nodelay(true) {
                            error!("Failed to set TCP_NODELAY: {:?}", e);
                        }

                        let factory_ref = factory.clone();
                        tokio::spawn(async move {
                            if let Err(e) = process_socket(stream, None, factory_ref).await {
                                error!("Connection error for {}: {:?}", peer_addr, e);
                            }
                            info!("Connection closed for {}", peer_addr);
                        });
                    }
                    Err(e) => {
                        error!("Failed to accept incoming connection: {:?}", e);
                    }
                }
            }
            _ = shutdown_rx.recv() => {
                info!("Shutdown signal received. Initiating graceful shutdown...");
                break;
            }
        }
    }

    info!("PGWire Gateway Service shut down successfully.");
    Ok(())
}
