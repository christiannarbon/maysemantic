//! Snowflake dialect adapter for the SQL Compilation Engine.
//!
//! Implements the `SqlDialect` trait to generate Snowflake-compliant SQL
//! from the generic `SqlNode` AST. Snowflake has specific rules around
//! identifier case-sensitivity and provides custom functions for JSON extraction.

use crate::ast::Expr;
use crate::dialects::core::write_sql_string_literal;
use crate::dialects::{DialectError, SqlDialect};

/// A dialect adapter that generates Snowflake-compliant SQL.
///
/// Snowflake's dialect differs from ANSI primarily in identifier case-sensitivity
/// (unquoted identifiers are treated as uppercase) and JSON/variant handling.
#[derive(Debug)]
pub struct SnowflakeDialect;

impl SqlDialect for SnowflakeDialect {
    /// Quotes an identifier for Snowflake.
    ///
    /// Snowflake stores and resolves unquoted identifiers as UPPERCASE.
    /// If we blindly wrap a lowercase semantic model identifier in double quotes
    /// (e.g., `"users"`), Snowflake will treat it as case-sensitive and fail
    /// to find the `USERS` table.
    ///
    /// To ensure safe generation, we normalize all identifiers to uppercase
    /// before quoting, mimicking Snowflake's default unquoted resolution behavior.
    fn quote_identifier(&self, ident: &str) -> String {
        // Calculate exact capacity to avoid reallocations:
        // 2 quotes + base length + extra space for escaped quotes
        let escape_count = ident.chars().filter(|&c| c == '"').count();
        let capacity = ident.len() + 2 + escape_count;

        // Allocate exactly once
        let mut buf = String::with_capacity(capacity);
        buf.push('"');

        // Process and uppercase in a single pass
        for c in ident.chars() {
            let upper_c = c.to_ascii_uppercase();
            if upper_c == '"' {
                buf.push_str("\"\""); // escape quotes
            } else {
                buf.push(upper_c);
            }
        }
        buf.push('"');
        buf
    }

    /// Writes Snowflake's `DATE_TRUNC` function.
    ///
    /// Uses standard `DATE_TRUNC('granularity', column)` syntax. The granularity
    /// is explicitly uppercased to follow Snowflake documentation conventions.
    fn write_date_trunc(
        &self,
        buf: &mut String,
        granularity: &str,
        column: &str,
    ) -> Result<(), DialectError> {
        buf.push_str("DATE_TRUNC('");

        // Write uppercased chars directly to avoid allocating a new String
        for c in granularity.chars() {
            buf.push(c.to_ascii_uppercase());
        }

        buf.push_str("', ");

        // quote_schema_qualified returns a String
        let quoted = self.quote_schema_qualified(column);
        buf.push_str(&quoted);

        buf.push(')');
        Ok(())
    }

    /// Snowflake VARIANT path read: `GET_PATH("COL", 'path')`.
    fn write_json_access(
        &self,
        buf: &mut String,
        column: &Expr,
        path: &str,
    ) -> Result<(), DialectError> {
        buf.push_str("GET_PATH(");
        self.write_expr(buf, column)?;
        buf.push_str(", ");
        write_sql_string_literal(buf, path);
        buf.push(')');
        Ok(())
    }
}
