use crate::error::SecretsError;
use crate::models::DwhSecret;
use async_trait::async_trait;

#[async_trait]
pub trait SecretsProvider: Send + Sync {
    async fn get_secret(&self, name: &str) -> Result<DwhSecret, SecretsError>;
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let provider: Arc<dyn SecretsProvider> = Arc::new(MockProvider);

        // Just verify it compiles and exists.
        assert!(std::ptr::addr_of!(provider) != std::ptr::null());
    }
}
