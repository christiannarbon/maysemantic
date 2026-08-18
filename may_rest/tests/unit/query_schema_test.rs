use may_core::{FilterOperator, SemanticFilter, SemanticRequest};
use may_rest::routes::query::{QueryMappingError, QueryRequest, QueryResponse, TimeRange};

#[test]
fn single_metric_maps_fields() {
    let req = QueryRequest {
        metrics: vec!["revenue_by_status".into()],
        dimensions: vec!["status".into()],
        filters: vec![SemanticFilter {
            dimension: "status".into(),
            operator: FilterOperator::Eq,
            value: "completed".into(),
        }],
        time_range: Some(TimeRange {
            granularity: Some("day".into()),
            start: None,
            end: None,
        }),
        limit: Some(100),
    };

    let sr = SemanticRequest::try_from(req).expect("maps to SemanticRequest");
    assert_eq!(sr.metric_name, "revenue_by_status");
    assert_eq!(sr.dimensions, vec!["status".to_string()]);
    assert_eq!(sr.filters.len(), 1);
    assert_eq!(sr.time_granularity.as_deref(), Some("day"));
    assert_eq!(sr.limit, Some(100));
}

#[test]
fn empty_metrics_maps_to_error() {
    let req = QueryRequest {
        metrics: vec![],
        dimensions: vec![],
        filters: vec![],
        time_range: None,
        limit: None,
    };
    assert_eq!(
        SemanticRequest::try_from(req),
        Err(QueryMappingError::EmptyMetrics)
    );
}

#[test]
fn two_metrics_maps_to_error() {
    let req = QueryRequest {
        metrics: vec!["a".into(), "b".into()],
        dimensions: vec![],
        filters: vec![],
        time_range: None,
        limit: None,
    };
    assert_eq!(
        SemanticRequest::try_from(req),
        Err(QueryMappingError::MultipleMetricsUnsupported { count: 2 })
    );
}

#[test]
fn minimal_json_deserializes_with_defaults() {
    let req: QueryRequest = serde_json::from_str(r#"{"metrics":["revenue"]}"#).expect("valid json");
    assert_eq!(req.metrics, vec!["revenue".to_string()]);
    assert!(req.dimensions.is_empty());
    assert!(req.filters.is_empty());
    assert!(req.time_range.is_none());
    assert!(req.limit.is_none());
}

#[test]
fn full_request_roundtrips() {
    let original = QueryRequest {
        metrics: vec!["revenue_by_status".into()],
        dimensions: vec!["status".into()],
        filters: vec![SemanticFilter {
            dimension: "status".into(),
            operator: FilterOperator::NotEq,
            value: "cancelled".into(),
        }],
        time_range: Some(TimeRange {
            granularity: Some("month".into()),
            start: Some("2024-01-01".into()),
            end: Some("2024-12-31".into()),
        }),
        limit: Some(50),
    };
    let json = serde_json::to_string(&original).expect("serializes");
    let back: QueryRequest = serde_json::from_str(&json).expect("deserializes");
    assert_eq!(original, back);
}

#[test]
fn unknown_field_is_rejected() {
    let result: Result<QueryRequest, _> =
        serde_json::from_str(r#"{"metrics":["rev"],"dimension":["status"]}"#);
    assert!(
        result.is_err(),
        "a misspelled field must not be silently ignored"
    );
}

#[test]
fn time_range_bounds_are_rejected_not_ignored() {
    let req: QueryRequest = serde_json::from_str(
        r#"{"metrics":["rev"],"time_range":{"start":"2024-01-01","end":"2024-12-31"}}"#,
    )
    .expect("valid json");
    assert_eq!(
        SemanticRequest::try_from(req),
        Err(QueryMappingError::TimeRangeBoundsUnsupported)
    );
}

#[test]
fn time_range_granularity_only_still_maps() {
    let req: QueryRequest =
        serde_json::from_str(r#"{"metrics":["rev"],"time_range":{"granularity":"day"}}"#)
            .expect("valid json");
    let sr = SemanticRequest::try_from(req).expect("maps to SemanticRequest");
    assert_eq!(sr.time_granularity.as_deref(), Some("day"));
}

#[test]
fn blank_metric_name_is_rejected() {
    let req: QueryRequest = serde_json::from_str(r#"{"metrics":["   "]}"#).expect("valid json");
    assert_eq!(
        SemanticRequest::try_from(req),
        Err(QueryMappingError::BlankMetricName)
    );
}

#[test]
fn out_of_range_limit_is_rejected() {
    let zero: QueryRequest =
        serde_json::from_str(r#"{"metrics":["rev"],"limit":0}"#).expect("valid json");
    assert!(matches!(
        SemanticRequest::try_from(zero),
        Err(QueryMappingError::LimitOutOfRange { limit: 0, .. })
    ));

    let huge: QueryRequest =
        serde_json::from_str(r#"{"metrics":["rev"],"limit":100000}"#).expect("valid json");
    assert!(matches!(
        SemanticRequest::try_from(huge),
        Err(QueryMappingError::LimitOutOfRange { limit: 100_000, .. })
    ));
}

#[test]
fn query_response_serializes_to_the_documented_shape() {
    let response = QueryResponse {
        metric: "revenue".into(),
        sql: "SELECT 1".into(),
        columns: vec!["revenue".into()],
        rows: vec![vec![serde_json::json!(42)]],
        row_count: 1,
    };
    let json = serde_json::to_value(&response).expect("serializes");
    assert_eq!(
        json,
        serde_json::json!({
            "metric": "revenue",
            "sql": "SELECT 1",
            "columns": ["revenue"],
            "rows": [[42]],
            "row_count": 1
        })
    );
}
