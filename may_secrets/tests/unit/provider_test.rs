use async_trait::async_trait;
use may_secrets::{DwhSecret, SecretsError, SecretsProvider};
use std::sync::Arc;

struct MockProvider;

#[async_trait]
impl SecretsProvider for MockProvider {
    async fn get_secret(&self, _name: &str) -> Result<DwhSecret, SecretsError> {
        Err(SecretsError::SecretNotFound("mock".to_string()))
    }
}

#[test]
fn test_secrets_provider_object_safety() {
    // This test ensures the trait is object-safe, meaning it can be boxed.
    let _provider: Arc<dyn SecretsProvider> = Arc::new(MockProvider);
}
