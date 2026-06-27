#[cfg(test)]
mod lowering_tests {
    use may_core::ast::{ColumnIdent, Expr, SqlNode, TableIdent};
    use may_core::compiler::lowering::{LoweringError, SemanticLowering};
    use may_core::models::{
        AggregationType, Dimension, DimensionType, Entity, EntityType, Measure, Metric,
        SemanticModel,
    };
    use may_core::SemanticState;
    use may_core::{PostgresDialect, SqlDialect};

    fn make_test_state() -> SemanticState {
        let orders = Entity {
            name: "orders".to_string(),
            description: None,
            table: "orders".to_string(),
            primary_key: "id".to_string(),
            dimensions: vec![Dimension {
                name: "region".to_string(),
                description: None,
                sql: "orders.country".to_string(),
                dimension_type: DimensionType::String,
            }],
            measures: vec![Measure {
                name: "revenue".to_string(),
                description: None,
                sql: "amount".to_string(),
                agg: AggregationType::Sum,
            }],
            entity_type: EntityType::Fact,
            rls_policies: vec![],
        };

        let users = Entity {
            name: "users".to_string(),
            description: None,
            table: "users".to_string(),
            primary_key: "id".to_string(),
            dimensions: vec![Dimension {
                name: "user_region".to_string(),
                description: None,
                sql: "users.region".to_string(),
                dimension_type: DimensionType::String,
            }],
            measures: vec![],
            entity_type: EntityType::Fact,
            rls_policies: vec![],
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
        let expr = Expr::DimensionRef {
            entity: "orders".to_string(),
            dimension: "region".to_string(),
        };
        assert_eq!(
            lowering.lower_expr(expr),
            Ok(Expr::Column(ColumnIdent("orders.country".to_string())))
        );
    }

    #[test]
    fn test_lower_measure_ref_resolves_to_function() {
        let state = make_test_state();
        let lowering = SemanticLowering::new(&state);
        let expr = Expr::MeasureRef {
            entity: "orders".to_string(),
            measure: "revenue".to_string(),
        };
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
        let expr = Expr::DimensionRef {
            entity: "nonexistent".to_string(),
            dimension: "region".to_string(),
        };
        assert_eq!(
            lowering.lower_expr(expr),
            Err(LoweringError::EntityNotFound {
                entity: "nonexistent".to_string()
            })
        );
    }

    #[test]
    fn test_lower_unknown_dimension_returns_error() {
        let state = make_test_state();
        let lowering = SemanticLowering::new(&state);
        let expr = Expr::DimensionRef {
            entity: "orders".to_string(),
            dimension: "bad_dim".to_string(),
        };
        assert_eq!(
            lowering.lower_expr(expr),
            Err(LoweringError::DimensionNotFound {
                entity: "orders".to_string(),
                dimension: "bad_dim".to_string()
            })
        );
    }

    #[test]
    fn test_lower_unknown_measure_returns_error() {
        let state = make_test_state();
        let lowering = SemanticLowering::new(&state);
        let expr = Expr::MeasureRef {
            entity: "orders".to_string(),
            measure: "does_not_exist".to_string(),
        };
        assert_eq!(
            lowering.lower_expr(expr),
            Err(LoweringError::MeasureNotFound {
                entity: "orders".to_string(),
                measure: "does_not_exist".to_string(),
            })
        );
    }

    #[test]
    fn test_lower_ref_nested_in_binary_op_in_where() {
        let state = make_test_state();
        let lowering = SemanticLowering::new(&state);
        let input = SqlNode::Where(Expr::BinaryOp {
            left: Box::new(Expr::DimensionRef {
                entity: "orders".to_string(),
                dimension: "region".to_string(),
            }),
            op: "=".to_string(),
            right: Box::new(Expr::Literal("'US'".to_string())),
        });
        let result = lowering.lower_node(input).expect("lowering failed");
        assert_eq!(
            result,
            SqlNode::Where(Expr::BinaryOp {
                left: Box::new(Expr::Column(ColumnIdent("orders.country".to_string()))),
                op: "=".to_string(),
                right: Box::new(Expr::Literal("'US'".to_string())),
            })
        );
    }

    #[test]
    fn test_lower_ambiguous_dimension_returns_error() {
        let mut state = make_test_state();
        let analytics_model = SemanticModel {
            name: "analytics".to_string(),
            entities: vec![Entity {
                name: "orders".to_string(),
                description: None,
                table: "analytics_orders".to_string(),
                primary_key: "id".to_string(),
                dimensions: vec![Dimension {
                    name: "region".to_string(),
                    description: None,
                    sql: "region_code".to_string(),
                    dimension_type: DimensionType::String,
                }],
                measures: vec![],
                entity_type: EntityType::Fact,
                rls_policies: vec![],
            }],
            metrics: vec![],
            joins: vec![],
        };
        state
            .models
            .insert("analytics".to_string(), analytics_model);

        let lowering = SemanticLowering::new(&state);
        let expr = Expr::DimensionRef {
            entity: "orders".to_string(),
            dimension: "region".to_string(),
        };

        let err = lowering.lower_expr(expr).unwrap_err();
        if let LoweringError::AmbiguousDimension {
            entity,
            dimension,
            models,
        } = err
        {
            assert_eq!(entity, "orders");
            assert_eq!(dimension, "region");
            assert_eq!(models.len(), 2);
            assert!(models.contains(&"ecommerce".to_string()));
            assert!(models.contains(&"analytics".to_string()));
        } else {
            panic!("Expected AmbiguousDimension error");
        }
    }

    #[test]
    fn test_lower_full_query_produces_compilable_ast() {
        let state = make_test_state();
        let lowering = SemanticLowering::new(&state);

        let query = SqlNode::Query {
            ctes: None,
            select: Box::new(SqlNode::Select(vec![
                Expr::DimensionRef {
                    entity: "orders".to_string(),
                    dimension: "region".to_string(),
                },
                Expr::MeasureRef {
                    entity: "orders".to_string(),
                    measure: "revenue".to_string(),
                },
            ])),
            from: Box::new(SqlNode::From {
                source: Box::new(SqlNode::Table(TableIdent("orders".to_string()))),
                joins: vec![],
            }),
            r#where: None,
            group_by: Some(Box::new(SqlNode::GroupBy(vec![Expr::DimensionRef {
                entity: "orders".to_string(),
                dimension: "region".to_string(),
            }]))),
            having: None,
        };

        let lowered_query = lowering.lower_node(query).expect("lowering failed");
        let sql = PostgresDialect
            .generate_sql(&lowered_query)
            .expect("sql generation failed");
        assert_eq!(
            sql,
            "SELECT \"orders\".\"country\", SUM(\"amount\") FROM \"orders\" GROUP BY \"orders\".\"country\""
        );
        assert!(!sql.contains("DimensionRef") && !sql.contains("MeasureRef"));
    }
}
