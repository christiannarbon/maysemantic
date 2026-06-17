use may_core::compiler::{CompilerError, SemanticCompiler, SemanticRequest};
use may_core::dialects::PostgresDialect;
use may_core::models::{SemanticModel, SemanticState};
use std::sync::Arc;

fn load_demo_state(path: &str) -> Result<SemanticState, Box<dyn std::error::Error>> {
    let mut state = SemanticState::new();
    let content = std::fs::read_to_string(format!("{}/ecommerce_model.yml", path))?;
    let model: SemanticModel = serde_norway::from_str(&content)?;
    state.models.insert(model.name.clone(), model);
    Ok(state)
}

#[test]
fn test_compile_demo_metric_produces_valid_sql() {
    // Load real demo state from disk
    let state = load_demo_state("../demos/valid_demo").expect("demo model must load successfully");
    let state = Arc::new(state);

    // Build compiler with Postgres dialect
    let compiler = SemanticCompiler::new(state, Box::new(PostgresDialect));

    // Use a REAL metric name from the demo YAML
    let request = SemanticRequest {
        metric_name: "revenue_by_status".to_string(),
        dimensions: vec![],
        filters: vec![],
        time_granularity: None,
        limit: None,
    };

    // Compile and assert
    let result = compiler.compile(request, None);
    let sql = result.expect("compile should succeed for a valid demo metric");

    // Strengthened assertions (FN-6)
    assert!(!sql.trim().is_empty(), "SQL output must not be empty");
    assert!(
        sql.contains("public.orders"),
        "SQL must contain the base table name"
    );
    assert!(
        sql.contains("SUM("),
        "SQL must contain the measure's aggregate call"
    );
    assert!(
        sql.contains("status"),
        "SQL must contain the resolved dimension column"
    );
    assert!(sql.contains("GROUP BY"), "SQL must contain GROUP BY");
    assert!(
        !sql.contains("DimensionRef"),
        "SQL must not contain DimensionRef"
    );
    assert!(
        !sql.contains("MeasureRef"),
        "SQL must not contain MeasureRef"
    );
}

#[test]
fn test_compile_rejects_unsupported_limit() {
    let state = load_demo_state("../demos/valid_demo").expect("demo model must load successfully");
    let state = Arc::new(state);
    let compiler = SemanticCompiler::new(state, Box::new(PostgresDialect));

    let request = SemanticRequest {
        metric_name: "revenue_by_status".to_string(),
        dimensions: vec![],
        filters: vec![],
        time_granularity: None,
        limit: Some(10),
    };

    let result = compiler.compile(request, None);
    match result {
        Err(CompilerError::UnsupportedRequestFeature(field)) => {
            assert_eq!(field, "limit");
        }
        other => panic!(
            "Expected CompilerError::UnsupportedRequestFeature(\"limit\"), got {:?}",
            other
        ),
    }
}

#[test]
fn test_compile_rejects_unsupported_filters() {
    let state = load_demo_state("../demos/valid_demo").expect("demo model must load successfully");
    let state = Arc::new(state);
    let compiler = SemanticCompiler::new(state, Box::new(PostgresDialect));

    let request = SemanticRequest {
        metric_name: "revenue_by_status".to_string(),
        dimensions: vec![],
        filters: vec![may_core::compiler::SemanticFilter {
            dimension: "status".to_string(),
            operator: may_core::compiler::FilterOperator::Eq,
            value: "completed".to_string(),
        }],
        time_granularity: None,
        limit: None,
    };

    let result = compiler.compile(request, None);
    match result {
        Err(CompilerError::UnsupportedRequestFeature(field)) => {
            assert_eq!(field, "filters");
        }
        other => panic!(
            "Expected CompilerError::UnsupportedRequestFeature(\"filters\"), got {:?}",
            other
        ),
    }
}

