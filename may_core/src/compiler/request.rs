use serde::{Deserialize, Serialize};

/// The comparison operator for a row-level filter predicate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterOperator {
    Eq,
    NotEq,
    Gt,
    Lt,
    Gte,
    Lte,
    In,
}

/// A single filter predicate applied to a dimension column.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SemanticFilter {
    pub dimension: String,
    pub operator: FilterOperator,
    pub value: String,
}

/// The typed input contract for the SQL Compilation Engine.
///
/// Passed from the PGWire, REST, and MCP entry layers into the compiler.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SemanticRequest {
    pub metric_name: String,
    #[serde(default)]
    pub dimensions: Vec<String>,
    #[serde(default)]
    pub filters: Vec<SemanticFilter>,
    pub time_granularity: Option<String>,
    pub limit: Option<u32>,
}
