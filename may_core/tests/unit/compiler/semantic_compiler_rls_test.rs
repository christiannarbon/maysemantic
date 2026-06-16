#[cfg(test)]
mod semantic_compiler_rls_test {
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
        SemanticState { models }
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
            sql.contains("user_region = 'EMEA'"),
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
            !sql.contains("user_region = 'EMEA'"),
            "RLS predicate must NOT appear when context is None, got: {sql}"
        );
    }
}
