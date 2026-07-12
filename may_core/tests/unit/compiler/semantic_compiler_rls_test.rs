#![cfg(test)]

use may_core::compiler::SemanticCompiler;
use may_core::{
    AggregationType, Dimension, DimensionType, Entity, EntityType, Measure, Metric,
    PostgresDialect, RlsPolicy, SemanticModel, SemanticRequest, SemanticState, UserContext,
};
use std::collections::HashMap;
use std::sync::Arc;

fn state_with_policy() -> SemanticState {
    let orders = Entity {
        name: "orders".to_string(),
        description: None,
        table: "orders".to_string(),
        primary_key: "id".to_string(),
        dimensions: vec![Dimension {
            name: "user_region".to_string(),
            description: None,
            dimension_type: DimensionType::String,
            sql: "region".to_string(),
        }],
        measures: vec![Measure {
            name: "revenue".to_string(),
            description: None,
            agg: AggregationType::Sum,
            sql: "amount".to_string(),
        }],
        entity_type: EntityType::Fact,
        rls_policies: vec![RlsPolicy {
            claim_key: "region".to_string(),
            dimension: "user_region".to_string(),
        }],
    };
    let metric = Metric {
        name: "revenue_by_region".to_string(),
        description: None,
        measure: "revenue".to_string(),
        dimensions: vec!["user_region".to_string()],
    };
    let model = SemanticModel {
        name: "sales".to_string(),
        entities: vec![orders],
        metrics: vec![metric],
        joins: vec![],
    };
    let mut models = HashMap::new();
    models.insert(model.name.clone(), model);
    let mut state = SemanticState::new();
    state.models = models;
    state
}

#[test]
fn test_compile_with_user_context_injects_where() {
    let state = state_with_policy();
    let user = UserContext {
        claims: HashMap::from([("region".to_string(), "EMEA".to_string())]),
    };

    let compiler = SemanticCompiler::new(Arc::new(state), Box::new(PostgresDialect));
    let request = SemanticRequest {
        metric_name: "revenue_by_region".to_string(),
        dimensions: vec![],
        filters: vec![],
        time_granularity: None,
        limit: None,
    };

    let sql = compiler
        .compile(request, Some(&user))
        .expect("compile should succeed");

    assert!(
        sql.contains("\"region\" = 'EMEA'"),
        "expected RLS predicate in SQL, got: {sql}"
    );
}

#[test]
fn test_compile_with_none_context_has_no_rls_filter() {
    let state = state_with_policy();
    let compiler = SemanticCompiler::new(Arc::new(state), Box::new(PostgresDialect));
    let request = SemanticRequest {
        metric_name: "revenue_by_region".to_string(),
        dimensions: vec![],
        filters: vec![],
        time_granularity: None,
        limit: None,
    };

    let sql = compiler
        .compile(request, None)
        .expect("compile should succeed");

    assert!(
        !sql.contains("region = 'EMEA'"),
        "RLS predicate must NOT appear when context is None, got: {sql}"
    );
}

#[test]
fn test_compile_with_missing_claim_denies_all_rows() {
    // F2: a UserContext that lacks the policy's claim must NOT leak rows.
    // The injector fails closed by emitting a never-true guard (1 = 0).
    let state = state_with_policy();
    let user = UserContext {
        claims: HashMap::new(), // no "region" claim
    };
    let compiler = SemanticCompiler::new(Arc::new(state), Box::new(PostgresDialect));
    let request = SemanticRequest {
        metric_name: "revenue_by_region".to_string(),
        dimensions: vec![],
        filters: vec![],
        time_granularity: None,
        limit: None,
    };
    let sql = compiler
        .compile(request, Some(&user))
        .expect("compile should succeed");
    assert!(
        sql.contains("1 = 0"),
        "expected deny-all guard when claim is missing, got: {sql}"
    );
}

fn state_with_bad_policy() -> SemanticState {
    let mut state = state_with_policy();
    // Point the policy at a dimension that does not exist on the entity.
    for model in state.models.values_mut() {
        for entity in model.entities.iter_mut() {
            for policy in entity.rls_policies.iter_mut() {
                policy.dimension = "does_not_exist".to_string();
            }
        }
    }
    state
}

#[test]
fn test_compile_with_unknown_policy_dimension_errors() {
    // F1: a policy referencing a non-existent dimension is a misconfiguration
    // and must fail loudly, not silently.
    let state = state_with_bad_policy();
    let user = UserContext {
        claims: HashMap::from([("region".to_string(), "EMEA".to_string())]),
    };
    let compiler = SemanticCompiler::new(Arc::new(state), Box::new(PostgresDialect));
    let request = SemanticRequest {
        metric_name: "revenue_by_region".to_string(),
        dimensions: vec![],
        filters: vec![],
        time_granularity: None,
        limit: None,
    };
    let err = compiler
        .compile(request, Some(&user))
        .expect_err("compile should fail when a policy references an unknown dimension");
    let msg = err.to_string();
    assert!(
        msg.contains("unknown dimension"),
        "expected an RLS misconfiguration error, got: {msg}"
    );
}

