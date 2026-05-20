//! Snowflake dialect adapter for the SQL Compilation Engine.
//!
//! Implements the `SqlDialect` trait to generate Snowflake-compliant SQL
//! from the generic `SqlNode` AST. Snowflake has specific rules around
//! identifier case-sensitivity and provides custom functions for JSON extraction.

use crate::ast::Expr;
use crate::dialect::{DialectError, SqlDialect};
use std::fmt::Write;

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
        let upper_ident = ident.to_ascii_uppercase();
        if upper_ident.contains('"') {
            let escaped = upper_ident.replace('"', "\"\"");
            format!("\"{escaped}\"")
        } else {
            format!("\"{upper_ident}\"")
        }
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
        write!(
            buf,
            "DATE_TRUNC('{}', {})",
            granularity.to_ascii_uppercase(),
            self.quote_identifier(column)
        )?;
        Ok(())
    }
}

impl SnowflakeDialect {
    /// Writes a Snowflake-specific JSON extraction expression.
    ///
    /// Generates the `GET_PATH(column, 'path')` function call, which safely
    /// extracts variant/JSON data. This helper will be wired into `write_expr`
    /// when an `Expr::JsonAccess` variant is added to the AST.
    pub fn write_json_extract(
        &self,
        buf: &mut String,
        column: &Expr,
        path: &str,
    ) -> Result<(), DialectError> {
        buf.push_str("GET_PATH(");
        self.write_expr(buf, column)?;
        write!(buf, ", '{}')", path)?;
        Ok(())
    }
}
