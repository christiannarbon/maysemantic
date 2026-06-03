use async_trait::async_trait;
use may_connectors::{BigQueryConnector, ConnectorError, WarehouseConnector};
use may_secrets::{DwhSecret, SecretsProvider};
use std::sync::{Arc, Mutex};

struct MockSecretsProvider {
    called: Mutex<bool>,
}

#[async_trait]
impl SecretsProvider for MockSecretsProvider {
    async fn get_secret(&self, _name: &str) -> Result<DwhSecret, may_secrets::SecretsError> {
        *self.called.lock().unwrap() = true;
        Ok(DwhSecret::ServiceAccountKey {
            json: "invalid json".to_string(),
        })
    }
}

#[tokio::test]
async fn test_bigquery_connector_caching_logic() {
    let secrets = Arc::new(MockSecretsProvider {
        called: Mutex::new(false),
    });
    let connector = BigQueryConnector::new("proj-123", "secret-456", secrets.clone());

    // Seed cache with a valid-looking token and an expiry way in the future (3600 seconds)
    connector
        .seed_cache_for_test("cached-token".to_string(), 3600)
        .await;

    // Because the cache is valid, `execute` will use it to make an HTTP request to BQ with the fake token, which fails with 401 Unauthenticated
    let stream_res = connector.execute("SELECT 1").await;
    match stream_res {
        Err(err) => {
            assert!(matches!(err, ConnectorError::QueryFailed(_)));
        }
        Ok(_) => panic!("Expected error, got Ok"),
    }

    // Now seed cache with an expired token (expires in 30 seconds, less than 60s TTL)
    connector
        .seed_cache_for_test("expired-token".to_string(), 30)
        .await;

    let stream_res = connector.execute("SELECT 1").await;

    // Because the cache is expired, it will try to get a new token using the invalid JSON, failing with ConnectionFailed
    match stream_res {
        Err(err) => {
            assert!(matches!(err, ConnectorError::ConnectionFailed(_)));
        }
        Ok(_) => panic!("Expected error, got Ok"),
    }
}

#[tokio::test]
async fn test_bigquery_null_value_mapping() {
    use may_connectors::dwh::bigquery::map_cell;
    use serde_json::json;

    let null_val = json!(null);
    let res = map_cell(&null_val, "STRING").unwrap();
    assert!(matches!(res, may_connectors::ColumnValue::Null));

    let nested_val = json!({"foo": "bar"});
    let res2 = map_cell(&nested_val, "RECORD").unwrap();
    assert!(matches!(res2, may_connectors::ColumnValue::Text(s) if s == "{\"foo\":\"bar\"}"));
}
