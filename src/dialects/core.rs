use crate::ast::{Expr, JoinType, SqlNode};
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

impl std::error::Error for DialectError {}

impl From<std::fmt::Error> for DialectError {
    fn from(err: std::fmt::Error) -> Self {
        DialectError::FormatError(err)
    }
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
    /// The main entry point for the compiler to generate SQL.
    fn generate_sql(&self, ast: &SqlNode) -> Result<String, DialectError> {
        let mut buf = String::with_capacity(1024);
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
            // Table identifiers are written raw (not quoted) because they may contain
            // schema-qualified names (e.g., `public.users`). Individual segment quoting
            // will be handled when schema-aware identifier parsing is implemented.
            SqlNode::Table(ident) => {
                buf.push_str(&ident.0);
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
            // Column identifiers are written raw (not quoted) because they may contain
            // table-qualified names (e.g., `users.id`). Individual segment quoting
            // will be handled when schema-aware identifier parsing is implemented.
            Expr::Column(ident) => {
                buf.push_str(&ident.0);
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
            Expr::DimensionRef { entity, dimension } => Err(DialectError::UnsupportedExpr(
                format!("DimensionRef({entity}.{dimension})"),
            )),
            Expr::MeasureRef { entity, measure } => Err(DialectError::UnsupportedExpr(format!(
                "MeasureRef({entity}.{measure})"
            ))),
            Expr::Raw(sql) => {
                // SAFETY: Raw SQL is written verbatim. The caller must guarantee
                // this string does not contain unsanitized user input.
                buf.push_str(sql);
                Ok(())
            }
        }
    }

    /// Quotes an identifier according to the dialect's rules (default ANSI: "ident")
    fn quote_identifier(&self, ident: &str) -> String {
        if ident.contains('"') {
            let escaped = ident.replace('"', "\"\"");
            format!("\"{escaped}\"")
        } else {
            format!("\"{ident}\"")
        }
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

    /// Provides dialect-specific date truncation syntax.
    ///
    /// **Note:** This method is not automatically called during AST traversal
    /// because no `Expr::DateTrunc` variant exists yet. It is provided as a
    /// hook for dialect implementations to override, and will be wired in
    /// when the `Expr` enum is extended with a `DateTrunc` variant.
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

/// A dummy dialect for testing standard ANSI implementations.
#[derive(Debug)]
pub struct DummyDialect;

impl SqlDialect for DummyDialect {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{ColumnIdent, TableIdent};

    #[test]
    fn test_dummy_dialect_generates_basic_sql() {
        let ast = SqlNode::Query {
            ctes: None,
            select: Box::new(SqlNode::Select(vec![
                Expr::Column(ColumnIdent("users.id".to_string())),
                Expr::Column(ColumnIdent("users.name".to_string())),
            ])),
            from: Box::new(SqlNode::From {
                source: Box::new(SqlNode::Table(TableIdent("public.users".to_string()))),
                joins: vec![],
            }),
            r#where: Some(Box::new(SqlNode::Where(Expr::BinaryOp {
                left: Box::new(Expr::Column(ColumnIdent("users.status".to_string()))),
                op: "=".to_string(),
                right: Box::new(Expr::Literal("'active'".to_string())),
            }))),
            group_by: None,
            having: None,
        };

        let dialect = DummyDialect;
        let sql = dialect.generate_sql(&ast).expect("SQL generation failed");
        assert_eq!(
            sql,
            "SELECT users.id, users.name FROM public.users WHERE users.status = 'active'"
        );
    }

    #[test]
    fn test_dummy_dialect_generates_joins() {
        let ast = SqlNode::Query {
            ctes: None,
            select: Box::new(SqlNode::Select(vec![Expr::Column(ColumnIdent(
                "orders.amount".to_string(),
            ))])),
            from: Box::new(SqlNode::From {
                source: Box::new(SqlNode::Table(TableIdent("orders".to_string()))),
                joins: vec![SqlNode::Join {
                    join_type: JoinType::Left,
                    relation: Box::new(SqlNode::Table(TableIdent("users".to_string()))),
                    on: Expr::BinaryOp {
                        left: Box::new(Expr::Column(ColumnIdent("orders.user_id".to_string()))),
                        op: "=".to_string(),
                        right: Box::new(Expr::Column(ColumnIdent("users.id".to_string()))),
                    },
                }],
            }),
            r#where: None,
            group_by: None,
            having: None,
        };

        let dialect = DummyDialect;
        let sql = dialect.generate_sql(&ast).expect("SQL generation failed");
        assert_eq!(
            sql,
            "SELECT orders.amount FROM orders LEFT JOIN users ON orders.user_id = users.id"
        );
    }

    #[test]
    fn test_dummy_dialect_generates_ctes() {
        let cte = SqlNode::CTE {
            alias: TableIdent("active_users".to_string()),
            query: Box::new(SqlNode::Query {
                ctes: None,
                select: Box::new(SqlNode::Select(vec![Expr::Column(ColumnIdent(
                    "id".to_string(),
                ))])),
                from: Box::new(SqlNode::From {
                    source: Box::new(SqlNode::Table(TableIdent("users".to_string()))),
                    joins: vec![],
                }),
                r#where: None,
                group_by: None,
                having: None,
            }),
        };

        let ast = SqlNode::Query {
            ctes: Some(vec![cte]),
            select: Box::new(SqlNode::Select(vec![Expr::Column(ColumnIdent(
                "*".to_string(),
            ))])),
            from: Box::new(SqlNode::From {
                source: Box::new(SqlNode::Table(TableIdent("active_users".to_string()))),
                joins: vec![],
            }),
            r#where: None,
            group_by: None,
            having: None,
        };

        let dialect = DummyDialect;
        let sql = dialect.generate_sql(&ast).expect("SQL generation failed");
        assert_eq!(
            sql,
            "WITH \"active_users\" AS (SELECT id FROM users) SELECT * FROM active_users"
        );
    }
}
