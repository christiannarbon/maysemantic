use may_secrets::bootstrap::bootstrap_secrets_provider;
use std::io::Write;
use tempfile::NamedTempFile;

#[tokio::test]
async fn test_bootstrap_missing_file_fallback() {
    let temp_dir = tempfile::tempdir().expect("tempdir should be created");
    let missing_path = temp_dir.path().join("does-not-exist.yaml");

    // Should return EnvSecretsProvider gracefully without error
    let provider = bootstrap_secrets_provider(&missing_path).await;
    assert!(provider.is_ok(), "Failed to fallback to EnvSecretsProvider");
}

#[tokio::test]
async fn test_bootstrap_managed() {
    let mut file = NamedTempFile::new().expect("NamedTempFile should be created");
    writeln!(
        file,
        "mode: managed\nauth_method: token\ntoken: 'managed-token'"
    )
    .expect("writing YAML fixture should succeed");

    let path = file.path();
    let provider = bootstrap_secrets_provider(path).await;
    assert!(provider.is_ok(), "Failed to bootstrap Managed provider");
}

#[tokio::test]
async fn test_bootstrap_byov() {
    let mut file = NamedTempFile::new().expect("NamedTempFile should be created");
    writeln!(
        file,
        "mode: byov\nvault_address: 'http://localhost:8200'\nauth_method: approle\nrole_id: 'r1'\nsecret_id: 's1'"
    )
    .expect("writing YAML fixture should succeed");

    let path = file.path();
    let provider = bootstrap_secrets_provider(path).await;
    assert!(provider.is_ok(), "Failed to bootstrap Byov provider");
}

#[tokio::test]
async fn test_bootstrap_invalid_yaml() {
    let mut file = NamedTempFile::new().expect("NamedTempFile should be created");
    writeln!(file, ": invalid: {{yaml").expect("writing YAML fixture should succeed");

    let path = file.path();
    let provider = bootstrap_secrets_provider(path).await;
    assert!(
        matches!(
            provider,
            Err(may_secrets::error::SecretsError::InvalidConfig(_))
        ),
        "Expected InvalidConfig error for broken YAML"
    );
}

#[tokio::test]
async fn test_bootstrap_invalid_config_validation() {
    let mut file = NamedTempFile::new().expect("NamedTempFile should be created");
    // Missing vault_address for byov mode
    writeln!(file, "mode: byov\nauth_method: token\ntoken: 'test'")
        .expect("writing YAML fixture should succeed");

    let path = file.path();
    let provider = bootstrap_secrets_provider(path).await;
    assert!(
        matches!(
            provider,
            Err(may_secrets::error::SecretsError::InvalidConfig(_))
        ),
        "Expected InvalidConfig error for invalid validation"
    );
}
