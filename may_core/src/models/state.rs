//! State Manager for the Semantic Layer.
//!
//! Manages the lifecycle of `SemanticModel` objects parsed from YAML definitions,
//! providing thread-safe read/write access via `Arc<RwLock<SemanticState>>`.

use crate::models::SemanticModel;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use thiserror::Error;
use validator::Validate;

#[derive(Error, Debug)]
pub enum StateError {
    #[error("Failed to parse YAML: {0}")]
    YamlError(#[from] serde_norway::Error),
    #[error("Validation failed: {0}")]
    ValidationError(#[from] validator::ValidationErrors),
    #[error("Failed to acquire state lock")]
    LockError,
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
}

pub type CachedGraph = Arc<(crate::graph::engine::SemanticGraph, HashMap<String, petgraph::graph::NodeIndex>)>;

pub struct SemanticState {
    pub models: HashMap<String, SemanticModel>,
    graph_cache: std::sync::OnceLock<CachedGraph>,
}

impl Default for SemanticState {
    fn default() -> Self {
        Self {
            models: HashMap::new(),
            graph_cache: std::sync::OnceLock::new(),
        }
    }
}

impl Clone for SemanticState {
    fn clone(&self) -> Self {
        Self {
            models: self.models.clone(),
            // When cloning state, start with a fresh uninitialized OnceLock cache cell
            // so that graph is rebuilt for the new state revision on first access.
            graph_cache: std::sync::OnceLock::new(),
        }
    }
}

impl SemanticState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Cheaply retrieves the cached graph, lazily building it once on first access.
    pub fn get_graph(&self) -> Result<CachedGraph, crate::graph::GraphError> {
        if let Some(cached) = self.graph_cache.get() {
            return Ok(cached.clone());
        }
        let (graph, node_indices) = crate::graph::build_semantic_graph(self)?;
        let val = Arc::new((graph, node_indices));
        let _ = self.graph_cache.set(val.clone());
        Ok(val)
    }
}

pub struct StateMgr {
    state: Arc<RwLock<Arc<SemanticState>>>,
}

impl Default for StateMgr {
    fn default() -> Self {
        Self {
            state: Arc::new(RwLock::new(Arc::new(SemanticState::new()))),
        }
    }
}

/// Named container for state statistics, replacing the opaque `(usize, usize, usize)` tuple.
///
/// Using a struct instead of a positional tuple prevents silent misassignment bugs at call sites.
pub struct StateStats {
    pub model_count: usize,
    pub entity_count: usize,
    pub metric_count: usize,
}

impl StateMgr {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_state(&self) -> Arc<RwLock<Arc<SemanticState>>> {
        self.state.clone()
    }

    /// Cheap O(1) snapshot of the current state — clones the inner Arc, not the data.
    pub fn snapshot(&self) -> Result<Arc<SemanticState>, StateError> {
        let guard = self.state.read().map_err(|_| StateError::LockError)?;
        Ok(guard.clone())
    }

    pub fn load_from_yaml(&self, yaml_content: &str) -> Result<(), StateError> {
        let model: SemanticModel = serde_norway::from_str(yaml_content)?;
        model.validate()?;

        let mut guard = self.state.write().map_err(|_| StateError::LockError)?;
        let mut next = (**guard).clone();
        next.models.insert(model.name.clone(), model);
        *guard = Arc::new(next);

        Ok(())
    }

    pub async fn load_dir(&self, path: impl AsRef<std::path::Path>) -> Result<(), StateError> {
        use tokio::fs;
        use validator::Validate;
        let path = path.as_ref();

        let mut entries = Vec::new();
        if path.is_file() {
            entries.push(path.to_path_buf());
        } else {
            let mut dir = fs::read_dir(path).await?;
            while let Some(entry) = dir.next_entry().await? {
                let p = entry.path();
                if p.extension()
                    .is_some_and(|ext| ext == "yml" || ext == "yaml")
                {
                    entries.push(p);
                }
            }
        }

        let mut models = Vec::with_capacity(entries.len());
        for entry in entries {
            let content = fs::read_to_string(&entry).await?;
            let model: SemanticModel = serde_norway::from_str(&content)?;
            model.validate()?;
            models.push(model);
        }

        let mut guard = self.state.write().map_err(|_| StateError::LockError)?;
        let mut next = (**guard).clone();
        for model in models {
            next.models.insert(model.name.clone(), model);
        }
        *guard = Arc::new(next);

        Ok(())
    }

    pub fn get_model(&self, name: &str) -> Result<Option<SemanticModel>, StateError> {
        let guard = self.state.read().map_err(|_| StateError::LockError)?;
        Ok(guard.models.get(name).cloned())
    }

    pub fn get_all_models(&self) -> Result<Vec<SemanticModel>, StateError> {
        let guard = self.state.read().map_err(|_| StateError::LockError)?;
        Ok(guard.models.values().cloned().collect())
    }

    pub fn get_default_model(&self) -> Result<Option<SemanticModel>, StateError> {
        let guard = self.state.read().map_err(|_| StateError::LockError)?;
        Ok(guard.models.values().next().cloned())
    }

    /// Returns aggregate counts of models, entities, and metrics in the current state.
    pub fn get_stats(&self) -> Result<StateStats, StateError> {
        let guard = self.state.read().map_err(|_| StateError::LockError)?;
        Ok(StateStats {
            model_count: guard.models.len(),
            entity_count: guard.models.values().map(|m| m.entities.len()).sum(),
            metric_count: guard.models.values().map(|m| m.metrics.len()).sum(),
        })
    }
}
