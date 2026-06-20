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

    let expected_sql = "WITH \"orders_agg\" AS (SELECT customer_id, SUM(amount) AS \"amount\" FROM public.orders GROUP BY customer_id), \"returns_agg\" AS (SELECT customer_id FROM public.returns GROUP BY customer_id) SELECT region, return_id, orders_agg.amount FROM orders_agg LEFT JOIN public.customers ON orders_agg.customer_id = public.customers.customer_id LEFT JOIN returns_agg ON public.customers.customer_id = returns_agg.customer_id GROUP BY region, return_id";
    assert_eq!(sql, expected_sql);
}
