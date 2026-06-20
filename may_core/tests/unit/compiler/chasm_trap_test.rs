use may_core::ast::{ColumnIdent, Expr, JoinType, SqlNode, TableIdent};
use may_core::compiler::{
    ChasmTrapError, ChasmTrapHandler, FactPreAgg, MeasureProjection, PathClassification,
};
use may_core::models::AggregationType;

fn create_helper_query() -> SqlNode {
    let select_node = SqlNode::Select(vec![
        Expr::Column(ColumnIdent("orders.revenue".to_string())),
        Expr::Column(ColumnIdent("returns.refund_amount".to_string())),
    ]);

    let join_condition = Expr::BinaryOp {
        left: Box::new(Expr::Column(ColumnIdent("orders.user_id".to_string()))),
        op: "=".to_string(),
        right: Box::new(Expr::Column(ColumnIdent("returns.user_id".to_string()))),
    };

    let join_node = SqlNode::Join {
        join_type: JoinType::Left,
        relation: Box::new(SqlNode::Table(TableIdent("returns".to_string()))),
        on: join_condition,
    };

    let from_node = SqlNode::From {
        source: Box::new(SqlNode::Table(TableIdent("orders".to_string()))),
        joins: vec![join_node],
    };

    SqlNode::Query {
        ctes: None,
        select: Box::new(select_node),
        from: Box::new(from_node),
        r#where: None,
        group_by: None,
        having: None,
    }
}

#[test]
fn test_single_fact_returns_query_unchanged() {
    let query = create_helper_query();
    let classification = PathClassification::SingleFact;
    let result = ChasmTrapHandler::inject_ctes(query.clone(), &classification, &[]).unwrap();
    assert_eq!(result, query);
}

#[test]
fn test_pure_dimension_returns_query_unchanged() {
    let query = create_helper_query();
    let classification = PathClassification::PureDimension;
    let result = ChasmTrapHandler::inject_ctes(query.clone(), &classification, &[]).unwrap();
    assert_eq!(result, query);
}

#[test]
fn test_multi_fact_injects_two_ctes() {
    let query = create_helper_query();
    let classification = PathClassification::MultiFactJoin {
        fact_tables: vec!["orders".to_string(), "returns".to_string()],
    };
    let facts = vec![
        FactPreAgg {
            entity: "orders".to_string(),
            table: "orders".to_string(),
            group_key: "user_id".to_string(),
            measures: vec![MeasureProjection {
                name: "amount".to_string(),
                agg: AggregationType::Sum,
                sql: "amount".to_string(),
            }],
        },
        FactPreAgg {
            entity: "returns".to_string(),
            table: "returns".to_string(),
            group_key: "user_id".to_string(),
            measures: vec![],
        },
    ];
    let result = ChasmTrapHandler::inject_ctes(query, &classification, &facts).unwrap();
    if let SqlNode::Query { ctes, .. } = &result {
        assert_eq!(ctes.as_ref().unwrap().len(), 2);
    } else {
        panic!("Expected Query node");
    }
}

#[test]
fn test_multi_fact_cte_aliases_are_correct() {
    let query = create_helper_query();
    let classification = PathClassification::MultiFactJoin {
        fact_tables: vec!["orders".to_string(), "returns".to_string()],
    };
    let facts = vec![
        FactPreAgg {
            entity: "orders".to_string(),
            table: "orders".to_string(),
            group_key: "user_id".to_string(),
            measures: vec![MeasureProjection {
                name: "amount".to_string(),
                agg: AggregationType::Sum,
                sql: "amount".to_string(),
            }],
        },
        FactPreAgg {
            entity: "returns".to_string(),
            table: "returns".to_string(),
            group_key: "user_id".to_string(),
            measures: vec![],
        },
    ];
    let result = ChasmTrapHandler::inject_ctes(query, &classification, &facts).unwrap();
    if let SqlNode::Query { ctes, .. } = result {
        let ctes = ctes.unwrap();
        assert_eq!(ctes.len(), 2);
        if let SqlNode::CTE { alias: alias0, .. } = &ctes[0] {
            assert_eq!(alias0.0, "orders_agg");
        } else {
            panic!("Expected CTE");
        }
        if let SqlNode::CTE { alias: alias1, .. } = &ctes[1] {
            assert_eq!(alias1.0, "returns_agg");
        } else {
            panic!("Expected CTE");
        }
    } else {
        panic!("Expected Query node");
    }
}

