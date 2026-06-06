use may_core::compiler::{FilterOperator, SemanticRequest};

#[test]
fn test_semantic_request_roundtrip_full() {
    let json_data = r#"{
        "metric_name": "revenue_by_region",
        "dimensions": ["region", "cohort"],
        "filters": [
            {
                "dimension": "region",
                "operator": "eq",
                "value": "US"
            },
            {
                "dimension": "cohort",
                "operator": "not_eq",
                "value": "2024"
            }
        ],
        "time_granularity": "day",
        "limit": 100
    }"#;

    let request: SemanticRequest = serde_json::from_str(json_data).unwrap();

    assert_eq!(request.metric_name, "revenue_by_region");
    assert_eq!(request.dimensions, vec!["region", "cohort"]);
    assert_eq!(request.filters.len(), 2);
    assert_eq!(request.filters[0].dimension, "region");
    assert_eq!(request.filters[0].operator, FilterOperator::Eq);
    assert_eq!(request.filters[0].value, "US");
    assert_eq!(request.filters[1].dimension, "cohort");
    assert_eq!(request.filters[1].operator, FilterOperator::NotEq);
    assert_eq!(request.filters[1].value, "2024");
    assert_eq!(request.time_granularity, Some("day".to_string()));
    assert_eq!(request.limit, Some(100));

    // Serialize back to check
    let serialized = serde_json::to_string(&request).unwrap();
    let deserialized_again: SemanticRequest = serde_json::from_str(&serialized).unwrap();
    assert_eq!(request, deserialized_again);
}

#[test]
fn test_semantic_request_roundtrip_minimal() {
    let json_data = r#"{
        "metric_name": "minimal_metric"
    }"#;

    let request: SemanticRequest = serde_json::from_str(json_data).unwrap();

    assert_eq!(request.metric_name, "minimal_metric");
    assert!(request.dimensions.is_empty());
    assert!(request.filters.is_empty());
    assert_eq!(request.time_granularity, None);
    assert_eq!(request.limit, None);
}

#[test]
fn test_filter_operator_serde() {
    let op = FilterOperator::NotEq;
    let serialized = serde_json::to_string(&op).unwrap();
    assert_eq!(serialized, "\"not_eq\"");

    let deserialized: FilterOperator = serde_json::from_str("\"not_eq\"").unwrap();
    assert_eq!(deserialized, FilterOperator::NotEq);
}
