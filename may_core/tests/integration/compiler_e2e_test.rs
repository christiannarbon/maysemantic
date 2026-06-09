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
    let result = compiler.compile(request);
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

    let result = compiler.compile(request);
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

    let result = compiler.compile(request);
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

    let result = compiler.compile(request);
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
