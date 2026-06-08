#[cfg(test)]
mod lowering_tests {
    use may_core::ast::{ColumnIdent, Expr, SqlNode, TableIdent};
    use may_core::compiler::lowering::{LoweringError, SemanticLowering};
    use may_core::models::{Dimension, DimensionType, Entity, Measure, Metric, SemanticModel, AggregationType};
    use may_core::SemanticState;
    use may_core::{PostgresDialect, SqlDialect};

    fn make_test_state() -> SemanticState {
        let orders = Entity {
            name: "orders".to_string(),
            description: None,
            table: "orders".to_string(),
            primary_key: "id".to_string(),
            dimensions: vec![
                Dimension {
                    name: "region".to_string(),
                    description: None,
                    sql: "orders.country".to_string(),
                    dimension_type: DimensionType::String,
                }
            ],
            measures: vec![
                Measure {
                    name: "revenue".to_string(),
                    description: None,
                    sql: "amount".to_string(),
                    agg: AggregationType::Sum,
                }
            ],
        };

        let users = Entity {
            name: "users".to_string(),
            description: None,
            table: "users".to_string(),
            primary_key: "id".to_string(),
            dimensions: vec![
                Dimension {
                    name: "user_region".to_string(),
                    description: None,
                    sql: "users.region".to_string(),
                    dimension_type: DimensionType::String,
                }
            ],
            measures: vec![],
        };

        let metric = Metric {
            name: "revenue_by_region".to_string(),
            description: None,
            measure: "revenue".to_string(),
            dimensions: vec!["region".to_string()],
        };

        let model = SemanticModel {
            name: "ecommerce".to_string(),
            entities: vec![orders, users],
            metrics: vec![metric],
            joins: vec![],
        };

        let mut state = SemanticState::new();
        state.models.insert(model.name.clone(), model);
        state
    }

    #[test]
    fn test_lower_dimension_ref_resolves_to_column() {
        let state = make_test_state();
        let lowering = SemanticLowering::new(&state);
        let expr = Expr::DimensionRef { entity: "orders".to_string(), dimension: "region".to_string() };
        assert_eq!(
            lowering.lower_expr(expr),
            Ok(Expr::Column(ColumnIdent("orders.country".to_string())))
        );
    }

    #[test]
    fn test_lower_measure_ref_resolves_to_function() {
        let state = make_test_state();
        let lowering = SemanticLowering::new(&state);
        let expr = Expr::MeasureRef { entity: "orders".to_string(), measure: "revenue".to_string() };
        assert_eq!(
            lowering.lower_expr(expr),
            Ok(Expr::Function {
                name: "SUM".to_string(),
                args: vec![Expr::Column(ColumnIdent("amount".to_string()))]
            })
        );
    }

    #[test]
    fn test_lower_unknown_entity_returns_error() {
        let state = make_test_state();
        let lowering = SemanticLowering::new(&state);
        let expr = Expr::DimensionRef { entity: "nonexistent".to_string(), dimension: "region".to_string() };
        assert_eq!(
            lowering.lower_expr(expr),
            Err(LoweringError::EntityNotFound { entity: "nonexistent".to_string() })
        );
    }

    #[test]
    fn test_lower_unknown_dimension_returns_error() {
        let state = make_test_state();
        let lowering = SemanticLowering::new(&state);
        let expr = Expr::DimensionRef { entity: "orders".to_string(), dimension: "bad_dim".to_string() };
        assert_eq!(
            lowering.lower_expr(expr),
            Err(LoweringError::DimensionNotFound { entity: "orders".to_string(), dimension: "bad_dim".to_string() })
        );
    }

    #[test]
    fn test_lower_full_query_produces_compilable_ast() {
        let state = make_test_state();
        let lowering = SemanticLowering::new(&state);

        let query = SqlNode::Query {
            ctes: None,
            select: Box::new(SqlNode::Select(vec![
                Expr::DimensionRef { entity: "orders".to_string(), dimension: "region".to_string() },
                Expr::MeasureRef { entity: "orders".to_string(), measure: "revenue".to_string() },
            ])),
            from: Box::new(SqlNode::From {
                source: Box::new(SqlNode::Table(TableIdent("orders".to_string()))),
                joins: vec![],
            }),
            r#where: None,
            group_by: Some(Box::new(SqlNode::GroupBy(vec![
                Expr::DimensionRef { entity: "orders".to_string(), dimension: "region".to_string() }
            ]))),
            having: None,
        };

        let lowered_query = lowering.lower_node(query).expect("lowering failed");
        let sql = PostgresDialect.generate_sql(&lowered_query).expect("sql generation failed");
        assert!(!sql.is_empty());
    }
}
