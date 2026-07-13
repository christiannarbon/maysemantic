#[cfg(test)]
mod metric_resolver_tests {
    use may_core::{
        AggregationType, Dimension, DimensionType, Entity, EntityType, Measure, Metric,
        SemanticModel,
    };
    use may_core::{MetricResolutionError, MetricResolver};

    fn make_test_model() -> SemanticModel {
        SemanticModel {
            name: "test_model".to_string(),
            entities: vec![
                Entity {
                    name: "users".to_string(),
                    description: None,
                    table: "users_tbl".to_string(),
                    primary_key: "id".to_string(),
                    dimensions: vec![Dimension {
                        name: "user_country".to_string(),
                        description: None,
                        dimension_type: DimensionType::String,
                        sql: "country".to_string(),
                    }],
                    measures: vec![],
                    entity_type: EntityType::Fact,
                    rls_policies: vec![],
                },
                Entity {
                    name: "orders".to_string(),
                    description: None,
                    table: "orders_tbl".to_string(),
                    primary_key: "id".to_string(),
                    dimensions: vec![Dimension {
                        name: "order_status".to_string(),
                        description: None,
                        dimension_type: DimensionType::String,
                        sql: "status".to_string(),
                    }],
                    measures: vec![Measure {
                        name: "revenue".to_string(),
                        description: None,
                        agg: AggregationType::Sum,
                        sql: "amount".to_string(),
                    }],
                    entity_type: EntityType::Fact,
                    rls_policies: vec![],
                },
                Entity {
                    name: "dupe1".to_string(),
                    description: None,
                    table: "dupe1_tbl".to_string(),
                    primary_key: "id".to_string(),
                    dimensions: vec![Dimension {
                        name: "duplicate_dimension".to_string(),
                        description: None,
                        dimension_type: DimensionType::String,
                        sql: "dim".to_string(),
                    }],
                    measures: vec![Measure {
                        name: "duplicate_measure".to_string(),
                        description: None,
                        agg: AggregationType::Sum,
                        sql: "meas".to_string(),
                    }],
                    entity_type: EntityType::Fact,
                    rls_policies: vec![],
                },
                Entity {
                    name: "dupe2".to_string(),
                    description: None,
                    table: "dupe2_tbl".to_string(),
                    primary_key: "id".to_string(),
                    dimensions: vec![Dimension {
                        name: "duplicate_dimension".to_string(),
                        description: None,
                        dimension_type: DimensionType::String,
                        sql: "dim".to_string(),
                    }],
                    measures: vec![Measure {
                        name: "duplicate_measure".to_string(),
                        description: None,
                        agg: AggregationType::Sum,
                        sql: "meas".to_string(),
                    }],
                    entity_type: EntityType::Fact,
                    rls_policies: vec![],
                },
            ],
            metrics: vec![
                Metric {
                    name: "total_revenue".to_string(),
                    description: None,
                    measure: "revenue".to_string(),
                    dimensions: vec!["user_country".to_string(), "order_status".to_string()],
                },
                Metric {
                    name: "invalid_measure_metric".to_string(),
                    description: None,
                    measure: "nonexistent_measure".to_string(),
                    dimensions: vec![],
                },
                Metric {
                    name: "invalid_dimension_metric".to_string(),
                    description: None,
                    measure: "revenue".to_string(),
                    dimensions: vec!["nonexistent_dim".to_string()],
                },
                Metric {
                    name: "partial_invalid_dimensions".to_string(),
                    description: None,
                    measure: "revenue".to_string(),
                    dimensions: vec!["user_country".to_string(), "nonexistent_dim".to_string()],
                },
                Metric {
                    name: "ambiguous_measure_metric".to_string(),
                    description: None,
                    measure: "duplicate_measure".to_string(),
                    dimensions: vec![],
                },
                Metric {
                    name: "ambiguous_dimension_metric".to_string(),
                    description: None,
                    measure: "revenue".to_string(),
                    dimensions: vec!["duplicate_dimension".to_string()],
                },
                Metric {
                    name: "duplicate_dimensions_metric".to_string(),
                    description: None,
                    measure: "revenue".to_string(),
                    dimensions: vec![
                        "user_country".to_string(),
                        "order_status".to_string(),
                        "user_country".to_string(),
                    ],
                },
                Metric {
                    name: "distinct_dimensions_metric".to_string(),
                    description: None,
                    measure: "revenue".to_string(),
                    dimensions: vec![
                        "order_status".to_string(),
                        "user_country".to_string(),
                    ],
                },
            ],
            joins: vec![],
        }
    }

