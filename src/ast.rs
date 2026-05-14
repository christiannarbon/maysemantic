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
    /// Represents a complete SQL query block.
    ///
    /// This acts as the root node for a standard statement, composing the distinct
    /// clauses together.
    Query {
        /// The SELECT clause node.
        select: Box<SqlNode>,
        /// The FROM clause node.
        from: Box<SqlNode>,
        /// An optional WHERE clause node.
        r#where: Option<Box<SqlNode>>,
        /// An optional GROUP BY clause node.
        group_by: Option<Box<SqlNode>>,
    },

    /// Represents the projection list of a SELECT clause.
    Select(Vec<SqlNode>),

    /// Represents the source of a FROM clause (e.g., a table or subquery).
    From(Box<SqlNode>),

    /// Represents the conditional logic of a WHERE clause.
    Where(Box<SqlNode>),

    /// Represents a binary operation (e.g., `id = 1` or `price > 100`).
    BinaryOp {
        /// The left side of the operation.
        left: Box<SqlNode>,
        /// The operator string (e.g., "=", ">", "AND").
        op: String,
        /// The right side of the operation.
        right: Box<SqlNode>,
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

        let select_node = SqlNode::Select(vec![
            SqlNode::Column {
                name: "user_id".to_string(),
            },
            SqlNode::Raw("raw_data".to_string()),
        ]);

        let from_node = SqlNode::From(Box::new(SqlNode::Table {
            name: "users".to_string(),
        }));

        let where_condition = SqlNode::BinaryOp {
            left: Box::new(SqlNode::Column {
                name: "id".to_string(),
            }),
            op: "=".to_string(),
            right: Box::new(SqlNode::Raw("1".to_string())),
        };

        let where_node = Some(Box::new(SqlNode::Where(Box::new(where_condition))));

        let ast = SqlNode::Query {
            select: Box::new(select_node),
            from: Box::new(from_node),
            r#where: where_node,
            group_by: None,
        };

        // Validate structure through basic pattern matching
        match ast {
            SqlNode::Query {
                select,
                from,
                r#where,
                group_by,
            } => {
                // Validate Select
                if let SqlNode::Select(projection) = *select {
                    assert_eq!(projection.len(), 2);
                } else {
                    panic!("Expected Select node");
                }

                // Validate inner recursive 'From' node
                if let SqlNode::From(inner_from) = *from {
                    if let SqlNode::Table { name } = *inner_from {
                        assert_eq!(name, "users");
                    } else {
                        panic!("Expected Table node inside FROM clause");
                    }
                } else {
                    panic!("Expected From node");
                }

                // Validate optional inner 'Where' node and BinaryOp
                let where_outer = *r#where.expect("Expected WHERE clause");
                if let SqlNode::Where(inner_where) = where_outer {
                    if let SqlNode::BinaryOp { left, op, right } = *inner_where {
                        assert_eq!(op, "=");
                        if let SqlNode::Column { name } = *left {
                            assert_eq!(name, "id");
                        } else {
                            panic!("Expected Column left operand");
                        }
                        if let SqlNode::Raw(val) = *right {
                            assert_eq!(val, "1");
                        } else {
                            panic!("Expected Raw right operand");
                        }
                    } else {
                        panic!("Expected BinaryOp inside WHERE clause");
                    }
                } else {
                    panic!("Expected Where node");
                }

                assert!(group_by.is_none());
            }
            _ => panic!("Expected Query node at root"),
        }
    }
}
