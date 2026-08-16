use may_core::{SemanticFilter, SemanticRequest};
use serde::{Deserialize, Serialize};
use thiserror::Error;

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

/// The standardized response envelope for `POST /api/v1/query`.
///
/// `sql` is the real compiled dialect SQL. `columns`/`rows` are a mocked result
/// set in this epic — warehouse execution is owned by the Connectors epic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueryResponse {
    pub metric: String,
    pub sql: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<serde_json::Value>>,
    pub row_count: usize,
}

/// Error raised while translating a REST `QueryRequest` into a `SemanticRequest`.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum QueryMappingError {
    #[error("`metrics` must contain exactly one metric; got none")]
    EmptyMetrics,
    #[error("multi-metric queries are not supported yet; got {count} metrics")]
    MultipleMetricsUnsupported { count: usize },
}

impl TryFrom<QueryRequest> for SemanticRequest {
    type Error = QueryMappingError;

    fn try_from(req: QueryRequest) -> Result<Self, Self::Error> {
        let metric_name = match req.metrics.len() {
            0 => return Err(QueryMappingError::EmptyMetrics),
            1 => req.metrics.into_iter().next().unwrap_or_default(),
            count => return Err(QueryMappingError::MultipleMetricsUnsupported { count }),
        };

        let time_granularity = req.time_range.and_then(|t| t.granularity);

        Ok(SemanticRequest {
            metric_name,
            dimensions: req.dimensions,
            filters: req.filters,
            time_granularity,
            limit: req.limit,
        })
    }
}
