use may_core::ast::JoinType;
use may_core::build_semantic_graph;
use may_core::models::{Entity, JoinDefinition, SemanticModel};
use may_core::SemanticState;
use may_core::{JoinResolutionError, JoinResolver};

/// Builds a SemanticState containing three connected entities:
/// `orders` --[order_user_id = user_id]--> `users` --[user_team_id = team_id]--> `teams`
///
/// This forms a linear chain to verify the acceptance criterion:
/// "Orders -> Users -> Teams returns [Orders_to_Users, Users_to_Teams]"
fn build_three_node_state() -> SemanticState {
    let orders = Entity {
        name: "orders".to_string(),
        description: None,
        table: "public.orders".to_string(),
        primary_key: "id".to_string(),
        dimensions: vec![],
        measures: vec![],
    };

    let users = Entity {
        name: "users".to_string(),
        description: None,
        table: "public.users".to_string(),
        primary_key: "id".to_string(),
        dimensions: vec![],
        measures: vec![],
    };

    let teams = Entity {
        name: "teams".to_string(),
        description: None,
        table: "public.teams".to_string(),
        primary_key: "id".to_string(),
        dimensions: vec![],
        measures: vec![],
    };

    // orders.order_user_id = users.id
    let orders_to_users = JoinDefinition {
        left_entity: "orders".to_string(),
        left_column: "order_user_id".to_string(),
        right_entity: "users".to_string(),
        right_column: "id".to_string(),
        join_type: JoinType::Left,
    };

    // users.user_team_id = teams.id
    let users_to_teams = JoinDefinition {
        left_entity: "users".to_string(),
        left_column: "user_team_id".to_string(),
        right_entity: "teams".to_string(),
        right_column: "id".to_string(),
        join_type: JoinType::Inner,
    };

    let model = SemanticModel {
        name: "test_model".to_string(),
        entities: vec![orders, users, teams],
        metrics: vec![],
        joins: vec![orders_to_users, users_to_teams],
    };

    let mut state = SemanticState::new();
    state.models.insert(model.name.clone(), model);
    state
}

/// Constructs a JoinResolver from the state.
fn build_resolver(state: &SemanticState) -> JoinResolver {
    let (graph, indices) = build_semantic_graph(state).expect("Failed to build semantic graph");
    JoinResolver::new(graph, indices)
}

/// Acceptance Criterion:
/// Starting at `orders` and ending at `teams`, the resolver must return
/// an ordered Vec of exactly TWO GraphEdges: [orders→users, users→teams].
/// This validates both the algorithm's correctness and the edge reconstruction.
#[test]
fn test_find_two_hop_path_orders_to_teams() {
    let state = build_three_node_state();
    let resolver = build_resolver(&state);

    let path = resolver
        .find_join_path("orders", "teams")
        .expect("Expected a valid join path from orders to teams");

    // Must have exactly 2 edges for a 2-hop path through users.
    assert_eq!(path.len(), 2, "Expected exactly 2 edges in the path");

    // First edge: orders → users
    assert_eq!(
        path[0].left_column, "order_user_id",
        "First hop left column should be order_user_id"
    );
    assert_eq!(
        path[0].right_column, "id",
        "First hop right column should be id"
    );
    assert_eq!(
        path[0].join_type,
        JoinType::Left,
        "First hop join type should be Left"
    );

    // Second edge: users → teams
    assert_eq!(
        path[1].left_column, "user_team_id",
        "Second hop left column should be user_team_id"
    );
    assert_eq!(
        path[1].right_column, "id",
        "Second hop right column should be id"
    );
    assert_eq!(
        path[1].join_type,
        JoinType::Inner,
        "Second hop join type should be Inner"
    );
}

/// Tests that a single-hop path returns exactly one edge.
#[test]
fn test_find_one_hop_path_orders_to_users() {
    let state = build_three_node_state();
    let resolver = build_resolver(&state);

    let path = resolver
        .find_join_path("orders", "users")
        .expect("Expected a valid join path from orders to users");

    assert_eq!(path.len(), 1, "Expected exactly 1 edge for a direct join");
    assert_eq!(path[0].left_column, "order_user_id");
    assert_eq!(path[0].right_column, "id");
}

