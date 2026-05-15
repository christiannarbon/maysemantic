use maysemantic::ast::JoinType;
use maysemantic::graph_engine::build_semantic_graph;
use maysemantic::models::{Entity, JoinDefinition, SemanticModel};
use maysemantic::state_mgr::SemanticState;

/// Tests that the GraphEngine can correctly translate a SemanticModel
/// containing multiple entities and joins into a directed `petgraph::DiGraph`.
///
/// Validates:
/// 1. All entities are inserted as GraphNodes.
/// 2. Joins are correctly translated into GraphEdges.
/// 3. The internal string-to-NodeIndex HashMap accurately maps entities.
#[test]
fn test_build_semantic_graph() {
    // Define a mock 'users' entity representing a physical table.
    let users_entity = Entity {
        name: "users".to_string(),
        description: None,
        table: "public.users".to_string(),
        primary_key: "id".to_string(),
        dimensions: vec![],
        measures: vec![],
    };

    // Define a mock 'orders' entity representing a second physical table.
    let orders_entity = Entity {
        name: "orders".to_string(),
        description: None,
        table: "public.orders".to_string(),
        primary_key: "id".to_string(),
        dimensions: vec![],
        measures: vec![],
    };

    // Define a Left Join relationship connecting the two entities:
    // users.id = orders.user_id
    let join = JoinDefinition {
        left_entity: "users".to_string(),
        left_column: "id".to_string(),
        right_entity: "orders".to_string(),
        right_column: "user_id".to_string(),
        join_type: JoinType::Left,
    };

    // Package the entities and join into a unified SemanticModel.
    let model = SemanticModel {
        name: "test_model".to_string(),
        entities: vec![users_entity, orders_entity],
        metrics: vec![],
        joins: vec![join],
    };

    // Initialize the active SemanticState and insert our mock model.
    let mut state = SemanticState::new();
    state.models.insert(model.name.clone(), model);

    // Run the GraphEngine to translate the state into a DiGraph.
    let (graph, indices) = build_semantic_graph(&state).expect("Failed to build graph");

    // Validate Nodes
    assert_eq!(graph.node_count(), 2);
    assert!(indices.contains_key("users"));
    assert!(indices.contains_key("orders"));

    // Validate Edges
    assert_eq!(graph.edge_count(), 1);

    let left_idx = indices["users"];
    let right_idx = indices["orders"];

    // The edge should connect left to right
    let edge_idx = graph
        .find_edge(left_idx, right_idx)
        .expect("Edge not found between users and orders");

    let edge_weight = graph.edge_weight(edge_idx).unwrap();
    assert_eq!(edge_weight.left_column, "id");
    assert_eq!(edge_weight.right_column, "user_id");
    assert_eq!(edge_weight.join_type, JoinType::Left);
}
