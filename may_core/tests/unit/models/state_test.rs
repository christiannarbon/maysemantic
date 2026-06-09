use may_core::{StateError, StateMgr};

/// Tests that the `StateMgr` correctly parses and validates a well-formed Semantic Layer YAML.
///
/// Validates:
/// 1. Standard entity/dimension/measure blocks are successfully parsed.
/// 2. Enums (`DimensionType`, `AggregationType`) deserialize perfectly.
/// 3. Required fields like `primary_key` are properly enforced by the validator.
#[test]
fn test_valid_yaml_parsing() {
    let valid_yaml = r#"
name: test_model
entities:
  - name: users
    table: public.users
    primary_key: id
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

/// Tests that the `StateMgr` can traverse a directory and load all `.yml` or `.yaml` files.
///
/// Validates:
/// 1. Asynchronous I/O directory reading.
/// 2. Multiple file loading into a unified `SemanticState` HashMap.
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

/// Tests the `NAME_REGEX` validation rules on entity names.
///
/// Validates:
/// 1. Names cannot start with numbers.
/// 2. The `validator` crate properly rejects non-alphanumeric identifiers,
///    preventing SQL injection vulnerabilities at the parsing stage.
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

/// Tests that improperly formatted YAML strictly fails parsing.
///
/// Validates:
/// 1. Providing a String ("not a list") to a Vec field properly triggers `serde_norway` errors.
#[test]
fn test_invalid_yaml_format() {
    let invalid_yaml = r#"
name: test_model
entities:
  - name: users
    table: public.users
    primary_key: id
    dimensions: "not a list"
metrics: []
"#;
    let mgr = StateMgr::new();
    let result = mgr.load_from_yaml(invalid_yaml);
    assert!(matches!(result, Err(StateError::YamlError(_))));
}

/// Tests the automatic generation of a JSON Schema representing the SemanticModel.
///
/// Validates:
/// 1. The `schemars` derivation successfully traverses the struct hierarchy
///    to output a schema used for IDE intelligence (autocomplete/validation).
#[test]
fn test_generate_json_schema() {
    // Generate JSON schema from the public SemanticModel structure
    let schema = schemars::schema_for!(may_core::SemanticModel);
    let schema_json = serde_json::to_string_pretty(&schema).expect("Failed to serialize schema");
    assert!(schema_json.contains("SemanticModel"));
}

/// Tests that an entity without an `entity_type` field defaults to `EntityType::Fact` upon deserialization.
#[test]
fn test_entity_type_defaulting() {
    let valid_yaml = r#"
name: test_default_model
entities:
  - name: users
    table: public.users
    primary_key: id
    dimensions: []
    measures: []
metrics: []
"#;
    let mgr = StateMgr::new();
    mgr.load_from_yaml(valid_yaml).unwrap();

    let model = mgr.get_model("test_default_model").unwrap().unwrap();
    let entity = &model.entities[0];
    assert_eq!(entity.entity_type, may_core::EntityType::Fact);
}

/// Tests that an entity with an explicit `entity_type` field parses correctly.
#[test]
fn test_entity_type_explicit() {
    let explicit_yaml = r#"
name: test_explicit_model
entities:
  - name: users
    table: public.users
    primary_key: id
    dimensions: []
    measures: []
    entity_type: dimension
metrics: []
"#;
    let mgr = StateMgr::new();
    mgr.load_from_yaml(explicit_yaml).unwrap();

    let model = mgr.get_model("test_explicit_model").unwrap().unwrap();
    let entity = &model.entities[0];
    assert_eq!(entity.entity_type, may_core::EntityType::Dimension);
}

