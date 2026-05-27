use may_secrets::{DwhSecret, EnvSecretsProvider, SecretsError, SecretsProvider};
use serial_test::serial;
use std::env;

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
        DwhSecret::UsernamePassword {
            host,
            port,
            database,
            username,
            password,
        } => {
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
