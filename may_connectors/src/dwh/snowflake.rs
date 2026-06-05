use crate::{ColumnValue, ConnectorError, QueryResult, Row, WarehouseConnector};
use async_stream::stream;
use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD as B64_STANDARD};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use may_secrets::{DwhSecret, SecretsProvider};
use reqwest::Client;
use rsa::RsaPrivateKey;
use rsa::pkcs1::EncodeRsaPrivateKey;
use rsa::pkcs8::{DecodePrivateKey, EncodePublicKey};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize)]
struct JwtClaims {
    iss: String,
    sub: String,
    iat: usize,
    exp: usize,
}

#[derive(Clone)]
pub struct SnowflakeConnector {
    account: String,
    secret_name: String,
    secrets: Arc<dyn SecretsProvider>,
    http_client: Client,
    base_url_override: Option<String>,
    delay_ms_override: Option<u64>,
}

impl SnowflakeConnector {
    /// Creates a new `SnowflakeConnector`.
    ///
    /// # Errors
    ///
    /// Returns a `ConnectorError::ConnectionFailed` if the underlying HTTP client cannot be built.
    pub fn new(
        account: impl Into<String>,
        secret_name: impl Into<String>,
        secrets: Arc<dyn SecretsProvider>,
    ) -> Result<Self, ConnectorError> {
        #[allow(
            clippy::duration_suboptimal_units,
            reason = "60 seconds is idiomatic for timeouts"
        )]
        let http_client = reqwest::ClientBuilder::new()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .map_err(|e| ConnectorError::ConnectionFailed(e.to_string()))?;

        Ok(Self {
            account: account.into(),
            secret_name: secret_name.into(),
            secrets,
            http_client,
            base_url_override: None,
            delay_ms_override: None,
        })
    }

    #[doc(hidden)]
    #[must_use]
    pub fn with_test_overrides(mut self, url: String, delay_ms: u64) -> Self {
        self.base_url_override = Some(url);
        self.delay_ms_override = Some(delay_ms);
        self
    }

    // TODO: Cache the JWT token and refresh only when within 60 seconds of expiry.
    async fn get_jwt_token(&self) -> Result<String, ConnectorError> {
        let secret = self
            .secrets
            .get_secret(&self.secret_name)
            .await
            .map_err(|e| {
                ConnectorError::ConnectionFailed(format!("Failed to retrieve secret: {e}"))
            })?;

        let DwhSecret::KeyPair {
            account: account_name,
            username,
            private_key: private_key_pem,
            passphrase,
        } = secret
        else {
            return Err(ConnectorError::ConnectionFailed(
                "Invalid secret type for Snowflake connector, expected KeyPair".to_string(),
            ));
        };

        // Parse RSA private key
        let private_key = if let Some(pass) = &passphrase {
            RsaPrivateKey::from_pkcs8_encrypted_pem(&private_key_pem, pass).map_err(|e| {
                ConnectorError::ConnectionFailed(format!("Failed to parse encrypted RSA PEM: {e}"))
            })?
        } else {
            RsaPrivateKey::from_pkcs8_pem(&private_key_pem).map_err(|e| {
                ConnectorError::ConnectionFailed(format!("Failed to parse RSA PEM: {e}"))
            })?
        };

        // Generate public key fingerprint
        let public_key = private_key.to_public_key();
        let public_key_der = public_key.to_public_key_der().map_err(|e| {
            ConnectorError::ConnectionFailed(format!("Failed to encode public key to DER: {e}"))
        })?;
        let hash = Sha256::digest(public_key_der.as_bytes());
        let fingerprint_b64 = B64_STANDARD.encode(hash);
        let fingerprint = format!("SHA256:{fingerprint_b64}");

        let account_upper = account_name.to_uppercase();
        let username_upper = username.to_uppercase();

        let now = chrono::Utc::now().timestamp();
        let iat = usize::try_from(now).map_err(|e| {
            ConnectorError::ConnectionFailed(format!("Timestamp conversion failed: {e}"))
        })?;
        let exp = usize::try_from(now + 3600).map_err(|e| {
            ConnectorError::ConnectionFailed(format!("Timestamp conversion failed: {e}"))
        })?;

        let claims = JwtClaims {
            iss: format!("{account_upper}.{username_upper}.{fingerprint}"),
            sub: format!("{account_upper}.{username_upper}"),
            iat,
            exp,
        };

        let header = Header::new(Algorithm::RS256);

        // Re-encode private key to unencrypted DER so jsonwebtoken can parse it
        let der = private_key
            .to_pkcs1_der()
            .map_err(|e| ConnectorError::ConnectionFailed(format!("Failed to encode DER: {e}")))?;
        let encoding_key = EncodingKey::from_rsa_der(der.as_bytes());

        let token = encode(&header, &claims, &encoding_key)
            .map_err(|e| ConnectorError::ConnectionFailed(format!("Failed to sign JWT: {e}")))?;
        Ok(token)
    }
}