#[test]
fn test_not_a_query_node_returns_error() {
    let query = SqlNode::Table(TableIdent("orders".to_string()));
    let classification = PathClassification::MultiFactJoin {
        fact_tables: vec!["orders".to_string(), "returns".to_string()],
    };
    let facts = vec![
        FactPreAgg {
            entity: "orders".to_string(),
            table: "orders".to_string(),
            group_key: "user_id".to_string(),
            measures: vec![MeasureProjection {
                name: "amount".to_string(),
                agg: AggregationType::Sum,
                sql: "amount".to_string(),
            }],
        },
        FactPreAgg {
            entity: "returns".to_string(),
            table: "returns".to_string(),
            group_key: "user_id".to_string(),
            measures: vec![],
        },
    ];
    let result = ChasmTrapHandler::inject_ctes(query, &classification, &facts);
    assert_eq!(result.unwrap_err(), ChasmTrapError::NotAQueryNode);
}

#[test]
fn test_inject_ctes_single_fact() {
    let query = SqlNode::Table(TableIdent("orders".to_string()));
    let classification = PathClassification::SingleFact;
    let result = ChasmTrapHandler::inject_ctes(query.clone(), &classification, &[]);
    assert_eq!(result.unwrap(), query);
}

#[test]
fn test_inject_ctes_pure_dimension() {
    let query = SqlNode::Table(TableIdent("users".to_string()));
    let classification = PathClassification::PureDimension;
    let result = ChasmTrapHandler::inject_ctes(query.clone(), &classification, &[]);
    assert_eq!(result.unwrap(), query);
}

#[test]
fn test_inject_ctes_empty_fact_tables() {
    let query = SqlNode::Table(TableIdent("orders".to_string()));
    let classification = PathClassification::MultiFactJoin {
        fact_tables: vec![],
    };
    let result = ChasmTrapHandler::inject_ctes(query, &classification, &[]);
    assert_eq!(result.unwrap_err(), ChasmTrapError::EmptyFactTableList);
}

#[test]
fn test_inject_ctes_not_a_query_node() {
    let query = SqlNode::Table(TableIdent("orders".to_string()));
    let classification = PathClassification::MultiFactJoin {
        fact_tables: vec!["orders".to_string(), "returns".to_string()],
    };
    let facts = vec![
        FactPreAgg {
            entity: "orders".to_string(),
            table: "orders".to_string(),
            group_key: "user_id".to_string(),
            measures: vec![MeasureProjection {
                name: "amount".to_string(),
                agg: AggregationType::Sum,
                sql: "amount".to_string(),
            }],
        },
        FactPreAgg {
            entity: "returns".to_string(),
            table: "returns".to_string(),
            group_key: "user_id".to_string(),
            measures: vec![],
        },
    ];
    let result = ChasmTrapHandler::inject_ctes(query, &classification, &facts);
    assert_eq!(result.unwrap_err(), ChasmTrapError::NotAQueryNode);
}

