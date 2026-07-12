use may_core::graph::JoinResolver;
use may_core::models::StateMgr;
use std::sync::Arc;

#[test]
fn test_graph_cache_lazy_loading_and_invalidation() {
    let state_mgr = StateMgr::new();

    // 1. Initial empty state
    let state_v1 = state_mgr.snapshot().expect("Failed to get snapshot v1");
    let graph_v1_a = state_v1.get_graph().expect("Failed to get graph v1");
    let graph_v1_b = state_v1
        .get_graph()
        .expect("Failed to get graph v1 second time");

    // Assert that the exact same Arc is returned (cached)
    assert!(
        Arc::ptr_eq(&graph_v1_a, &graph_v1_b),
        "Second call must return cached graph pointer"
    );

    // 2. Load model 1 (orders & users)
    let yaml_1 = r#"
name: ecommerce
entities:
  - name: orders
    table: public.orders
    primary_key: id
    entity_type: fact
    dimensions: []
    measures: []
  - name: users
    table: public.users
    primary_key: id
    entity_type: fact
    dimensions: []
    measures: []
joins:
  - left_entity: orders
    left_column: user_id
    right_entity: users
    right_column: id
    join_type: left
metrics: []
"#;
    state_mgr
        .load_from_yaml(yaml_1)
        .expect("Failed to load yaml_1");

    let state_v2 = state_mgr.snapshot().expect("Failed to get snapshot v2");
    let graph_v2_a = state_v2.get_graph().expect("Failed to get graph v2");

    // Assert that the graph pointer is different from graph_v1_a (cache invalidated)
    assert!(
        !Arc::ptr_eq(&graph_v1_a, &graph_v2_a),
        "New state revision must invalidate cached graph"
    );

    // Verify resolving path orders -> users succeeds
    let resolver_v2 = JoinResolver::new(graph_v2_a.0.clone(), graph_v2_a.1.clone());
    let path = resolver_v2
        .find_join_path("orders", "users")
        .expect("Path should exist");
    assert_eq!(path.len(), 1);

    // 3. Load model 2 (adding teams)
    let yaml_2 = r#"
name: team_model
entities:
  - name: teams
    table: public.teams
    primary_key: id
    entity_type: fact
    dimensions: []
    measures: []
joins:
  - left_entity: users
    left_column: team_id
    right_entity: teams
    right_column: id
    join_type: inner
metrics: []
"#;
    state_mgr
        .load_from_yaml(yaml_2)
        .expect("Failed to load yaml_2");

    let state_v3 = state_mgr.snapshot().expect("Failed to get snapshot v3");
    let graph_v3_a = state_v3.get_graph().expect("Failed to get graph v3");

    // Assert that the graph pointer changed again
    assert!(
        !Arc::ptr_eq(&graph_v2_a, &graph_v3_a),
        "Third state revision must invalidate cached graph"
    );

    // Verify resolving path orders -> teams (requires both models) succeeds!
    let resolver_v3 = JoinResolver::new(graph_v3_a.0.clone(), graph_v3_a.1.clone());
    let path_teams = resolver_v3
        .find_join_path("orders", "teams")
        .expect("Path should exist");
    assert_eq!(path_teams.len(), 2);
}
