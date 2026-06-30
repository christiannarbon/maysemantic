use may_core::ast::{ColumnIdent, Expr, JoinType, SqlNode, TableIdent};
use may_core::BigQueryDialect;
use may_core::DialectError;
use may_core::SqlDialect;

#[test]
fn test_bigquery_dialect_generates_basic_select() {
    let ast = SqlNode::Query {
        ctes: None,
        select: Box::new(SqlNode::Select(vec![
            Expr::Column(ColumnIdent("users.id".to_string())),
            Expr::Column(ColumnIdent("users.name".to_string())),
        ])),
        from: Box::new(SqlNode::From {
            source: Box::new(SqlNode::Table(TableIdent("public.users".to_string()))),
            joins: vec![],
        }),
        r#where: Some(Box::new(SqlNode::Where(Expr::BinaryOp {
            left: Box::new(Expr::Column(ColumnIdent("users.status".to_string()))),
            op: "=".to_string(),
            right: Box::new(Expr::Literal("'active'".to_string())),
        }))),
        group_by: None,
        having: None,
        order_by: vec![],
        limit: None,
        offset: None,
    };

    let dialect = BigQueryDialect;
    let sql = dialect.generate_sql(&ast).expect("SQL generation failed");
    // Table/column identifiers are currently written raw from ast.rs, so they don't get backticks automatically here.
    // However, BigQuery uses backticks in CTE aliases and specific functions like write_date_trunc.
    assert_eq!(
        sql,
        "SELECT `users`.`id`, `users`.`name` FROM `public`.`users` WHERE `users`.`status` = 'active'"
    );
}

#[test]
fn test_bigquery_dialect_generates_joins() {
    let ast = SqlNode::Query {
        ctes: None,
        select: Box::new(SqlNode::Select(vec![
            Expr::Column(ColumnIdent("orders.amount".to_string())),
            Expr::Column(ColumnIdent("users.name".to_string())),
        ])),
        from: Box::new(SqlNode::From {
            source: Box::new(SqlNode::Table(TableIdent("orders".to_string()))),
            joins: vec![SqlNode::Join {
                join_type: JoinType::Left,
                relation: Box::new(SqlNode::Table(TableIdent("users".to_string()))),
                on: Expr::BinaryOp {
                    left: Box::new(Expr::Column(ColumnIdent("orders.user_id".to_string()))),
                    op: "=".to_string(),
                    right: Box::new(Expr::Column(ColumnIdent("users.id".to_string()))),
                },
            }],
        }),
        r#where: None,
        group_by: None,
        having: None,
        order_by: vec![],
        limit: None,
        offset: None,
    };

    let dialect = BigQueryDialect;
    let sql = dialect.generate_sql(&ast).expect("SQL generation failed");
    assert_eq!(
        sql,
        "SELECT `orders`.`amount`, `users`.`name` FROM `orders` LEFT JOIN `users` ON `orders`.`user_id` = `users`.`id`"
    );
}

#[test]
fn test_bigquery_dialect_write_date_trunc() {
    let dialect = BigQueryDialect;
    let mut buf = String::new();
    dialect
        .write_date_trunc(&mut buf, "month", "created_at")
        .expect("write_date_trunc failed");
    // BigQuery reverses the arguments and uses backticks for identifiers
    assert_eq!(buf, "DATE_TRUNC(`created_at`, MONTH)");
}

#[test]
fn test_bigquery_dialect_write_date_trunc_qualified() {
    let dialect = BigQueryDialect;
    let mut buf = String::new();
    dialect
        .write_date_trunc(&mut buf, "month", "users.created_at")
        .expect("write_date_trunc failed");
    assert_eq!(buf, "DATE_TRUNC(`users`.`created_at`, MONTH)");
}

#[test]
fn test_bigquery_dialect_write_date_trunc_with_granularities() {
    let dialect = BigQueryDialect;

    for (granularity, expected) in [
        ("day", "DATE_TRUNC(`order_date`, DAY)"),
        ("week", "DATE_TRUNC(`order_date`, WEEK)"),
        ("quarter", "DATE_TRUNC(`order_date`, QUARTER)"),
        ("year", "DATE_TRUNC(`order_date`, YEAR)"),
    ] {
        let mut buf = String::new();
        dialect
            .write_date_trunc(&mut buf, granularity, "order_date")
            .expect("write_date_trunc failed");
        assert_eq!(buf, expected, "Failed for granularity: {granularity}");
    }
}

#[test]
fn test_bigquery_dialect_write_unnest() {
    let dialect = BigQueryDialect;
    let mut buf = String::new();
    dialect
        .write_unnest_expr(
            &mut buf,
            &Expr::Column(ColumnIdent("user.tags".to_string())),
        )
        .expect("write_unnest_expr failed");

    assert_eq!(buf, "UNNEST(`user`.`tags`)");
}

