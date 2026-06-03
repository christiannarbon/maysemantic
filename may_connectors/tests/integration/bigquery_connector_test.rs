use futures::StreamExt;
use may_connectors::{BigQueryConnector, ColumnValue, WarehouseConnector};
use may_secrets::EnvSecretsProvider;
use std::env;
use std::sync::{Arc, Mutex};

// A mutex for test serialization if needed, though this test mainly reads env vars
static ENV_LOCK: Mutex<()> = Mutex::new(());

#[tokio::test]
#[allow(
    clippy::await_holding_lock,
    reason = "Integration tests might serialize mutations"
)]
async fn test_bigquery_connector_executes_query() -> Result<(), Box<dyn std::error::Error>> {
    if env::var("BQ_TESTS").is_err() {
        return Ok(());
    }

    let _guard = ENV_LOCK.lock().unwrap();

    let secrets = Arc::new(EnvSecretsProvider::new());

    let project_id = env::var("BQ_PROJECT_ID").expect("BQ_PROJECT_ID required for BQ_TESTS");
    let secret_name = env::var("BQ_SECRET_NAME").expect("BQ_SECRET_NAME required for BQ_TESTS");

    let connector = BigQueryConnector::new(project_id, secret_name, secrets);

    let mut result_stream = connector
        .execute("SELECT state, gender, year, name, number FROM `bigquery-public-data.usa_names.usa_1910_2013` LIMIT 5")
        .await
        .expect("Query execution failed");

    let mut row_count = 0;

    while let Some(row_res) = result_stream.next().await {
        let row = row_res.expect("Stream yielded error");
        row_count += 1;
        assert_eq!(row.len(), 5);

        let state = &row[0];
        assert!(matches!(state, ColumnValue::Text(_)));

        let name = &row[3];
        assert!(matches!(name, ColumnValue::Text(_)));

        let number = &row[4];
        assert!(matches!(number, ColumnValue::Int64(_)));
    }

    assert_eq!(row_count, 5);
    Ok(())
}
