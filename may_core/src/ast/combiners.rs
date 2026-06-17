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

/// Builds an **unquoted** numeric SQL literal from an untrusted value.
///
/// Returns `None` when `val` is not a valid finite number, so callers can fail
/// closed rather than emit an unquoted caller-controlled token (an injection
/// vector). The emitted token is the re-formatted parsed value, so no stray
/// characters from the input can survive.
pub fn literal_num(val: &str) -> Option<Expr> {
    if let Ok(i) = val.trim().parse::<i64>() {
        return Some(Expr::Literal(i.to_string()));
    }
    if let Ok(f) = val.trim().parse::<f64>() {
        if f.is_finite() {
            return Some(Expr::Literal(f.to_string()));
        }
    }
    None
}

/// Builds a boolean SQL literal (`TRUE`/`FALSE`) from an untrusted value.
///
/// Accepts only `"true"`/`"false"` (case-insensitive, trimmed); any other value
/// returns `None` so callers can fail closed.
pub fn literal_bool(val: &str) -> Option<Expr> {
    match val.trim().to_ascii_lowercase().as_str() {
        "true" => Some(Expr::Literal("TRUE".to_string())),
        "false" => Some(Expr::Literal("FALSE".to_string())),
        _ => None,
    }
}
