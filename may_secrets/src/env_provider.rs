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

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn set_env(k: &str, v: &str) {
        unsafe { env::set_var(k, v) };
    }

    fn remove_env(k: &str) {
        unsafe { env::remove_var(k) };
    }

    #[tokio::test]
    #[serial]
    async fn test_env_provider_username_password() -> Result<(), Box<dyn std::error::Error>> {
        set_env("MAY_SECRET_MY_TEST_TYPE", "username_password");
        set_env("MAY_SECRET_MY_TEST_HOST", "localhost");
        set_env("MAY_SECRET_MY_TEST_PORT", "5432");
        set_env("MAY_SECRET_MY_TEST_DATABASE", "mydb");
        set_env("MAY_SECRET_MY_TEST_USERNAME", "admin");
        set_env("MAY_SECRET_MY_TEST_PASSWORD", "secret123");

        let provider = EnvSecretsProvider::new();
        let secret = provider.get_secret("my-test").await?;

        match secret {
            DwhSecret::UsernamePassword { host, port, database, username, password } => {
                assert_eq!(host, "localhost");
                assert_eq!(port, 5432);
                assert_eq!(database, "mydb");
                assert_eq!(username, "admin");
                assert_eq!(password, "secret123");
            }
            _ => return Err("Expected UsernamePassword variant".into()),
        }

        remove_env("MAY_SECRET_MY_TEST_TYPE");
        remove_env("MAY_SECRET_MY_TEST_HOST");
        remove_env("MAY_SECRET_MY_TEST_PORT");
        remove_env("MAY_SECRET_MY_TEST_DATABASE");
        remove_env("MAY_SECRET_MY_TEST_USERNAME");
        remove_env("MAY_SECRET_MY_TEST_PASSWORD");

        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn test_env_provider_missing_variable() {
        set_env("MAY_SECRET_MISSING_VAR_TYPE", "service_account_key");
        remove_env("MAY_SECRET_MISSING_VAR_JSON");

        let provider = EnvSecretsProvider::new();
        let result = provider.get_secret("missing-var").await;

        match result {
            Err(SecretsError::MissingVariable { var_name }) => {
                assert_eq!(var_name, "MAY_SECRET_MISSING_VAR_JSON");
            }
            _ => panic!("Expected MissingVariable error"),
        }

        remove_env("MAY_SECRET_MISSING_VAR_TYPE");
    }

    #[tokio::test]
    #[serial]
    async fn test_env_provider_unknown_type() {
        set_env("MAY_SECRET_UNKNOWN_TYPE_TYPE", "invalid_type_here");

        let provider = EnvSecretsProvider::new();
        let result = provider.get_secret("unknown-type").await;

        match result {
            Err(SecretsError::InvalidConfig(msg)) => {
                assert!(msg.contains("invalid_type_here"));
            }
            _ => panic!("Expected InvalidConfig error"),
        }

        remove_env("MAY_SECRET_UNKNOWN_TYPE_TYPE");
    }
}
