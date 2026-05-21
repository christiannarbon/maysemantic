//! BigQuery dialect adapter for the SQL Compilation Engine.
//!
//! Implements the `SqlDialect` trait to generate Google BigQuery-compliant SQL
//! from the generic `SqlNode` AST. BigQuery has specific rules around
//! identifier quoting (backticks instead of double quotes) and function
//! argument ordering (e.g., DATE_TRUNC).

use crate::ast::Expr;
use crate::dialect::{DialectError, SqlDialect};

/// A dialect adapter that generates BigQuery-compliant SQL.
///
/// BigQuery's dialect diverges from standard ANSI primarily by using backticks
/// (`` ` ``) for identifier quoting, requiring specific argument ordering for
/// temporal functions, and using `UNNEST()` for array expansion.
#[derive(Debug)]
pub struct BigQueryDialect;

impl SqlDialect for BigQueryDialect {
    /// Quotes an identifier for BigQuery using backticks.
    ///
    /// Standard ANSI uses double quotes (`"`), but BigQuery uses backticks (`` ` ``).
    /// If an identifier contains a backtick, it is escaped using a backslash (`` \` ``).
    /// This implementation strictly follows zero-allocation principles,
    /// allocating the exact capacity required in a single pass.
    fn quote_identifier(&self, ident: &str) -> String {
        // Calculate exact capacity to avoid reallocations:
        // 2 backticks + base length + extra space for escaped backticks (each adds 1 char for '\')
        let mut capacity = ident.len() + 2;
        if ident.contains('`') {
            capacity += ident.chars().filter(|&c| c == '`').count();
        }

        // Allocate exactly once
        let mut buf = String::with_capacity(capacity);
        buf.push('`');

        // Process and escape in a single pass
        for c in ident.chars() {
            if c == '`' {
                buf.push_str("\\`"); // escape backticks with backslash
            } else {
                buf.push(c);
            }
        }
        buf.push('`');
        buf
    }

    /// Writes BigQuery's `DATE_TRUNC` function.
    ///
    /// BigQuery uses `DATE_TRUNC(column, GRANULARITY)` syntax, which reverses
    /// the argument order compared to standard Postgres/Snowflake.
    /// The granularity is written as an unquoted identifier in uppercase.
    fn write_date_trunc(
        &self,
        buf: &mut String,
        granularity: &str,
        column: &str,
    ) -> Result<(), DialectError> {
        buf.push_str("DATE_TRUNC(");

        // First argument is the column (identifier)
        let quoted = self.quote_identifier(column);
        buf.push_str(&quoted);

        buf.push_str(", ");

        // Second argument is the granularity (unquoted, uppercase)
        // Write uppercased chars directly to avoid allocating a new String
        for c in granularity.chars() {
            buf.push(c.to_ascii_uppercase());
        }

        buf.push(')');
        Ok(())
    }
}

impl BigQueryDialect {
    /// Writes a BigQuery-specific UNNEST extraction expression.
    ///
    /// Generates the `UNNEST(array_column)` function call, which expands arrays
    /// into sets of rows. This helper will be wired into the AST tree traversal
    /// when an `Expr::Unnest` variant is added to the AST.
    pub fn write_unnest(&self, buf: &mut String, expr: &Expr) -> Result<(), DialectError> {
        buf.push_str("UNNEST(");
        self.write_expr(buf, expr)?;
        buf.push(')');
        Ok(())
    }
}
