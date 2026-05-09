use crate::models::SemanticModel;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use thiserror::Error;
use validator::Validate;

#[derive(Error, Debug)]
pub enum StateError {
    #[error("Failed to parse YAML: {0}")]
    YamlError(#[from] serde_yaml::Error),
    #[error("Validation failed: {0}")]
    ValidationError(#[from] validator::ValidationErrors),
    #[error("Failed to acquire state lock")]
    LockError,
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
}

pub struct SemanticState {
    pub models: HashMap<String, SemanticModel>,
}

impl Default for SemanticState {
    fn default() -> Self {
        Self::new()
    }
}

impl SemanticState {
    pub fn new() -> Self {
        Self {
            models: HashMap::new(),
        }
    }
}

pub struct StateMgr {
    state: Arc<RwLock<SemanticState>>,
}

impl Default for StateMgr {
    fn default() -> Self {
        Self::new()
    }
}

impl StateMgr {
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(SemanticState::new())),
        }
    }

    pub fn load_from_yaml(&self, yaml_content: &str) -> Result<(), StateError> {
        let model: SemanticModel = serde_yaml::from_str(yaml_content)?;
        model.validate()?;

        let mut state = self.state.write().map_err(|_| StateError::LockError)?;
        state.models.insert(model.name.clone(), model);

        Ok(())
    }

    pub async fn load_dir(&self, path: impl AsRef<std::path::Path>) -> Result<(), StateError> {
        use tokio::fs;
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

        for entry in entries {
            let content = fs::read_to_string(&entry).await?;
            self.load_from_yaml(&content)?;
        }

        Ok(())
    }

    pub fn get_model(&self, name: &str) -> Result<Option<SemanticModel>, StateError> {
        let state = self.state.read().map_err(|_| StateError::LockError)?;
        Ok(state.models.get(name).cloned())
    }

    pub fn get_stats(&self) -> Result<(usize, usize, usize), StateError> {
        let state = self.state.read().map_err(|_| StateError::LockError)?;
        let model_count = state.models.len();
        let entity_count = state.models.values().map(|m| m.entities.len()).sum();
        let metric_count = state.models.values().map(|m| m.metrics.len()).sum();
        Ok((model_count, entity_count, metric_count))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_yaml_parsing() {
        let valid_yaml = r#"
name: test_model
entities:
  - name: users
    table: public.users
    dimensions:
      - name: user_id
        type: number
        sql: id
      - name: email
        type: string
        sql: email
    measures:
      - name: total_users
        agg: count
        sql: id
metrics:
  - name: active_users_metric
    measure: total_users
    dimensions: [user_id]
"#;
        let mgr = StateMgr::new();
        let result = mgr.load_from_yaml(valid_yaml);
        assert!(
            result.is_ok(),
            "Failed to load valid YAML: {:?}",
            result.err()
        );

        let model = mgr.get_model("test_model").unwrap().unwrap();
        assert_eq!(model.name, "test_model");
        assert_eq!(model.entities.len(), 1);
        assert_eq!(model.metrics.len(), 1);
    }

    #[tokio::test]
    async fn test_load_dir() {
        use std::io::Write;
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("model.yml");
        let mut file = std::fs::File::create(&file_path).unwrap();
        writeln!(file, "name: dir_model\nentities: []\nmetrics: []").unwrap();

        let mgr = StateMgr::new();
        mgr.load_dir(temp_dir.path()).await.unwrap();

        let model = mgr.get_model("dir_model").unwrap().unwrap();
        assert_eq!(model.name, "dir_model");
    }

    #[test]
    fn test_invalid_name_validation() {
        let invalid_yaml = r#"
name: 1InvalidName
entities: []
metrics: []
"#;
        let mgr = StateMgr::new();
        let result = mgr.load_from_yaml(invalid_yaml);
        assert!(matches!(result, Err(StateError::ValidationError(_))));
    }

    #[test]
    fn test_invalid_yaml_format() {
        let invalid_yaml = r#"
name: test_model
entities:
  - name: users
    table: public.users
    dimensions: "not a list"
metrics: []
"#;
        let mgr = StateMgr::new();
        let result = mgr.load_from_yaml(invalid_yaml);
        assert!(matches!(result, Err(StateError::YamlError(_))));
    }

    #[test]
    fn test_generate_json_schema() {
        let schema = schemars::schema_for!(crate::models::SemanticModel);
        let schema_json =
            serde_json::to_string_pretty(&schema).expect("Failed to serialize schema");
        assert!(schema_json.contains("SemanticModel"));
    }
}