/// Tests that querying the same source and target returns an empty Vec
/// (no JOINs needed to join a table to itself).
#[test]
fn test_same_source_and_target_returns_empty_path() {
    let state = build_three_node_state();
    let resolver = build_resolver(&state);

    let path = resolver
        .find_join_path("users", "users")
        .expect("Same-entity path should succeed with an empty Vec");

    assert!(
        path.is_empty(),
        "Expected an empty path for same source and target"
    );
}

/// Tests that requesting a path to a structurally disconnected entity
/// surfaces a NoPathFound error instead of panicking or returning garbage.
#[test]
fn test_no_path_returns_error_for_disconnected_graph() {
    // Build a state with a disconnected "reports" entity that has no join
    // definitions linking it to anything else.
    let isolated = Entity {
        name: "reports".to_string(),
        description: None,
        table: "public.reports".to_string(),
        primary_key: "id".to_string(),
        dimensions: vec![],
        measures: vec![],
    };

    let model = SemanticModel {
        name: "isolated_model".to_string(),
        entities: vec![isolated],
        metrics: vec![],
        joins: vec![],
    };

    let mut state = build_three_node_state();
    state.models.insert(model.name.clone(), model);

    let resolver = build_resolver(&state);

    let result = resolver.find_join_path("orders", "reports");
    assert!(
        matches!(result, Err(JoinResolutionError::UnreachablePath { .. })),
        "Expected UnreachablePath for a disconnected entity, got: {result:?}"
    );
}

/// Tests that requesting an unknown entity name surfaces UnknownEntity
/// with the correct name embedded in the error.
#[test]
fn test_unknown_source_entity_returns_error() {
    let state = build_three_node_state();
    let resolver = build_resolver(&state);

    let result = resolver.find_join_path("nonexistent_table", "orders");
    assert_eq!(
        result,
        Err(JoinResolutionError::TableNotFound(
            "nonexistent_table".to_string()
        )),
        "Expected TableNotFound error for a source entity that does not exist"
    );
}

/// Tests that requesting an unknown target entity surfaces UnknownEntity.
#[test]
fn test_unknown_target_entity_returns_error() {
    let state = build_three_node_state();
    let resolver = build_resolver(&state);

    let result = resolver.find_join_path("orders", "ghost_table");
    assert_eq!(
        result,
        Err(JoinResolutionError::TableNotFound(
            "ghost_table".to_string()
        )),
        "Expected TableNotFound error for a target entity that does not exist"
    );
}

/// Tests `find_join_path_resolved` connects orders to teams and builds correct SQL
#[test]
fn test_find_join_path_resolved_orders_to_teams() {
    use may_core::ast::builder::build_from_join_path;
    use may_core::graph::GraphNode;
    use may_core::{PostgresDialect, SqlDialect};

    let state = build_three_node_state();
    let resolver = build_resolver(&state);

    let resolved_path = resolver
        .find_join_path_resolved("orders", "teams")
        .expect("Expected a valid resolved join path from orders to teams");

    assert_eq!(resolved_path.len(), 2);
    assert_eq!(resolved_path[0].left_table.entity_name, "orders");
    assert_eq!(resolved_path[0].right_table.entity_name, "users");
    assert_eq!(resolved_path[1].left_table.entity_name, "users");
    assert_eq!(resolved_path[1].right_table.entity_name, "teams");

    let orders_node = GraphNode {
        entity_name: "orders".to_string(),
        table_name: "public.orders".to_string(),
        primary_key: "id".to_string(),
    };

    let from_node = build_from_join_path(&orders_node, &resolved_path);

    let query = may_core::ast::SqlNode::Query {
        ctes: None,
        select: Box::new(may_core::ast::SqlNode::Select(vec![
            may_core::ast::Expr::Raw("1".to_string()),
        ])),
        from: Box::new(from_node),
        r#where: None,
        group_by: None,
        having: None,
    };

    let sql = PostgresDialect
        .generate_sql(&query)
        .expect("should generate SQL");

    assert!(sql.contains("FROM public.orders"));
    assert!(sql.contains("LEFT JOIN public.users ON public.orders.order_user_id = public.users.id"));
    assert!(sql.contains("INNER JOIN public.teams ON public.users.user_team_id = public.teams.id"));
}
