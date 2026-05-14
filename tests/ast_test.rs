use maysemantic::{JoinType, SqlNode};

#[test]
fn test_ast_recursion_model() {
    // Construct a nested AST representing:
    // SELECT user_id, SUM(amount)
    // FROM users
    // LEFT JOIN orders ON users.id = orders.user_id
    // WHERE users.is_active = 1
    // GROUP BY user_id
    // HAVING SUM(amount) > 100

    let select_node = SqlNode::Select(vec![
        SqlNode::Column {
            name: "user_id".to_string(),
        },
        SqlNode::Function {
            name: "SUM".to_string(),
            args: vec![SqlNode::Column {
                name: "amount".to_string(),
            }],
        },
    ]);

    let base_table = SqlNode::Table {
        name: "users".to_string(),
    };

    let join_condition = SqlNode::BinaryOp {
        left: Box::new(SqlNode::Column {
            name: "users.id".to_string(),
        }),
        op: "=".to_string(),
        right: Box::new(SqlNode::Column {
            name: "orders.user_id".to_string(),
        }),
    };

    let join_node = SqlNode::Join {
        join_type: JoinType::Left,
        relation: Box::new(SqlNode::Table {
            name: "orders".to_string(),
        }),
        on: Box::new(join_condition),
    };

    let from_node = SqlNode::From {
        source: Box::new(base_table),
        joins: vec![join_node],
    };

    let where_condition = SqlNode::BinaryOp {
        left: Box::new(SqlNode::Column {
            name: "users.is_active".to_string(),
        }),
        op: "=".to_string(),
        right: Box::new(SqlNode::Raw("1".to_string())),
    };

    let where_node = Some(Box::new(SqlNode::Where(Box::new(where_condition))));

    let group_by_node = Some(Box::new(SqlNode::GroupBy(vec![SqlNode::Column {
        name: "user_id".to_string(),
    }])));

    let having_condition = SqlNode::BinaryOp {
        left: Box::new(SqlNode::Function {
            name: "SUM".to_string(),
            args: vec![SqlNode::Column {
                name: "amount".to_string(),
            }],
        }),
        op: ">".to_string(),
        right: Box::new(SqlNode::Raw("100".to_string())),
    };

    let having_node = Some(Box::new(SqlNode::Having(Box::new(having_condition))));

    let ast = SqlNode::Query {
        ctes: None,
        select: Box::new(select_node),
        from: Box::new(from_node),
        r#where: where_node,
        group_by: group_by_node,
        having: having_node,
    };

    // Validate structure through basic pattern matching
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

            // Validate Select
            if let SqlNode::Select(projection) = *select {
                assert_eq!(projection.len(), 2);
                if let SqlNode::Function { name, args } = &projection[1] {
                    assert_eq!(name, "SUM");
                    assert_eq!(args.len(), 1);
                } else {
                    panic!("Expected Function node in projection");
                }
            } else {
                panic!("Expected Select node");
            }

            // Validate inner recursive 'From' and 'Joins'
            if let SqlNode::From { source, joins } = *from {
                if let SqlNode::Table { name } = *source {
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
                    if let SqlNode::Table { name } = &**relation {
                        assert_eq!(name, "orders");
                    } else {
                        panic!("Expected Table in join relation");
                    }
                    if let SqlNode::BinaryOp { op, .. } = &**on {
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

            // Validate optional inner 'Where' node
            assert!(r#where.is_some());

            // Validate GroupBy
            let group_outer = *group_by.expect("Expected GROUP BY clause");
            if let SqlNode::GroupBy(cols) = group_outer {
                assert_eq!(cols.len(), 1);
            } else {
                panic!("Expected GroupBy node");
            }

            // Validate Having
            let having_outer = *having.expect("Expected HAVING clause");
            if let SqlNode::Having(inner_having) = having_outer {
                if let SqlNode::BinaryOp { left, op, right: _ } = *inner_having {
                    assert_eq!(op, ">");
                    if let SqlNode::Function { name, .. } = *left {
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
    // Construct a nested AST representing:
    // WITH agg_orders AS (
    //     SELECT user_id, SUM(amount) FROM orders GROUP BY user_id
    // )
    // SELECT user_id FROM agg_orders

    let inner_select = SqlNode::Select(vec![
        SqlNode::Column {
            name: "user_id".to_string(),
        },
        SqlNode::Function {
            name: "SUM".to_string(),
            args: vec![SqlNode::Column {
                name: "amount".to_string(),
            }],
        },
    ]);

    let inner_from = SqlNode::From {
        source: Box::new(SqlNode::Table {
            name: "orders".to_string(),
        }),
        joins: vec![],
    };

    let inner_group_by = Some(Box::new(SqlNode::GroupBy(vec![SqlNode::Column {
        name: "user_id".to_string(),
    }])));

    let inner_query = SqlNode::Query {
        ctes: None,
        select: Box::new(inner_select),
        from: Box::new(inner_from),
        r#where: None,
        group_by: inner_group_by,
        having: None,
    };

    let cte_node = SqlNode::CTE {
        alias: "agg_orders".to_string(),
        query: Box::new(inner_query),
    };

    let outer_select = SqlNode::Select(vec![SqlNode::Column {
        name: "user_id".to_string(),
    }]);

    let outer_from = SqlNode::From {
        source: Box::new(SqlNode::Table {
            name: "agg_orders".to_string(),
        }),
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

    // Validate
    match outer_query {
        SqlNode::Query {
            ctes, select, from, ..
        } => {
            // Validate CTE
            let ctes = ctes.expect("Expected CTEs in outer query");
            assert_eq!(ctes.len(), 1);

            if let SqlNode::CTE { alias, query } = &ctes[0] {
                assert_eq!(alias, "agg_orders");
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

            // Validate Outer Select
            if let SqlNode::Select(proj) = *select {
                assert_eq!(proj.len(), 1);
            } else {
                panic!("Expected Select node in outer query");
            }

            // Validate Outer From points to CTE alias
            if let SqlNode::From { source, .. } = *from {
                if let SqlNode::Table { name } = *source {
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
    // Construct a semantic AST representing:
    // Revenue by Region
    // This is NOT physical SQL yet. It's the semantic interpretation of the user's intent.

    let select_node = SqlNode::Select(vec![
        SqlNode::DimensionRef {
            entity: "locations".to_string(),
            dimension: "region".to_string(),
        },
        SqlNode::MeasureRef {
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

    let group_by_node = Some(Box::new(SqlNode::GroupBy(vec![SqlNode::DimensionRef {
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

    // Validate structure
    match ast {
        SqlNode::Query {
            select,
            from,
            group_by,
            ..
        } => {
            // Validate Select
            if let SqlNode::Select(projection) = *select {
                assert_eq!(projection.len(), 2);

                if let SqlNode::DimensionRef { entity, dimension } = &projection[0] {
                    assert_eq!(entity, "locations");
                    assert_eq!(dimension, "region");
                } else {
                    panic!("Expected DimensionRef node in projection");
                }

                if let SqlNode::MeasureRef { entity, measure } = &projection[1] {
                    assert_eq!(entity, "orders");
                    assert_eq!(measure, "revenue");
                } else {
                    panic!("Expected MeasureRef node in projection");
                }
            } else {
                panic!("Expected Select node");
            }

            // Validate From
            if let SqlNode::From { source, .. } = *from {
                if let SqlNode::TimeSpine { granularity } = *source {
                    assert_eq!(granularity, "day");
                } else {
                    panic!("Expected TimeSpine node inside FROM source");
                }
            } else {
                panic!("Expected From node");
            }

            // Validate GroupBy
            let group_outer = *group_by.expect("Expected GROUP BY clause");
            if let SqlNode::GroupBy(cols) = group_outer {
                assert_eq!(cols.len(), 1);
                if let SqlNode::DimensionRef { entity, dimension } = &cols[0] {
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
