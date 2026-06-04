use crate::error::SecretsError;
use crate::models::dwh::DwhSecret;
use async_trait::async_trait;

#[async_trait]
pub trait SecretsProvider: Send + Sync {
    async fn get_secret(&self, name: &str) -> Result<DwhSecret, SecretsError>;
}
