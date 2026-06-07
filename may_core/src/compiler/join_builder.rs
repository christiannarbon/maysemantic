use crate::graph::{GraphEdge, GraphNode};

/// A join hop enriched with the table names needed to emit SQL.
#[derive(Debug, Clone)]
pub struct ResolvedJoin {
    /// The left (source) entity for this hop.
    pub left_table: GraphNode,
    /// The right (target) entity being joined.
    pub right_table: GraphNode,
    /// The edge defining columns and join type.
    pub edge: GraphEdge,
}
