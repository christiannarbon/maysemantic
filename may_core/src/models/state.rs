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

#[derive(Default, Clone)]
pub struct SemanticState {
    pub models: HashMap<String, SemanticModel>,
}

impl SemanticState {
    pub fn new() -> Self {
        Self::default()
    }
}

pub struct StateMgr {
    state: Arc<RwLock<SemanticState>>,
}

impl Default for StateMgr {
    fn default() -> Self {
        Self {
            state: Arc::new(RwLock::new(SemanticState::new())),
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

    pub fn get_state(&self) -> Arc<RwLock<SemanticState>> {
        self.state.clone()
    }

    pub fn load_from_yaml(&self, yaml_content: &str) -> Result<(), StateError> {
        let model: SemanticModel = serde_norway::from_str(yaml_content)?;
        model.validate()?;

        let mut state = self.state.write().map_err(|_| StateError::LockError)?;
        state.models.insert(model.name.clone(), model);

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

        let mut state = self.state.write().map_err(|_| StateError::LockError)?;
        for model in models {
            state.models.insert(model.name.clone(), model);
        }

        Ok(())
    }

    pub fn get_model(&self, name: &str) -> Result<Option<SemanticModel>, StateError> {
        let state = self.state.read().map_err(|_| StateError::LockError)?;
        Ok(state.models.get(name).cloned())
    }

    pub fn get_all_models(&self) -> Result<Vec<SemanticModel>, StateError> {
        let state = self.state.read().map_err(|_| StateError::LockError)?;
        Ok(state.models.values().cloned().collect())
    }

    pub fn get_default_model(&self) -> Result<Option<SemanticModel>, StateError> {
        let state = self.state.read().map_err(|_| StateError::LockError)?;
        Ok(state.models.values().next().cloned())
    }

    /// Returns aggregate counts of models, entities, and metrics in the current state.
    pub fn get_stats(&self) -> Result<StateStats, StateError> {
        let state = self.state.read().map_err(|_| StateError::LockError)?;
        Ok(StateStats {
            model_count: state.models.len(),
            entity_count: state.models.values().map(|m| m.entities.len()).sum(),
            metric_count: state.models.values().map(|m| m.metrics.len()).sum(),
        })
    }
}
