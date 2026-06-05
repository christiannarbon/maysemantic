use futures::StreamExt;
use may_connectors::{ColumnValue, SnowflakeConnector, WarehouseConnector};
use may_secrets::EnvSecretsProvider;
use std::env;
use std::sync::Arc;

#[tokio::test]
async fn test_snowflake_connector_executes_query() -> Result<(), Box<dyn std::error::Error>> {
    if env::var("SNOWFLAKE_TESTS").is_err() {
        return Ok(());
    }

    let secrets = Arc::new(EnvSecretsProvider::new());

    let account =
        env::var("SNOWFLAKE_ACCOUNT").expect("SNOWFLAKE_ACCOUNT required for SNOWFLAKE_TESTS");
    let secret_name =
        env::var("SNOWFLAKE_SECRET").expect("SNOWFLAKE_SECRET required for SNOWFLAKE_TESTS");

    let connector =
        SnowflakeConnector::new(account, secret_name, secrets).expect("Failed to build connector");

    let mut result_stream = connector
        .execute("SELECT CURRENT_VERSION()")
        .await
        .expect("Query execution failed");

    let mut row_count = 0;

    while let Some(row_res) = result_stream.next().await {
        let row = row_res.expect("Stream yielded error");
        row_count += 1;
        assert_eq!(row.len(), 1);

        let version = &row[0];
        assert!(matches!(version, ColumnValue::Text(_)));
    }

    assert_eq!(row_count, 1);
    Ok(())
}
