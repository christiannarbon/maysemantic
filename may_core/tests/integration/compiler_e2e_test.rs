use may_core::compiler::{SemanticCompiler, SemanticRequest};
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
    assert!(!sql.trim().is_empty(), "SQL output must not be empty");
    assert!(
        sql.to_uppercase().contains("SELECT"),
        "SQL output must contain SELECT, got: {sql}"
    );
}