#[test]
fn test_inject_ctes_multi_fact_join_success() {
    let select_node = SqlNode::Select(vec![Expr::Column(ColumnIdent("user_id".to_string()))]);
    let from_node = SqlNode::From {
        source: Box::new(SqlNode::Table(TableIdent("users".to_string()))),
        joins: vec![],
    };
    let query = SqlNode::Query {
        ctes: None,
        select: Box::new(select_node),
        from: Box::new(from_node),
        r#where: None,
        group_by: None,
        having: None,
    };

    let classification = PathClassification::MultiFactJoin {
        fact_tables: vec!["orders".to_string(), "returns".to_string()],
    };

    let facts = vec![
        FactPreAgg {
            entity: "orders".to_string(),
            table: "orders".to_string(),
            group_key: "user_id".to_string(),
            measures: vec![MeasureProjection {
                name: "amount".to_string(),
                agg: AggregationType::Sum,
                sql: "amount".to_string(),
            }],
        },
        FactPreAgg {
            entity: "returns".to_string(),
            table: "returns".to_string(),
            group_key: "user_id".to_string(),
            measures: vec![],
        },
    ];

    let result = ChasmTrapHandler::inject_ctes(query, &classification, &facts).unwrap();

    if let SqlNode::Query { ctes, .. } = result {
        let ctes = ctes.expect("Expected CTEs to be injected");
        assert_eq!(ctes.len(), 2);

        // Verify the first CTE
        if let SqlNode::CTE { alias, query: body } = &ctes[0] {
            assert_eq!(alias.0, "orders_agg");
            if let SqlNode::Query {
                ctes: inner_ctes,
                select: inner_select,
                from: inner_from,
                group_by: inner_group_by,
                ..
            } = &**body
            {
                assert!(inner_ctes.is_none());
                if let SqlNode::Select(proj) = &**inner_select {
                    assert_eq!(proj.len(), 2);
                    assert_eq!(proj[0], Expr::Column(ColumnIdent("user_id".to_string())));
                    assert_eq!(
                        proj[1],
                        Expr::Aliased {
                            expr: Box::new(Expr::Function {
                                name: "SUM".to_string(),
                                args: vec![Expr::Column(ColumnIdent("amount".to_string()))],
                            }),
                            alias: "amount".to_string(),
                        }
                    );
                } else {
                    panic!("Expected Select node in CTE query");
                }
                if let SqlNode::From { source, joins } = &**inner_from {
                    assert!(joins.is_empty());
                    if let SqlNode::Table(TableIdent(tbl)) = &**source {
                        assert_eq!(tbl, "orders");
                    } else {
                        panic!("Expected Table node inside FROM source");
                    }
                } else {
                    panic!("Expected From node in CTE query");
                }
                let group_outer = inner_group_by.as_ref().expect("Expected GroupBy node");
                if let SqlNode::GroupBy(keys) = &**group_outer {
                    assert_eq!(keys.len(), 1);
                    assert_eq!(keys[0], Expr::Column(ColumnIdent("user_id".to_string())));
                } else {
                    panic!("Expected GroupBy node");
                }
            } else {
                panic!("Expected Query node body in CTE");
            }
        } else {
            panic!("Expected CTE node at index 0");
        }

        // Verify the second CTE
        if let SqlNode::CTE { alias, query: body } = &ctes[1] {
            assert_eq!(alias.0, "returns_agg");
            if let SqlNode::Query {
                from: inner_from, ..
            } = &**body
            {
                if let SqlNode::From { source, .. } = &**inner_from {
                    if let SqlNode::Table(TableIdent(tbl)) = &**source {
                        assert_eq!(tbl, "returns");
                    } else {
                        panic!("Expected Table node inside FROM source");
                    }
                } else {
                    panic!("Expected From node in CTE query");
                }
            } else {
                panic!("Expected Query node body in CTE");
            }
        } else {
            panic!("Expected CTE node at index 1");
        }
    } else {
        panic!("Expected Query node at root");
    }
}

#[test]
fn test_inject_ctes_preserves_existing_ctes() {
    let select_node = SqlNode::Select(vec![Expr::Column(ColumnIdent("user_id".to_string()))]);
    let from_node = SqlNode::From {
        source: Box::new(SqlNode::Table(TableIdent("users".to_string()))),
        joins: vec![],
    };

    // Build an existing CTE named "existing_cte"
    let existing_subquery = SqlNode::Query {
        ctes: None,
        select: Box::new(SqlNode::Select(vec![Expr::Column(ColumnIdent(
            "user_id".to_string(),
        ))])),
        from: Box::new(SqlNode::From {
            source: Box::new(SqlNode::Table(TableIdent("users".to_string()))),
            joins: vec![],
        }),
        r#where: None,
        group_by: None,
        having: None,
    };
    let existing_cte = SqlNode::CTE {
        alias: TableIdent("existing_cte".to_string()),
        query: Box::new(existing_subquery),
    };

    let query = SqlNode::Query {
        ctes: Some(vec![existing_cte]),
        select: Box::new(select_node),
        from: Box::new(from_node),
        r#where: None,
        group_by: None,
        having: None,
    };

    let classification = PathClassification::MultiFactJoin {
        fact_tables: vec!["orders".to_string()],
    };

    let facts = vec![FactPreAgg {
        entity: "orders".to_string(),
        table: "orders".to_string(),
        group_key: "user_id".to_string(),
        measures: vec![MeasureProjection {
            name: "amount".to_string(),
            agg: AggregationType::Sum,
            sql: "amount".to_string(),
        }],
    }];

    let result = ChasmTrapHandler::inject_ctes(query, &classification, &facts).unwrap();

    if let SqlNode::Query { ctes, .. } = result {
        let ctes = ctes.expect("Expected CTEs to be present");
        assert_eq!(ctes.len(), 2);

        // Assert first is existing_cte
        if let SqlNode::CTE { alias, .. } = &ctes[0] {
            assert_eq!(alias.0, "existing_cte");
        } else {
            panic!("Expected CTE at index 0");
        }

        // Assert second is orders_agg
        if let SqlNode::CTE { alias, .. } = &ctes[1] {
            assert_eq!(alias.0, "orders_agg");
        } else {
            panic!("Expected CTE at index 1");
        }
    } else {
        panic!("Expected Query node at root");
    }
}

