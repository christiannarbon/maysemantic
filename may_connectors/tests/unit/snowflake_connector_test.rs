use async_trait::async_trait;
use futures::stream::StreamExt;
use may_connectors::SnowflakeConnector;
use may_connectors::{ColumnValue, ConnectorError, WarehouseConnector};
use may_secrets::{DwhSecret, SecretsProvider};
use rsa::RsaPrivateKey;
use rsa::pkcs8::EncodePrivateKey;
use serde_json::json;
use std::sync::{Arc, Mutex};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

// TODO: Extract into a shared test helper module when a third connector test is added.
struct MockSecretsProvider {
    secret: DwhSecret,
    called: Mutex<bool>,
}

#[async_trait]
impl SecretsProvider for MockSecretsProvider {
    async fn get_secret(&self, _name: &str) -> Result<DwhSecret, may_secrets::SecretsError> {
        *self.called.lock().unwrap() = true;
        Ok(self.secret.clone())
    }
}

fn generate_test_rsa_key_pem() -> String {
    let mut rng = rand::thread_rng();
    let priv_key = RsaPrivateKey::new(&mut rng, 2048).expect("Failed to generate test key");
    let pem = priv_key
        .to_pkcs8_pem(rsa::pkcs8::LineEnding::LF)
        .expect("Failed to encode PEM");
    pem.to_string()
}

#[tokio::test]
async fn test_snowflake_connector_instantiation() {
    let secrets = Arc::new(MockSecretsProvider {
        secret: DwhSecret::KeyPair {
            account: "test_account".to_string(),
            username: "test_user".to_string(),
            private_key: generate_test_rsa_key_pem(),
            passphrase: None,
        },
        called: Mutex::new(false),
    });

    let connector = SnowflakeConnector::new("test_account", "test_secret", secrets.clone());
    assert!(connector.is_ok());
}

#[tokio::test]
async fn test_snowflake_exponential_backoff() {
    let mock_server = MockServer::start().await;

    let secrets = Arc::new(MockSecretsProvider {
        secret: DwhSecret::KeyPair {
            account: "test_account".to_string(),
            username: "test_user".to_string(),
            private_key: generate_test_rsa_key_pem(),
            passphrase: None,
        },
        called: Mutex::new(false),
    });

    let connector = SnowflakeConnector::new("test_account", "test_secret", secrets)
        .expect("Failed to build connector")
        .with_test_overrides(mock_server.uri(), 1);

    let initial_response = json!({
        "statementHandle": "12345",
        "message": "Query execution in progress."
    });

    Mock::given(method("POST"))
        .and(path("/api/v2/statements"))
        .respond_with(ResponseTemplate::new(202).set_body_json(initial_response))
        .mount(&mock_server)
        .await;

    // First two polls return 202
    Mock::given(method("GET"))
        .and(path("/api/v2/statements/12345"))
        .respond_with(ResponseTemplate::new(202).set_body_json(json!({
            "statementHandle": "12345",
            "message": "Still running."
        })))
        .up_to_n_times(2)
        .mount(&mock_server)
        .await;

    // Third poll returns 200 with result
    let final_response = json!({
        "statementHandle": "12345",
        "code": "090001",
        "message": "Statement executed successfully.",
        "resultSetMetaData": {
            "rowType": [
                { "name": "COL_1", "type": "FIXED", "scale": 0 }
            ]
        },
        "data": [
            [ "42" ]
        ]
    });

    Mock::given(method("GET"))
        .and(path("/api/v2/statements/12345"))
        .respond_with(ResponseTemplate::new(200).set_body_json(final_response))
        .mount(&mock_server)
        .await;

    // Since we wait 500ms + 1000ms = 1.5s total, it's fast enough without pause.
    let mut stream = connector.execute("SELECT 1").await.unwrap();

    let row = stream.next().await.unwrap().unwrap();
    assert_eq!(row[0], ColumnValue::Int64(42));

    assert!(stream.next().await.is_none());
}

