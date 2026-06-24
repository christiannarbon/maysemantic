use may_core::ast::{ColumnIdent, Expr, JoinType, SqlNode, TableIdent};
use may_core::DialectError;
use may_core::SnowflakeDialect;
use may_core::SqlDialect;

#[test]
fn test_snowflake_dialect_generates_basic_select() {
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
    };

    let dialect = SnowflakeDialect;
    let sql = dialect.generate_sql(&ast).expect("SQL generation failed");
    // Snowflake dialect converts identifiers to uppercase before quoting.
    // The table identifier in SqlNode::Table is raw (not quoted in trait),
    // but column identifiers are also raw. Wait, SqlNode::Table writes raw.
    // Let's check how the ANSI default handles Table and Column. They write raw.
    // Snowflake dialect only overrides quote_identifier, which is used for CTEs
    // and write_date_trunc and JSON extraction.
    // Therefore, the raw output should match the exact strings provided, like Postgres.
    assert_eq!(
        sql,
        "SELECT users.id, users.name FROM public.users WHERE users.status = 'active'"
    );
}

#[test]
fn test_snowflake_dialect_generates_joins() {
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
    };

    let dialect = SnowflakeDialect;
    let sql = dialect.generate_sql(&ast).expect("SQL generation failed");
    assert_eq!(
        sql,
        "SELECT orders.amount, users.name FROM orders LEFT JOIN users ON orders.user_id = users.id"
    );
}

#[test]
fn test_snowflake_dialect_write_date_trunc() {
    let dialect = SnowflakeDialect;
    let mut buf = String::new();
    dialect
        .write_date_trunc(&mut buf, "month", "created_at")
        .expect("write_date_trunc failed");
    // Snowflake uppercases both the granularity and the identifier.
    assert_eq!(buf, "DATE_TRUNC('MONTH', \"CREATED_AT\")");
}

#[test]
fn test_snowflake_dialect_write_date_trunc_with_granularities() {
    let dialect = SnowflakeDialect;

    for (granularity, expected) in [
        ("day", "DATE_TRUNC('DAY', \"ORDER_DATE\")"),
        ("week", "DATE_TRUNC('WEEK', \"ORDER_DATE\")"),
        ("quarter", "DATE_TRUNC('QUARTER', \"ORDER_DATE\")"),
        ("year", "DATE_TRUNC('YEAR', \"ORDER_DATE\")"),
    ] {
        let mut buf = String::new();
        dialect
            .write_date_trunc(&mut buf, granularity, "order_date")
            .expect("write_date_trunc failed");
        assert_eq!(buf, expected, "Failed for granularity: {granularity}");
    }
}

#[test]
fn test_snowflake_dialect_write_json_extract() {
    let dialect = SnowflakeDialect;
    let mut buf = String::new();
    dialect
        .write_json_access(
            &mut buf,
            &Expr::Column(ColumnIdent("raw_data".to_string())),
            "user.name",
        )
        .expect("write_json_access failed");

    // The column identifier inside write_json_extract is passed as an Expr,
    // which delegates to write_expr. write_expr for Column pushes raw.
    // So it should be `GET_PATH(raw_data, 'user.name')`.
    assert_eq!(buf, "GET_PATH(raw_data, 'user.name')");
}

#[test]
fn test_snowflake_dialect_quote_identifier_uppercases() {
    let dialect = SnowflakeDialect;
    assert_eq!(dialect.quote_identifier("users"), "\"USERS\"");
    // Should handle existing uppercase
    assert_eq!(dialect.quote_identifier("ORDERS"), "\"ORDERS\"");
    // Should escape quotes while uppercasing
    assert_eq!(dialect.quote_identifier("my\"table"), "\"MY\"\"TABLE\"");
}

#[test]
fn test_snowflake_dialect_ctes_are_uppercased() {
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
    };

    let dialect = SnowflakeDialect;
    let sql = dialect.generate_sql(&ast).expect("SQL generation failed");
    // CTE aliases use quote_identifier, which will uppercase it
    assert_eq!(
        sql,
        "WITH \"ACTIVE_USERS\" AS (SELECT id FROM users) SELECT * FROM active_users"
    );
}

#[test]
fn test_snowflake_date_trunc_expr_variant() {
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
    };

    let dialect = SnowflakeDialect;
    let sql = dialect.generate_sql(&ast).expect("SQL generation failed");
    assert_eq!(
        sql,
        "SELECT DATE_TRUNC('MONTH', \"CREATED_AT\") FROM public.users"
    );
}

#[test]
fn test_snowflake_date_trunc_invalid_target_rejected() {
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
    };
    let dialect = SnowflakeDialect;
    let err = dialect.generate_sql(&ast).unwrap_err();
    assert!(matches!(err, DialectError::UnsupportedExpr(_)));
}

#[test]
fn test_snowflake_date_trunc_invalid_granularity_rejected() {
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
    };
    let dialect = SnowflakeDialect;
    let err = dialect.generate_sql(&ast).unwrap_err();
    assert!(matches!(err, DialectError::UnsupportedExpr(_)));
}
