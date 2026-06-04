use async_trait::async_trait;
use jsonwebtoken::{DecodingKey, Validation, decode, decode_header};
use may_connectors::SnowflakeConnector;
use may_secrets::{DwhSecret, SecretsProvider};
use rsa::RsaPrivateKey;
use rsa::pkcs8::{DecodePrivateKey, EncodePrivateKey, EncodePublicKey};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Debug, Serialize, Deserialize)]
struct JwtClaims {
    iss: String,
    sub: String,
    iat: usize,
    exp: usize,
}

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
async fn test_snowflake_connector_jwt_generation() {
    let pem = generate_test_rsa_key_pem();
    let secrets = Arc::new(MockSecretsProvider {
        secret: DwhSecret::KeyPair {
            account: "test_account".to_string(),
            username: "test_user".to_string(),
            private_key: pem.clone(),
            passphrase: None,
        },
        called: Mutex::new(false),
    });

    let connector = SnowflakeConnector::new("test_account", "test_secret", secrets)
        .expect("Failed to build connector");
    let token = connector
        .get_jwt_token()
        .await
        .expect("Failed to generate token");

    // Verify headers
    let header = decode_header(&token).expect("Failed to decode JWT header");
    assert_eq!(header.alg, jsonwebtoken::Algorithm::RS256);

    // Verify token can be decoded with the public key
    let priv_key = RsaPrivateKey::from_pkcs8_pem(&pem).expect("Failed to parse original test key");
    let pub_key_pem = priv_key
        .to_public_key()
        .to_public_key_pem(rsa::pkcs8::LineEnding::LF)
        .expect("Failed to get public key PEM");

    let mut validation = Validation::new(jsonwebtoken::Algorithm::RS256);
    validation.validate_exp = false;
    validation.required_spec_claims.clear();

    let decoded = decode::<JwtClaims>(
        &token,
        &DecodingKey::from_rsa_pem(pub_key_pem.as_bytes()).expect("Failed to create decoding key"),
        &validation,
    )
    .expect("Failed to decode and verify JWT");

    let claims = decoded.claims;
    assert_eq!(claims.sub, "TEST_ACCOUNT.TEST_USER");
    assert!(claims.iss.starts_with("TEST_ACCOUNT.TEST_USER.SHA256:"));
}
