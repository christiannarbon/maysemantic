use may_secrets::bootstrap::bootstrap_secrets_provider;
use std::io::Write;
use tempfile::NamedTempFile;

#[tokio::test]
async fn test_bootstrap_missing_file_fallback() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let missing_path = temp_dir.path().join("does-not-exist.yaml");

    // Should return EnvSecretsProvider gracefully without error
    let provider = bootstrap_secrets_provider(&missing_path).await;
    assert!(provider.is_ok(), "Failed to fallback to EnvSecretsProvider");
}

#[tokio::test]
async fn test_bootstrap_managed() {
    let mut file = NamedTempFile::new().expect("Failed to create temp file");
    writeln!(
        file,
        "mode: managed\nauth_method: token\ntoken: 'managed-token'"
    )
    .expect("Failed to write to temp file");

    let path = file.path();
    let provider = bootstrap_secrets_provider(path).await;
    assert!(provider.is_ok(), "Failed to bootstrap Managed provider");
}

#[tokio::test]
async fn test_bootstrap_byov() {
    let mut file = NamedTempFile::new().expect("Failed to create temp file");
    writeln!(
        file,
        "mode: byov\nvault_address: 'http://localhost:8200'\nauth_method: approle\nrole_id: 'r1'\nsecret_id: 's1'"
    )
    .expect("Failed to write to temp file");

    let path = file.path();
    let provider = bootstrap_secrets_provider(path).await;
    assert!(provider.is_ok(), "Failed to bootstrap Byov provider");
}
