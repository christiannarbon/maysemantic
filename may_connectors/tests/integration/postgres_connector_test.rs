use futures::StreamExt;
use may_connectors::{ConnectorError, PostgresConnector, WarehouseConnector};
use may_secrets::EnvSecretsProvider;
use std::env;
use std::sync::{Arc, Mutex, OnceLock};

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn init_crypto() {
    static CRYPTO_INIT: OnceLock<()> = OnceLock::new();
    CRYPTO_INIT.get_or_init(|| {
        rustls::crypto::ring::default_provider()
            .install_default()
            .expect("Failed to install rustls crypto provider");
    });
}

#[allow(unsafe_code, reason = "setting environment variables for test setup")]
fn setup_pagila_env() {
    unsafe {
        env::set_var("MAY_SECRET_PAGILA_TYPE", "username_password");
        env::set_var("MAY_SECRET_PAGILA_HOST", "127.0.0.1");
        env::set_var("MAY_SECRET_PAGILA_PORT", "5433");
        env::set_var("MAY_SECRET_PAGILA_DATABASE", "pagila");
        env::set_var("MAY_SECRET_PAGILA_USERNAME", "postgres");
        env::set_var("MAY_SECRET_PAGILA_PASSWORD", "may_password");
    }
}

#[tokio::test]
#[allow(
    clippy::await_holding_lock,
    reason = "Integration tests must serialize env mutations across awaits"
)]
async fn test_postgres_connector_executes_query() -> Result<(), Box<dyn std::error::Error>> {
    if env::var("PAGILA_TESTS").is_err() {
        return Ok(());
    }
    let _guard = ENV_LOCK.lock().unwrap();
    init_crypto();
    setup_pagila_env();

    let secrets = Arc::new(EnvSecretsProvider::new());
    let connector = PostgresConnector::new("pagila", secrets);

    let mut stream = connector
        .execute("SELECT actor_id, first_name, last_name FROM actor LIMIT 10")
        .await?;

    let mut row_count = 0;
    let mut first_row_cols = 0;
    while let Some(res) = stream.next().await {
        let row = res?;
        if row_count == 0 {
            first_row_cols = row.len();
        }
        row_count += 1;
    }

    assert!(row_count > 0);
    assert!(row_count <= 10);
    assert!(first_row_cols > 0);
    Ok(())
}

#[tokio::test]
#[allow(
    clippy::await_holding_lock,
    reason = "Integration tests must serialize env mutations across awaits"
)]
async fn test_postgres_connector_handles_invalid_query() -> Result<(), Box<dyn std::error::Error>> {
    if env::var("PAGILA_TESTS").is_err() {
        return Ok(());
    }
    let _guard = ENV_LOCK.lock().unwrap();
    init_crypto();
    setup_pagila_env();

    let secrets = Arc::new(EnvSecretsProvider::new());
    let connector = PostgresConnector::new("pagila", secrets);

    let mut stream = match connector.execute("SELECT * FROM nonexistent_table").await {
        Ok(s) => s,
        Err(e) => {
            assert!(matches!(e, ConnectorError::QueryFailed(_)));
            return Ok(());
        }
    };

    match stream.next().await {
        Some(Err(ConnectorError::QueryFailed(_))) => {}
        other => panic!("Expected QueryFailed error, got {other:?}"),
    }
    Ok(())
}

#[tokio::test]
#[allow(
    clippy::await_holding_lock,
    reason = "Integration tests must serialize env mutations across awaits"
)]
async fn test_postgres_connector_handles_connection_failure()
-> Result<(), Box<dyn std::error::Error>> {
    if env::var("PAGILA_TESTS").is_err() {
        return Ok(());
    }
    let _guard = ENV_LOCK.lock().unwrap();
    init_crypto();

    #[allow(unsafe_code, reason = "setting environment variables for test setup")]
    unsafe {
        env::set_var("MAY_SECRET_BAD_DB_TYPE", "username_password");
        env::set_var("MAY_SECRET_BAD_DB_HOST", "127.0.0.1");
        env::set_var("MAY_SECRET_BAD_DB_PORT", "9999"); // Wrong port
        env::set_var("MAY_SECRET_BAD_DB_DATABASE", "pagila");
        env::set_var("MAY_SECRET_BAD_DB_USERNAME", "postgres");
        env::set_var("MAY_SECRET_BAD_DB_PASSWORD", "wrong_password");
    }

    let secrets = Arc::new(EnvSecretsProvider::new());
    let connector = PostgresConnector::new("bad_db", secrets);

    let result = connector.execute("SELECT 1").await;
    assert!(matches!(result, Err(ConnectorError::ConnectionFailed(_))));
    Ok(())
}
