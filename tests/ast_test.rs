use maysemantic::SqlNode;

#[test]
fn test_ast_recursion_model() {
    // Construct a nested AST representing:
    // SELECT user_id, raw_data FROM users WHERE id = 1

    let select_node = SqlNode::Select(vec![
        SqlNode::Column {
            name: "user_id".to_string(),
        },
        SqlNode::Raw("raw_data".to_string()),
    ]);

    let from_node = SqlNode::From(Box::new(SqlNode::Table {
        name: "users".to_string(),
    }));

    let where_condition = SqlNode::BinaryOp {
        left: Box::new(SqlNode::Column {
            name: "id".to_string(),
        }),
        op: "=".to_string(),
        right: Box::new(SqlNode::Raw("1".to_string())),
    };

    let where_node = Some(Box::new(SqlNode::Where(Box::new(where_condition))));

    let ast = SqlNode::Query {
        select: Box::new(select_node),
        from: Box::new(from_node),
        r#where: where_node,
        group_by: None,
    };

    // Validate structure through basic pattern matching
    match ast {
        SqlNode::Query {
            select,
            from,
            r#where,
            group_by,
        } => {
            // Validate Select
            if let SqlNode::Select(projection) = *select {
                assert_eq!(projection.len(), 2);
            } else {
                panic!("Expected Select node");
            }

            // Validate inner recursive 'From' node
            if let SqlNode::From(inner_from) = *from {
                if let SqlNode::Table { name } = *inner_from {
                    assert_eq!(name, "users");
                } else {
                    panic!("Expected Table node inside FROM clause");
                }
            } else {
                panic!("Expected From node");
            }

            // Validate optional inner 'Where' node and BinaryOp
            let where_outer = *r#where.expect("Expected WHERE clause");
            if let SqlNode::Where(inner_where) = where_outer {
                if let SqlNode::BinaryOp { left, op, right } = *inner_where {
                    assert_eq!(op, "=");
                    if let SqlNode::Column { name } = *left {
                        assert_eq!(name, "id");
                    } else {
                        panic!("Expected Column left operand");
                    }
                    if let SqlNode::Raw(val) = *right {
                        assert_eq!(val, "1");
                    } else {
                        panic!("Expected Raw right operand");
                    }
                } else {
                    panic!("Expected BinaryOp inside WHERE clause");
                }
            } else {
                panic!("Expected Where node");
            }

            assert!(group_by.is_none());
        }
        _ => panic!("Expected Query node at root"),
    }
}
