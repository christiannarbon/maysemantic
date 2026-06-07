//! Helper module for constructing the Abstract Syntax Tree (AST).
//!
//! Provides ergonomic builder functions to abstract away memory allocations (e.g., `Box::new()`)
//! and raw variant instantiation when constructing semantic queries.

use crate::ast::{ColumnIdent, Expr, SqlNode, TableIdent};
use crate::compiler::ResolvedJoin;
use crate::graph::GraphNode;

/// Builds a SELECT node containing dimension and measure references.
///
/// # Arguments
/// * `dimensions` - A list of (entity_name, dimension_name) tuples.
/// * `measures` - A list of (entity_name, measure_name) tuples.
pub fn build_semantic_select(dimensions: &[(&str, &str)], measures: &[(&str, &str)]) -> SqlNode {
    // Iterator chains eliminate manual Vec::with_capacity bookkeeping and
    // express the pure transformation intent more clearly.
    let projection: Vec<Expr> = dimensions
        .iter()
        .map(|(entity, dim)| Expr::DimensionRef {
            entity: entity.to_string(),
            dimension: dim.to_string(),
        })
        .chain(measures.iter().map(|(entity, measure)| Expr::MeasureRef {
            entity: entity.to_string(),
            measure: measure.to_string(),
        }))
        .collect();

    SqlNode::Select(projection)
}

/// Builds a GROUP BY node from dimension references.
///
/// # Arguments
/// * `dimensions` - A list of (entity_name, dimension_name) tuples.
pub fn build_semantic_group_by(dimensions: &[(&str, &str)]) -> SqlNode {
    let cols: Vec<Expr> = dimensions
        .iter()
        .map(|(entity, dim)| Expr::DimensionRef {
            entity: entity.to_string(),
            dimension: dim.to_string(),
        })
        .collect();
    SqlNode::GroupBy(cols)
}

/// Builds a root Query node using a TimeSpine as the primary temporal source.
///
/// # Arguments
/// * `granularity` - The time spine granularity (e.g., "day").
/// * `select` - The SELECT node to project.
/// * `group_by` - An optional GROUP BY node.
pub fn build_semantic_timespine_query(
    granularity: &str,
    select: SqlNode,
    group_by: Option<SqlNode>,
) -> SqlNode {
    SqlNode::Query {
        ctes: None,
        select: Box::new(select),
        from: Box::new(SqlNode::From {
            source: Box::new(SqlNode::TimeSpine {
                granularity: granularity.to_string(),
            }),
            joins: vec![],
        }),
        r#where: None,
        group_by: group_by.map(Box::new),
        having: None,
    }
}

/// Builds a Join node from a resolved join hop.
pub fn build_join(resolved_join: &ResolvedJoin) -> SqlNode {
    let left_col = format!(
        "{}.{}",
        resolved_join.left_table.table_name, resolved_join.edge.left_column
    );
    let right_col = format!(
        "{}.{}",
        resolved_join.right_table.table_name, resolved_join.edge.right_column
    );

    SqlNode::Join {
        join_type: resolved_join.edge.join_type,
        relation: Box::new(SqlNode::Table(TableIdent(
            resolved_join.right_table.table_name.clone(),
        ))),
        on: Expr::BinaryOp {
            left: Box::new(Expr::Column(ColumnIdent(left_col))),
            op: "=".to_string(),
            right: Box::new(Expr::Column(ColumnIdent(right_col))),
        },
    }
}

/// Builds a From node including all resolved join paths.
pub fn build_from_join_path(base_entity: &GraphNode, joins: &[ResolvedJoin]) -> SqlNode {
    // Invariant: the first hop must start at the base entity, and each hop must continue
    // from where the previous one ended. These are debug-only guards; the function stays
    // infallible for callers. A violation indicates the caller passed a malformed path.
    if let Some(first) = joins.first() {
        debug_assert_eq!(
            first.left_table.table_name, base_entity.table_name,
            "build_from_join_path: first hop must start at the base entity"
        );
    }
    for pair in joins.windows(2) {
        debug_assert_eq!(
            pair[0].right_table.table_name, pair[1].left_table.table_name,
            "build_from_join_path: join hops must chain (prev.right == next.left)"
        );
    }

    let joins_nodes: Vec<SqlNode> = joins.iter().map(build_join).collect();

    SqlNode::From {
        source: Box::new(SqlNode::Table(TableIdent(base_entity.table_name.clone()))),
        joins: joins_nodes,
    }
}