fn map_row(
    row_arr: &[Value],
    col_names: &[String],
    col_types: &[String],
    col_scales: &[Option<i64>],
) -> Result<Row, ConnectorError> {
    let mut row = Row::new();
    for (i, cell) in row_arr.iter().enumerate() {
        let col_name = col_names.get(i).map_or("unknown", String::as_str);
        let col_type = col_types.get(i).map_or("TEXT", String::as_str);
        let col_scale = col_scales.get(i).copied().flatten();
        row.push(map_cell(cell, col_type, col_scale, col_name)?);
    }
    Ok(row)
}

fn map_cell(
    value: &Value,
    field_type: &str,
    scale: Option<i64>,
    col_name: &str,
) -> Result<ColumnValue, ConnectorError> {
    if value.is_null() {
        return Ok(ColumnValue::Null);
    }

    let Some(val_str) = value.as_str() else {
        return Err(ConnectorError::QueryFailed(format!(
            "Expected string value for column '{col_name}', but got: {value}"
        )));
    };

    let field_type_upper = field_type.to_uppercase();
    match field_type_upper.as_str() {
        "FIXED" | "NUMBER" => {
            if scale.unwrap_or(0) == 0 {
                if let Ok(i) = val_str.parse::<i64>() {
                    Ok(ColumnValue::Int64(i))
                } else {
                    Err(ConnectorError::QueryFailed(format!(
                        "Failed to parse FIXED (scale=0) for column '{col_name}': {val_str}"
                    )))
                }
            } else {
                if let Ok(f) = val_str.parse::<f64>() {
                    Ok(ColumnValue::Float64(f))
                } else {
                    Err(ConnectorError::QueryFailed(format!(
                        "Failed to parse FIXED (scale>0) for column '{col_name}': {val_str}"
                    )))
                }
            }
        }
        "REAL" | "FLOAT" | "DOUBLE" => {
            if let Ok(f) = val_str.parse::<f64>() {
                Ok(ColumnValue::Float64(f))
            } else {
                Err(ConnectorError::QueryFailed(format!(
                    "Failed to parse REAL/FLOAT for column '{col_name}': {val_str}"
                )))
            }
        }
        "BOOLEAN" => match val_str.to_lowercase().as_str() {
            "true" | "1" => Ok(ColumnValue::Bool(true)),
            "false" | "0" => Ok(ColumnValue::Bool(false)),
            _ => Err(ConnectorError::QueryFailed(format!(
                "Failed to parse BOOLEAN for column '{col_name}': {val_str}"
            ))),
        },
        "BINARY" => {
            let bytes = B64_STANDARD.decode(val_str).map_err(|e| {
                ConnectorError::QueryFailed(format!(
                    "Failed to decode base64 BINARY for column '{col_name}': {e}"
                ))
            })?;
            Ok(ColumnValue::Bytes(bytes))
        }
        _ => Ok(ColumnValue::Text(val_str.to_string())),
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "connector implementation contains complex polling loop"
)]
#[async_trait]
impl WarehouseConnector for SnowflakeConnector {
    async fn execute(&self, sql: &str) -> Result<QueryResult, ConnectorError> {
        let jwt_token = self.get_jwt_token().await?;
        let account = self.account.clone();
        let client = self.http_client.clone();
        let sql = sql.to_string();
        let base_url = self.base_url_override.clone();
        let delay_ms_override = self.delay_ms_override;

        let stream = stream! {
            let url = if let Some(base) = &base_url {
                format!("{base}/api/v2/statements")
            } else {
                format!("https://{account}.snowflakecomputing.com/api/v2/statements")
            };
            let body = serde_json::json!({
                "statement": sql,
                "timeout": 60
            });

            let res = match client
                .post(&url)
                .header("Authorization", format!("Bearer {jwt_token}"))
                .header("Content-Type", "application/json")
                .header("Accept", "application/json")
                .header("X-Snowflake-Authorization-Type", "KEYPAIR_JWT")
                .json(&body)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    yield Err(ConnectorError::ConnectionFailed(e.to_string()));
                    return;
                }
            };

