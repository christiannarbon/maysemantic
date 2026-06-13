use may_core::ast::{and, eq, literal_str, or};
use may_core::{ColumnIdent, Expr};

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
            right: Box::new(Expr::Literal("EMEA".to_string())),
        }
    );
}

#[test]
fn test_literal_str_builds_literal() {
    assert_eq!(literal_str("EMEA"), Expr::Literal("EMEA".to_string()));
}

