use crate::{ColumnValue, ConnectorError, QueryResult, Row, WarehouseConnector};
use async_stream::stream;
use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD as B64_STANDARD};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use may_secrets::{DwhSecret, SecretsProvider};
use reqwest::Client;
use rsa::RsaPrivateKey;
use rsa::pkcs8::{DecodePrivateKey, EncodePrivateKey, EncodePublicKey};
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
        })
    }

    #[doc(hidden)]
    pub async fn get_jwt_token(&self) -> Result<String, ConnectorError> {
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

        let claims = JwtClaims {
            iss: format!("{account_upper}.{username_upper}.{fingerprint}"),
            sub: format!("{account_upper}.{username_upper}"),
            iat: usize::try_from(chrono::Utc::now().timestamp()).unwrap_or(0),
            exp: usize::try_from((chrono::Utc::now() + chrono::Duration::hours(1)).timestamp())
                .unwrap_or(0),
        };

        let header = Header::new(Algorithm::RS256);

        // Re-encode private key to unencrypted PEM so jsonwebtoken can parse it
        let unencrypted_pem = private_key
            .to_pkcs8_pem(rsa::pkcs8::LineEnding::LF)
            .map_err(|e| {
                ConnectorError::ConnectionFailed(format!(
                    "Failed to re-encode private key to PEM: {e}"
                ))
            })?;
        let encoding_key = EncodingKey::from_rsa_pem(unencrypted_pem.as_bytes()).map_err(|e| {
            ConnectorError::ConnectionFailed(format!("Failed to create encoding key: {e}"))
        })?;

        let token = encode(&header, &claims, &encoding_key)
            .map_err(|e| ConnectorError::ConnectionFailed(format!("Failed to sign JWT: {e}")))?;
        Ok(token)
    }
}

fn map_cell(
    value: &Value,
    field_type: &str,
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
        "TEXT" | "VARCHAR" => Ok(ColumnValue::Text(val_str.to_string())),
        "FIXED" | "NUMBER" => {
            if let Ok(i) = val_str.parse::<i64>() {
                Ok(ColumnValue::Int64(i))
            } else if let Ok(f) = val_str.parse::<f64>() {
                Ok(ColumnValue::Float64(f))
            } else {
                Err(ConnectorError::QueryFailed(format!(
                    "Failed to parse NUMBER/FIXED for column '{col_name}': {val_str}"
                )))
            }
        }
        "REAL" | "FLOAT" => {
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
            // Snowflake returns binary as hex strings
            Ok(ColumnValue::Bytes(val_str.as_bytes().to_vec()))
        }
        _ => {
            // Default to text if type is unknown
            Ok(ColumnValue::Text(val_str.to_string()))
        }
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

        let stream = stream! {
            let url = format!("https://{account}.snowflakecomputing.com/api/v2/statements");
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

            if !status.is_success() {
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

            // If async (202), poll until completion
            if is_async {
                let Some(sh) = json.get("statementHandle").and_then(Value::as_str) else {
                    yield Err(ConnectorError::QueryFailed("No statementHandle found in 202 response".to_string()));
                    return;
                };
                let statement_handle = sh.to_string();

                loop {
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

                    let poll_url = format!("https://{account}.snowflakecomputing.com/api/v2/statements/{statement_handle}");
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

            // Extract metadata and data
            let Some(metadata) = json.get("resultSetMetaData").and_then(|m| m.get("rowType")).and_then(Value::as_array) else {
                // It's possible the query returned no results, e.g. an INSERT.
                return;
            };

            let mut col_names = Vec::new();
            let mut col_types = Vec::new();

            for col in metadata {
                let name = col.get("name").and_then(Value::as_str).unwrap_or("unknown").to_string();
                let c_type = col.get("type").and_then(Value::as_str).unwrap_or("TEXT").to_string();
                col_names.push(name);
                col_types.push(c_type);
            }

            // Handle multiple partitions if they exist, or just inline data
            // Snowflake sometimes paginates the data via partitionInfo. For simplicity we map the inline "data".
            // A production-grade implementation would fetch all partitions if "partitionInfo" is present.
            if let Some(data) = json.get("data").and_then(Value::as_array) {
                for row_val in data {
                    if let Some(row_arr) = row_val.as_array() {
                        let mut row = Row::new();
                        for (i, cell) in row_arr.iter().enumerate() {
                            let col_name = col_names.get(i).map_or("unknown", String::as_str);
                            let col_type = col_types.get(i).map_or("TEXT", String::as_str);
                            match map_cell(cell, col_type, col_name) {
                                Ok(val) => row.push(val),
                                Err(e) => {
                                    yield Err(e);
                                    return;
                                }
                            }
                        }
                        yield Ok(row);
                    }
                }
            }
        };

        Ok(Box::pin(stream))
    }
}
