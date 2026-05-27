use crate::error::SecretsError;
use crate::models::DwhSecret;
use crate::provider::SecretsProvider;
use async_trait::async_trait;
use std::env;

#[derive(Default)]
pub struct EnvSecretsProvider;

impl EnvSecretsProvider {
    #[must_use]
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
        let type_var = format!("{prefix}_TYPE");

        let secret_type = Self::require_env(&type_var)?;
        let secret_kind: crate::secret_kind::SecretKind = secret_type.parse()?;

        match secret_kind {
            crate::secret_kind::SecretKind::UsernamePassword => {
                let host = Self::require_env(&format!("{prefix}_HOST"))?;
                let port_str = Self::require_env(&format!("{prefix}_PORT"))?;
                let port: u16 = port_str.parse().map_err(|_| {
                    SecretsError::InvalidConfig(format!("Invalid port value: {port_str}"))
                })?;
                let database = Self::require_env(&format!("{prefix}_DATABASE"))?;
                let username = Self::require_env(&format!("{prefix}_USERNAME"))?;
                let password = Self::require_env(&format!("{prefix}_PASSWORD"))?;

                Ok(DwhSecret::UsernamePassword {
                    host,
                    port,
                    database,
                    username,
                    password,
                })
            }
            crate::secret_kind::SecretKind::ServiceAccountKey => {
                let json = Self::require_env(&format!("{prefix}_JSON"))?;
                Ok(DwhSecret::ServiceAccountKey { json })
            }
            crate::secret_kind::SecretKind::KeyPair => {
                let account = Self::require_env(&format!("{prefix}_ACCOUNT"))?;
                let username = Self::require_env(&format!("{prefix}_USERNAME"))?;
                let private_key = Self::require_env(&format!("{prefix}_PRIVATE_KEY"))?;
                let passphrase = env::var(format!("{prefix}_PASSPHRASE")).ok();

                Ok(DwhSecret::KeyPair {
                    account,
                    username,
                    private_key,
                    passphrase,
                })
            }
        }
    }
}
