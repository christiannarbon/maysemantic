//! Abstract Syntax Tree (AST) definitions for the SQL Compilation Engine.
//!
//! The `SqlNode` acts as the intermediate representation (IR) bridging the gap between
//! the high-level semantic models (YAML definitions) and the final target-specific
//! SQL strings.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Strongly-typed wrapper for Table or CTE identifiers to prevent string-based mixups.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableIdent(pub String);

impl std::fmt::Display for TableIdent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Strongly-typed wrapper for Column identifiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnIdent(pub String);

impl std::fmt::Display for ColumnIdent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Specifies the type of SQL Join operation.
///
/// This is the single source of truth for join type across both the AST and the
/// YAML configuration models, eliminating any duplication.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum JoinType {
    /// Standard INNER JOIN (records must exist in both tables).
    Inner,
    /// LEFT OUTER JOIN (preserves all records from the left table).
    Left,
    /// FULL OUTER JOIN (preserves all records from both tables).
    Full,
}

/// Represents evaluable SQL expressions (values, columns, functions, and binary operations).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    /// Represents a column reference (e.g., `user_id` or `users.id`).
    Column(ColumnIdent),

    /// Represents a raw string literal or numeric value.
    Literal(String),

    /// Represents a binary operation (e.g., `a = b` or `x > 10`).
    BinaryOp {
        /// The left-hand operand.
        left: Box<Expr>,
        /// The operator string (`=`, `>`, `<`, etc.).
        op: String,
        /// The right-hand operand.
        right: Box<Expr>,
    },

    /// Represents a standard SQL function (e.g., `SUM(amount)`, `COUNT(id)`).
    Function {
        /// The name of the function.
        name: String,
        /// The arguments passed to the function.
        args: Vec<Expr>,
    },

    /// Represents a reference to a semantic dimension defined in the configuration.
    DimensionRef {
        /// The entity containing the dimension.
        entity: String,
        /// The name of the dimension.
        dimension: String,
    },

    /// Represents a reference to a semantic measure defined in the configuration.
    MeasureRef {
        /// The entity containing the measure.
        entity: String,
        /// The name of the measure.
        measure: String,
    },

    /// Represents raw, unescaped SQL text injected directly into an expression.
    ///
    /// # Safety
    /// This variant writes its contents directly into the SQL output buffer
    /// with **zero escaping or validation**. It must NEVER be constructed from
    /// unsanitized user input. Callers are responsible for ensuring the string
    /// is safe, trusted SQL.
    Raw(String),
}

/// A highly recursive Abstract Syntax Tree node used to model both physical
/// relational algebra and semantic metric queries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SqlNode {
    /// This acts as the root node for a standard statement, composing the distinct
    /// clauses together.
    Query {
        /// Optional Common Table Expressions (WITH clauses).
        ctes: Option<Vec<SqlNode>>,
        /// The SELECT clause node.
        select: Box<SqlNode>,
        /// The FROM clause node.
        from: Box<SqlNode>,
        /// The optional WHERE clause node.
        r#where: Option<Box<SqlNode>>,
        /// The optional GROUP BY clause node.
        group_by: Option<Box<SqlNode>>,
        /// The optional HAVING clause node.
        having: Option<Box<SqlNode>>,
    },

    /// Represents a Common Table Expression (CTE), typically found in a WITH clause.
    CTE {
        /// The alias of the CTE (e.g., `pre_aggregated_orders`).
        alias: TableIdent,
        /// The inner recursive query defining the CTE body.
        query: Box<SqlNode>,
    },

    /// Represents the projection list of a SELECT clause.
    Select(Vec<Expr>),

    /// Represents the FROM clause, containing a primary source and optional JOINs.
    From {
        /// The primary driving table or subquery.
        source: Box<SqlNode>,
        /// A sequential list of JOIN operations applied to the source.
        joins: Vec<SqlNode>,
    },

    /// Represents a single JOIN operation within a FROM clause.
    Join {
        /// The type of join (Inner, Left, Full).
        join_type: JoinType,
        /// The target relation being joined (Table or subquery).
        relation: Box<SqlNode>,
        /// The boolean expression defining the join constraint.
        on: Expr,
    },

    /// Represents the WHERE clause for row-level filtering.
    Where(Expr),

    /// Represents the GROUP BY clause for defining aggregation levels.
    GroupBy(Vec<Expr>),

    /// Represents the HAVING clause for post-aggregation filtering.
    Having(Expr),

    /// Represents a base table or view reference (e.g., `users`).
    Table(TableIdent),

    /// Represents a synthetic date/time dimension table used as a temporal scaffold.
    TimeSpine {
        /// The temporal granularity (e.g., "day", "month", "year").
        granularity: String,
    },
}
