//! Helper module for constructing the Abstract Syntax Tree (AST).
//!
//! Provides ergonomic builder functions to abstract away memory allocations (e.g., `Box::new()`)
//! and raw variant instantiation when constructing semantic queries.

use crate::ast::{Expr, SqlNode};

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
