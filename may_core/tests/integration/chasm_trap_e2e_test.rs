use may_core::compiler::{SemanticCompiler, SemanticRequest, CompilerError, ChasmTrapError};
use may_core::dialects::PostgresDialect;
use may_core::models::{SemanticModel, SemanticState};
use std::sync::Arc;
use tokio_postgres::NoTls;

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
      - name: customer_id
        type: number
        sql: customer_id
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
      - name: return_customer_id
        type: number
        sql: customer_id
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
    dimensions: [region, return_customer_id]
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

    let expected_sql = "WITH \"orders_agg\" AS (SELECT customer_id, SUM(amount) AS \"amount\" FROM public.orders GROUP BY customer_id), \"returns_agg\" AS (SELECT customer_id FROM public.returns GROUP BY customer_id) SELECT region, returns_agg.customer_id, SUM(orders_agg.amount) FROM orders_agg LEFT JOIN public.customers ON orders_agg.customer_id = public.customers.customer_id LEFT JOIN returns_agg ON public.customers.customer_id = returns_agg.customer_id GROUP BY region, returns_agg.customer_id";
    assert_eq!(sql, expected_sql);
}

#[tokio::test]
async fn test_execute_chasm_trap_positive_case() {
    if std::env::var("PAGILA_TESTS").is_err() {
        eprintln!("Skipping Pagila chasm trap E2E positive test (set PAGILA_TESTS=1 to run)");
        return;
    }

    let mut state = SemanticState::new();
    let model_content = r#"
name: pagila_chasm_trap_model
entities:
  - name: customers
    table: public.customer
    primary_key: customer_id
    entity_type: dimension
    dimensions:
      - name: customer_id
        type: number
        sql: customer_id
      - name: store_id
        type: number
        sql: store_id
    measures: []
  - name: payments
    table: public.payment
    primary_key: payment_id
    entity_type: fact
    dimensions:
      - name: customer_id
        type: number
        sql: customer_id
    measures:
      - name: amount
        agg: sum
        sql: amount
  - name: rentals
    table: public.rental
    primary_key: rental_id
    entity_type: fact
    dimensions:
      - name: rental_id
        type: number
        sql: rental_id
      - name: rental_customer_id
        type: number
        sql: customer_id
    measures: []
joins:
  - left_entity: payments
    left_column: customer_id
    right_entity: customers
    right_column: customer_id
    join_type: left
  - left_entity: customers
    left_column: customer_id
    right_entity: rentals
    right_column: customer_id
    join_type: left
metrics:
  - name: payments_amount_by_store
    measure: amount
    dimensions: [store_id, rental_customer_id]
"#;

    let model: SemanticModel = serde_norway::from_str(model_content).expect("parse model");
    state.models.insert(model.name.clone(), model);

    let state = Arc::new(state);
    let compiler = SemanticCompiler::new(state, Box::new(PostgresDialect));

    let request = SemanticRequest {
        metric_name: "payments_amount_by_store".to_string(),
        dimensions: vec![],
        filters: vec![],
        time_granularity: None,
        limit: None,
    };

    let sql = compiler.compile(request, None).expect("compile should succeed");

    // Connect to Pagila database
    let connect_str = "host=localhost port=5433 user=postgres password=may_password dbname=pagila";
    let (client, connection) = tokio_postgres::connect(connect_str, NoTls).await.expect(
        "Failed to connect to Pagila Postgres database on port 5433.",
    );
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });

    // 1. Get true store-level totals using reference query
    let reference_rows = client
        .query(
            "SELECT c.store_id::int, SUM(p.amount)::float8 FROM public.payment p JOIN public.customer c ON p.customer_id = c.customer_id GROUP BY c.store_id",
            &[],
        )
        .await
        .expect("failed to execute reference query");

    let mut true_store_totals = std::collections::HashMap::new();
    for row in reference_rows {
        let store_id: i32 = row.get(0);
        let amount: f64 = row.get(1);
        true_store_totals.insert(store_id, amount);
    }

    // Wrap the semantic query to cast columns to simple float8/int types for tokio-postgres compatibility
    let execute_sql = format!(
        "SELECT store_id::int, customer_id::int, sum::float8 FROM ({}) AS sub",
        sql
    );

    // 2. Execute semantic compiled query
    let rows = client
        .query(&execute_sql, &[])
        .await
        .expect("failed to execute semantic query");

    let mut computed_store_totals = std::collections::HashMap::new();
    for row in rows {
        let store_id: i32 = row.get(0);
        let amount: f64 = row.get(2);
        *computed_store_totals.entry(store_id).or_insert(0.0) += amount;
    }

    assert!(!true_store_totals.is_empty(), "true totals should not be empty");
    assert_eq!(computed_store_totals.len(), true_store_totals.len(), "should have the same number of stores");

    for (store_id, true_total) in &true_store_totals {
        let computed_total = computed_store_totals.get(store_id).copied().unwrap_or(0.0);
        // Assert within small tolerance to handle floating-point representation differences
        let diff = (computed_total - *true_total).abs();
        assert!(
            diff < 1e-5,
            "Total mismatch for store {}: computed {}, true {}, diff {}",
            store_id, computed_total, true_total, diff
        );
    }
}

#[test]
fn test_execute_chasm_trap_negative_case() {
    let mut state = SemanticState::new();
    let model_content = r#"
name: pagila_chasm_trap_model
entities:
  - name: customers
    table: public.customer
    primary_key: customer_id
    entity_type: dimension
    dimensions:
      - name: customer_id
        type: number
        sql: customer_id
      - name: store_id
        type: number
        sql: store_id
    measures: []
  - name: payments
    table: public.payment
    primary_key: payment_id
    entity_type: fact
    dimensions:
      - name: customer_id
        type: number
        sql: customer_id
    measures:
      - name: amount
        agg: sum
        sql: amount
  - name: rentals
    table: public.rental
    primary_key: rental_id
    entity_type: fact
    dimensions:
      - name: rental_id
        type: number
        sql: rental_id
    measures: []
joins:
  - left_entity: payments
    left_column: customer_id
    right_entity: customers
    right_column: customer_id
    join_type: left
  - left_entity: customers
    left_column: customer_id
    right_entity: rentals
    right_column: customer_id
    join_type: left
metrics:
  - name: payments_amount_by_store_invalid
    measure: amount
    dimensions: [store_id, rental_id]
"#;

    let model: SemanticModel = serde_norway::from_str(model_content).expect("parse model");
    state.models.insert(model.name.clone(), model);

    let state = Arc::new(state);
    let compiler = SemanticCompiler::new(state, Box::new(PostgresDialect));

    let request = SemanticRequest {
        metric_name: "payments_amount_by_store_invalid".to_string(),
        dimensions: vec![],
        filters: vec![],
        time_granularity: None,
        limit: None,
    };

    let result = compiler.compile(request, None);
    assert!(
        matches!(
            result,
            Err(CompilerError::ChasmTrapHandlingFailed(ChasmTrapError::FinerThanConformedGrain { .. }))
        ),
        "Expected FinerThanConformedGrain error, got {:?}",
        result
    );
    if let Err(CompilerError::ChasmTrapHandlingFailed(ChasmTrapError::FinerThanConformedGrain { dimension, entity, conformed_grain })) = result {
        assert_eq!(dimension, "rental_id");
        assert_eq!(entity, "rentals");
        assert_eq!(conformed_grain, "customer_id");
    }
}
