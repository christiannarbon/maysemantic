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
async fn test_bigquery_connector_initialization() {
    let secrets = Arc::new(MockSecretsProvider {
        called: Mutex::new(false),
    });
    let _connector = BigQueryConnector::new("proj-123", "secret-456", secrets.clone());

    // get_secret is lazy, shouldn't be called on initialization
    assert!(!*secrets.called.lock().unwrap());
}

#[tokio::test]
async fn test_bigquery_connector_fails_on_invalid_json() {
    let secrets = Arc::new(MockSecretsProvider {
        called: Mutex::new(false),
    });
    let connector = BigQueryConnector::new("proj-123", "secret-456", secrets.clone());

    let result = connector.execute("SELECT 1").await;

    match result {
        Err(err) => {
            assert!(matches!(err, ConnectorError::ConnectionFailed(_)));
        }
        Ok(_) => panic!("Expected error, got Ok"),
    }
    // Should be called during execute
    assert!(*secrets.called.lock().unwrap());
}
