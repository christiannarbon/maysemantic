#[cfg(test)]
mod join_builder_tests {
    use may_core::ast::{ColumnIdent, Expr, JoinType, SqlNode, TableIdent};
    use may_core::ast::builder::{build_from_join_path, build_join};
    use may_core::compiler::ResolvedJoin;
    use may_core::graph::{GraphEdge, GraphNode};
    use may_core::{PostgresDialect, SqlDialect};

    fn make_orders_node() -> GraphNode {
        GraphNode {
            entity_name: "orders".to_string(),
            table_name: "orders_tbl".to_string(),
            primary_key: "id".to_string(),
        }
    }

    fn make_users_node() -> GraphNode {
        GraphNode {
            entity_name: "users".to_string(),
            table_name: "users_tbl".to_string(),
            primary_key: "id".to_string(),
        }
    }

    fn make_teams_node() -> GraphNode {
        GraphNode {
            entity_name: "teams".to_string(),
            table_name: "teams_tbl".to_string(),
            primary_key: "id".to_string(),
        }
    }

    fn make_resolved_join() -> ResolvedJoin {
        ResolvedJoin {
            left_table: make_orders_node(),
            right_table: make_users_node(),
            edge: GraphEdge {
                left_column: "user_id".to_string(),
                right_column: "id".to_string(),
                join_type: JoinType::Left,
            },
        }
    }

    #[test]
    fn test_build_join_produces_correct_sql() {
        let join_hop = make_resolved_join();
        let join_node = build_join(&join_hop);

        match join_node {
            SqlNode::Join {
                join_type,
                relation,
                on,
            } => {
                assert_eq!(join_type, JoinType::Left);
                assert_eq!(
                    *relation,
                    SqlNode::Table(TableIdent("users_tbl".to_string()))
                );
                
                match on {
                    Expr::BinaryOp { left, op, right } => {
                        assert_eq!(op, "=");
                        assert_eq!(*left, Expr::Column(ColumnIdent("orders_tbl.user_id".to_string())));
                        assert_eq!(*right, Expr::Column(ColumnIdent("users_tbl.id".to_string())));
                    }
                    _ => panic!("Expected Expr::BinaryOp"),
                }
            }
            _ => panic!("Expected SqlNode::Join"),
        }
    }

    #[test]
    fn test_build_from_single_hop() {
        let join_hop = make_resolved_join();
        let from_node = build_from_join_path(&make_orders_node(), &[join_hop]);

        let query = SqlNode::Query {
            ctes: None,
            select: Box::new(SqlNode::Select(vec![Expr::Raw("1".to_string())])), // dummy
            from: Box::new(from_node),
            r#where: None,
            group_by: None,
            having: None,
        };

        let sql = PostgresDialect.generate_sql(&query).unwrap();
        // check it contains the join string
        assert!(sql.contains("FROM orders_tbl"));
        assert!(sql.contains("LEFT JOIN users_tbl ON orders_tbl.user_id = users_tbl.id"));
    }

    #[test]
    fn test_build_from_two_hops() {
        let hop1 = make_resolved_join(); // orders -> users
        let hop2 = ResolvedJoin {
            left_table: make_users_node(),
            right_table: make_teams_node(),
            edge: GraphEdge {
                left_column: "team_id".to_string(),
                right_column: "id".to_string(),
                join_type: JoinType::Inner,
            },
        };

        let from_node = build_from_join_path(&make_orders_node(), &[hop1, hop2]);

        let query = SqlNode::Query {
            ctes: None,
            select: Box::new(SqlNode::Select(vec![Expr::Raw("1".to_string())])),
            from: Box::new(from_node),
            r#where: None,
            group_by: None,
            having: None,
        };

        let sql = PostgresDialect.generate_sql(&query).unwrap();
        
        assert!(sql.contains("FROM orders_tbl"));
        assert!(sql.contains("LEFT JOIN users_tbl ON orders_tbl.user_id = users_tbl.id"));
        assert!(sql.contains("INNER JOIN teams_tbl ON users_tbl.team_id = teams_tbl.id"));
    }

    #[test]
    fn test_build_from_empty_joins() {
        let from_node = build_from_join_path(&make_orders_node(), &[]);

        let query = SqlNode::Query {
            ctes: None,
            select: Box::new(SqlNode::Select(vec![Expr::Raw("1".to_string())])),
            from: Box::new(from_node),
            r#where: None,
            group_by: None,
            having: None,
        };

        let sql = PostgresDialect.generate_sql(&query).unwrap();
        assert!(sql.contains("FROM orders_tbl"));
        assert!(!sql.contains("JOIN"));
    }
}