            let status = res.status();
            let is_async = status == reqwest::StatusCode::ACCEPTED;

            if !status.is_success() && status != reqwest::StatusCode::ACCEPTED {
                let err_text = res.text().await.unwrap_or_default();
                yield Err(ConnectorError::QueryFailed(format!("Initial request failed: {err_text}")));
                return;
            }

            let mut json: Value = match res.json().await {
                Ok(j) => j,
                Err(e) => {
                    yield Err(ConnectorError::QueryFailed(format!("Failed to parse JSON response: {e}")));
                    return;
                }
            };

            let statement_handle = json.get("statementHandle").and_then(Value::as_str).unwrap_or("").to_string();

            // If async (202), poll until completion
            if is_async {
                if statement_handle.is_empty() {
                    yield Err(ConnectorError::QueryFailed("No statementHandle found in 202 response".to_string()));
                    return;
                }

                let mut retries = 0;
                let max_retries = 10;
                let mut delay_ms = delay_ms_override.unwrap_or(500);

                // Exponential backoff: starting at 500ms, doubling per retry, capped at 30s.
                loop {
                    if retries >= max_retries {
                        yield Err(ConnectorError::Timeout);
                        return;
                    }

                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;

                    let poll_url = if let Some(base) = &base_url {
                        format!("{base}/api/v2/statements/{statement_handle}")
                    } else {
                        format!("https://{account}.snowflakecomputing.com/api/v2/statements/{statement_handle}")
                    };
                    let poll_res = match client
                        .get(&poll_url)
                        .header("Authorization", format!("Bearer {jwt_token}"))
                        .header("Accept", "application/json")
                        .header("X-Snowflake-Authorization-Type", "KEYPAIR_JWT")
                        .send()
                        .await
                    {
                        Ok(r) => r,
                        Err(e) => {
                            yield Err(ConnectorError::ConnectionFailed(format!("Polling failed: {e}")));
                            return;
                        }
                    };

                    let poll_status = poll_res.status();

                    if poll_status == reqwest::StatusCode::ACCEPTED {
                        retries += 1;
                        delay_ms = (delay_ms * 2).min(30_000);
                        continue;
                    }
                    if poll_status.is_success() {
                        json = match poll_res.json().await {
                            Ok(j) => j,
                            Err(e) => {
                                yield Err(ConnectorError::QueryFailed(format!("Failed to parse poll JSON: {e}")));
                                return;
                            }
                        };
                        break;
                    }
                    let err_text = poll_res.text().await.unwrap_or_default();
                    yield Err(ConnectorError::QueryFailed(format!("Polling request returned error: {err_text}")));
                    return;
                }
            }

            // Check if status is explicitly "FAILED"
            let sf_code = json.get("code").and_then(Value::as_str).unwrap_or("");
            if sf_code.eq_ignore_ascii_case("failed") {
                let msg = json.get("message").and_then(Value::as_str).unwrap_or("Unknown failure");
                yield Err(ConnectorError::QueryFailed(msg.to_string()));
                return;
            }

            // Extract metadata and data
            let Some(metadata) = json.get("resultSetMetaData").and_then(|m| m.get("rowType")).and_then(Value::as_array) else {
                return;
            };

            let mut col_names = Vec::new();
            let mut col_types = Vec::new();
            let mut col_scales = Vec::new();

            for col in metadata {
                let name = col.get("name").and_then(Value::as_str).unwrap_or("unknown").to_string();
                let c_type = col.get("type").and_then(Value::as_str).unwrap_or("TEXT").to_string();
                let scale = col.get("scale").and_then(Value::as_i64);
                col_names.push(name);
                col_types.push(c_type);
                col_scales.push(scale);
            }

