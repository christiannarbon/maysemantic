use crate::compiler::{LoweringError, MetricResolutionError, RequestParseError, SemanticRequest};
use crate::dialects::SqlDialect;
use crate::graph::JoinResolutionError;
use crate::models::SemanticState;
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
}


pub struct SemanticCompiler {
    state: Arc<SemanticState>,
    dialect: Box<dyn SqlDialect + Send + Sync>,
}

impl SemanticCompiler {
    pub fn new(state: Arc<SemanticState>, dialect: Box<dyn SqlDialect + Send + Sync>) -> Self {
        Self { state, dialect }
    }

    pub fn compile(&self, request: SemanticRequest) -> Result<String, CompilerError> {
        let state_ref = self.state.as_ref();

        // STEP 1: Validate request
        crate::compiler::RequestParser::new(state_ref).validate(&request)?;

        // STEP 2: Find the model containing the metric
        let model = state_ref.models.values()
            .find(|m| m.metrics.iter().any(|metric| metric.name == request.metric_name))
            .ok_or_else(|| CompilerError::RequestParsing(RequestParseError::MetricNotFound(request.metric_name.clone())))?;

        // STEP 3: Resolve metric to typed structs
        let resolved_metric = crate::compiler::MetricResolver::new(model).resolve(&request.metric_name)?;

        // STEP 4: Build semantic graph from the model
        let (graph, node_indices) = crate::graph::build_semantic_graph(state_ref)
            .map_err(|e| CompilerError::CodeGeneration(e.to_string()))?;

        // STEP 5 & 6: Resolve join path with A* (JoinResolver) and zip edges
        let join_resolver = crate::graph::JoinResolver::new(graph, node_indices);
        let mut all_joins = Vec::new();
        let base_entity_name = &resolved_metric.measure_entity.name;

        for (dim_entity, _) in &resolved_metric.dimensions {
            if &dim_entity.name != base_entity_name {
                let path_joins = join_resolver.find_join_path_resolved(base_entity_name, &dim_entity.name)?;
                for j in path_joins {
                    if !all_joins.iter().any(|existing: &crate::compiler::ResolvedJoin| existing.edge == j.edge) {
                        all_joins.push(j);
                    }
                }
            }
        }

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
        let select_node = crate::ast::builder::build_semantic_select(&dims_for_select, &measures_for_select);

        // STEP 9: Build GROUP BY from dimensions
        let group_by_node = if !dims_for_select.is_empty() {
            Some(crate::ast::builder::build_semantic_group_by(&dims_for_select))
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
        };

        // STEP 11: Lower semantic nodes to physical
        let lowered_query = crate::compiler::lowering::SemanticLowering::new(state_ref).lower_node(query_node)?;

        // STEP 12: Generate SQL
        self.dialect.generate_sql(&lowered_query)
            .map_err(|e| CompilerError::CodeGeneration(e.to_string()))
    }
}
