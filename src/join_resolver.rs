//! Join Resolver for the Semantic Layer.
//!
//! Given a populated `SemanticGraph`, this module uses the A* search algorithm
//! (`petgraph::algo::astar`) to find the shortest sequence of edges (JOIN conditions)
//! connecting any two entities. The result is a strictly ordered `Vec<GraphEdge>`
//! that the SQL compiler uses to emit the exact JOIN clauses needed.
//!
//! ## Algorithm Guarantee
//! A* with a uniform edge cost of `1` and a zero heuristic is mathematically
//! equivalent to Dijkstra's algorithm. It provably returns the minimum number
//! of JOINs required to connect the source to the target, with no redundant hops.

use crate::graph_engine::{GraphEdge, GraphNode, SemanticGraph};
use petgraph::algo::astar;
use petgraph::graph::NodeIndex;
use std::collections::HashMap;
use thiserror::Error;

/// Errors that can occur during join path resolution.
#[derive(Error, Debug, PartialEq)]
pub enum JoinResolutionError {
    /// The requested source or target entity name is not present in the graph.
    #[error("Unknown entity: '{0}' was not found in the semantic graph.")]
    UnknownEntity(String),

    /// A* found no path connecting the two entities.
    /// This means the tables are structurally disconnected in the YAML definitions.
    #[error(
        "No join path exists between '{from_entity}' and '{to_entity}'. \
         Check that a join definition connects these entities."
    )]
    NoPathFound {
        from_entity: String,
        to_entity: String,
    },

    /// An edge expected to exist between two consecutive path nodes was not found.
    /// This indicates an internal graph consistency error and should never occur
    /// in a correctly constructed `SemanticGraph`.
    #[error(
        "Internal graph error: expected an edge between '{from_entity}' and '{to_entity}' \
         in the resolved path, but none was found."
    )]
    BrokenPath {
        from_entity: String,
        to_entity: String,
    },
}

/// Resolves join paths between entities by running shortest-path search
/// over the in-memory `SemanticGraph`.
pub struct JoinResolver {
    graph: SemanticGraph,
    /// Maps entity names (e.g., `"orders"`) to their `NodeIndex` in `graph`.
    node_indices: HashMap<String, NodeIndex>,
}

impl JoinResolver {
    /// Constructs a `JoinResolver` from a pre-built `SemanticGraph` and its
    /// associated name-to-index mapping.
    ///
    /// Both are produced by `graph_engine::build_semantic_graph`.
    pub fn new(graph: SemanticGraph, node_indices: HashMap<String, NodeIndex>) -> Self {
        Self {
            graph,
            node_indices,
        }
    }

    /// Returns the `GraphNode` metadata for an entity by name, or `None` if missing.
    fn get_node(&self, entity_name: &str) -> Option<(NodeIndex, &GraphNode)> {
        let idx = self.node_indices.get(entity_name)?;
        let node = self.graph.node_weight(*idx)?;
        Some((*idx, node))
    }

    /// Finds the shortest ordered sequence of JOIN conditions connecting
    /// `source_table` to `target_table` within the `SemanticGraph`.
    ///
    /// # Algorithm
    /// Runs `petgraph::algo::astar` with:
    /// - **Edge cost:** uniform `1` per hop (no edge is preferred over another).
    /// - **Heuristic:** `0` (zero heuristic degrades A* to Dijkstra's, guaranteeing
    ///   the globally shortest path, not just a locally greedy one).
    ///
    /// A* returns the path as a `Vec<NodeIndex>`. This function then iterates
    /// over consecutive node pairs using `.windows(2)`, resolves the connecting
    /// `GraphEdge` via `find_edge`, and returns the ordered join sequence.
    ///
    /// # Errors
    /// - `JoinResolutionError::UnknownEntity` if either name is not in the graph.
    /// - `JoinResolutionError::NoPathFound` if the tables are disconnected.
    /// - `JoinResolutionError::BrokenPath` if graph internal consistency fails.
    pub fn find_join_path(
        &self,
        source_table: &str,
        target_table: &str,
    ) -> Result<Vec<GraphEdge>, JoinResolutionError> {
        // Resolve both entity names to NodeIndexes, failing fast on unknown names.
        let (source_idx, _) = self
            .get_node(source_table)
            .ok_or_else(|| JoinResolutionError::UnknownEntity(source_table.to_string()))?;

        let (target_idx, _) = self
            .get_node(target_table)
            .ok_or_else(|| JoinResolutionError::UnknownEntity(target_table.to_string()))?;

        // Short-circuit: source and target are the same entity, no JOINs needed.
        if source_idx == target_idx {
            return Ok(vec![]);
        }

        // Run A* with uniform edge weight (equivalent to Dijkstra's).
        // The heuristic is 0 because we have no spatial or cost estimate
        // that would allow us to prioritize one unexplored node over another.
        let astar_result = astar(
            &self.graph,
            source_idx,
            |node| node == target_idx,
            |_edge| 1u32,
            |_node| 0u32,
        );

        // Extract the ordered path of NodeIndexes, or surface a clear error.
        let (_cost, path) = astar_result.ok_or_else(|| JoinResolutionError::NoPathFound {
            from_entity: source_table.to_string(),
            to_entity: target_table.to_string(),
        })?;

        // Reconstruct the edge sequence from the node path.
        // path = [n0, n1, n2, ...]. windows(2) yields [n0,n1], [n1,n2], ...
        // For each consecutive pair, we resolve the GraphEdge that connects them.
        path.windows(2)
            .map(|window| {
                let (from_idx, to_idx) = (window[0], window[1]);

                // Resolve the edge. This must always succeed for a valid graph path,
                // but we guard it to avoid any silent data corruption.
                let edge_idx = self.graph.find_edge(from_idx, to_idx).ok_or_else(|| {
                    JoinResolutionError::BrokenPath {
                        from_entity: self
                            .graph
                            .node_weight(from_idx)
                            .map(|n| n.entity_name.as_str())
                            .unwrap_or("unknown")
                            .to_string(),
                        to_entity: self
                            .graph
                            .node_weight(to_idx)
                            .map(|n| n.entity_name.as_str())
                            .unwrap_or("unknown")
                            .to_string(),
                    }
                })?;

                // edge_weight is infallible for a valid EdgeIndex in the same graph.
                let edge = self.graph.edge_weight(edge_idx).ok_or_else(|| {
                    JoinResolutionError::BrokenPath {
                        from_entity: from_idx.index().to_string(),
                        to_entity: to_idx.index().to_string(),
                    }
                })?;

                Ok(edge.clone())
            })
            .collect()
    }
}