#[tokio::test]
async fn test_snowflake_timeout() {
    let mock_server = MockServer::start().await;

    let secrets = Arc::new(MockSecretsProvider {
        secret: DwhSecret::KeyPair {
            account: "test".to_string(),
            username: "test".to_string(),
            private_key: generate_test_rsa_key_pem(),
            passphrase: None,
        },
        called: Mutex::new(false),
    });

    let connector = SnowflakeConnector::new("test", "test", secrets)
        .unwrap()
        .with_test_overrides(mock_server.uri(), 1);

    let initial_response = json!({
        "statementHandle": "timeout-handle"
    });

    Mock::given(method("POST"))
        .and(path("/api/v2/statements"))
        .respond_with(ResponseTemplate::new(202).set_body_json(initial_response))
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v2/statements/timeout-handle"))
        .respond_with(ResponseTemplate::new(202).set_body_json(json!({})))
        .mount(&mock_server)
        .await;

    let mut stream = connector.execute("SELECT 1").await.unwrap();

    let res = stream.next().await.unwrap();
    match res {
        Err(ConnectorError::Timeout) => {}
        other => panic!("Expected Timeout error, got {other:?}"),
    }
}

#[tokio::test]
async fn test_snowflake_partition_merging() {
    let mock_server = MockServer::start().await;

    let secrets = Arc::new(MockSecretsProvider {
        secret: DwhSecret::KeyPair {
            account: "test".to_string(),
            username: "test".to_string(),
            private_key: generate_test_rsa_key_pem(),
            passphrase: None,
        },
        called: Mutex::new(false),
    });

    let connector = SnowflakeConnector::new("test", "test", secrets)
        .unwrap()
        .with_test_overrides(mock_server.uri(), 1);

    let initial_response = json!({
        "statementHandle": "part123",
        "code": "090001",
        "resultSetMetaData": {
            "rowType": [ { "name": "COL_1", "type": "TEXT" } ],
            "partitionInfo": [
                { "rowCount": 1 },
                { "rowCount": 1 }
            ]
        },
        "data": [ [ "part0_row" ] ]
    });

    Mock::given(method("POST"))
        .and(path("/api/v2/statements"))
        .respond_with(ResponseTemplate::new(200).set_body_json(initial_response))
        .mount(&mock_server)
        .await;

    let part1_response = json!({
        "data": [ [ "part1_row" ] ]
    });

    Mock::given(method("GET"))
        .and(path("/api/v2/statements/part123"))
        .and(query_param("partition", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(part1_response))
        .mount(&mock_server)
        .await;

    let mut stream = connector.execute("SELECT").await.unwrap();

    let row0 = stream.next().await.unwrap().unwrap();
    assert_eq!(row0[0], ColumnValue::Text("part0_row".to_string()));

    let row1 = stream.next().await.unwrap().unwrap();
    assert_eq!(row1[0], ColumnValue::Text("part1_row".to_string()));

    assert!(stream.next().await.is_none());
}

#[tokio::test]
async fn test_jwt_token_is_cached() {
    let secrets = Arc::new(MockSecretsProvider {
        secret: DwhSecret::KeyPair {
            account: "test_account".to_string(),
            username: "test_user".to_string(),
            private_key: generate_test_rsa_key_pem(),
            passphrase: None,
        },
        called: Mutex::new(false),
    });

    let connector = SnowflakeConnector::new("test_account", "test_secret", secrets.clone())
        .expect("Failed to build connector");

    // Call get_jwt_token once
    let token1 = connector.get_jwt_token().await.expect("Failed to generate token 1");
    assert!(*secrets.called.lock().unwrap(), "MockSecretsProvider should have been called");

    // Reset called flag
    *secrets.called.lock().unwrap() = false;

    // Call get_jwt_token again
    let token2 = connector.get_jwt_token().await.expect("Failed to generate token 2");

    // Assert tokens are identical (cached)
    assert_eq!(token1, token2, "Token should be retrieved from cache");
    assert!(!*secrets.called.lock().unwrap(), "MockSecretsProvider should NOT have been called again");
}

