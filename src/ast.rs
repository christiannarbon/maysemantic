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