#[test]
fn test_bigquery_dialect_quote_identifier_backticks() {
    let dialect = BigQueryDialect;
    assert_eq!(dialect.quote_identifier("users"), "`users`");
    // Should escape internal backticks with backslash
    assert_eq!(dialect.quote_identifier("my`table"), "`my\\`table`");
}

#[test]
fn test_bigquery_dialect_quote_schema_qualified() {
    let dialect = BigQueryDialect;
    assert_eq!(
        dialect.quote_schema_qualified("public.users"),
        "`public`.`users`"
    );
    assert_eq!(dialect.quote_schema_qualified("users"), "`users`");
}

#[test]
fn test_bigquery_dialect_ctes_use_backticks() {
    let cte = SqlNode::CTE {
        alias: TableIdent("active_users".to_string()),
        query: Box::new(SqlNode::Query {
            ctes: None,
            select: Box::new(SqlNode::Select(vec![Expr::Column(ColumnIdent(
                "id".to_string(),
            ))])),
            from: Box::new(SqlNode::From {
                source: Box::new(SqlNode::Table(TableIdent("users".to_string()))),
                joins: vec![],
            }),
            r#where: None,
            group_by: None,
            having: None,
        order_by: vec![],
        limit: None,
        offset: None,
        }),
    };

    let ast = SqlNode::Query {
        ctes: Some(vec![cte]),
        select: Box::new(SqlNode::Select(vec![Expr::Column(ColumnIdent(
            "*".to_string(),
        ))])),
        from: Box::new(SqlNode::From {
            source: Box::new(SqlNode::Table(TableIdent("active_users".to_string()))),
            joins: vec![],
        }),
        r#where: None,
        group_by: None,
        having: None,
        order_by: vec![],
        limit: None,
        offset: None,
    };

    let dialect = BigQueryDialect;
    let sql = dialect.generate_sql(&ast).expect("SQL generation failed");
    // CTE aliases use quote_identifier, which will wrap in backticks
    assert_eq!(
        sql,
        "WITH `active_users` AS (SELECT `id` FROM `users`) SELECT `*` FROM `active_users`"
    );
}

#[test]
fn test_bigquery_date_trunc_expr_variant() {
    let ast = SqlNode::Query {
        ctes: None,
        select: Box::new(SqlNode::Select(vec![Expr::DateTrunc {
            granularity: "month".to_string(),
            column: Box::new(Expr::Column(ColumnIdent("created_at".to_string()))),
        }])),
        from: Box::new(SqlNode::From {
            source: Box::new(SqlNode::Table(TableIdent("public.users".to_string()))),
            joins: vec![],
        }),
        r#where: None,
        group_by: None,
        having: None,
        order_by: vec![],
        limit: None,
        offset: None,
    };

    let dialect = BigQueryDialect;
    let sql = dialect.generate_sql(&ast).expect("SQL generation failed");
    assert_eq!(
        sql,
        "SELECT DATE_TRUNC(`created_at`, MONTH) FROM `public`.`users`"
    );
}

#[test]
fn test_bigquery_date_trunc_invalid_target_rejected() {
    let ast = SqlNode::Query {
        ctes: None,
        select: Box::new(SqlNode::Select(vec![Expr::DateTrunc {
            granularity: "month".to_string(),
            column: Box::new(Expr::Literal("'2020-01-01'".to_string())),
        }])),
        from: Box::new(SqlNode::From {
            source: Box::new(SqlNode::Table(TableIdent("public.users".to_string()))),
            joins: vec![],
        }),
        r#where: None,
        group_by: None,
        having: None,
        order_by: vec![],
        limit: None,
        offset: None,
    };
    let dialect = BigQueryDialect;
    let err = dialect.generate_sql(&ast).unwrap_err();
    assert!(matches!(err, DialectError::UnsupportedExpr(_)));
}

#[test]
fn test_bigquery_date_trunc_invalid_granularity_rejected() {
    let ast = SqlNode::Query {
        ctes: None,
        select: Box::new(SqlNode::Select(vec![Expr::DateTrunc {
            granularity: "monthh".to_string(),
            column: Box::new(Expr::Column(ColumnIdent("created_at".to_string()))),
        }])),
        from: Box::new(SqlNode::From {
            source: Box::new(SqlNode::Table(TableIdent("public.users".to_string()))),
            joins: vec![],
        }),
        r#where: None,
        group_by: None,
        having: None,
        order_by: vec![],
        limit: None,
        offset: None,
    };
    let dialect = BigQueryDialect;
    let err = dialect.generate_sql(&ast).unwrap_err();
    assert!(matches!(err, DialectError::UnsupportedExpr(_)));
}

#[test]
fn test_bigquery_dialect_write_json_access_escaped() {
    let dialect = BigQueryDialect;
    let mut buf = String::new();
    dialect
        .write_json_access(
            &mut buf,
            &Expr::Column(ColumnIdent("data".to_string())),
            "a'b",
        )
        .expect("write_json_access failed");
    assert_eq!(buf, "JSON_EXTRACT_SCALAR(`data`, 'a''b')");
}

