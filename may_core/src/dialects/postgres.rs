//! PostgreSQL dialect adapter for the SQL Compilation Engine.
//!
//! Implements the `SqlDialect` trait to generate PostgreSQL-compliant SQL
//! from the generic `SqlNode` AST. PostgreSQL uses standard ANSI double-quote
//! escaping for identifiers, so most default trait implementations are inherited.
//!
//! This module also provides Postgres-specific helpers (e.g., shorthand casting
//! via `::type`) that will be wired into the trait as the AST is extended in
//! future tickets.

use crate::ast::Expr;
use crate::dialects::{DialectError, SqlDialect};
use std::fmt::Write;

/// A dialect adapter that generates PostgreSQL-compliant SQL.
///
/// PostgreSQL uses standard ANSI SQL for most constructs. This adapter
/// inherits the default trait implementations and overrides only where
/// Postgres diverges (e.g., `DATE_TRUNC` syntax, shorthand casts).
///
/// Future Postgres-specific overrides (JSON operators `->>`/`#>>`,
/// array unnesting `UNNEST()`, regex `~` matching) will be added here
/// as the AST is extended with the corresponding expression variants.
#[derive(Debug)]
pub struct PostgresDialect;

impl SqlDialect for PostgresDialect {
    /// Writes Postgres-standard `DATE_TRUNC('granularity', "column")` syntax.
    ///
    /// While the default ANSI implementation produces identical output,
    /// this explicit override establishes `PostgresDialect` as the concrete
    /// extension point for future Postgres-specific temporal functions
    /// (e.g., `AGE()`, `EXTRACT()`, interval arithmetic).
    fn write_date_trunc(
        &self,
        buf: &mut String,
        granularity: &str,
        column: &str,
    ) -> Result<(), DialectError> {
        write!(
            buf,
            "DATE_TRUNC('{granularity}', {})",
            self.quote_identifier(column)
        )?;
        Ok(())
    }
}

impl PostgresDialect {
    /// Writes a Postgres-style shorthand cast expression: `expression::type`.
    ///
    /// This is a Postgres-specific helper that will be called from a future
    /// `write_expr` override once an `Expr::Cast` variant is added to the AST.
    ///
    /// # Example Output
    /// Given `Expr::Column("created_at")` and `target_type = "DATE"`:
    /// ```sql
    /// created_at::DATE
    /// ```
    pub fn write_cast(
        &self,
        buf: &mut String,
        expr: &Expr,
        target_type: &str,
    ) -> Result<(), DialectError> {
        self.write_expr(buf, expr)?;
        buf.push_str("::");
        buf.push_str(target_type);
        Ok(())
    }
}
