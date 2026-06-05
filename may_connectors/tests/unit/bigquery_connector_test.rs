use async_trait::async_trait;
use may_connectors::{BigQueryConnector, ConnectorError, WarehouseConnector};
use may_secrets::{DwhSecret, SecretsError, SecretsProvider};
use std::sync::{Arc, Mutex};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// TODO: Extract into a shared test helper module when a third connector test is added.
struct MockSecretsProvider {
    called: Mutex<bool>,
}

#[async_trait]
impl SecretsProvider for MockSecretsProvider {
    async fn get_secret(&self, _name: &str) -> Result<DwhSecret, SecretsError> {
        *self.called.lock().unwrap() = true;
        Ok(DwhSecret::ServiceAccountKey {
            json: "invalid json".to_string(),
        })
    }
}

#[tokio::test]
async fn test_bigquery_connector_caching_logic() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/bigquery/v2/projects/proj-123/queries"))
        .respond_with(ResponseTemplate::new(401).set_body_string("Unauthorized"))
        .mount(&mock_server)
        .await;

    let secrets = Arc::new(MockSecretsProvider {
        called: Mutex::new(false),
    });

    let connector = BigQueryConnector::new("proj-123", "secret-456", secrets.clone())
        .unwrap()
        .with_test_overrides(mock_server.uri());

    // Seed cache with a valid-looking token and an expiry way in the future (3600 seconds)
    connector
        .seed_cache_for_test("cached-token".to_string(), 3600)
        .await;

    let stream_res = connector.execute("SELECT 1").await;
    match stream_res {
        Err(ConnectorError::QueryFailed(err_text)) => {
            assert_eq!(err_text, "Unauthorized");
        }
        _ => panic!("Expected QueryFailed, got Ok or another error"),
    }

    // Now seed cache with an expired token (expires in 30 seconds, less than 60s TTL)
    connector
        .seed_cache_for_test("expired-token".to_string(), 30)
        .await;

    let stream_res = connector.execute("SELECT 1").await;

    // Because the cache is expired, it will try to get a new token using the invalid JSON, failing with ConnectionFailed
    match stream_res {
        Err(ConnectorError::ConnectionFailed(_)) => {}
        _ => panic!("Expected ConnectionFailed, got Ok or another error"),
    }
}
