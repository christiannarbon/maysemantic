#[cfg(test)]
mod join_builder_tests {
    use may_core::ast::builder::{build_from_join_path, build_join};
    use may_core::ast::{ColumnIdent, Expr, JoinType, SqlNode, TableIdent};
    use may_core::compiler::ResolvedJoin;
    use may_core::graph::{GraphEdge, GraphNode};
    use may_core::{PostgresDialect, SqlDialect};

    fn render(node: &SqlNode) -> String {
        PostgresDialect
            .generate_sql(node)
            .expect("should generate SQL")
    }

    fn orders_node() -> GraphNode {
        GraphNode {
            entity_name: "orders".to_string(),
            table_name: "orders".to_string(),
            primary_key: "id".to_string(),
        }
    }

    fn users_node() -> GraphNode {
        GraphNode {
            entity_name: "users".to_string(),
            table_name: "users".to_string(),
            primary_key: "id".to_string(),
        }
    }

    fn teams_node() -> GraphNode {
        GraphNode {
            entity_name: "teams".to_string(),
            table_name: "teams".to_string(),
            primary_key: "id".to_string(),
        }
    }

    fn left_join(left_col: &str, right_col: &str) -> GraphEdge {
        GraphEdge {
            left_column: left_col.to_string(),
            right_column: right_col.to_string(),
            join_type: JoinType::Left,
        }
    }

    fn inner_join(left_col: &str, right_col: &str) -> GraphEdge {
        GraphEdge {
            left_column: left_col.to_string(),
            right_column: right_col.to_string(),
            join_type: JoinType::Inner,
        }
    }

    #[test]
    fn test_build_join_correct_structure() {
        let resolved_join = ResolvedJoin {
            left_table: orders_node(),
            right_table: users_node(),
            edge: left_join("user_id", "id"),
        };
        let result = build_join(&resolved_join);

        match result {
            SqlNode::Join {
                join_type,
                relation,
                ..
            } => {
                assert_eq!(join_type, JoinType::Left);
                assert_eq!(*relation, SqlNode::Table(TableIdent("users".to_string())));
            }
            _ => panic!("Expected SqlNode::Join"),
        }
    }

    #[test]
    fn test_on_clause_is_fully_qualified() {
        let resolved_join = ResolvedJoin {
            left_table: orders_node(),
            right_table: users_node(),
            edge: left_join("user_id", "id"),
        };
        let result = build_join(&resolved_join);

        match result {
            SqlNode::Join { on, .. } => match on {
                Expr::BinaryOp { left, right, .. } => {
                    assert_eq!(
                        *left,
                        Expr::Column(ColumnIdent("orders.user_id".to_string()))
                    );
                    assert_eq!(*right, Expr::Column(ColumnIdent("users.id".to_string())));
                }
                _ => panic!("Expected Expr::BinaryOp"),
            },
            _ => panic!("Expected SqlNode::Join"),
        }
    }

    #[test]
    fn test_build_from_single_hop() {
        let resolved_join_orders_to_users = ResolvedJoin {
            left_table: orders_node(),
            right_table: users_node(),
            edge: left_join("user_id", "id"),
        };
        let result = build_from_join_path(&orders_node(), &[resolved_join_orders_to_users]);

        match &result {
            SqlNode::From { source, joins } => {
                assert_eq!(**source, SqlNode::Table(TableIdent("orders".to_string())));
                assert_eq!(joins.len(), 1);
            }
            _ => panic!("Expected SqlNode::From"),
        }

        let sql = render(&result);
        assert_eq!(
            sql,
            "FROM \"orders\" LEFT JOIN \"users\" ON \"orders\".\"user_id\" = \"users\".\"id\""
        );
    }

    #[test]
    fn test_build_join_inner_type() {
        let resolved_join = ResolvedJoin {
            left_table: orders_node(),
            right_table: users_node(),
            edge: inner_join("user_id", "id"),
        };
        let result = build_join(&resolved_join);

        let sql = render(&result);
        assert!(sql.starts_with("INNER JOIN \"users\" ON "));
    }

    #[test]
    fn test_build_from_two_hops() {
        let hop1 = ResolvedJoin {
            left_table: orders_node(),
            right_table: users_node(),
            edge: left_join("user_id", "id"),
        };
        let hop2 = ResolvedJoin {
            left_table: users_node(),
            right_table: teams_node(),
            edge: left_join("team_id", "id"),
        };
        let result = build_from_join_path(&orders_node(), &[hop1, hop2]);

        match result {
            SqlNode::From { joins, .. } => {
                assert_eq!(joins.len(), 2);

                match &joins[0] {
                    SqlNode::Join { relation, .. } => {
                        assert_eq!(**relation, SqlNode::Table(TableIdent("users".to_string())));
                    }
                    _ => panic!("Expected first join to be SqlNode::Join"),
                }

                match &joins[1] {
                    SqlNode::Join { relation, .. } => {
                        assert_eq!(**relation, SqlNode::Table(TableIdent("teams".to_string())));
                    }
                    _ => panic!("Expected second join to be SqlNode::Join"),
                }
            }
            _ => panic!("Expected SqlNode::From"),
        }
    }

    #[test]
    fn test_build_from_empty_joins() {
        let result = build_from_join_path(&orders_node(), &[]);

        match result {
            SqlNode::From { source, joins } => {
                assert_eq!(*source, SqlNode::Table(TableIdent("orders".to_string())));
                assert!(joins.is_empty());
            }
            _ => panic!("Expected SqlNode::From"),
        }
    }
}