            let partition_count = json
                .get("resultSetMetaData")
                .and_then(|m| m.get("partitionInfo"))
                .and_then(Value::as_array)
                .map_or(0, Vec::len);

            // First partition is inline in "data" array
            if let Some(data) = json.get("data").and_then(Value::as_array) {
                for row_val in data {
                    if let Some(row_arr) = row_val.as_array() {
                        match map_row(row_arr, &col_names, &col_types, &col_scales) {
                            Ok(row) => yield Ok(row),
                            Err(e) => {
                                yield Err(e);
                                return;
                            }
                        }
                    }
                }
            }

            // If there are more partitions
            if partition_count > 1 {
                for partition_idx in 1..partition_count {
                    let partition_url = if let Some(base) = &base_url {
                        format!("{base}/api/v2/statements/{statement_handle}?partition={partition_idx}")
                    } else {
                        format!("https://{account}.snowflakecomputing.com/api/v2/statements/{statement_handle}?partition={partition_idx}")
                    };
                    let part_res = match client
                        .get(&partition_url)
                        .header("Authorization", format!("Bearer {jwt_token}"))
                        .header("Accept", "application/json")
                        .header("X-Snowflake-Authorization-Type", "KEYPAIR_JWT")
                        .send()
                        .await
                    {
                        Ok(r) => r,
                        Err(e) => {
                            yield Err(ConnectorError::ConnectionFailed(format!("Partition fetch failed: {e}")));
                            return;
                        }
                    };

                    if !part_res.status().is_success() {
                        let err_text = part_res.text().await.unwrap_or_default();
                        yield Err(ConnectorError::QueryFailed(format!("Failed to fetch partition {partition_idx}: {err_text}")));
                        return;
                    }

                    let part_json: Value = match part_res.json().await {
                        Ok(j) => j,
                        Err(e) => {
                            yield Err(ConnectorError::QueryFailed(format!("Failed to parse partition JSON: {e}")));
                            return;
                        }
                    };

                    if let Some(data) = part_json.get("data").and_then(Value::as_array) {
                        for row_val in data {
                            if let Some(row_arr) = row_val.as_array() {
                                match map_row(row_arr, &col_names, &col_types, &col_scales) {
                                    Ok(row) => yield Ok(row),
                                    Err(e) => {
                                        yield Err(e);
                                        return;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        };

        Ok(Box::pin(stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{DecodingKey, Validation, decode, decode_header};
    use may_secrets::SecretsError;
    use rsa::pkcs8::{EncodePrivateKey, EncodePublicKey, LineEnding};
    use std::sync::Mutex;

    struct MockSecretsProvider {
        secret: DwhSecret,
        called: Mutex<bool>,
    }

    #[async_trait]
    impl SecretsProvider for MockSecretsProvider {
        async fn get_secret(&self, _name: &str) -> Result<DwhSecret, SecretsError> {
            *self.called.lock().unwrap() = true;
            Ok(self.secret.clone())
        }
    }

    fn generate_test_rsa_key_pem() -> String {
        let mut rng = rand::thread_rng();
        let priv_key = RsaPrivateKey::new(&mut rng, 2048).expect("Failed to generate test key");
        priv_key
            .to_pkcs8_pem(LineEnding::LF)
            .expect("Failed to encode PEM")
            .to_string()
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
        let priv_key =
            RsaPrivateKey::from_pkcs8_pem(&pem).expect("Failed to parse original test key");
        let pub_key_pem = priv_key
            .to_public_key()
            .to_public_key_pem(LineEnding::LF)
            .expect("Failed to get public key PEM");

        let mut validation = Validation::new(jsonwebtoken::Algorithm::RS256);
        validation.validate_exp = false;
        validation.required_spec_claims.clear();

        let decoded = decode::<JwtClaims>(
            &token,
            &DecodingKey::from_rsa_pem(pub_key_pem.as_bytes())
                .expect("Failed to create decoding key"),
            &validation,
        )
        .expect("Failed to decode and verify JWT");

        let claims = decoded.claims;
        assert_eq!(claims.sub, "TEST_ACCOUNT.TEST_USER");
        assert!(claims.iss.starts_with("TEST_ACCOUNT.TEST_USER.SHA256:"));
    }
}
