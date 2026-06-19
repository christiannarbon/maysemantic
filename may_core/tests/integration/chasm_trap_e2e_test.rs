use may_core::compiler::{SemanticCompiler, SemanticRequest};
use may_core::dialects::PostgresDialect;
use may_core::models::{SemanticModel, SemanticState};
use std::sync::Arc;

#[test]
fn test_compile_chasm_trap_cte_rewriting_e2e() {
    let mut state = SemanticState::new();
    let model_content = r#"
name: chasm_trap_e2e_model
entities:
  - name: customers
    table: public.customers
    primary_key: customer_id
    entity_type: dimension
    dimensions:
      - name: customer_id
        type: number
        sql: customer_id
      - name: region
        type: string
        sql: region
    measures: []
  - name: orders
    table: public.orders
    primary_key: order_id
    entity_type: fact
    dimensions:
      - name: order_id
        type: number
        sql: order_id
    measures:
      - name: amount
        agg: sum
        sql: amount
  - name: returns
    table: public.returns
    primary_key: return_id
    entity_type: fact
    dimensions:
      - name: return_id
        type: number
        sql: return_id
    measures: []
joins:
  - left_entity: orders
    left_column: customer_id
    right_entity: customers
    right_column: customer_id
    join_type: left
  - left_entity: customers
    left_column: customer_id
    right_entity: returns
    right_column: customer_id
    join_type: left
metrics:
  - name: orders_amount_by_region
    measure: amount
    dimensions: [region, return_id]
"#;

    let model: SemanticModel = serde_norway::from_str(model_content).expect("parse model");
    state.models.insert(model.name.clone(), model);

    let state = Arc::new(state);
    let compiler = SemanticCompiler::new(state, Box::new(PostgresDialect));

    let request = SemanticRequest {
        metric_name: "orders_amount_by_region".to_string(),
        dimensions: vec![],
        filters: vec![],
        time_granularity: None,
        limit: None,
    };

    let result = compiler.compile(request, None);
    let sql = result.expect("compile should succeed and rewrite outer query to CTEs");

    println!("Generated SQL:\n{}", sql);

    // Assert that the pre-aggregation CTEs are defined
    assert!(
        sql.contains("\"public.orders_agg\" AS (") || sql.contains("orders_agg AS ("),
        "Should contain orders_agg CTE"
    );
    assert!(
        sql.contains("\"public.returns_agg\" AS (") || sql.contains("returns_agg AS ("),
        "Should contain returns_agg CTE"
    );

    // Verify aggregation in CTE
    // e.g., orders_agg should select customer_id and SUM(amount) AS amount
    assert!(sql.contains("SUM(amount)"), "orders_agg CTE should sum the amount measure");

    // Assert that the outer SELECT projects from orders_agg and NOT raw orders,
    // and is NOT double-aggregated (so it selects orders_agg.amount, not SUM(orders_agg.amount) or SUM(amount))
    assert!(
        sql.contains("public.orders_agg.amount") || sql.contains("orders_agg.amount"),
        "Outer SELECT should project orders_agg.amount"
    );
    assert!(!sql.contains("SUM(public.orders_agg.amount)"), "Outer SELECT must not double aggregate orders_agg.amount");

    // Verify outer FROM/JOIN references the aggregates
    assert!(
        sql.contains("public.orders_agg") || sql.contains("orders_agg"),
        "Outer FROM/JOIN should reference orders_agg table"
    );
    assert!(
        sql.contains("public.returns_agg") || sql.contains("returns_agg"),
        "Outer FROM/JOIN should reference returns_agg table"
    );
}
