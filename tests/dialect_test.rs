use maysemantic::ast::{ColumnIdent, Expr, SqlNode, TableIdent};
use maysemantic::dialect::DummyDialect;
use maysemantic::SqlDialect;

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
        "SELECT region, SUM(amount) FROM orders GROUP BY region HAVING SUM(amount) > 1000"
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