#[test]
fn test_compile_ambiguous_metric_returns_error() {
    let mut state = SemanticState::new();

    // Model 1
    let model1_content = r#"
name: model1
entities:
  - name: orders
    table: public.orders
    primary_key: order_id
    dimensions:
      - name: status
        type: string
        sql: status
    measures:
      - name: total_revenue
        agg: sum
        sql: amount
metrics:
  - name: shared_metric
    measure: total_revenue
    dimensions: [status]
"#;
    let model1: SemanticModel = serde_norway::from_str(model1_content).expect("parse model1");
    state.models.insert(model1.name.clone(), model1);

    // Model 2
    let model2_content = r#"
name: model2
entities:
  - name: sales
    table: public.sales
    primary_key: sale_id
    dimensions:
      - name: status
        type: string
        sql: status
    measures:
      - name: total_revenue
        agg: sum
        sql: amount
metrics:
  - name: shared_metric
    measure: total_revenue
    dimensions: [status]
"#;
    let model2: SemanticModel = serde_norway::from_str(model2_content).expect("parse model2");
    state.models.insert(model2.name.clone(), model2);

    let state = Arc::new(state);
    let compiler = SemanticCompiler::new(state, Box::new(PostgresDialect));

    let request = SemanticRequest {
        metric_name: "shared_metric".to_string(),
        dimensions: vec![],
        filters: vec![],
        time_granularity: None,
        limit: None,
    };

    let result = compiler.compile(request, None);
    match result {
        Err(CompilerError::AmbiguousMetric { metric, models }) => {
            assert_eq!(metric, "shared_metric");
            assert_eq!(models.len(), 2);
            assert!(models.contains(&"model1".to_string()));
            assert!(models.contains(&"model2".to_string()));
        }
        other => panic!("Expected CompilerError::AmbiguousMetric, got {:?}", other),
    }
}

#[test]
fn test_compile_multi_fact_joins_conformed_dimension() {
    let mut state = SemanticState::new();
    let model_content = r#"
name: chasm_trap_model
entities:
  - name: orders
    table: public.orders
    primary_key: order_id
    entity_type: fact
    dimensions:
      - name: order_id
        type: number
        sql: id
    measures:
      - name: total_revenue
        agg: sum
        sql: amount
  - name: returns
    table: public.returns
    primary_key: return_id
    entity_type: fact
    dimensions:
      - name: return_id
        type: number
        sql: id
    measures:
      - name: total_refunds
        agg: sum
        sql: amount
  - name: users
    table: public.users
    primary_key: user_id
    entity_type: dimension
    dimensions:
      - name: user_id
        type: number
        sql: id
    measures: []
  - name: currencies
    table: public.currencies
    primary_key: currency_id
    entity_type: dimension
    dimensions:
      - name: currency_code
        type: string
        sql: code
    measures: []
joins:
  - left_entity: orders
    left_column: user_id
    right_entity: users
    right_column: user_id
    join_type: left
  - left_entity: users
    left_column: user_id
    right_entity: returns
    right_column: user_id
    join_type: left
  - left_entity: returns
    left_column: currency_id
    right_entity: currencies
    right_column: currency_id
    join_type: left
metrics:
  - name: revenue_with_dimensions
    measure: total_revenue
    dimensions: [user_id, currency_code, return_id]
"#;
    let model: SemanticModel = serde_norway::from_str(model_content).expect("parse model");
    state.models.insert(model.name.clone(), model);

    let state = Arc::new(state);
    let compiler = SemanticCompiler::new(state, Box::new(PostgresDialect));

    let request = SemanticRequest {
        metric_name: "revenue_with_dimensions".to_string(),
        dimensions: vec![],
        filters: vec![],
        time_granularity: None,
        limit: None,
    };

    let result = compiler.compile(request, None);
    let sql = result.expect("compile should succeed and resolve conformed dimension");

    // The query involves orders and returns, which should trigger a MultiFactJoin classification
    // and select users as the conformed dimension (link key: user_id).
    // Verify that injected CTEs use 'user_id' as the link key.
    assert!(
        sql.contains("orders_agg"),
        "SQL should contain injected orders_agg CTE"
    );
    assert!(
        sql.contains("returns_agg"),
        "SQL should contain injected returns_agg CTE"
    );
    assert!(
        sql.contains("user_id"),
        "SQL should reference the link key user_id"
    );
}

