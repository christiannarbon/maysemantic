use may_core::{FilterOperator, SemanticFilter, SemanticRequest};
use may_rest::routes::query::{QueryMappingError, QueryRequest, TimeRange};

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
    let req: QueryRequest =
        serde_json::from_str(r#"{"metrics":["revenue"]}"#).expect("valid json");
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
