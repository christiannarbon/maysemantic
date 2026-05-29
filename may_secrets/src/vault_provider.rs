use crate::error::SecretsError;
use crate::models::DwhSecret;
use crate::provider::SecretsProvider;
use crate::secret_kind::SecretKind;
use async_trait::async_trait;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub enum VaultAuth {
    Token(String),
    AppRole { role_id: String, secret_id: String },
}

#[derive(Debug, Clone)]
pub struct VaultConfig {
    pub address: String,
    pub mount: String,
    pub auth: VaultAuth,
    pub cache_ttl_secs: u64,
}

impl Default for VaultConfig {
    fn default() -> Self {
        Self {
            address: "http://127.0.0.1:8200".to_string(),
            mount: "secret".to_string(),
            auth: VaultAuth::Token(String::new()),
            cache_ttl_secs: 300,
        }
    }
}

pub struct VaultSecretsProvider {
    config: VaultConfig,
    client: Client,
    cache: RwLock<HashMap<String, (DwhSecret, Instant)>>,
    approle_token: RwLock<Option<String>>,
}

impl VaultSecretsProvider {
    /// Creates a new `VaultSecretsProvider`.
    ///
    /// # Errors
    /// Returns `SecretsError::InvalidConfig` if the underlying HTTP client cannot be built.
    pub fn new(config: VaultConfig) -> Result<Self, SecretsError> {
        let client = Client::builder().build().map_err(|e| {
            SecretsError::InvalidConfig(format!("Failed to build HTTP client: {e}"))
        })?;
        Ok(Self {
            config,
            client,
            cache: RwLock::new(HashMap::new()),
            approle_token: RwLock::new(None),
        })
    }

    async fn get_token(&self) -> Result<String, SecretsError> {
        match &self.config.auth {
            VaultAuth::Token(t) => Ok(t.clone()),
            VaultAuth::AppRole { role_id, secret_id } => {
                {
                    let read_guard = self.approle_token.read().await;
                    if let Some(token) = &*read_guard {
                        return Ok(token.clone());
                    }
                }

                let mut write_guard = self.approle_token.write().await;
                if let Some(token) = &*write_guard {
                    return Ok(token.clone());
                }

                let url = format!("{}/v1/auth/approle/login", self.config.address);
                let req_body = AppRoleLoginRequest { role_id, secret_id };

                let response = self
                    .client
                    .post(&url)
                    .json(&req_body)
                    .send()
                    .await
                    .map_err(|e| {
                        SecretsError::InvalidConfig(format!(
                            "Failed to authenticate with AppRole: {e}"
                        ))
                    })?;

                let status = response.status();
                if !status.is_success() {
                    let message = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Unknown Vault error".to_string());
                    return Err(SecretsError::VaultError {
                        status: status.as_u16(),
                        message,
                    });
                }

                let login_resp: AppRoleLoginResponse = response.json().await.map_err(|e| {
                    SecretsError::InvalidConfig(format!("Failed to parse AppRole response: {e}"))
                })?;

                let token = login_resp.auth.client_token;
                *write_guard = Some(token.clone());
                Ok(token)
            }
        }
    }
}

#[derive(Deserialize)]
struct KvV2Response {
    data: KvV2Data,
}

#[derive(Deserialize)]
struct KvV2Data {
    data: serde_json::Value,
}

#[async_trait]
impl SecretsProvider for VaultSecretsProvider {
    #[allow(
        clippy::too_many_lines,
        reason = "JSON extraction from Vault requires verbose match blocks"
    )]
    async fn get_secret(&self, name: &str) -> Result<DwhSecret, SecretsError> {
        {
            let cache_read = self.cache.read().await;
            if let Some((secret, timestamp)) = cache_read.get(name)
                && timestamp.elapsed() < Duration::from_secs(self.config.cache_ttl_secs)
            {
                return Ok(secret.clone());
            }
        }

        let token = self.get_token().await?;

        // Vault KV-v2 read path: {address}/v1/{mount}/data/{name}
        let url = format!(
            "{}/v1/{}/data/{}",
            self.config.address, self.config.mount, name
        );

        let response = self
            .client
            .get(&url)
            .header("X-Vault-Token", token)
            .send()
            .await
            .map_err(|e| {
                SecretsError::InvalidConfig(format!("Failed to send request to Vault: {e}"))
            })?;

        let status = response.status();
        if !status.is_success() {
            if status == StatusCode::NOT_FOUND {
                return Err(SecretsError::SecretNotFound(name.to_string()));
            }

            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown Vault error".to_string());

            return Err(SecretsError::VaultError {
                status: status.as_u16(),
                message,
            });
        }

        let kv_response: KvV2Response = response.json().await.map_err(|e| {
            SecretsError::InvalidConfig(format!("Failed to parse Vault response: {e}"))
        })?;

        let secret_data = kv_response.data.data;

        // Ensure it's an object
        let map = secret_data.as_object().ok_or_else(|| {
            SecretsError::InvalidConfig("Vault data field is not a JSON object".to_string())
        })?;

        // We expect a "type" field inside data to map to SecretKind
        let type_val = map.get("type").and_then(|v| v.as_str()).ok_or_else(|| {
            SecretsError::InvalidConfig("Missing 'type' field in Vault secret".to_string())
        })?;

        let secret_kind: SecretKind = type_val.parse()?;

        let extract_str = |key: &str| -> Result<String, SecretsError> {
            map.get(key)
                .and_then(|v| v.as_str())
                .map(ToString::to_string)
                .ok_or_else(|| {
                    SecretsError::InvalidConfig(format!("Missing or invalid field '{key}'"))
                })
        };

        let secret = match secret_kind {
            SecretKind::UsernamePassword => {
                let host = extract_str("host")?;
                let port_val = map
                    .get("port")
                    .ok_or_else(|| SecretsError::InvalidConfig("Missing 'port'".to_string()))?;

                let port = if let Some(n) = port_val.as_u64() {
                    u16::try_from(n).map_err(|_| {
                        SecretsError::InvalidConfig("Port value too large".to_string())
                    })?
                } else if let Some(s) = port_val.as_str() {
                    s.parse().map_err(|_| {
                        SecretsError::InvalidConfig("Invalid port string".to_string())
                    })?
                } else {
                    return Err(SecretsError::InvalidConfig(
                        "Invalid port format".to_string(),
                    ));
                };

                let database = extract_str("database")?;
                let username = extract_str("username")?;
                let password = extract_str("password")?;

                DwhSecret::UsernamePassword {
                    host,
                    port,
                    database,
                    username,
                    password,
                }
            }
            SecretKind::ServiceAccountKey => {
                let json = extract_str("json")?;
                DwhSecret::ServiceAccountKey { json }
            }
            SecretKind::KeyPair => {
                let account = extract_str("account")?;
                let username = extract_str("username")?;
                let private_key = extract_str("private_key")?;
                let passphrase = map
                    .get("passphrase")
                    .and_then(|v| v.as_str())
                    .map(ToString::to_string);

                DwhSecret::KeyPair {
                    account,
                    username,
                    private_key,
                    passphrase,
                }
            }
        };

        let mut cache_write = self.cache.write().await;
        cache_write.insert(name.to_string(), (secret.clone(), Instant::now()));

        Ok(secret)
    }
}

#[derive(Serialize)]
struct AppRoleLoginRequest<'a> {
    role_id: &'a str,
    secret_id: &'a str,
}

#[derive(Deserialize)]
struct AppRoleLoginResponse {
    auth: AppRoleAuth,
}

#[derive(Deserialize)]
struct AppRoleAuth {
    client_token: String,
}
