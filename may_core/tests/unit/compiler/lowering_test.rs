#[cfg(test)]
mod lowering_tests {
    use may_core::ast::{ColumnIdent, Expr, SqlNode};
    use may_core::compiler::lowering::SemanticLowering;
    use may_core::models::{Dimension, Entity, Measure, SemanticModel, AggregationType};
    use may_core::SemanticState;
    use may_core::{PostgresDialect, SqlDialect};

    fn build_test_state() -> SemanticState {
        let orders = Entity {
            name: "orders".to_string(),
            description: None,
            table: "public.orders".to_string(),
            primary_key: "id".to_string(),
            dimensions: vec![
                Dimension {
                    name: "region".to_string(),
                    description: None,
                    sql: "country".to_string(),
                    dimension_type: may_core::models::DimensionType::String,
                }
            ],
            measures: vec![
                Measure {
                    name: "total_amount".to_string(),
                    description: None,
                    sql: "amount".to_string(),
                    agg: AggregationType::Sum,
                }
            ],
        };

        let model = SemanticModel {
            name: "test_model".to_string(),
            entities: vec![orders],
            metrics: vec![],
            joins: vec![],
        };

        let mut state = SemanticState::new();
        state.models.insert(model.name.clone(), model);
        state
    }

    #[test]
    fn test_lower_node_select_with_dimension_ref() {
        let state = build_test_state();
        let lowering = SemanticLowering::new(&state);

        let input = SqlNode::Select(vec![Expr::DimensionRef {
            entity: "orders".to_string(),
            dimension: "region".to_string(),
        }]);

        let result = lowering.lower_node(input).expect("lowering failed");

        assert_eq!(
            result,
            SqlNode::Select(vec![Expr::Column(ColumnIdent("public.orders.country".to_string()))])
        );
    }

    #[test]
    fn test_lower_node_query_clears_all_semantic_nodes() {
        let state = build_test_state();
        let lowering = SemanticLowering::new(&state);

        let input = SqlNode::Query {
            ctes: None,
            select: Box::new(SqlNode::Select(vec![
                Expr::DimensionRef {
                    entity: "orders".to_string(),
                    dimension: "region".to_string(),
                },
                Expr::MeasureRef {
                    entity: "orders".to_string(),
                    measure: "total_amount".to_string(),
                },
            ])),
            from: Box::new(SqlNode::Table(may_core::ast::TableIdent("public.orders".to_string()))),
            r#where: None,
            group_by: None,
            having: None,
        };

        let result = lowering.lower_node(input).expect("lowering failed");
        
        let sql = PostgresDialect.generate_sql(&result).expect("generate_sql failed");
        
        // Assert no error is returned and sql is valid
        assert!(sql.contains("public.orders.country"));
        assert!(sql.contains("SUM(public.orders.amount)"));
    }
}
