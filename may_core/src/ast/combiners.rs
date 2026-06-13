//! Ergonomic constructors for composing `Expr` predicates.
//!
//! These wrap the verbose `Expr::BinaryOp { left: Box::new(..), op: .., right: Box::new(..) }`
//! pattern so callers (notably the RLS injector) can build predicates without
//! repeating allocation boilerplate or magic operator strings.

use crate::ast::node::Expr;

/// Combines two expressions with a logical `AND`.
pub fn and(left: Expr, right: Expr) -> Expr {
    Expr::BinaryOp {
        left: Box::new(left),
        op: "AND".to_string(),
        right: Box::new(right),
    }
}

/// Combines two expressions with a logical `OR`.
pub fn or(left: Expr, right: Expr) -> Expr {
    Expr::BinaryOp {
        left: Box::new(left),
        op: "OR".to_string(),
        right: Box::new(right),
    }
}

/// Builds an equality predicate `left = right`.
pub fn eq(left: Expr, right: Expr) -> Expr {
    Expr::BinaryOp {
        left: Box::new(left),
        op: "=".to_string(),
        right: Box::new(right),
    }
}

/// Builds a string literal expression.
pub fn literal_str(val: &str) -> Expr {
    Expr::Literal(val.to_string())
}
