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
    pub token_ttl_secs: u64,
}

impl Default for VaultConfig {
    fn default() -> Self {
        Self {
            address: "http://127.0.0.1:8200".to_string(),
            mount: "secret".to_string(),
            auth: VaultAuth::Token(std::env::var("VAULT_TOKEN").unwrap_or_default()),
            cache_ttl_secs: 300,
            token_ttl_secs: 3600,
        }
    }
}

impl VaultConfig {
    #[must_use] 
    pub fn builder() -> VaultConfigBuilder {
        VaultConfigBuilder::default()
    }
}

#[derive(Default)]
pub struct VaultConfigBuilder {
    address: Option<String>,
    mount: Option<String>,
    auth: Option<VaultAuth>,
    cache_ttl_secs: Option<u64>,
    token_ttl_secs: Option<u64>,
}

impl VaultConfigBuilder {
    #[must_use]
    pub fn address(mut self, address: impl Into<String>) -> Self {
        self.address = Some(address.into());
        self
    }

    #[must_use]
    pub fn mount(mut self, mount: impl Into<String>) -> Self {
        self.mount = Some(mount.into());
        self
    }

    #[must_use]
    pub fn auth(mut self, auth: VaultAuth) -> Self {
        self.auth = Some(auth);
        self
    }

    #[must_use]
    pub fn token(mut self, token: impl Into<String>) -> Self {
        self.auth = Some(VaultAuth::Token(token.into()));
        self
    }

    #[must_use]
    pub fn approle(mut self, role_id: impl Into<String>, secret_id: impl Into<String>) -> Self {
        self.auth = Some(VaultAuth::AppRole {
            role_id: role_id.into(),
            secret_id: secret_id.into(),
        });
        self
    }

    #[must_use]
    pub fn cache_ttl_secs(mut self, secs: u64) -> Self {
        self.cache_ttl_secs = Some(secs);
        self
    }

    #[must_use]
    pub fn token_ttl_secs(mut self, secs: u64) -> Self {
        self.token_ttl_secs = Some(secs);
        self
    }

    /// Builds the `VaultConfig`.
    ///
    /// # Errors
    /// Returns `SecretsError::InvalidConfig` if `address` or `auth` are missing.
    pub fn build(self) -> Result<VaultConfig, SecretsError> {
        let address = self
            .address
            .ok_or_else(|| SecretsError::InvalidConfig("Missing address".to_string()))?;
        let auth = self
            .auth
            .ok_or_else(|| SecretsError::InvalidConfig("Missing auth".to_string()))?;

        Ok(VaultConfig {
            address,
            mount: self.mount.unwrap_or_else(|| "secret".to_string()),
            auth,
            cache_ttl_secs: self.cache_ttl_secs.unwrap_or(300),
            token_ttl_secs: self.token_ttl_secs.unwrap_or(3600),
        })
    }
}

pub struct VaultSecretsProvider {
    config: VaultConfig,
    client: Client,
    cache: RwLock<HashMap<String, (DwhSecret, Instant)>>,
    approle_token: RwLock<Option<(String, Instant)>>,
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
                    if let Some((token, ts)) = &*read_guard
                        && ts.elapsed() < Duration::from_secs(self.config.token_ttl_secs) {
                            return Ok(token.clone());
                        }
                }

                let mut write_guard = self.approle_token.write().await;
                if let Some((token, ts)) = &*write_guard
                    && ts.elapsed() < Duration::from_secs(self.config.token_ttl_secs) {
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
                        .unwrap_or_else(|e| format!("Failed to read error body: {e}"));
                    return Err(SecretsError::VaultError {
                        status: status.as_u16(),
                        message,
                    });
                }

                let login_resp: AppRoleLoginResponse = response.json().await.map_err(|e| {
                    SecretsError::InvalidConfig(format!("Failed to parse AppRole response: {e}"))
                })?;

                let token = login_resp.auth.client_token;
                *write_guard = Some((token.clone(), Instant::now()));
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

            if status == StatusCode::FORBIDDEN
                && matches!(self.config.auth, VaultAuth::AppRole { .. })
            {
                let mut guard = self.approle_token.write().await;
                *guard = None;
            }

            let message = response
                .text()
                .await
                .unwrap_or_else(|e| format!("Failed to read error body: {e}"));

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

        let secret = crate::secret_data::build_dwh_secret(secret_kind, secret_data)?;

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

impl std::fmt::Debug for VaultSecretsProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VaultSecretsProvider")
            .field("config", &self.config)
            .field("approle_token", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}
