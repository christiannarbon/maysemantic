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
        select: Box::new(select_node),
        from: Box::new(from_node),
        r#where: where_node,
        group_by: group_by_node,
        having: having_node,
    };

    // Validate structure through basic pattern matching
    match ast {
        SqlNode::Query {
            select,
            from,
            r#where,
            group_by,
            having,
        } => {
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