#[test]
fn test_compile_multi_fact_joins_no_conformed_dimension_error() {
    let mut state = SemanticState::new();
    let model_content = r#"
name: chasm_trap_no_conformed_model
entities:
  - name: orders
    table: public.orders
    primary_key: order_id
    entity_type: fact
    dimensions:
      - name: order_id
        type: number
        sql: id
    measures:
      - name: total_revenue
        agg: sum
        sql: amount
  - name: returns
    table: public.returns
    primary_key: return_id
    entity_type: fact
    dimensions:
      - name: return_id
        type: number
        sql: id
    measures:
      - name: total_refunds
        agg: sum
        sql: amount
  - name: currencies
    table: public.currencies
    primary_key: currency_id
    entity_type: dimension
    dimensions:
      - name: currency_code
        type: string
        sql: code
    measures: []
joins:
  - left_entity: orders
    left_column: return_id
    right_entity: returns
    right_column: return_id
    join_type: left
  - left_entity: returns
    left_column: currency_id
    right_entity: currencies
    right_column: currency_id
    join_type: left
metrics:
  - name: revenue_no_conformed
    measure: total_revenue
    dimensions: [currency_code, return_id]
"#;
    let model: SemanticModel = serde_norway::from_str(model_content).expect("parse model");
    state.models.insert(model.name.clone(), model);

    let state = Arc::new(state);
    let compiler = SemanticCompiler::new(state, Box::new(PostgresDialect));

    let request = SemanticRequest {
        metric_name: "revenue_no_conformed".to_string(),
        dimensions: vec![],
        filters: vec![],
        time_granularity: None,
        limit: None,
    };

    let result = compiler.compile(request, None);
    match result {
        Err(CompilerError::ChasmTrapHandlingFailed(
            may_core::compiler::ChasmTrapError::LinkDimensionNotFound,
        )) => {}
        other => panic!(
            "Expected CompilerError::ChasmTrapHandlingFailed(LinkDimensionNotFound), got {:?}",
            other
        ),
    }
}

#[test]
fn test_compile_with_rls_injection() {
    use may_core::UserContext;
    use std::collections::HashMap;

    let mut state = SemanticState::new();
    let model_content = r#"
name: rls_model
entities:
  - name: users
    table: public.users
    primary_key: user_id
    entity_type: dimension
    dimensions:
      - name: user_id
        type: number
        sql: id
      - name: tenant_id
        type: string
        sql: tenant
    measures: []
    rls_policies:
      - claim_key: tenant
        dimension: tenant_id
  - name: sales
    table: public.sales
    primary_key: sale_id
    entity_type: fact
    dimensions:
      - name: sale_id
        type: number
        sql: id
    measures:
      - name: total_sales
        agg: sum
        sql: amount
joins:
  - left_entity: sales
    left_column: user_ref
    right_entity: users
    right_column: user_id
    join_type: left
metrics:
  - name: sales_by_user
    measure: total_sales
    dimensions: [user_id]
"#;
    let model: SemanticModel = serde_norway::from_str(model_content).expect("parse model");
    state.models.insert(model.name.clone(), model);

    let state = Arc::new(state);
    let compiler = SemanticCompiler::new(state, Box::new(PostgresDialect));

    let request = SemanticRequest {
        metric_name: "sales_by_user".to_string(),
        dimensions: vec![],
        filters: vec![],
        time_granularity: None,
        limit: None,
    };

    // 1. Compile with user_context = None (RLS bypassed)
    let baseline_sql = compiler
        .compile(request.clone(), None)
        .expect("compile baseline");
    assert!(!baseline_sql.contains("tenant"));

    // 2. Compile with user_context containing claim (RLS injected)
    let mut claims = HashMap::new();
    claims.insert("tenant".to_string(), "tenant_123".to_string());
    let user_ctx = UserContext { claims };

    let rls_sql = compiler
        .compile(request, Some(&user_ctx))
        .expect("compile rls");
    assert!(
        rls_sql.contains("tenant = 'tenant_123'"),
        "RLS predicate must be injected: {}",
        rls_sql
    );
}
