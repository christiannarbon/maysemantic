use may_core::ast::{and, eq, literal_str, or};
use may_core::{ColumnIdent, Expr, SqlNode, TableIdent};
use may_core::{DummyDialect, SqlDialect};

fn col(name: &str) -> Expr {
    Expr::Column(ColumnIdent(name.to_string()))
}

#[test]
fn test_and_builds_and_binary_op() {
    assert_eq!(
        and(col("a"), col("b")),
        Expr::BinaryOp {
            left: Box::new(col("a")),
            op: "AND".to_string(),
            right: Box::new(col("b")),
        }
    );
}

#[test]
fn test_or_builds_or_binary_op() {
    assert_eq!(
        or(col("a"), col("b")),
        Expr::BinaryOp {
            left: Box::new(col("a")),
            op: "OR".to_string(),
            right: Box::new(col("b")),
        }
    );
}

#[test]
fn test_eq_builds_equality_binary_op() {
    assert_eq!(
        eq(col("user_region"), literal_str("EMEA")),
        Expr::BinaryOp {
            left: Box::new(col("user_region")),
            op: "=".to_string(),
            right: Box::new(Expr::Literal("'EMEA'".to_string())),
        }
    );
}

#[test]
fn test_literal_str_builds_literal() {
    assert_eq!(literal_str("EMEA"), Expr::Literal("'EMEA'".to_string()));
}

fn render_where(pred: Expr) -> String {
    let ast = SqlNode::Query {
        ctes: None,
        select: Box::new(SqlNode::Select(vec![col("id")])),
        from: Box::new(SqlNode::From {
            source: Box::new(SqlNode::Table(TableIdent("t".to_string()))),
            joins: vec![],
        }),
        r#where: Some(Box::new(SqlNode::Where(pred))),
        group_by: None,
        having: None,
    };
    DummyDialect
        .generate_sql(&ast)
        .expect("SQL generation failed")
}

#[test]
fn test_eq_with_literal_str_renders_quoted_value() {
    let sql = render_where(eq(col("user_region"), literal_str("EMEA")));
    assert_eq!(sql, "SELECT id FROM t WHERE user_region = 'EMEA'");
}

#[test]
fn test_literal_str_escapes_embedded_quotes() {
    // An injection attempt must stay safely inside the quoted literal.
    let sql = render_where(eq(col("user_region"), literal_str("x' OR '1'='1")));
    assert_eq!(
        sql,
        "SELECT id FROM t WHERE user_region = 'x'' OR ''1''=''1'"
    );
}
