use crate::ast::{Expr, JoinType, SortDirection, SqlNode};
use std::fmt::Write;

/// Errors that occur during AST-to-SQL generation.
#[derive(Debug, PartialEq)]
pub enum DialectError {
    /// An AST node was encountered that the dialect does not know how to compile
    /// (e.g. semantic references or unresolved nodes).
    UnsupportedASTNode(String),
    /// An expression was encountered that the dialect does not know how to compile.
    UnsupportedExpr(String),
    /// An underlying string formatting error occurred.
    FormatError(std::fmt::Error),
}

impl std::fmt::Display for DialectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DialectError::UnsupportedASTNode(msg) => write!(f, "Unsupported AST node: {msg}"),
            DialectError::UnsupportedExpr(msg) => write!(f, "Unsupported expression: {msg}"),
            DialectError::FormatError(e) => write!(f, "Format error: {e}"),
        }
    }
}

impl std::error::Error for DialectError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DialectError::FormatError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::fmt::Error> for DialectError {
    fn from(err: std::fmt::Error) -> Self {
        DialectError::FormatError(err)
    }
}

/// Writes `s` into `buf` as a single-quoted SQL string literal, escaping any
/// embedded single quote by doubling it (`'` -> `''`) so the value cannot break
/// out of the literal. Used for JSON/VARIANT path literals.
pub(crate) fn write_sql_string_literal(buf: &mut String, s: &str) {
    buf.push('\'');
    for c in s.chars() {
        if c == '\'' {
            buf.push_str("''");
        } else {
            buf.push(c);
        }
    }
    buf.push('\'');
}

/// The core SQL dialect trait.
///
/// This defines how the generic `SqlNode` AST is converted into a raw,
/// warehouse-specific SQL string. Dialects implement this trait to override
/// specific generation logic (e.g., date truncation, quoting rules).
///
/// To minimize string allocations, the trait passes a `&mut String` buffer
/// recursively through the AST via `write_node`.
pub trait SqlDialect: std::fmt::Debug + Send + Sync {
    /// Initial buffer capacity in bytes for SQL generation.
    /// Tuned for typical 3–5 clause queries. Dialects with complex default
    /// output (e.g., many CTEs) may override this.
    fn initial_buffer_capacity(&self) -> usize {
        512
    }

    /// The main entry point for the compiler to generate SQL.
    fn generate_sql(&self, ast: &SqlNode) -> Result<String, DialectError> {
        let mut buf = String::with_capacity(self.initial_buffer_capacity());
        self.write_node(&mut buf, ast)?;
        Ok(buf)
    }