#[test]
fn test_inject_ctes_deduplicates_aliases() {
    let select_node = SqlNode::Select(vec![Expr::Column(ColumnIdent("user_id".to_string()))]);
    let from_node = SqlNode::From {
        source: Box::new(SqlNode::Table(TableIdent("users".to_string()))),
        joins: vec![],
    };

    // Build an existing CTE named "orders_agg"
    let existing_subquery = SqlNode::Query {
        ctes: None,
        select: Box::new(SqlNode::Select(vec![Expr::Column(ColumnIdent(
            "user_id".to_string(),
        ))])),
        from: Box::new(SqlNode::From {
            source: Box::new(SqlNode::Table(TableIdent("users".to_string()))),
            joins: vec![],
        }),
        r#where: None,
        group_by: None,
        having: None,
    };
    let existing_cte = SqlNode::CTE {
        alias: TableIdent("orders_agg".to_string()),
        query: Box::new(existing_subquery),
    };

    let query = SqlNode::Query {
        ctes: Some(vec![existing_cte]),
        select: Box::new(select_node),
        from: Box::new(from_node),
        r#where: None,
        group_by: None,
        having: None,
    };

    // We pass "orders" twice (duplicate in fact_tables), and it matches the existing CTE "orders_agg".
    // It should not add any new CTE since both are duplicate of "orders_agg".
    let classification = PathClassification::MultiFactJoin {
        fact_tables: vec!["orders".to_string(), "orders".to_string()],
    };

    let facts = vec![
        FactPreAgg {
            entity: "orders".to_string(),
            table: "orders".to_string(),
            group_key: "user_id".to_string(),
            measures: vec![MeasureProjection {
                name: "amount".to_string(),
                agg: AggregationType::Sum,
                sql: "amount".to_string(),
            }],
        },
        FactPreAgg {
            entity: "orders".to_string(),
            table: "orders".to_string(),
            group_key: "user_id".to_string(),
            measures: vec![MeasureProjection {
                name: "amount".to_string(),
                agg: AggregationType::Sum,
                sql: "amount".to_string(),
            }],
        },
    ];

    let result = ChasmTrapHandler::inject_ctes(query, &classification, &facts).unwrap();

    if let SqlNode::Query { ctes, .. } = result {
        let ctes = ctes.expect("Expected CTEs to be present");
        assert_eq!(ctes.len(), 1);
        if let SqlNode::CTE { alias, .. } = &ctes[0] {
            assert_eq!(alias.0, "orders_agg");
        } else {
            panic!("Expected CTE");
        }
    } else {
        panic!("Expected Query node at root");
    }
}

