use crate::compiler::chasm_trap::{
    ChasmTrapError, ChasmTrapHandler, FactPreAgg, MeasureProjection,
};
use crate::compiler::fanout::{FanOutDetector, PathClassification};
use crate::compiler::rls::{RlsInjector, UserContext};
use crate::compiler::{LoweringError, MetricResolutionError, RequestParseError, SemanticRequest};
use crate::dialects::SqlDialect;
use crate::graph::{GraphError, JoinResolutionError};
use crate::models::EntityType;
use crate::models::SemanticState;
use std::collections::HashSet;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CompilerError {
    #[error("Request validation failed: {0}")]
    RequestParsing(#[from] RequestParseError),

    #[error("Metric resolution failed: {0}")]
    MetricResolution(#[from] MetricResolutionError),

    #[error("Join path resolution failed: {0}")]
    JoinResolution(#[from] JoinResolutionError),

    #[error("AST lowering failed: {0}")]
    Lowering(#[from] LoweringError),

    #[error("SQL code generation failed: {0}")]
    CodeGeneration(String),

    #[error("Unsupported request feature: {0} is not yet supported by the compiler.")]
    UnsupportedRequestFeature(String),

    #[error("Metric '{metric}' is ambiguous; defined in multiple models {models:?}.")]
    AmbiguousMetric { metric: String, models: Vec<String> },

    #[error("Graph construction failed: {0}")]
    GraphConstruction(#[from] GraphError),

    #[error("Chasm trap handling failed: {0}")]
    ChasmTrapHandlingFailed(#[from] ChasmTrapError),

    #[error("Row-level security failed: {0}")]
    Rls(#[from] crate::compiler::rls::RlsError),
}

pub struct SemanticCompiler {
    state: Arc<SemanticState>,
    dialect: Box<dyn SqlDialect + Send + Sync>,
}

impl SemanticCompiler {
    pub fn new(state: Arc<SemanticState>, dialect: Box<dyn SqlDialect + Send + Sync>) -> Self {
        Self { state, dialect }
    }

    pub fn compile(
        &self,
        request: SemanticRequest,
        user_context: Option<&UserContext>,
    ) -> Result<String, CompilerError> {
        if !request.filters.is_empty() {
            return Err(CompilerError::UnsupportedRequestFeature("filters".into()));
        }
        if request.limit.is_some() {
            return Err(CompilerError::UnsupportedRequestFeature("limit".into()));
        }
        if request.time_granularity.is_some() {
            return Err(CompilerError::UnsupportedRequestFeature(
                "time_granularity".into(),
            ));
        }
        if !request.dimensions.is_empty() {
            return Err(CompilerError::UnsupportedRequestFeature(
                "dimensions".into(),
            ));
        }

        let state_ref = self.state.as_ref();

        // STEP 1: Validate request
        crate::compiler::RequestParser::new(state_ref).validate(&request)?;

        // STEP 2: Find the model containing the metric
        let mut matched_models = Vec::new();
        for (name, m) in state_ref.models.iter() {
            if m.metrics
                .iter()
                .any(|metric| metric.name == request.metric_name)
            {
                matched_models.push((name, m));
            }
        }

        let model = match matched_models.len() {
            0 => {
                return Err(CompilerError::RequestParsing(
                    RequestParseError::MetricNotFound(request.metric_name.clone()),
                ));
            }
            1 => {
                let mut matched_iter = matched_models.into_iter();
                match matched_iter.next() {
                    Some((_, m)) => m,
                    None => {
                        return Err(CompilerError::RequestParsing(
                            RequestParseError::MetricNotFound(request.metric_name.clone()),
                        ));
                    }
                }
            }
            _ => {
                let models = matched_models
                    .into_iter()
                    .map(|(name, _)| name.clone())
                    .collect();
                return Err(CompilerError::AmbiguousMetric {
                    metric: request.metric_name.clone(),
                    models,
                });
            }
        };

        // STEP 3: Resolve metric to typed structs
        let resolved_metric =
            crate::compiler::MetricResolver::new(model).resolve(&request.metric_name)?;

        // STEP 4: Build semantic graph from the model
        let (graph, node_indices) = crate::graph::build_semantic_graph(state_ref)?;

        // STEP 5 & 6: Resolve join path with A* (JoinResolver) and zip edges
        let join_resolver = crate::graph::JoinResolver::new(graph, node_indices);
        let mut all_joins = Vec::new();
        let base_entity_name = &resolved_metric.measure_entity.name;

        for (dim_entity, _) in &resolved_metric.dimensions {
            if &dim_entity.name != base_entity_name {
                let path_joins =
                    join_resolver.find_join_path_resolved(base_entity_name, &dim_entity.name)?;
                for j in path_joins {
                    if !all_joins
                        .iter()
                        .any(|existing: &crate::compiler::ResolvedJoin| {
                            existing.left_table.entity_name == j.left_table.entity_name
                                && existing.right_table.entity_name == j.right_table.entity_name
                        })
                    {
                        all_joins.push(j);
                    }
                }
            }
        }

        // Collect path entities and call FanOutDetector::classify
        let find_entity = |name: &str| -> Result<&crate::models::Entity, CompilerError> {
            for m in state_ref.models.values() {
                if let Some(ent) = m.entities.iter().find(|e| e.name == name) {
                    return Ok(ent);
                }
            }
            Err(CompilerError::JoinResolution(
                crate::graph::JoinResolutionError::TableNotFound(name.to_string()),
            ))
        };

        let mut path_entities: Vec<&crate::models::Entity> = Vec::new();
        path_entities.push(find_entity(base_entity_name)?);
        for j in &all_joins {
            path_entities.push(find_entity(&j.right_table.entity_name)?);
        }

        let classification = FanOutDetector::classify(&path_entities);
        tracing::debug!(
            classification = ?classification,
            metric = %request.metric_name,
            "Fan-out classification"
        );

        // STEP 7: Build AST FROM clause
        let base_node = crate::graph::GraphNode {
            entity_name: resolved_metric.measure_entity.name.clone(),
            table_name: resolved_metric.measure_entity.table.clone(),
            primary_key: resolved_metric.measure_entity.primary_key.clone(),
        };
        let from_node = crate::ast::builder::build_from_join_path(&base_node, &all_joins);

        // STEP 8: Build SELECT from resolved_metric
        let mut dims_for_select = Vec::new();
        for (ent, dim) in &resolved_metric.dimensions {
            dims_for_select.push((ent.name.as_str(), dim.name.as_str()));
        }
        let measures_for_select = vec![(
            resolved_metric.measure_entity.name.as_str(),
            resolved_metric.measure.name.as_str(),
        )];
        let select_node =
            crate::ast::builder::build_semantic_select(&dims_for_select, &measures_for_select);

        // STEP 9: Build GROUP BY from dimensions
        let group_by_node = if !dims_for_select.is_empty() {
            Some(crate::ast::builder::build_semantic_group_by(
                &dims_for_select,
            ))
        } else {
            None
        };

        // STEP 10: Assemble SqlNode::Query
        let query_node = crate::ast::SqlNode::Query {
            ctes: None,
            select: Box::new(select_node),
            from: Box::new(from_node),
            r#where: None,
            group_by: group_by_node.map(Box::new),
            having: None,
            order_by: vec![],
            limit: None,
            offset: None,
        };

        let query_node = match &classification {
            PathClassification::MultiFactJoin { fact_tables } => {
                let link_entity = path_entities
                    .iter()
                    .filter(|e| e.entity_type == EntityType::Dimension)
                    .find(|d| {
                        let connected_facts: HashSet<&str> = all_joins
                            .iter()
                            .filter_map(|j| {
                                let left = &j.left_table.entity_name;
                                let right = &j.right_table.entity_name;
                                if left == &d.name && fact_tables.iter().any(|f| f == right) {
                                    Some(right.as_str())
                                } else if right == &d.name && fact_tables.iter().any(|f| f == left)
                                {
                                    Some(left.as_str())
                                } else {
                                    None
                                }
                            })
                            .collect();
                        connected_facts.len() >= 2
                    })
                    .ok_or(CompilerError::ChasmTrapHandlingFailed(
                        ChasmTrapError::LinkDimensionNotFound,
                    ))?;
                let mut facts: Vec<FactPreAgg> = Vec::new();
                for fact_name in fact_tables {
                    let fact_entity = find_entity(fact_name)?;
                    // Fact-side FK column of the hop joining this fact to the link dimension.
                    let key_col = all_joins
                        .iter()
                        .find_map(|j| {
                            if &j.left_table.entity_name == fact_name
                                && j.right_table.entity_name == link_entity.name
                            {
                                Some(j.edge.left_column.clone())
                            } else if &j.right_table.entity_name == fact_name
                                && j.left_table.entity_name == link_entity.name
                            {
                                Some(j.edge.right_column.clone())
                            } else {
                                None
                            }
                        })
                        .ok_or(CompilerError::ChasmTrapHandlingFailed(
                            ChasmTrapError::LinkDimensionNotFound,
                        ))?;

                    // Verify Class 3: fact dimension finer than the conformed key
                    for (dim_entity, dim) in &resolved_metric.dimensions {
                        if &dim_entity.name == fact_name && dim.sql != key_col {
                            return Err(CompilerError::ChasmTrapHandlingFailed(
                                ChasmTrapError::FinerThanConformedGrain {
                                    dimension: dim.name.clone(),
                                    entity: dim_entity.name.clone(),
                                    conformed_grain: key_col.clone(),
                                },
                            ));
                        }
                    }

                    let measures = if fact_entity.name == resolved_metric.measure_entity.name {
                        vec![MeasureProjection {
                            name: resolved_metric.measure.name.clone(),
                            agg: resolved_metric.measure.agg.clone(),
                            sql: resolved_metric.measure.sql.clone(),
                        }]
                    } else {
                        Vec::new()
                    };
                    facts.push(FactPreAgg {
                        entity: fact_entity.name.clone(),
                        table: fact_entity.table.clone(),
                        group_key: key_col,
                        measures,
                    });
                }
                ChasmTrapHandler::inject_ctes(query_node, &classification, &facts)
                    .map_err(CompilerError::ChasmTrapHandlingFailed)?
            }
            _ => query_node,
        };

        // STEP 11: Lower semantic nodes to physical
        let ast =
            crate::compiler::lowering::SemanticLowering::new(state_ref).lower_node(query_node)?;

        // Row-Level Security: inject JWT-claim-derived WHERE predicates.
        // Skipped entirely when no caller context is supplied (e.g. internal tooling).
        let ast = match user_context {
            Some(ctx) => RlsInjector::inject(ast, ctx, state_ref)?,
            None => ast,
        };

        // STEP 12: Generate SQL
        self.dialect
            .generate_sql(&ast)
            .map_err(|e| CompilerError::CodeGeneration(e.to_string()))
    }
}
