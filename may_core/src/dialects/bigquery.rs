//! BigQuery dialect adapter for the SQL Compilation Engine.
//!
//! Implements the `SqlDialect` trait to generate Google BigQuery-compliant SQL
//! from the generic `SqlNode` AST. BigQuery has specific rules around
//! identifier quoting (backticks instead of double quotes) and function
//! argument ordering (e.g., DATE_TRUNC).

use crate::ast::Expr;
use crate::dialects::core::write_sql_string_literal;
use crate::dialects::{DialectError, SqlDialect};

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
        let escape_count = ident.chars().filter(|&c| c == '`').count();
        let capacity = ident.len() + 2 + escape_count;

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

    /// BigQuery safe JSON read: `JSON_EXTRACT_SCALAR(col, '$.path')`.
    fn write_json_access(
        &self,
        buf: &mut String,
        column: &Expr,
        path: &str,
    ) -> Result<(), DialectError> {
        buf.push_str("JSON_EXTRACT_SCALAR(");
        self.write_expr(buf, column)?;
        buf.push_str(", ");
        write_sql_string_literal(buf, path);
        buf.push(')');
        Ok(())
    }

    /// BigQuery array expansion: `UNNEST(col)`.
    fn write_unnest_expr(&self, buf: &mut String, expr: &Expr) -> Result<(), DialectError> {
        buf.push_str("UNNEST(");
        self.write_expr(buf, expr)?;
        buf.push(')');
        Ok(())
    }
}
