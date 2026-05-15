use maysemantic::ast::{ColumnIdent, Expr, JoinType, SqlNode, TableIdent};

#[test]
fn test_ast_recursion_model() {
    let select_node = SqlNode::Select(vec![
        Expr::Column(ColumnIdent("user_id".to_string())),
        Expr::Function {
            name: "SUM".to_string(),
            args: vec![Expr::Column(ColumnIdent("amount".to_string()))],
        },
    ]);

    let base_table = SqlNode::Table(TableIdent("users".to_string()));

    let join_condition = Expr::BinaryOp {
        left: Box::new(Expr::Column(ColumnIdent("users.id".to_string()))),
        op: "=".to_string(),
        right: Box::new(Expr::Column(ColumnIdent("orders.user_id".to_string()))),
    };

    let join_node = SqlNode::Join {
        join_type: JoinType::Left,
        relation: Box::new(SqlNode::Table(TableIdent("orders".to_string()))),
        on: join_condition,
    };

    let from_node = SqlNode::From {
        source: Box::new(base_table),
        joins: vec![join_node],
    };

    let where_condition = Expr::BinaryOp {
        left: Box::new(Expr::Column(ColumnIdent("users.is_active".to_string()))),
        op: "=".to_string(),
        right: Box::new(Expr::Raw("1".to_string())),
    };

    let where_node = Some(Box::new(SqlNode::Where(where_condition)));

    let group_by_node = Some(Box::new(SqlNode::GroupBy(vec![Expr::Column(ColumnIdent(
        "user_id".to_string(),
    ))])));

    let having_condition = Expr::BinaryOp {
        left: Box::new(Expr::Function {
            name: "SUM".to_string(),
            args: vec![Expr::Column(ColumnIdent("amount".to_string()))],
        }),
        op: ">".to_string(),
        right: Box::new(Expr::Raw("100".to_string())),
    };

    let having_node = Some(Box::new(SqlNode::Having(having_condition)));

    let ast = SqlNode::Query {
        ctes: None,
        select: Box::new(select_node),
        from: Box::new(from_node),
        r#where: where_node,
        group_by: group_by_node,
        having: having_node,
    };

    match ast {
        SqlNode::Query {
            ctes,
            select,
            from,
            r#where,
            group_by,
            having,
        } => {
            assert!(ctes.is_none());

            if let SqlNode::Select(projection) = *select {
                assert_eq!(projection.len(), 2);
                if let Expr::Function { name, args } = &projection[1] {
                    assert_eq!(name, "SUM");
                    assert_eq!(args.len(), 1);
                } else {
                    panic!("Expected Function node in projection");
                }
            } else {
                panic!("Expected Select node");
            }

            if let SqlNode::From { source, joins } = *from {
                if let SqlNode::Table(TableIdent(name)) = *source {
                    assert_eq!(name, "users");
                } else {
                    panic!("Expected Table node inside FROM source");
                }

                assert_eq!(joins.len(), 1);
                if let SqlNode::Join {
                    join_type,
                    relation,
                    on,
                } = &joins[0]
                {
                    assert_eq!(*join_type, JoinType::Left);
                    if let SqlNode::Table(TableIdent(name)) = &**relation {
                        assert_eq!(name, "orders");
                    } else {
                        panic!("Expected Table in join relation");
                    }
                    if let Expr::BinaryOp { op, .. } = on {
                        assert_eq!(op, "=");
                    } else {
                        panic!("Expected BinaryOp in join condition");
                    }
                } else {
                    panic!("Expected Join node");
                }
            } else {
                panic!("Expected From node");
            }

            assert!(r#where.is_some());

            let group_outer = *group_by.expect("Expected GROUP BY clause");
            if let SqlNode::GroupBy(cols) = group_outer {
                assert_eq!(cols.len(), 1);
            } else {
                panic!("Expected GroupBy node");
            }

            let having_outer = *having.expect("Expected HAVING clause");
            if let SqlNode::Having(inner_having) = having_outer {
                if let Expr::BinaryOp { left, op, .. } = inner_having {
                    assert_eq!(op, ">");
                    if let Expr::Function { name, .. } = *left {
                        assert_eq!(name, "SUM");
                    } else {
                        panic!("Expected Function in HAVING left operand");
                    }
                } else {
                    panic!("Expected BinaryOp inside HAVING clause");
                }
            } else {
                panic!("Expected Having node");
            }
        }
        _ => panic!("Expected Query node at root"),
    }
}

#[test]
fn test_ast_cte_model() {
    let inner_select = SqlNode::Select(vec![
        Expr::Column(ColumnIdent("user_id".to_string())),
        Expr::Function {
            name: "SUM".to_string(),
            args: vec![Expr::Column(ColumnIdent("amount".to_string()))],
        },
    ]);

    let inner_from = SqlNode::From {
        source: Box::new(SqlNode::Table(TableIdent("orders".to_string()))),
        joins: vec![],
    };

    let inner_group_by = Some(Box::new(SqlNode::GroupBy(vec![Expr::Column(ColumnIdent(
        "user_id".to_string(),
    ))])));

    let inner_query = SqlNode::Query {
        ctes: None,
        select: Box::new(inner_select),
        from: Box::new(inner_from),
        r#where: None,
        group_by: inner_group_by,
        having: None,
    };

    let cte_node = SqlNode::CTE {
        alias: TableIdent("agg_orders".to_string()),
        query: Box::new(inner_query),
    };

    let outer_select = SqlNode::Select(vec![Expr::Column(ColumnIdent("user_id".to_string()))]);

    let outer_from = SqlNode::From {
        source: Box::new(SqlNode::Table(TableIdent("agg_orders".to_string()))),
        joins: vec![],
    };

    let outer_query = SqlNode::Query {
        ctes: Some(vec![cte_node]),
        select: Box::new(outer_select),
        from: Box::new(outer_from),
        r#where: None,
        group_by: None,
        having: None,
    };

    match outer_query {
        SqlNode::Query {
            ctes, select, from, ..
        } => {
            let ctes = ctes.expect("Expected CTEs in outer query");
            assert_eq!(ctes.len(), 1);

            if let SqlNode::CTE { alias, query } = &ctes[0] {
                assert_eq!(alias.0, "agg_orders");
                if let SqlNode::Query {
                    select: inner_sel, ..
                } = &**query
                {
                    if let SqlNode::Select(proj) = &**inner_sel {
                        assert_eq!(proj.len(), 2);
                    } else {
                        panic!("Expected Select node in inner query");
                    }
                } else {
                    panic!("Expected Query node inside CTE");
                }
            } else {
                panic!("Expected CTE node");
            }

            if let SqlNode::Select(proj) = *select {
                assert_eq!(proj.len(), 1);
            } else {
                panic!("Expected Select node in outer query");
            }

            if let SqlNode::From { source, .. } = *from {
                if let SqlNode::Table(TableIdent(name)) = *source {
                    assert_eq!(name, "agg_orders");
                } else {
                    panic!("Expected Table node inside FROM source");
                }
            } else {
                panic!("Expected From node in outer query");
            }
        }
        _ => panic!("Expected Query node at root"),
    }
}

#[test]
fn test_ast_semantic_model() {
    let select_node = SqlNode::Select(vec![
        Expr::DimensionRef {
            entity: "locations".to_string(),
            dimension: "region".to_string(),
        },
        Expr::MeasureRef {
            entity: "orders".to_string(),
            measure: "revenue".to_string(),
        },
    ]);

    let from_node = SqlNode::From {
        source: Box::new(SqlNode::TimeSpine {
            granularity: "day".to_string(),
        }),
        joins: vec![],
    };

    let group_by_node = Some(Box::new(SqlNode::GroupBy(vec![Expr::DimensionRef {
        entity: "locations".to_string(),
        dimension: "region".to_string(),
    }])));

    let ast = SqlNode::Query {
        ctes: None,
        select: Box::new(select_node),
        from: Box::new(from_node),
        r#where: None,
        group_by: group_by_node,
        having: None,
    };

    match ast {
        SqlNode::Query {
            select,
            from,
            group_by,
            ..
        } => {
            if let SqlNode::Select(projection) = *select {
                assert_eq!(projection.len(), 2);

                if let Expr::DimensionRef { entity, dimension } = &projection[0] {
                    assert_eq!(entity, "locations");
                    assert_eq!(dimension, "region");
                } else {
                    panic!("Expected DimensionRef node in projection");
                }

                if let Expr::MeasureRef { entity, measure } = &projection[1] {
                    assert_eq!(entity, "orders");
                    assert_eq!(measure, "revenue");
                } else {
                    panic!("Expected MeasureRef node in projection");
                }
            } else {
                panic!("Expected Select node");
            }

            if let SqlNode::From { source, .. } = *from {
                if let SqlNode::TimeSpine { granularity } = *source {
                    assert_eq!(granularity, "day");
                } else {
                    panic!("Expected TimeSpine node inside FROM source");
                }
            } else {
                panic!("Expected From node");
            }

            let group_outer = *group_by.expect("Expected GROUP BY clause");
            if let SqlNode::GroupBy(cols) = group_outer {
                assert_eq!(cols.len(), 1);
                if let Expr::DimensionRef { entity, dimension } = &cols[0] {
                    assert_eq!(entity, "locations");
                    assert_eq!(dimension, "region");
                } else {
                    panic!("Expected DimensionRef inside GroupBy");
                }
            } else {
                panic!("Expected GroupBy node");
            }
        }
        _ => panic!("Expected Query node at root"),
    }
}
