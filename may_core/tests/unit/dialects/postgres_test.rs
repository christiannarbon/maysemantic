use may_core::ast::{ColumnIdent, Expr, JoinType, SqlNode, TableIdent};
use may_core::PostgresDialect;
use may_core::SqlDialect;
use may_core::DialectError;

#[test]
fn test_postgres_dialect_generates_basic_select() {
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

    let dialect = PostgresDialect;
    let sql = dialect.generate_sql(&ast).expect("SQL generation failed");
    assert_eq!(
        sql,
        "SELECT users.id, users.name FROM public.users WHERE users.status = 'active'"
    );
}

#[test]
fn test_postgres_dialect_generates_joins() {
    let ast = SqlNode::Query {
        ctes: None,
        select: Box::new(SqlNode::Select(vec![
            Expr::Column(ColumnIdent("orders.amount".to_string())),
            Expr::Column(ColumnIdent("users.name".to_string())),
        ])),
        from: Box::new(SqlNode::From {
            source: Box::new(SqlNode::Table(TableIdent("orders".to_string()))),
            joins: vec![SqlNode::Join {
                join_type: JoinType::Inner,
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

    let dialect = PostgresDialect;
    let sql = dialect.generate_sql(&ast).expect("SQL generation failed");
    assert_eq!(
        sql,
        "SELECT orders.amount, users.name FROM orders INNER JOIN users ON orders.user_id = users.id"
    );
}

#[test]
fn test_postgres_dialect_write_date_trunc() {
    let dialect = PostgresDialect;
    let mut buf = String::new();
    dialect
        .write_date_trunc(&mut buf, "month", "created_at")
        .expect("write_date_trunc failed");
    assert_eq!(buf, "DATE_TRUNC('month', \"created_at\")");
}

#[test]
fn test_postgres_dialect_write_date_trunc_with_granularities() {
    let dialect = PostgresDialect;

    for (granularity, expected) in [
        ("day", "DATE_TRUNC('day', \"order_date\")"),
        ("week", "DATE_TRUNC('week', \"order_date\")"),
        ("quarter", "DATE_TRUNC('quarter', \"order_date\")"),
        ("year", "DATE_TRUNC('year', \"order_date\")"),
    ] {
        let mut buf = String::new();
        dialect
            .write_date_trunc(&mut buf, granularity, "order_date")
            .expect("write_date_trunc failed");
        assert_eq!(buf, expected, "Failed for granularity: {granularity}");
    }
}

#[test]
fn test_postgres_dialect_write_cast_column() {
    let dialect = PostgresDialect;
    let mut buf = String::new();
    dialect
        .write_cast(
            &mut buf,
            &Expr::Column(ColumnIdent("created_at".to_string())),
            "DATE",
        )
        .expect("write_cast failed");
    assert_eq!(buf, "created_at::DATE");
}

#[test]
fn test_postgres_dialect_write_cast_literal() {
    let dialect = PostgresDialect;
    let mut buf = String::new();
    dialect
        .write_cast(
            &mut buf,
            &Expr::Literal("'2024-01-01'".to_string()),
            "TIMESTAMP",
        )
        .expect("write_cast failed");
    assert_eq!(buf, "'2024-01-01'::TIMESTAMP");
}

#[test]
fn test_postgres_dialect_write_cast_function() {
    let dialect = PostgresDialect;
    let mut buf = String::new();
    dialect
        .write_cast(
            &mut buf,
            &Expr::Function {
                name: "COUNT".to_string(),
                args: vec![Expr::Column(ColumnIdent("id".to_string()))],
            },
            "VARCHAR",
        )
        .expect("write_cast failed");
    assert_eq!(buf, "COUNT(id)::VARCHAR");
}

#[test]
fn test_postgres_dialect_quote_identifier_uses_ansi_default() {
    let dialect = PostgresDialect;
    // Postgres inherits the ANSI default — verify it works correctly
    assert_eq!(dialect.quote_identifier("users"), "\"users\"");
    assert_eq!(dialect.quote_identifier("my\"table"), "\"my\"\"table\"");
}

#[test]
fn test_postgres_date_trunc_expr_variant() {
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

    let dialect = PostgresDialect;
    let sql = dialect.generate_sql(&ast).expect("SQL generation failed");
    assert_eq!(
        sql,
        "SELECT DATE_TRUNC('month', \"created_at\") FROM public.users"
    );
}

#[test]
fn test_postgres_date_trunc_invalid_target_rejected() {
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
    let dialect = PostgresDialect;
    let err = dialect.generate_sql(&ast).unwrap_err();
    assert!(matches!(err, DialectError::UnsupportedExpr(_)));
}

#[test]
fn test_postgres_date_trunc_invalid_granularity_rejected() {
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
    let dialect = PostgresDialect;
    let err = dialect.generate_sql(&ast).unwrap_err();
    assert!(matches!(err, DialectError::UnsupportedExpr(_)));
}