    /// Recursively writes an AST node to the string buffer.
    fn write_node(&self, buf: &mut String, node: &SqlNode) -> Result<(), DialectError> {
        match node {
            SqlNode::Query {
                ctes,
                select,
                from,
                r#where,
                group_by,
                having,
                order_by,
                limit,
                offset,
            } => {
                if let Some(ctes) = ctes {
                    if !ctes.is_empty() {
                        buf.push_str("WITH ");
                        for (i, cte) in ctes.iter().enumerate() {
                            if i > 0 {
                                buf.push_str(", ");
                            }
                            self.write_node(buf, cte)?;
                        }
                        buf.push(' ');
                    }
                }
                self.write_node(buf, select)?;
                buf.push(' ');
                self.write_node(buf, from)?;

                if let Some(w) = r#where {
                    buf.push(' ');
                    self.write_node(buf, w)?;
                }

                if let Some(g) = group_by {
                    buf.push(' ');
                    self.write_node(buf, g)?;
                }

                if let Some(h) = having {
                    buf.push(' ');
                    self.write_node(buf, h)?;
                }

                if !order_by.is_empty() {
                    buf.push_str(" ORDER BY ");
                    for (i, ob) in order_by.iter().enumerate() {
                        if i > 0 {
                            buf.push_str(", ");
                        }
                        self.write_expr(buf, &ob.expr)?;
                        match ob.direction {
                            SortDirection::Asc => buf.push_str(" ASC"),
                            SortDirection::Desc => buf.push_str(" DESC"),
                        }
                    }
                }
                if let Some(limit) = limit {
                    write!(buf, " LIMIT {limit}")?;
                }
                if let Some(offset) = offset {
                    write!(buf, " OFFSET {offset}")?;
                }
                Ok(())
            }
            SqlNode::CTE { alias, query } => {
                write!(buf, "{} AS (", self.quote_identifier(&alias.0))?;
                self.write_node(buf, query)?;
                buf.push(')');
                Ok(())
            }
            SqlNode::Select(exprs) => self.write_select(buf, exprs),
            SqlNode::From { source, joins } => self.write_from(buf, source, joins),
            SqlNode::Join {
                join_type,
                relation,
                on,
            } => self.write_join(buf, join_type, relation, on),
            SqlNode::Where(expr) => {
                buf.push_str("WHERE ");
                self.write_expr(buf, expr)
            }
            SqlNode::GroupBy(exprs) => {
                buf.push_str("GROUP BY ");
                for (i, expr) in exprs.iter().enumerate() {
                    if i > 0 {
                        buf.push_str(", ");
                    }
                    self.write_expr(buf, expr)?;
                }
                Ok(())
            }
            SqlNode::Having(expr) => {
                buf.push_str("HAVING ");
                self.write_expr(buf, expr)
            }
            // Table identifiers are schema-qualified quoted segment-by-segment.
            SqlNode::Table(ident) => {
                buf.push_str(&self.quote_schema_qualified(&ident.0));
                Ok(())
            }
            SqlNode::AliasedTable { table, alias } => {
                buf.push_str(&self.quote_schema_qualified(&table.0));
                buf.push_str(" AS ");
                buf.push_str(&self.quote_identifier(&alias.0));
                Ok(())
            }
            SqlNode::TimeSpine { granularity } => Err(DialectError::UnsupportedASTNode(format!(
                "TimeSpine({granularity})"
            ))),
        }
    }

    /// Recursively writes an Expression to the string buffer.
    fn write_expr(&self, buf: &mut String, expr: &Expr) -> Result<(), DialectError> {
        match expr {
            // Column identifiers are schema-qualified quoted segment-by-segment.
            Expr::Column(ident) => {
                buf.push_str(&self.quote_schema_qualified(&ident.0));
                Ok(())
            }
            Expr::Literal(val) => {
                buf.push_str(val);
                Ok(())
            }
            Expr::BinaryOp { left, op, right } => {
                self.write_expr(buf, left)?;
                buf.push(' ');
                buf.push_str(op);
                buf.push(' ');
                self.write_expr(buf, right)?;
                Ok(())
            }
            Expr::Function { name, args } => {
                buf.push_str(name);
                buf.push('(');
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        buf.push_str(", ");
                    }
                    self.write_expr(buf, arg)?;
                }
                buf.push(')');
                Ok(())
            }
            Expr::DateTrunc {
                granularity,
                column,
            } => {
                const VALID_DATE_PARTS: [&str; 8] = [
                    "year", "quarter", "month", "week", "day", "hour", "minute", "second",
                ];

                if !VALID_DATE_PARTS
                    .iter()
                    .any(|p| p.eq_ignore_ascii_case(granularity))
                {
                    return Err(DialectError::UnsupportedExpr(format!(
                        "DateTrunc: unsupported granularity {granularity:?}"
                    )));
                }

                // `write_date_trunc` takes the column as a raw &str. We only support a
                // simple `Expr::Column` as the truncation target for now; anything else
                // is rejected explicitly rather than silently producing wrong SQL.
                if let Expr::Column(ident) = column.as_ref() {
                    self.write_date_trunc(buf, granularity, &ident.0)
                } else {
                    Err(DialectError::UnsupportedExpr(
                        "DateTrunc currently only supports a simple Column as its argument"
                            .to_string(),
                    ))
                }
            }
            Expr::Cast { expr, target_type } => self.write_cast_expr(buf, expr, target_type),
            Expr::JsonAccess { column, path } => self.write_json_access(buf, column, path),
            Expr::Unnest { expr } => self.write_unnest_expr(buf, expr),
            Expr::DimensionRef {
                model: _,
                entity,
                dimension,
            } => Err(DialectError::UnsupportedExpr(format!(
                "DimensionRef({entity}.{dimension})"
            ))),
            Expr::MeasureRef {
                model: _,
                entity,
                measure,
            } => Err(DialectError::UnsupportedExpr(format!(
                "MeasureRef({entity}.{measure})"
            ))),
            Expr::Raw(sql) => {
                // SAFETY: Raw SQL is written verbatim. The caller must guarantee
                // this string does not contain unsanitized user input.
                buf.push_str(sql);
                Ok(())
            }
            Expr::CountDistinct(inner) => {
                buf.push_str("COUNT(DISTINCT ");
                self.write_expr(buf, inner)?;
                buf.push(')');
                Ok(())
            }
            Expr::CountStar => {
                buf.push_str("COUNT(*)");
                Ok(())
            }
            Expr::Aliased { expr, alias } => {
                self.write_expr(buf, expr)?;
                buf.push_str(" AS ");
                buf.push_str(&self.quote_identifier(alias));
                Ok(())
            }
        }
    }

    /// Quotes an identifier according to the dialect's rules (default ANSI: "ident")
    fn quote_identifier(&self, ident: &str) -> String {
        let escape_count = ident.chars().filter(|&c| c == '"').count();
        let capacity = ident.len() + 2 + escape_count;

        let mut buf = String::with_capacity(capacity);
        buf.push('"');
        for c in ident.chars() {
            if c == '"' {
                buf.push_str("\"\"");
            } else {
                buf.push(c);
            }
        }
        buf.push('"');
        buf
    }

    /// Quotes a possibly dot-qualified identifier (e.g., `public.users`, `users.id`)
    /// by quoting each `.`-separated segment with `quote_identifier` and rejoining
    /// with `.`. A single identifier (no dot) is quoted as-is.
    fn quote_schema_qualified(&self, ident: &str) -> String {
        if !ident.contains('.') {
            return self.quote_identifier(ident);
        }
        ident
            .split('.')
            .map(|segment| self.quote_identifier(segment))
            .collect::<Vec<_>>()
            .join(".")
    }

    /// Writes the SELECT projection list.
    fn write_select(&self, buf: &mut String, exprs: &[Expr]) -> Result<(), DialectError> {
        buf.push_str("SELECT ");
        for (i, expr) in exprs.iter().enumerate() {
            if i > 0 {
                buf.push_str(", ");
            }
            self.write_expr(buf, expr)?;
        }
        Ok(())
    }

    /// Writes the FROM clause including its joins.
    fn write_from(
        &self,
        buf: &mut String,
        source: &SqlNode,
        joins: &[SqlNode],
    ) -> Result<(), DialectError> {
        buf.push_str("FROM ");
        self.write_node(buf, source)?;
        for join in joins {
            buf.push(' ');
            self.write_node(buf, join)?;
        }
        Ok(())
    }

    /// Writes a single JOIN clause.
    fn write_join(
        &self,
        buf: &mut String,
        join_type: &JoinType,
        relation: &SqlNode,
        on: &Expr,
    ) -> Result<(), DialectError> {
        match join_type {
            JoinType::Inner => buf.push_str("INNER JOIN "),
            JoinType::Left => buf.push_str("LEFT JOIN "),
            JoinType::Full => buf.push_str("FULL JOIN "),
        }
        self.write_node(buf, relation)?;
        buf.push_str(" ON ");
        self.write_expr(buf, on)?;
        Ok(())
    }

    /// Provides dialect-specific date-truncation syntax for `Expr::DateTrunc`.
    ///
    /// This is the default ANSI rendering (`DATE_TRUNC('<gran>', "<col>")`). It is invoked
    /// from the `Expr::DateTrunc` arm of `write_expr`. Dialects override it where their
    /// syntax differs (BigQuery reverses the argument order; Snowflake uppercases the part).
    fn write_date_trunc(
        &self,
        buf: &mut String,
        granularity: &str,
        column: &str,
    ) -> Result<(), DialectError> {
        write!(
            buf,
            "DATE_TRUNC('{granularity}', {})",
            self.quote_schema_qualified(column)
        )?;
        Ok(())
    }

    /// Writes a type-cast expression. Default is ANSI `CAST(expr AS type)`.
    ///
    /// Dialects with a different syntax (e.g. Postgres `expr::type`) override this.
    fn write_cast_expr(
        &self,
        buf: &mut String,
        expr: &Expr,
        target_type: &str,
    ) -> Result<(), DialectError> {
        buf.push_str("CAST(");
        self.write_expr(buf, expr)?;
        buf.push_str(" AS ");
        buf.push_str(target_type);
        buf.push(')');
        Ok(())
    }

    /// Writes a JSON/VARIANT path extraction. Default: unsupported.
    fn write_json_access(
        &self,
        _buf: &mut String,
        _column: &Expr,
        _path: &str,
    ) -> Result<(), DialectError> {
        Err(DialectError::UnsupportedExpr(
            "JSON path access is not supported by this dialect".to_string(),
        ))
    }

    /// Writes an array UNNEST expression. Default: unsupported.
    fn write_unnest_expr(&self, _buf: &mut String, _expr: &Expr) -> Result<(), DialectError> {
        Err(DialectError::UnsupportedExpr(
            "UNNEST is not supported by this dialect".to_string(),
        ))
    }
}
