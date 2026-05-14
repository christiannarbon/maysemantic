use maysemantic::{StateError, StateMgr};

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
    // Generate JSON schema from the public SemanticModel structure
    let schema = schemars::schema_for!(maysemantic::SemanticModel);
    let schema_json = serde_json::to_string_pretty(&schema).expect("Failed to serialize schema");
    assert!(schema_json.contains("SemanticModel"));
}
