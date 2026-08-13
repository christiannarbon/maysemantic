use may_core::SemanticFilter;
use serde::{Deserialize, Serialize};

/// Optional time window / grain for a query.
///
/// v1 uses only `granularity` (mapped to `SemanticRequest.time_granularity`).
/// `start` / `end` are accepted for forward-compatibility but are **not yet**
/// applied by the compiler — documented as reserved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimeRange {
    #[serde(default)]
    pub granularity: Option<String>,
    #[serde(default)]
    pub start: Option<String>,
    #[serde(default)]
    pub end: Option<String>,
}

/// The JSON body accepted by `POST /api/v1/query`.
///
/// `metrics` is an array for forward-compatibility, but v1 requires exactly one
/// entry (enforced by `TryFrom<QueryRequest> for SemanticRequest`, added in 2.T2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueryRequest {
    pub metrics: Vec<String>,
    #[serde(default)]
    pub dimensions: Vec<String>,
    #[serde(default)]
    pub filters: Vec<SemanticFilter>,
    #[serde(default)]
    pub time_range: Option<TimeRange>,
    #[serde(default)]
    pub limit: Option<u32>,
}
