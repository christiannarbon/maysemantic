//! Abstract Syntax Tree (AST) definitions for the SQL Compilation Engine.
//!
//! This module defines the intermediate representation used by the compiler
//! before generating dialect-specific SQL strings.

/// Defines the types of standard SQL joins supported by the semantic engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JoinType {
    Inner,
    Left,
    Full,
}

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
        /// Optional Common Table Expressions (WITH clauses).
        ctes: Option<Vec<SqlNode>>,
        /// The SELECT clause node.
        select: Box<SqlNode>,
        /// The FROM clause node.
        from: Box<SqlNode>,
        /// An optional WHERE clause node.
        r#where: Option<Box<SqlNode>>,
        /// An optional GROUP BY clause node.
        group_by: Option<Box<SqlNode>>,
        /// An optional HAVING clause node.
        having: Option<Box<SqlNode>>,
    },

    /// Represents a Common Table Expression (CTE), typically found in a WITH clause.
    CTE {
        /// The alias of the CTE (e.g., `pre_aggregated_orders`).
        alias: String,
        /// The inner recursive query defining the CTE body.
        query: Box<SqlNode>,
    },

    /// Represents the projection list of a SELECT clause.
    Select(Vec<SqlNode>),

    /// Represents the source of a FROM clause, potentially accompanied by a linear list of joins.
    From {
        /// The base table or subquery.
        source: Box<SqlNode>,
        /// An ordered list of Join nodes.
        joins: Vec<SqlNode>,
    },

    /// Represents a relational JOIN.
    Join {
        /// The type of join (Inner, Left, Full).
        join_type: JoinType,
        /// The target relation being joined.
        relation: Box<SqlNode>,
        /// The ON condition for the join.
        on: Box<SqlNode>,
    },

    /// Represents the conditional logic of a WHERE clause.
    Where(Box<SqlNode>),

    /// Represents the list of columns or expressions in a GROUP BY clause.
    GroupBy(Vec<SqlNode>),

    /// Represents the conditional logic of a HAVING clause.
    Having(Box<SqlNode>),

    /// Represents a binary operation (e.g., `id = 1` or `price > 100`).
    BinaryOp {
        /// The left side of the operation.
        left: Box<SqlNode>,
        /// The operator string (e.g., "=", ">", "AND").
        op: String,
        /// The right side of the operation.
        right: Box<SqlNode>,
    },

    /// Represents an aggregate or scalar function (e.g., `SUM(amount)` or `COUNT(1)`).
    Function {
        /// The name of the function (e.g., "SUM").
        name: String,
        /// The arguments passed to the function.
        args: Vec<SqlNode>,
    },

    /// Represents a base table or view reference (e.g., `users`).
    Table {
        /// The name of the table.
        name: String,
    },

    /// Represents a column reference (e.g., `user_id` or `users.id`).
    Column {
        /// The name of the column, optionally fully qualified.
        name: String,
    },

    /// A dummy variant containing a raw string payload.
    ///
    /// This is used to validate the base recursion model and provides an escape
    /// hatch for raw SQL injection during complex edge cases.
    Raw(String),
}