fn state_with_two_entities_same_table() -> SemanticState {
    let orders = Entity {
        name: "orders".to_string(),
        description: None,
        table: "orders".to_string(),
        primary_key: "id".to_string(),
        dimensions: vec![Dimension {
            name: "user_region".to_string(),
            description: None,
            dimension_type: DimensionType::String,
            sql: "region".to_string(),
        }],
        measures: vec![Measure {
            name: "revenue".to_string(),
            description: None,
            agg: AggregationType::Sum,
            sql: "amount".to_string(),
        }],
        entity_type: EntityType::Fact,
        rls_policies: vec![RlsPolicy {
            claim_key: "region".to_string(),
            dimension: "user_region".to_string(),
        }],
    };
    // Second entity on the SAME physical table, with a different policy.
    let orders_audit = Entity {
        name: "orders_audit".to_string(),
        description: None,
        table: "orders".to_string(),
        primary_key: "id".to_string(),
        dimensions: vec![Dimension {
            name: "tenant".to_string(),
            description: None,
            dimension_type: DimensionType::String,
            sql: "tenant_id".to_string(),
        }],
        measures: vec![],
        entity_type: EntityType::Dimension,
        rls_policies: vec![RlsPolicy {
            claim_key: "tenant".to_string(),
            dimension: "tenant".to_string(),
        }],
    };
    let metric = Metric {
        name: "revenue_by_region".to_string(),
        description: None,
        measure: "revenue".to_string(),
        dimensions: vec![],
    };
    let model = SemanticModel {
        name: "sales".to_string(),
        entities: vec![orders, orders_audit],
        metrics: vec![metric],
        joins: vec![],
    };
    let mut models = HashMap::new();
    models.insert(model.name.clone(), model);
    let mut state = SemanticState::new();
    state.models = models;
    state
}

#[test]
fn test_compile_injects_policies_from_all_entities_on_table() {
    // F4: both entities mapping to `orders` contribute their RLS predicate.
    let state = state_with_two_entities_same_table();
    let user = UserContext {
        claims: HashMap::from([
            ("region".to_string(), "EMEA".to_string()),
            ("tenant".to_string(), "acme".to_string()),
        ]),
    };
    let compiler = SemanticCompiler::new(Arc::new(state), Box::new(PostgresDialect));
    let request = SemanticRequest {
        metric_name: "revenue_by_region".to_string(),
        dimensions: vec![],
        filters: vec![],
        time_granularity: None,
        limit: None,
    };
    let sql = compiler
        .compile(request, Some(&user))
        .expect("compile should succeed");
    assert!(
        sql.contains("\"region\" = 'EMEA'"),
        "expected first entity's RLS predicate, got: {sql}"
    );
    assert!(
        sql.contains("\"tenant_id\" = 'acme'"),
        "expected second entity's RLS predicate, got: {sql}"
    );
}

#[test]
fn test_inject_recurses_into_cte_subqueries() {
    use may_core::{ColumnIdent, Expr, RlsInjector, SqlDialect, SqlNode, TableIdent};

    let state = state_with_policy();
    let user = UserContext {
        claims: HashMap::from([("region".to_string(), "EMEA".to_string())]),
    };

    // CTE body: SELECT region FROM orders  (a base table carrying a policy).
    let inner = SqlNode::Query {
        ctes: None,
        select: Box::new(SqlNode::Select(vec![Expr::Column(ColumnIdent(
            "region".to_string(),
        ))])),
        from: Box::new(SqlNode::From {
            source: Box::new(SqlNode::Table(TableIdent("orders".to_string()))),
            joins: vec![],
        }),
        r#where: None,
        group_by: None,
        having: None,
        order_by: vec![],
        limit: None,
        offset: None,
    };
    // Outer query selects FROM the CTE alias (which maps to no entity).
    let outer = SqlNode::Query {
        ctes: Some(vec![SqlNode::CTE {
            alias: TableIdent("orders_cte".to_string()),
            query: Box::new(inner),
        }]),
        select: Box::new(SqlNode::Select(vec![Expr::Column(ColumnIdent(
            "region".to_string(),
        ))])),
        from: Box::new(SqlNode::From {
            source: Box::new(SqlNode::Table(TableIdent("orders_cte".to_string()))),
            joins: vec![],
        }),
        r#where: None,
        group_by: None,
        having: None,
        order_by: vec![],
        limit: None,
        offset: None,
    };

    let injected = RlsInjector::inject(outer, &user, &state).expect("inject should succeed");
    let sql = PostgresDialect
        .generate_sql(&injected)
        .expect("SQL generation failed");

    // Without CTE recursion the inner query would have no WHERE, so this
    // substring proves the predicate landed inside the CTE body.
    assert!(
        sql.contains("\"region\" = 'EMEA'"),
        "expected RLS predicate inside the CTE body, got: {sql}"
    );
}
