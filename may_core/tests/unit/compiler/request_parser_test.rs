use may_core::compiler::{RequestParseError, RequestParser, SemanticRequest};
use may_core::models::{
    AggregationType, Dimension, DimensionType, Entity, EntityType, Measure, Metric, SemanticModel,
    SemanticState,
};
use std::collections::HashMap;

fn make_test_state() -> SemanticState {
    let dimension = Dimension {
        name: "region".to_string(),
        description: None,
        dimension_type: DimensionType::String,
        sql: "orders.region".to_string(),
    };

    let measure = Measure {
        name: "revenue".to_string(),
        description: None,
        agg: AggregationType::Sum,
        sql: "amount".to_string(),
    };

    let entity = Entity {
        name: "orders".to_string(),
        description: None,
        table: "orders_table".to_string(),
        primary_key: "id".to_string(),
        dimensions: vec![dimension],
        measures: vec![measure],
        entity_type: EntityType::Fact,
        rls_policies: vec![],
    };

    let metric = Metric {
        name: "revenue_by_region".to_string(),
        description: None,
        measure: "revenue".to_string(),
        dimensions: vec!["region".to_string()],
    };

    let model = SemanticModel {
        name: "ecommerce".to_string(),
        entities: vec![entity],
        metrics: vec![metric],
        joins: vec![],
    };

    let mut models = HashMap::new();
    models.insert("ecommerce".to_string(), model);

    let mut state = SemanticState::new();
    state.models = models;
    state
}

#[test]
fn test_valid_request_passes() {
    let state = make_test_state();
    let parser = RequestParser::new(&state);
    let request = SemanticRequest {
        metric_name: "revenue_by_region".to_string(),
        dimensions: vec!["region".to_string()],
        filters: vec![],
        time_granularity: None,
        limit: None,
    };
    let result = parser.validate(&request);
    assert_eq!(result, Ok(()));
}

#[test]
fn test_empty_metric_name_rejected() {
    let state = make_test_state();
    let parser = RequestParser::new(&state);
    let request = SemanticRequest {
        metric_name: "".to_string(),
        dimensions: vec!["region".to_string()],
        filters: vec![],
        time_granularity: None,
        limit: None,
    };
    let result = parser.validate(&request);
    assert_eq!(result, Err(RequestParseError::EmptyMetricName));
}

#[test]
fn test_whitespace_metric_name_rejected() {
    let state = make_test_state();
    let parser = RequestParser::new(&state);
    let request = SemanticRequest {
        metric_name: "   ".to_string(),
        dimensions: vec!["region".to_string()],
        filters: vec![],
        time_granularity: None,
        limit: None,
    };
    let result = parser.validate(&request);
    assert_eq!(result, Err(RequestParseError::EmptyMetricName));
}

#[test]
fn test_unknown_metric_rejected() {
    let state = make_test_state();
    let parser = RequestParser::new(&state);
    let request = SemanticRequest {
        metric_name: "nonexistent".to_string(),
        dimensions: vec!["region".to_string()],
        filters: vec![],
        time_granularity: None,
        limit: None,
    };
    let result = parser.validate(&request);
    assert_eq!(
        result,
        Err(RequestParseError::MetricNotFound("nonexistent".to_string()))
    );
}

#[test]
fn test_unknown_dimension_rejected() {
    let state = make_test_state();
    let parser = RequestParser::new(&state);
    let request = SemanticRequest {
        metric_name: "revenue_by_region".to_string(),
        dimensions: vec!["nonexistent_dim".to_string()],
        filters: vec![],
        time_granularity: None,
        limit: None,
    };
    let result = parser.validate(&request);
    assert_eq!(
        result,
        Err(RequestParseError::DimensionNotFound(
            "nonexistent_dim".to_string()
        ))
    );
}