#[test]
fn test_inject_ctes_rewrites_outer_query() {
    // Select measure ref: Expr::MeasureRef { entity: "orders", measure: "amount" }
    let select_node = SqlNode::Select(vec![
        Expr::MeasureRef {
            entity: "orders".to_string(),
            measure: "amount".to_string(),
        },
        Expr::Column(ColumnIdent("users.region".to_string())),
    ]);

    // FROM orders JOIN returns ON orders.user_id = returns.user_id
    let join_condition = Expr::BinaryOp {
        left: Box::new(Expr::Column(ColumnIdent("orders.user_id".to_string()))),
        op: "=".to_string(),
        right: Box::new(Expr::Column(ColumnIdent("returns.user_id".to_string()))),
    };
    let join_node = SqlNode::Join {
        join_type: JoinType::Left,
        relation: Box::new(SqlNode::Table(TableIdent("returns".to_string()))),
        on: join_condition,
    };
    let from_node = SqlNode::From {
        source: Box::new(SqlNode::Table(TableIdent("orders".to_string()))),
        joins: vec![join_node],
    };

    let query = SqlNode::Query {
        ctes: None,
        select: Box::new(select_node),
        from: Box::new(from_node),
        r#where: None,
        group_by: None,
        having: None,
    };

    let classification = PathClassification::MultiFactJoin {
        fact_tables: vec!["orders".to_string(), "returns".to_string()],
    };

    let facts = vec![
        FactPreAgg {
            entity: "orders".to_string(),
            table: "orders".to_string(),
            group_key: "user_id".to_string(),
            measures: vec![MeasureProjection {
                name: "amount".to_string(),
                agg: AggregationType::Sum,
                sql: "amount".to_string(),
            }],
        },
        FactPreAgg {
            entity: "returns".to_string(),
            table: "returns".to_string(),
            group_key: "user_id".to_string(),
            measures: vec![],
        },
    ];

    let result = ChasmTrapHandler::inject_ctes(query, &classification, &facts).unwrap();

    if let SqlNode::Query { select, from, .. } = result {
        // Verify SELECT is rewritten from MeasureRef to Column
        if let SqlNode::Select(exprs) = *select {
            assert_eq!(exprs.len(), 2);
            assert_eq!(
                exprs[0],
                Expr::Function {
                    name: "SUM".to_string(),
                    args: vec![Expr::Column(ColumnIdent("orders_agg.amount".to_string()))],
                }
            );
            assert_eq!(
                exprs[1],
                Expr::Column(ColumnIdent("users.region".to_string()))
            );
        } else {
            panic!("Expected Select node");
        }

        // Verify FROM and JOIN tables and ON condition are rewritten
        if let SqlNode::From { source, joins } = *from {
            if let SqlNode::Table(TableIdent(tbl)) = *source {
                assert_eq!(tbl, "orders_agg");
            } else {
                panic!("Expected Table node");
            }

            assert_eq!(joins.len(), 1);
            if let SqlNode::Join {
                join_type,
                relation,
                on,
            } = &joins[0]
            {
                assert_eq!(*join_type, JoinType::Left);
                if let SqlNode::Table(TableIdent(tbl)) = &**relation {
                    assert_eq!(tbl, "returns_agg");
                } else {
                    panic!("Expected Table node");
                }

                assert_eq!(
                    *on,
                    Expr::BinaryOp {
                        left: Box::new(Expr::Column(ColumnIdent("orders_agg.user_id".to_string()))),
                        op: "=".to_string(),
                        right: Box::new(Expr::Column(ColumnIdent(
                            "returns_agg.user_id".to_string()
                        ))),
                    }
                );
            } else {
                panic!("Expected Join node");
            }
        } else {
            panic!("Expected From node");
        }
    } else {
        panic!("Expected Query node at root");
    }
}

#[test]
fn test_rewrite_expr_preserves_column_suffix_sql_engine_rev_1_1_0() {
    let fact = FactPreAgg {
        entity: "orders".to_string(),
        table: "orders".to_string(),
        group_key: "customer_id".to_string(),
        measures: vec![],
    };
    let input = Expr::Column(ColumnIdent("orders.foo".to_string()));
    let result = ChasmTrapHandler::rewrite_expr(input, &fact);
    assert_eq!(
        result,
        Expr::Column(ColumnIdent("orders_agg.foo".to_string()))
    );
}

#[test]
fn test_reaggregate_measure_wrapping_by_type() {
    let select_node = SqlNode::Select(vec![
        Expr::MeasureRef { entity: "orders".to_string(), measure: "count_measure".to_string() },
        Expr::MeasureRef { entity: "orders".to_string(), measure: "sum_measure".to_string() },
        Expr::MeasureRef { entity: "orders".to_string(), measure: "min_measure".to_string() },
        Expr::MeasureRef { entity: "orders".to_string(), measure: "max_measure".to_string() },
    ]);
    let query = SqlNode::Query {
        ctes: None,
        select: Box::new(select_node),
        from: Box::new(SqlNode::From {
            source: Box::new(SqlNode::Table(TableIdent("orders".to_string()))),
            joins: vec![],
        }),
        r#where: None,
        group_by: None,
        having: None,
    };
    let classification = PathClassification::MultiFactJoin {
        fact_tables: vec!["orders".to_string()],
    };
    let facts = vec![FactPreAgg {
        entity: "orders".to_string(),
        table: "orders".to_string(),
        group_key: "customer_id".to_string(),
        measures: vec![
            MeasureProjection {
                name: "count_measure".to_string(),
                agg: AggregationType::Count,
                sql: "order_id".to_string(),
            },
            MeasureProjection {
                name: "sum_measure".to_string(),
                agg: AggregationType::Sum,
                sql: "amount".to_string(),
            },
            MeasureProjection {
                name: "min_measure".to_string(),
                agg: AggregationType::Min,
                sql: "amount".to_string(),
            },
            MeasureProjection {
                name: "max_measure".to_string(),
                agg: AggregationType::Max,
                sql: "amount".to_string(),
            },
        ],
    }];
    let result = ChasmTrapHandler::inject_ctes(query, &classification, &facts).unwrap();
    if let SqlNode::Query { select, .. } = result {
        if let SqlNode::Select(exprs) = *select {
            assert_eq!(exprs.len(), 4);
            // Count -> SUM
            assert_eq!(
                exprs[0],
                Expr::Function {
                    name: "SUM".to_string(),
                    args: vec![Expr::Column(ColumnIdent("orders_agg.order_id".to_string()))],
                }
            );
            // Sum -> SUM
            assert_eq!(
                exprs[1],
                Expr::Function {
                    name: "SUM".to_string(),
                    args: vec![Expr::Column(ColumnIdent("orders_agg.amount".to_string()))],
                }
            );
            // Min -> MIN
            assert_eq!(
                exprs[2],
                Expr::Function {
                    name: "MIN".to_string(),
                    args: vec![Expr::Column(ColumnIdent("orders_agg.amount".to_string()))],
                }
            );
            // Max -> MAX
            assert_eq!(
                exprs[3],
                Expr::Function {
                    name: "MAX".to_string(),
                    args: vec![Expr::Column(ColumnIdent("orders_agg.amount".to_string()))],
                }
            );
        } else {
            panic!("Expected Select node");
        }
    } else {
        panic!("Expected Query node");
    }
}

