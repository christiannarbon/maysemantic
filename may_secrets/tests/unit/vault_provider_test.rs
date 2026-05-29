use may_secrets::{DwhSecret, SecretsError, SecretsProvider, VaultConfig, VaultSecretsProvider};
use serial_test::serial;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
#[serial]
async fn test_vault_token_cache_hit() {
    let mock_server = MockServer::start().await;

    let response_body = serde_json::json!({
        "data": {
            "data": {
                "type": "username_password",
                "host": "db.example.com",
                "port": 5432,
                "database": "mydb",
                "username": "admin",
                "password": "secret"
            }
        }
    });

    Mock::given(method("GET"))
        .and(path("/v1/secret/data/my-conn"))
        .and(header("X-Vault-Token", "test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
        .expect(1) // EXACTLY 1 REQUEST
        .mount(&mock_server)
        .await;

    let config = VaultConfig::builder()
        .address(mock_server.uri())
        .token("test-token")
        .build()
        .unwrap();

    let provider = VaultSecretsProvider::new(config).unwrap();

    // First call fetches from mock
    let secret = provider.get_secret("my-conn").await.unwrap();
    match secret {
        DwhSecret::UsernamePassword {
            host,
            port,
            database,
            username,
            password,
        } => {
            assert_eq!(host, "db.example.com");
            assert_eq!(port, 5432);
            assert_eq!(database, "mydb");
            assert_eq!(username, "admin");
            assert_eq!(password, "secret");
        }
        _ => panic!("Expected UsernamePassword"),
    }

    // Second call serves from cache (Mock expectation confirms 1 request)
    let secret2 = provider.get_secret("my-conn").await.unwrap();
    assert!(matches!(secret2, DwhSecret::UsernamePassword { .. }));
}

#[tokio::test]
#[serial]
async fn test_vault_secret_not_found() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/secret/data/missing-conn"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock_server)
        .await;

    let config = VaultConfig::builder()
        .address(mock_server.uri())
        .token("test-token")
        .build()
        .unwrap();

    let provider = VaultSecretsProvider::new(config).unwrap();
    let err = provider.get_secret("missing-conn").await.unwrap_err();

    assert!(matches!(err, SecretsError::SecretNotFound(_)));
}

#[tokio::test]
#[serial]
async fn test_vault_error_non_404() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/secret/data/error-conn"))
        .respond_with(ResponseTemplate::new(500).set_body_string("internal error"))
        .mount(&mock_server)
        .await;

    let config = VaultConfig::builder()
        .address(mock_server.uri())
        .token("test-token")
        .build()
        .unwrap();

    let provider = VaultSecretsProvider::new(config).unwrap();
    let err = provider.get_secret("error-conn").await.unwrap_err();

    match err {
        SecretsError::VaultError { status, message } => {
            assert_eq!(status, 500);
            assert!(message.contains("internal error"));
        }
        _ => panic!("Expected VaultError"),
    }
}

#[tokio::test]
#[serial]
async fn test_vault_approle_login_and_fetch() {
    let mock_server = MockServer::start().await;

    let auth_response = serde_json::json!({
        "auth": {
            "client_token": "approle-token"
        }
    });

    Mock::given(method("POST"))
        .and(path("/v1/auth/approle/login"))
        .respond_with(ResponseTemplate::new(200).set_body_json(auth_response))
        .expect(1) // Called exactly once
        .mount(&mock_server)
        .await;

    let kv_response = serde_json::json!({
        "data": {
            "data": {
                "type": "service_account_key",
                "json": "{\"key\": \"value\"}"
            }
        }
    });

    Mock::given(method("GET"))
        .and(path("/v1/secret/data/my-sa"))
        .and(header("X-Vault-Token", "approle-token")) // Uses AppRole token
        .respond_with(ResponseTemplate::new(200).set_body_json(kv_response))
        .expect(1)
        .mount(&mock_server)
        .await;

    let config = VaultConfig::builder()
        .address(mock_server.uri())
        .approle("role-id", "secret-id")
        .build()
        .unwrap();

    let provider = VaultSecretsProvider::new(config).unwrap();

    let secret = provider.get_secret("my-sa").await.unwrap();
    match secret {
        DwhSecret::ServiceAccountKey { json } => assert_eq!(json, "{\"key\": \"value\"}"),
        _ => panic!("Expected ServiceAccountKey"),
    }

    let secret2 = provider.get_secret("my-sa").await.unwrap();
    assert!(matches!(secret2, DwhSecret::ServiceAccountKey { .. }));
}
