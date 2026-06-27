use may_core::ast::{ColumnIdent, Expr, JoinType, SqlNode, TableIdent};
use may_core::DummyDialect;
use may_core::SqlDialect;

#[test]
fn test_dummy_dialect_generates_group_by_and_having() {
    let ast = SqlNode::Query {
        ctes: None,
        select: Box::new(SqlNode::Select(vec![
            Expr::Column(ColumnIdent("region".to_string())),
            Expr::Function {
                name: "SUM".to_string(),
                args: vec![Expr::Column(ColumnIdent("amount".to_string()))],
            },
        ])),
        from: Box::new(SqlNode::From {
            source: Box::new(SqlNode::Table(TableIdent("orders".to_string()))),
            joins: vec![],
        }),
        r#where: None,
        group_by: Some(Box::new(SqlNode::GroupBy(vec![Expr::Column(ColumnIdent(
            "region".to_string(),
        ))]))),
        having: Some(Box::new(SqlNode::Having(Expr::BinaryOp {
            left: Box::new(Expr::Function {
                name: "SUM".to_string(),
                args: vec![Expr::Column(ColumnIdent("amount".to_string()))],
            }),
            op: ">".to_string(),
            right: Box::new(Expr::Literal("1000".to_string())),
        }))),
    };

    let dialect = DummyDialect;
    let sql = dialect.generate_sql(&ast).expect("SQL generation failed");
    assert_eq!(
        sql,
        "SELECT \"region\", SUM(\"amount\") FROM \"orders\" GROUP BY \"region\" HAVING SUM(\"amount\") > 1000"
    );
}

#[test]
fn test_dummy_dialect_rejects_unresolved_dimension_ref() {
    let ast = SqlNode::Query {
        ctes: None,
        select: Box::new(SqlNode::Select(vec![Expr::DimensionRef {
            entity: "users".to_string(),
            dimension: "region".to_string(),
        }])),
        from: Box::new(SqlNode::From {
            source: Box::new(SqlNode::Table(TableIdent("users".to_string()))),
            joins: vec![],
        }),
        r#where: None,
        group_by: None,
        having: None,
    };

    let dialect = DummyDialect;
    let result = dialect.generate_sql(&ast);
    assert!(
        result.is_err(),
        "Expected error for unresolved DimensionRef"
    );
}

#[test]
fn test_dummy_dialect_generates_basic_sql() {
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

    let dialect = DummyDialect;
    let sql = dialect.generate_sql(&ast).expect("SQL generation failed");
    assert_eq!(
        sql,
        "SELECT \"users\".\"id\", \"users\".\"name\" FROM \"public\".\"users\" WHERE \"users\".\"status\" = 'active'"
    );
}

#[test]
fn test_dummy_dialect_generates_joins() {
    let ast = SqlNode::Query {
        ctes: None,
        select: Box::new(SqlNode::Select(vec![Expr::Column(ColumnIdent(
            "orders.amount".to_string(),
        ))])),
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

    let dialect = DummyDialect;
    let sql = dialect.generate_sql(&ast).expect("SQL generation failed");
    assert_eq!(
        sql,
        "SELECT \"orders\".\"amount\" FROM \"orders\" LEFT JOIN \"users\" ON \"orders\".\"user_id\" = \"users\".\"id\""
    );
}

#[test]
fn test_dummy_dialect_generates_ctes() {
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

    let dialect = DummyDialect;
    let sql = dialect.generate_sql(&ast).expect("SQL generation failed");
    assert_eq!(
        sql,
        "WITH \"active_users\" AS (SELECT \"id\" FROM \"users\") SELECT \"*\" FROM \"active_users\""
    );
}
