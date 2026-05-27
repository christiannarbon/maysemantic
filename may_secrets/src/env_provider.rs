use crate::error::SecretsError;
use crate::models::DwhSecret;
use crate::provider::SecretsProvider;
use async_trait::async_trait;
use std::env;

#[derive(Default)]
pub struct EnvSecretsProvider;

impl EnvSecretsProvider {
    pub fn new() -> Self {
        Self
    }

    fn require_env(var_name: &str) -> Result<String, SecretsError> {
        env::var(var_name).map_err(|_| SecretsError::MissingVariable {
            var_name: var_name.to_string(),
        })
    }
}

#[async_trait]
impl SecretsProvider for EnvSecretsProvider {
    async fn get_secret(&self, name: &str) -> Result<DwhSecret, SecretsError> {
        let prefix = format!("MAY_SECRET_{}", name.to_uppercase().replace('-', "_"));
        let type_var = format!("{}_TYPE", prefix);

        let secret_type = Self::require_env(&type_var)?;

        match secret_type.as_str() {
            "username_password" => {
                let host = Self::require_env(&format!("{}_HOST", prefix))?;
                let port_str = Self::require_env(&format!("{}_PORT", prefix))?;
                let port: u16 = port_str.parse().map_err(|_| {
                    SecretsError::InvalidConfig(format!("Invalid port value: {}", port_str))
                })?;
                let database = Self::require_env(&format!("{}_DATABASE", prefix))?;
                let username = Self::require_env(&format!("{}_USERNAME", prefix))?;
                let password = Self::require_env(&format!("{}_PASSWORD", prefix))?;

                Ok(DwhSecret::UsernamePassword {
                    host,
                    port,
                    database,
                    username,
                    password,
                })
            }
            "service_account_key" => {
                let json = Self::require_env(&format!("{}_JSON", prefix))?;
                Ok(DwhSecret::ServiceAccountKey { json })
            }
            "key_pair" => {
                let account = Self::require_env(&format!("{}_ACCOUNT", prefix))?;
                let username = Self::require_env(&format!("{}_USERNAME", prefix))?;
                let private_key = Self::require_env(&format!("{}_PRIVATE_KEY", prefix))?;
                let passphrase = env::var(format!("{}_PASSPHRASE", prefix)).ok();

                Ok(DwhSecret::KeyPair {
                    account,
                    username,
                    private_key,
                    passphrase,
                })
            }
            _ => Err(SecretsError::InvalidConfig(format!(
                "Unknown secret type: {}",
                secret_type
            ))),
        }
    }
}
