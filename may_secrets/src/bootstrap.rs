use crate::config::{MaySecretsConfig, SecretsMode, VaultAuthMethod};
use crate::env_provider::EnvSecretsProvider;
use crate::error::SecretsError;
use crate::provider::SecretsProvider;
use crate::vault_provider::{VaultAuth, VaultConfig, VaultSecretsProvider};
use std::path::Path;
use std::sync::Arc;
use tokio::fs;

/// Bootstraps the secrets provider based on the provided configuration file path.
///
/// If the file does not exist, it falls back to the `EnvSecretsProvider`.
/// Otherwise, it parses the YAML configuration and constructs a `VaultSecretsProvider`.
///
/// # Errors
/// Returns `SecretsError::InvalidConfig` if the file exists but cannot be read, parsed,
/// or if it contains an invalid configuration.
pub async fn bootstrap_secrets_provider(
    config_path: &Path,
) -> Result<Arc<dyn SecretsProvider>, SecretsError> {
    let config_content = match fs::read_to_string(config_path).await {
        Ok(content) => content,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            tracing::warn!(
                "Secrets configuration file not found at {}. Falling back to environment variables.",
                config_path.display()
            );
            return Ok(Arc::new(EnvSecretsProvider));
        }
        Err(e) => {
            return Err(SecretsError::InvalidConfig(format!(
                "Failed to read secrets config: {e}"
            )));
        }
    };

    let config: MaySecretsConfig = serde_norway::from_str(&config_content)
        .map_err(|e| SecretsError::InvalidConfig(format!("Failed to parse secrets config: {e}")))?;

    let address = match config.mode {
        SecretsMode::Managed => "http://may-vault:8200".to_string(),
        SecretsMode::Byov => config.vault_address.ok_or_else(|| {
            SecretsError::InvalidConfig("Missing vault_address for Byov mode".to_string())
        })?,
    };

    let auth = match config.auth_method {
        VaultAuthMethod::Token => VaultAuth::Token(
            config
                .token
                .ok_or_else(|| SecretsError::InvalidConfig("Missing token".to_string()))?,
        ),
        VaultAuthMethod::AppRole => VaultAuth::AppRole {
            role_id: config
                .role_id
                .ok_or_else(|| SecretsError::InvalidConfig("Missing role_id".to_string()))?,
            secret_id: config
                .secret_id
                .ok_or_else(|| SecretsError::InvalidConfig("Missing secret_id".to_string()))?,
        },
    };

    let vault_config = VaultConfig::builder()
        .address(address)
        .mount(config.vault_mount)
        .auth(auth)
        .cache_ttl_secs(config.cache_ttl_secs)
        .build()?;

    let provider = VaultSecretsProvider::new(vault_config)?;
    Ok(Arc::new(provider))
}
