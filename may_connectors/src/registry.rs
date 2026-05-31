use crate::traits::WarehouseConnector;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Default, Clone)]
pub struct ConnectorRegistry {
    connectors: HashMap<String, Arc<dyn WarehouseConnector + Send + Sync>>,
}

impl ConnectorRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, name: &str, connector: Arc<dyn WarehouseConnector + Send + Sync>) {
        self.connectors.insert(name.to_string(), connector);
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<Arc<dyn WarehouseConnector + Send + Sync>> {
        self.connectors.get(name).cloned()
    }
}
