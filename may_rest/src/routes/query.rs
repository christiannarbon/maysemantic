use std::sync::Arc;

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
};
use may_core::{
    SemanticFilter, SemanticRequest,
    compiler::{CompilerError, SemanticCompiler},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

use crate::{
    AppState,
    error::ApiError,
    middleware::{auth::AuthClaims, json::ValidatedJson},
};

/// Routes for the semantic query API. Nested under `/api/v1` by the caller.
pub fn router() -> Router<AppState> {
    Router::new().route("/query", post(query_handler))
}

/// Compiles the incoming semantic query request into dialect SQL and returns
/// the standardized response envelope.
///
/// # Errors
///
/// Returns `QueryApiError` with HTTP 400 if mapping or metric resolution fails,
/// or HTTP 500 if an internal compiler or lock error occurs.
pub async fn query_handler(
    State(state): State<AppState>,
    // Authenticated like every other non-health route. RLS scoping (JWT -> UserContext)
    // stays deferred per the epic; this only establishes *who* is asking.
    _claims: AuthClaims,
    ValidatedJson(payload): ValidatedJson<QueryRequest>,
) -> Result<Json<QueryResponse>, QueryApiError> {
    // 1. REST DTO -> compiler contract (400 on mapping error via `?`).
    let request = SemanticRequest::try_from(payload)?;
    let metric = request.metric_name.clone();

    // 2. Compile in a tight scope so the read lock is dropped before we return.
    let sql = {
        let state_lock = state.state_mgr.get_state();
        let state_guard = state_lock.read().map_err(|_| {
            QueryApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to acquire read lock on semantic state",
            )
        })?;

        let state_arc = Arc::clone(&state_guard);
        let compiler = SemanticCompiler::new(state_arc, may_core::dialect_for(&state.dialect_kind));

        // RLS user context is `None` for now, exactly as may_pgwire does today.
        compiler
            .compile(request, None)
            .map_err(QueryApiError::from_compiler_error)?
    }; // read lock released here, before building the response

    // 3. Standardized envelope. Rows are a documented mock (no warehouse execution yet).
    let columns = vec![metric.clone()];
    let rows: Vec<Vec<serde_json::Value>> = vec![vec![serde_json::Value::Null]];
    let row_count = rows.len();

    Ok(Json(QueryResponse {
        metric,
        sql,
        columns,
        rows,
        row_count,
    }))
}

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

impl QueryResponse {
    /// Builds a response with `row_count` derived from `rows`, so the two cannot drift.
    /// This is the only supported way to construct one.
    #[must_use]
    pub fn new(
        metric: String,
        sql: String,
        columns: Vec<String>,
        rows: Vec<Vec<serde_json::Value>>,
    ) -> Self {
        Self {
            row_count: rows.len(),
            metric,
            sql,
            columns,
            rows,
        }
    }
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

impl From<QueryMappingError> for ApiError {
    fn from(err: QueryMappingError) -> Self {
        // Every variant is a client-side contract violation, so they all map to 400.
        // Story 4 adds `From<CompilerError>` here with its own status mapping.
        crate::error::api_error(axum::http::StatusCode::BAD_REQUEST, err.to_string())
    }
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

/// An error surfaced by `POST /api/v1/query`, carrying its own HTTP status.
#[derive(Debug)]
pub struct QueryApiError {
    pub status: StatusCode,
    pub message: String,
}

impl QueryApiError {
    #[must_use]
    pub(crate) fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    /// Maps a compiler error to an HTTP status: caller-caused problems become
    /// `400`, internal/config failures become `500`.
    #[must_use]
    #[allow(
        clippy::needless_pass_by_value,
        reason = "consumed by callers via Result::map_err(QueryApiError::from_compiler_error)"
    )]
    pub fn from_compiler_error(err: CompilerError) -> Self {
        let status = match &err {
            CompilerError::RequestParsing(_)
            | CompilerError::MetricResolution(_)
            | CompilerError::JoinResolution(_)
            | CompilerError::UnsupportedRequestFeature(_)
            | CompilerError::AmbiguousMetric { .. } => StatusCode::BAD_REQUEST,

            CompilerError::Lowering(_)
            | CompilerError::CodeGeneration(_)
            | CompilerError::GraphConstruction(_)
            | CompilerError::ChasmTrapHandlingFailed(_)
            | CompilerError::Rls(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        Self::new(status, err.to_string())
    }
}

impl From<QueryMappingError> for QueryApiError {
    fn from(err: QueryMappingError) -> Self {
        Self::new(StatusCode::BAD_REQUEST, err.to_string())
    }
}

impl From<QueryApiError> for ApiError {
    fn from(err: QueryApiError) -> Self {
        crate::error::api_error(err.status, err.message)
    }
}

impl IntoResponse for QueryApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.message }))).into_response()
    }
}
