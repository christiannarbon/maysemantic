use may_core::ast::{ColumnIdent, Expr, SqlNode, TableIdent};
use may_core::compiler::{ChasmTrapError, ChasmTrapHandler, PathClassification};

#[test]
fn test_inject_ctes_single_fact() {
    let query = SqlNode::Table(TableIdent("orders".to_string()));
    let classification = PathClassification::SingleFact;
    let result = ChasmTrapHandler::inject_ctes(query.clone(), &classification, "user_id");
    assert_eq!(result.unwrap(), query);
}

#[test]
fn test_inject_ctes_pure_dimension() {
    let query = SqlNode::Table(TableIdent("users".to_string()));
    let classification = PathClassification::PureDimension;
    let result = ChasmTrapHandler::inject_ctes(query.clone(), &classification, "user_id");
    assert_eq!(result.unwrap(), query);
}

#[test]
fn test_inject_ctes_empty_fact_tables() {
    let query = SqlNode::Table(TableIdent("orders".to_string()));
    let classification = PathClassification::MultiFactJoin {
        fact_tables: vec![],
    };
    let result = ChasmTrapHandler::inject_ctes(query, &classification, "user_id");
    assert_eq!(result.unwrap_err(), ChasmTrapError::EmptyFactTableList);
}

#[test]
fn test_inject_ctes_not_a_query_node() {
    let query = SqlNode::Table(TableIdent("orders".to_string()));
    let classification = PathClassification::MultiFactJoin {
        fact_tables: vec!["orders".to_string(), "returns".to_string()],
    };
    let result = ChasmTrapHandler::inject_ctes(query, &classification, "user_id");
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

    let result = ChasmTrapHandler::inject_ctes(query, &classification, "user_id").unwrap();

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
                    assert_eq!(proj.len(), 1);
                    assert_eq!(proj[0], Expr::Column(ColumnIdent("user_id".to_string())));
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
