//! Graph Engine for the Semantic Layer.
//!
//! Converts the declarative YAML models into a directed graph (`petgraph::DiGraph`)
//! to allow for shortest-path algorithms to resolve JOIN paths automatically.

use crate::ast::JoinType;
use crate::state_mgr::SemanticState;
use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GraphError {
    #[error("Missing node: Entity '{0}' was referenced in a join but not found in the state.")]
    MissingNode(String),
}

/// Represents a table or entity in the graph.
#[derive(Debug, Clone, PartialEq)]
pub struct GraphNode {
    pub entity_name: String,
    pub table_name: String,
    pub primary_key: String,
}

/// Represents a relationship (JOIN) between two entities in the graph.
#[derive(Debug, Clone, PartialEq)]
pub struct GraphEdge {
    pub left_column: String,
    pub right_column: String,
    pub join_type: JoinType,
}

/// A specialized directed graph for resolving semantic joins.
pub type SemanticGraph = DiGraph<GraphNode, GraphEdge>;

/// Scans the entire SemanticState and constructs an active Directed Graph.
/// Returns the constructed graph and a mapping of entity names to their graph NodeIndex.
pub fn build_semantic_graph(
    state: &SemanticState,
) -> Result<(SemanticGraph, HashMap<String, NodeIndex>), GraphError> {
    let mut graph = SemanticGraph::new();
    let mut node_indices = HashMap::new();

    // Pass 1: Insert all entities as Nodes.
    // flat_map collapses models → entities into a single flat iterator, making
    // the true iteration subject (entity) immediately clear to the reader.
    for entity in state.models.values().flat_map(|m| m.entities.iter()) {
        let node = GraphNode {
            entity_name: entity.name.clone(),
            table_name: entity.table.clone(),
            primary_key: entity.primary_key.clone(),
        };
        let idx = graph.add_node(node);
        node_indices.insert(entity.name.clone(), idx);
    }

    // Pass 2: Insert all relationships as Edges.
    for model in state.models.values() {
        for join in &model.joins {
            let left_idx = node_indices
                .get(&join.left_entity)
                .ok_or_else(|| GraphError::MissingNode(join.left_entity.clone()))?;

            let right_idx = node_indices
                .get(&join.right_entity)
                .ok_or_else(|| GraphError::MissingNode(join.right_entity.clone()))?;

            let edge = GraphEdge {
                left_column: join.left_column.clone(),
                right_column: join.right_column.clone(),
                join_type: join.join_type.clone(),
            };

            // Add directed edge from Left to Right
            graph.add_edge(*left_idx, *right_idx, edge);
        }
    }

    Ok((graph, node_indices))
}
