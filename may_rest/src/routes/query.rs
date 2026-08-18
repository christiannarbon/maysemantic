use may_core::{SemanticFilter, SemanticRequest};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Upper bound on `QueryRequest.limit`, mirroring the input-validation style of
/// `routes::auth` (`MAX_USERNAME_LEN`). Guards the warehouse from an unbounded scan.
const MAX_LIMIT: u32 = 10_000;

/// Optional time window / grain for a query.
///
/// v1 supports only `granularity` (mapped to `SemanticRequest.time_granularity`).
/// `start` / `end` are part of the wire shape but are **rejected** with
/// `QueryMappingError::TimeRangeBoundsUnsupported` until the compiler can apply
/// them — an accepted-but-ignored time window would silently return the wrong rows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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
    #[error("metric name must not be blank")]
    BlankMetricName,
    #[error("`time_range.start` / `time_range.end` are not supported yet; use `granularity`")]
    TimeRangeBoundsUnsupported,
    #[error("`limit` must be between 1 and {max}; got {limit}")]
    LimitOutOfRange { limit: u32, max: u32 },
}

impl TryFrom<QueryRequest> for SemanticRequest {
    type Error = QueryMappingError;

    fn try_from(req: QueryRequest) -> Result<Self, Self::Error> {
        let count = req.metrics.len();
        let mut metrics = req.metrics.into_iter();
        let metric_name = match (metrics.next(), metrics.next()) {
            (None, _) => return Err(QueryMappingError::EmptyMetrics),
            (Some(name), None) => name,
            (Some(_), Some(_)) => {
                return Err(QueryMappingError::MultipleMetricsUnsupported { count });
            }
        };

        if metric_name.trim().is_empty() {
            return Err(QueryMappingError::BlankMetricName);
        }

        if let Some(limit) = req.limit.filter(|&l| l == 0 || l > MAX_LIMIT) {
            return Err(QueryMappingError::LimitOutOfRange {
                limit,
                max: MAX_LIMIT,
            });
        }

        let time_granularity = match req.time_range {
            Some(range) => {
                // Reject rather than ignore: silently dropping a time window would return
                // all-time numbers for a request that asked for a bounded window.
                if range.start.is_some() || range.end.is_some() {
                    return Err(QueryMappingError::TimeRangeBoundsUnsupported);
                }
                range.granularity
            }
            None => None,
        };

        Ok(SemanticRequest {
            metric_name,
            dimensions: req.dimensions,
            filters: req.filters,
            time_granularity,
            limit: req.limit,
        })
    }
}
