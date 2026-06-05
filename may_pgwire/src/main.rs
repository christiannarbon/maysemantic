use anyhow::Result;
use tokio::signal;
use tokio::sync::broadcast;
use tracing::info;

mod handler;
mod server;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let port = std::env::var("PGWIRE_PORT")
        .unwrap_or_else(|_| "5432".to_string())
        .parse::<u16>()
        .unwrap_or(5432);

    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

    let secrets = std::sync::Arc::new(may_secrets::EnvSecretsProvider::new());

    let server_task = tokio::spawn(async move {
        if let Err(e) = server::run_server(listener, None, secrets, shutdown_rx).await {
            tracing::error!("Server error: {:?}", e);
        }
    });

    match signal::ctrl_c().await {
        Ok(()) => {
            info!("Ctrl-C received, shutting down...");
            let _ = shutdown_tx.send(());
        }
        Err(err) => {
            tracing::error!("Unable to listen for shutdown signal: {}", err);
        }
    }

    let _ = server_task.await;
    Ok(())
}
