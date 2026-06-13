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

/// Builds a single-quoted SQL string literal, escaping embedded quotes.
///
/// The input is treated as an untrusted value: any single quote is doubled
/// (`'` -> `''`) so the value cannot break out of the literal. Use this for
/// caller-controlled values (e.g. RLS claim values) rather than constructing
/// `Expr::Literal` by hand.
pub fn literal_str(val: &str) -> Expr {
    Expr::Literal(format!("'{}'", val.replace('\'', "''")))
}
