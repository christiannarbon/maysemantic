use crate::error::ConnectorError;
use crate::models::QueryResult;
use async_trait::async_trait;

#[async_trait]
pub trait WarehouseConnector: Send + Sync {
    async fn execute(&self, sql: &str) -> Result<QueryResult, ConnectorError>;
}