#[test]
fn test_rewrite_dimension_ref_repoints_to_group_key() {
    let fact = FactPreAgg {
        entity: "orders".to_string(),
        table: "orders".to_string(),
        group_key: "customer_id".to_string(),
        measures: vec![],
    };
    let input = Expr::DimensionRef {
        entity: "orders".to_string(),
        dimension: "customer_id".to_string(),
    };
    let result = ChasmTrapHandler::rewrite_expr(input, &fact);
    assert_eq!(
        result,
        Expr::Column(ColumnIdent("orders_agg.customer_id".to_string()))
    );
}

fn get_test_chasm_trap_state(measure_agg: &str, dimensions: &[&str]) -> std::sync::Arc<may_core::models::SemanticState> {
    use may_core::models::{SemanticModel, SemanticState};
    let mut state = SemanticState::new();
    let model_content = format!(
        r#"
name: chasm_trap_unit_model
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
        agg: {}
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
  - name: test_metric
    measure: amount
    dimensions: {:?}
"#,
        measure_agg, dimensions
    );
    let model: SemanticModel = serde_norway::from_str(&model_content).expect("parse model");
    state.models.insert(model.name.clone(), model);
    std::sync::Arc::new(state)
}

#[test]
fn test_compiler_rejects_average_in_chasm_trap() {
    use may_core::compiler::{SemanticCompiler, SemanticRequest, CompilerError};
    use may_core::dialects::PostgresDialect;
    let state = get_test_chasm_trap_state("average", &["region", "return_customer_id"]);
    let compiler = SemanticCompiler::new(state, Box::new(PostgresDialect));
    let request = SemanticRequest {
        metric_name: "test_metric".to_string(),
        dimensions: vec![],
        filters: vec![],
        time_granularity: None,
        limit: None,
    };
    let result = compiler.compile(request, None);
    assert!(
        matches!(
            result,
            Err(CompilerError::ChasmTrapHandlingFailed(ChasmTrapError::UnsupportedAverageAggregation))
        ),
        "Expected ChasmTrapError::UnsupportedAverageAggregation, got {:?}",
        result
    );
}

#[test]
fn test_compiler_rejects_finer_grain_fact_dimension() {
    use may_core::compiler::{SemanticCompiler, SemanticRequest, CompilerError};
    use may_core::dialects::PostgresDialect;
    let state = get_test_chasm_trap_state("sum", &["region", "return_id"]);
    let compiler = SemanticCompiler::new(state, Box::new(PostgresDialect));
    let request = SemanticRequest {
        metric_name: "test_metric".to_string(),
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
        "Expected ChasmTrapError::FinerThanConformedGrain, got {:?}",
        result
    );
    if let Err(CompilerError::ChasmTrapHandlingFailed(ChasmTrapError::FinerThanConformedGrain { dimension, entity, conformed_grain })) = result {
        assert_eq!(dimension, "return_id");
        assert_eq!(entity, "returns");
        assert_eq!(conformed_grain, "customer_id");
    }
}