#[test]
fn test_bigquery_dialect_write_cast() {
    let dialect = BigQueryDialect;
    let mut buf = String::new();
    dialect
        .write_cast_expr(
            &mut buf,
            &Expr::Column(ColumnIdent("col".to_string())),
            "DATE",
        )
        .expect("write_cast_expr failed");
    assert_eq!(buf, "CAST(`col` AS DATE)");
}

#[test]
fn test_bigquery_schema_qualified_table() {
    let ast = SqlNode::Query {
        ctes: None,
        select: Box::new(SqlNode::Select(vec![Expr::Column(ColumnIdent(
            "id".to_string(),
        ))])),
        from: Box::new(SqlNode::From {
            source: Box::new(SqlNode::Table(TableIdent("public.users".to_string()))),
            joins: vec![],
        }),
        r#where: None,
        group_by: None,
        having: None,
        order_by: vec![],
        limit: None,
        offset: None,
    };
    let dialect = BigQueryDialect;
    let sql = dialect.generate_sql(&ast).expect("SQL generation failed");
    assert_eq!(sql, "SELECT `id` FROM `public`.`users`");
}

#[test]
fn test_bigquery_qualified_column() {
    let ast = SqlNode::Query {
        ctes: None,
        select: Box::new(SqlNode::Select(vec![Expr::Column(ColumnIdent(
            "users.id".to_string(),
        ))])),
        from: Box::new(SqlNode::From {
            source: Box::new(SqlNode::Table(TableIdent("users".to_string()))),
            joins: vec![],
        }),
        r#where: None,
        group_by: None,
        having: None,
        order_by: vec![],
        limit: None,
        offset: None,
    };
    let dialect = BigQueryDialect;
    let sql = dialect.generate_sql(&ast).expect("SQL generation failed");
    assert_eq!(sql, "SELECT `users`.`id` FROM `users`");
}

#[test]
fn test_bigquery_json_access_generate_sql() {
    let ast = SqlNode::Query {
        ctes: None,
        select: Box::new(SqlNode::Select(vec![Expr::JsonAccess {
            column: Box::new(Expr::Column(ColumnIdent("data".to_string()))),
            path: "$.user_id".to_string(),
        }])),
        from: Box::new(SqlNode::From {
            source: Box::new(SqlNode::Table(TableIdent("events".to_string()))),
            joins: vec![],
        }),
        r#where: None,
        group_by: None,
        having: None,
        order_by: vec![],
        limit: None,
        offset: None,
    };
    let dialect = BigQueryDialect;
    let sql = dialect.generate_sql(&ast).expect("SQL generation failed");
    assert_eq!(
        sql,
        "SELECT JSON_EXTRACT_SCALAR(`data`, '$.user_id') FROM `events`"
    );
}

#[test]
fn test_bigquery_unnest_generate_sql() {
    let ast = SqlNode::Query {
        ctes: None,
        select: Box::new(SqlNode::Select(vec![Expr::Unnest {
            expr: Box::new(Expr::Column(ColumnIdent("tags".to_string()))),
        }])),
        from: Box::new(SqlNode::From {
            source: Box::new(SqlNode::Table(TableIdent("events".to_string()))),
            joins: vec![],
        }),
        r#where: None,
        group_by: None,
        having: None,
        order_by: vec![],
        limit: None,
        offset: None,
    };
    let dialect = BigQueryDialect;
    let sql = dialect.generate_sql(&ast).expect("SQL generation failed");
    assert_eq!(sql, "SELECT UNNEST(`tags`) FROM `events`");
}

#[test]
fn test_bigquery_join_on_qualified_columns() {
    let ast = SqlNode::Query {
        ctes: None,
        select: Box::new(SqlNode::Select(vec![Expr::Column(ColumnIdent(
            "users.id".to_string(),
        ))])),
        from: Box::new(SqlNode::From {
            source: Box::new(SqlNode::Table(TableIdent("users".to_string()))),
            joins: vec![SqlNode::Join {
                join_type: JoinType::Inner,
                relation: Box::new(SqlNode::Table(TableIdent("orders".to_string()))),
                on: Expr::BinaryOp {
                    left: Box::new(Expr::Column(ColumnIdent("users.id".into()))),
                    op: "=".to_string(),
                    right: Box::new(Expr::Column(ColumnIdent("orders.user_id".into()))),
                },
            }],
        }),
        r#where: None,
        group_by: None,
        having: None,
        order_by: vec![],
        limit: None,
        offset: None,
    };
    let dialect = BigQueryDialect;
    let sql = dialect.generate_sql(&ast).expect("SQL generation failed");
    assert!(sql.contains("ON `users`.`id` = `orders`.`user_id`"));
}
