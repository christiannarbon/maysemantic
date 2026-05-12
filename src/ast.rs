//! Abstract Syntax Tree (AST) definitions for the SQL Compilation Engine.
//!
//! This module defines the intermediate representation used by the compiler
//! before generating dialect-specific SQL strings.

/// Represents a node within the SQL Abstract Syntax Tree (AST).
///
/// The `SqlNode` enum is highly recursive. To maintain an optimal memory footprint
/// and satisfy Rust's sizing requirements at compile time, recursive variants
/// explicitly box their nested `SqlNode` payloads (`Box<SqlNode>`).
/// Linear collections use `Vec<SqlNode>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SqlNode {
    /// Represents a complete SELECT statement.
    ///
    /// The `from` and `r#where` clauses are boxed to handle recursive subqueries
    /// or complex nested AST structures safely.
    Select {
        /// The projection list (e.g., `SELECT user_id, count(1)`).
        projection: Vec<SqlNode>,
        /// The FROM clause (e.g., `FROM users`). Must be boxed for recursion.
        from: Box<SqlNode>,
        /// An optional WHERE clause.
        r#where: Option<Box<SqlNode>>,
        /// The GROUP BY clause.
        group_by: Vec<SqlNode>,
    },

    /// Represents a base table or view reference (e.g., `users`).
    Table {
        /// The name of the table.
        name: String,
    },

    /// Represents a column reference (e.g., `user_id`).
    Column {
        /// The name of the column.
        name: String,
    },

    /// A dummy variant containing a raw string payload.
    ///
    /// This is used to validate the base recursion model and provides an escape
    /// hatch for raw SQL injection during complex edge cases.
    Raw(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ast_recursion_model() {
        // Construct a nested AST representing:
        // SELECT user_id, raw_data FROM users WHERE id = 1

        let projection = vec![
            SqlNode::Column {
                name: "user_id".to_string(),
            },
            SqlNode::Raw("raw_data".to_string()),
        ];

        let from_node = Box::new(SqlNode::Table {
            name: "users".to_string(),
        });
        let where_node = Some(Box::new(SqlNode::Raw("id = 1".to_string())));

        let ast = SqlNode::Select {
            projection,
            from: from_node,
            r#where: where_node,
            group_by: vec![],
        };

        // Validate structure through basic pattern matching
        match ast {
            SqlNode::Select {
                projection,
                from,
                r#where,
                group_by,
            } => {
                assert_eq!(projection.len(), 2);

                // Validate inner recursive 'from' node
                if let SqlNode::Table { name } = *from {
                    assert_eq!(name, "users");
                } else {
                    panic!("Expected Table node in FROM clause");
                }

                // Validate optional inner 'where' node
                let where_inner = *r#where.expect("Expected WHERE clause");
                if let SqlNode::Raw(raw_sql) = where_inner {
                    assert_eq!(raw_sql, "id = 1");
                } else {
                    panic!("Expected Raw node in WHERE clause");
                }

                assert!(group_by.is_empty());
            }
            _ => panic!("Expected Select node at root"),
        }
    }
}
