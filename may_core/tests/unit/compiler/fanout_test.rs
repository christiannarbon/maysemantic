use may_core::compiler::{FanOutDetector, PathClassification};
use may_core::models::{Entity, EntityType};

fn make_entity(name: &str, entity_type: EntityType) -> Entity {
    Entity {
        name: name.to_string(),
        description: None,
        table: format!("{}_tbl", name),
        primary_key: "id".to_string(),
        dimensions: vec![],
        measures: vec![],
        entity_type,
    }
}

#[test]
fn test_classify_pure_dimension() {
    let nodes = vec![
        make_entity("users", EntityType::Dimension),
        make_entity("teams", EntityType::Dimension),
    ];
    let classification = FanOutDetector::classify(&nodes);
    assert_eq!(classification, PathClassification::PureDimension);
}

#[test]
fn test_classify_single_fact() {
    let nodes = vec![
        make_entity("orders", EntityType::Fact),
        make_entity("users", EntityType::Dimension),
    ];
    let classification = FanOutDetector::classify(&nodes);
    assert_eq!(classification, PathClassification::SingleFact);
}

#[test]
fn test_classify_multi_fact_join() {
    let nodes = vec![
        make_entity("orders", EntityType::Fact),
        make_entity("users", EntityType::Dimension),
        make_entity("sales", EntityType::Fact),
    ];
    let classification = FanOutDetector::classify(&nodes);
    assert_eq!(
        classification,
        PathClassification::MultiFactJoin {
            fact_tables: vec!["orders".to_string(), "sales".to_string()]
        }
    );
}

#[test]
fn test_classify_empty_nodes() {
    let nodes = vec![];
    let classification = FanOutDetector::classify(&nodes);
    assert_eq!(classification, PathClassification::PureDimension);
}
