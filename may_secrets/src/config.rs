use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError};

fn default_vault_mount() -> String {
    "secret".to_string()
}

fn default_cache_ttl_secs() -> u64 {
    300
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SecretsMode {
    Managed,
    Byov,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum VaultAuthMethod {
    Token,
    AppRole,
}

#[derive(Debug, Clone, Deserialize, Serialize, Validate, JsonSchema)]
#[validate(schema(function = "validate_config"))]
pub struct MaySecretsConfig {
    pub mode: SecretsMode,

    #[validate(url)]
    pub vault_address: Option<String>,

    #[serde(default = "default_vault_mount")]
    pub vault_mount: String,

    pub auth_method: VaultAuthMethod,

    pub token: Option<String>,

    pub role_id: Option<String>,

    pub secret_id: Option<String>,

    #[validate(range(min = 30))]
    #[serde(default = "default_cache_ttl_secs")]
    pub cache_ttl_secs: u64,
}

fn validate_config(config: &MaySecretsConfig) -> Result<(), ValidationError> {
    if config.mode == SecretsMode::Byov && config.vault_address.is_none() {
        return Err(ValidationError::new("vault_address_required_for_byov"));
    }

    match config.auth_method {
        VaultAuthMethod::Token => {
            if config.token.is_none() {
                return Err(ValidationError::new("token_required_for_token_auth"));
            }
        }
        VaultAuthMethod::AppRole => {
            if config.role_id.is_none() {
                return Err(ValidationError::new("role_id_required_for_approle_auth"));
            }
            if config.secret_id.is_none() {
                return Err(ValidationError::new("secret_id_required_for_approle_auth"));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_managed_config() {
        let yaml = r#"
        mode: managed
        auth_method: token
        token: "test-token"
        "#;
        let config: MaySecretsConfig = serde_norway::from_str(yaml).unwrap();
        assert!(config.validate().is_ok());
        assert_eq!(config.mode, SecretsMode::Managed);
        assert_eq!(config.auth_method, VaultAuthMethod::Token);
        assert_eq!(config.vault_mount, "secret");
        assert_eq!(config.cache_ttl_secs, 300);
    }

    #[test]
    fn test_valid_byov_config() {
        let yaml = r#"
        mode: byov
        vault_address: "https://vault.example.com"
        vault_mount: "my_secrets"
        auth_method: approle
        role_id: "my-role"
        secret_id: "my-secret"
        cache_ttl_secs: 600
        "#;
        let config: MaySecretsConfig = serde_norway::from_str(yaml).unwrap();
        assert!(config.validate().is_ok());
        assert_eq!(config.mode, SecretsMode::Byov);
        assert_eq!(
            config.vault_address,
            Some("https://vault.example.com".to_string())
        );
        assert_eq!(config.auth_method, VaultAuthMethod::AppRole);
        assert_eq!(config.vault_mount, "my_secrets");
        assert_eq!(config.cache_ttl_secs, 600);
    }

    #[test]
    fn test_invalid_byov_missing_address() {
        let yaml = r#"
        mode: byov
        auth_method: token
        token: "test"
        "#;
        let config: MaySecretsConfig = serde_norway::from_str(yaml).unwrap();
        let err = config.validate().unwrap_err();
        assert!(err.field_errors().contains_key("__all__"));
    }

    #[test]
    fn test_invalid_url() {
        let yaml = r#"
        mode: byov
        vault_address: "not-a-url"
        auth_method: token
        token: "test"
        "#;
        let config: MaySecretsConfig = serde_norway::from_str(yaml).unwrap();
        let err = config.validate().unwrap_err();
        assert!(err.field_errors().contains_key("vault_address"));
    }

    #[test]
    fn test_invalid_approle_missing_role_id() {
        let yaml = r#"
        mode: byov
        vault_address: "https://vault.example.com"
        auth_method: approle
        secret_id: "my-secret"
        "#;
        let config: MaySecretsConfig = serde_norway::from_str(yaml).unwrap();
        let err = config.validate().unwrap_err();
        assert!(err.field_errors().contains_key("__all__"));

        let field_errors = err.field_errors();
        let all_errors = field_errors.get("__all__").unwrap();
        assert!(
            all_errors
                .iter()
                .any(|e| e.code == "role_id_required_for_approle_auth")
        );
    }
}