    #[test]
    fn test_resolve_valid_metric() {
        let model = make_test_model();
        let resolver = MetricResolver::new(&model);
        let result = resolver
            .resolve("total_revenue")
            .expect("Should resolve successfully");

        assert_eq!(result.metric.name, "total_revenue");
        assert_eq!(result.measure_entity.name, "orders");
        assert_eq!(result.measure.name, "revenue");
        assert_eq!(result.dimensions.len(), 2);
        assert_eq!(result.dimensions[0].0.name, "users");
        assert_eq!(result.dimensions[0].1.name, "user_country");
        assert_eq!(result.dimensions[1].0.name, "orders");
        assert_eq!(result.dimensions[1].1.name, "order_status");
    }

    #[test]
    fn test_resolve_metric_not_found() {
        let model = make_test_model();
        let resolver = MetricResolver::new(&model);
        let err = resolver.resolve("unknown_metric").unwrap_err();
        match err {
            MetricResolutionError::MetricNotFound(name) => assert_eq!(name, "unknown_metric"),
            _ => panic!("Expected MetricNotFound"),
        }
    }

    #[test]
    fn test_resolve_measure_not_found() {
        let model = make_test_model();
        let resolver = MetricResolver::new(&model);
        let err = resolver.resolve("invalid_measure_metric").unwrap_err();
        match err {
            MetricResolutionError::MeasureNotFound(measure, metric) => {
                assert_eq!(measure, "nonexistent_measure");
                assert_eq!(metric, "invalid_measure_metric");
            }
            _ => panic!("Expected MeasureNotFound"),
        }
    }

    #[test]
    fn test_resolve_dimension_not_found() {
        let model = make_test_model();
        let resolver = MetricResolver::new(&model);
        let err = resolver.resolve("invalid_dimension_metric").unwrap_err();
        match err {
            MetricResolutionError::DimensionNotFound(dim, metric) => {
                assert_eq!(dim, "nonexistent_dim");
                assert_eq!(metric, "invalid_dimension_metric");
            }
            _ => panic!("Expected DimensionNotFound"),
        }
    }

    #[test]
    fn test_resolve_partial_dimensions() {
        let model = make_test_model();
        let resolver = MetricResolver::new(&model);
        let err = resolver.resolve("partial_invalid_dimensions").unwrap_err();
        match err {
            MetricResolutionError::DimensionNotFound(dim, metric) => {
                assert_eq!(dim, "nonexistent_dim");
                assert_eq!(metric, "partial_invalid_dimensions");
            }
            _ => panic!("Expected DimensionNotFound"),
        }
    }

    #[test]
    fn test_resolve_ambiguous_measure() {
        let model = make_test_model();
        let resolver = MetricResolver::new(&model);
        let err = resolver.resolve("ambiguous_measure_metric").unwrap_err();
        match err {
            MetricResolutionError::AmbiguousMeasure {
                measure,
                metric,
                entities,
            } => {
                assert_eq!(measure, "duplicate_measure");
                assert_eq!(metric, "ambiguous_measure_metric");
                assert!(entities.contains(&"dupe1".to_string()));
                assert!(entities.contains(&"dupe2".to_string()));
            }
            _ => panic!("Expected AmbiguousMeasure"),
        }
    }

    #[test]
    fn test_resolve_ambiguous_dimension() {
        let model = make_test_model();
        let resolver = MetricResolver::new(&model);
        let err = resolver.resolve("ambiguous_dimension_metric").unwrap_err();
        match err {
            MetricResolutionError::AmbiguousDimension {
                dimension,
                metric,
                entities,
            } => {
                assert_eq!(dimension, "duplicate_dimension");
                assert_eq!(metric, "ambiguous_dimension_metric");
                assert!(entities.contains(&"dupe1".to_string()));
                assert!(entities.contains(&"dupe2".to_string()));
            }
            _ => panic!("Expected AmbiguousDimension"),
        }
    }

    #[test]
    fn test_resolve_de_duplicates_metric_dimensions_preserving_order() {
        let model = make_test_model();
        let resolver = MetricResolver::new(&model);
        let result = resolver
            .resolve("duplicate_dimensions_metric")
            .expect("Should resolve successfully");

        // The input has ["user_country", "order_status", "user_country"]
        // The output must de-duplicate user_country, leaving exactly two resolved dimensions
        // in first-seen order: ["user_country", "order_status"]
        assert_eq!(result.dimensions.len(), 2);
        assert_eq!(result.dimensions[0].1.name, "user_country");
        assert_eq!(result.dimensions[1].1.name, "order_status");
    }

    #[test]
    fn test_resolve_distinct_metric_dimensions_preserves_order() {
        let model = make_test_model();
        let resolver = MetricResolver::new(&model);
        let result = resolver
            .resolve("distinct_dimensions_metric")
            .expect("Should resolve successfully");

        // The input has ["order_status", "user_country"]
        // The output must retain both dimensions in original order
        assert_eq!(result.dimensions.len(), 2);
        assert_eq!(result.dimensions[0].1.name, "order_status");
        assert_eq!(result.dimensions[1].1.name, "user_country");
    }
}
