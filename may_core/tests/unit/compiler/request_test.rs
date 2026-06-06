use may_core::{FilterOperator, SemanticFilter, SemanticRequest};

#[test]
fn test_semantic_request_roundtrip_full() {
    let req = SemanticRequest {
        metric_name: "revenue".to_string(),
        dimensions: vec!["region".to_string()],
        filters: vec![SemanticFilter {
            dimension: "country".to_string(),
            operator: FilterOperator::Eq,
            value: "US".to_string(),
        }],
        time_granularity: Some("month".to_string()),
        limit: Some(100),
    };
    let json = serde_json::to_string(&req).expect("serialize");
    let back: SemanticRequest = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(req, back);
}

#[test]
fn test_semantic_request_roundtrip_minimal() {
    let json = r#"{"metric_name":"orders_count"}"#;
    let req: SemanticRequest = serde_json::from_str(json).expect("deserialize");
    assert_eq!(req.metric_name, "orders_count");
    assert!(req.dimensions.is_empty());
    assert!(req.filters.is_empty());
    assert!(req.time_granularity.is_none());
    assert!(req.limit.is_none());
}

#[test]
fn test_filter_operator_snake_case() {
    let op = FilterOperator::NotEq;
    let json = serde_json::to_string(&op).expect("serialize");
    assert_eq!(json, "\"not_eq\"");
}
