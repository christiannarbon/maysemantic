use may_core::models::{Entity, EntityType, SemanticModel};

fn entity_yaml(extra_field: &str) -> String {
    format!(
        "name: test_entity\ntable: test_table\nprimary_key: id\ndimensions: []\nmeasures: []\n{}",
        extra_field
    )
}

#[test]
fn test_entity_type_defaults_to_fact() {
    let yaml = entity_yaml("");
    let entity: Entity = serde_norway::from_str(&yaml).expect("Failed to deserialize entity");
    assert_eq!(entity.entity_type, EntityType::Fact);
}

#[test]
fn test_entity_type_explicit_fact() {
    let yaml = entity_yaml("entity_type: fact");
    let entity: Entity = serde_norway::from_str(&yaml).expect("Failed to deserialize entity");
    assert_eq!(entity.entity_type, EntityType::Fact);
}

#[test]
fn test_entity_type_explicit_dimension() {
    let yaml = entity_yaml("entity_type: dimension");
    let entity: Entity = serde_norway::from_str(&yaml).expect("Failed to deserialize entity");
    assert_eq!(entity.entity_type, EntityType::Dimension);
}

#[test]
fn test_entity_type_serde_roundtrip() {
    let entity_type = EntityType::Dimension;
    let serialized = serde_json::to_string(&entity_type).expect("Failed to serialize EntityType");
    assert_eq!(serialized, "\"dimension\"");
    let deserialized: EntityType =
        serde_json::from_str(&serialized).expect("Failed to deserialize EntityType");
    assert_eq!(deserialized, EntityType::Dimension);
}

#[test]
fn test_demo_yaml_loads_without_error() {
    let content = std::fs::read_to_string("../demos/valid_demo/ecommerce_model.yml")
        .expect("Failed to read ecommerce_model.yml");
    let result: Result<SemanticModel, serde_norway::Error> = serde_norway::from_str(&content);
    assert!(
        result.is_ok(),
        "Failed to deserialize ecommerce_model.yml: {:?}",
        result.err()
    );
}
