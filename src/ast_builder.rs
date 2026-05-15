//! Helper module for constructing the Abstract Syntax Tree (AST).
//!
//! Provides ergonomic builder functions to abstract away memory allocations (e.g., `Box::new()`)
//! and raw variant instantiation when constructing semantic queries.

use crate::ast::SqlNode;

/// A stateless builder for constructing `SqlNode` variants.
pub struct ASTBuilder;

impl ASTBuilder {
    /// Builds a SELECT node containing dimension and measure references.
    ///
    /// # Arguments
    /// * `dimensions` - A list of (entity_name, dimension_name) tuples.
    /// * `measures` - A list of (entity_name, measure_name) tuples.
    pub fn build_semantic_select(
        dimensions: &[(&str, &str)],
        measures: &[(&str, &str)],
    ) -> SqlNode {
        let mut projection = Vec::with_capacity(dimensions.len() + measures.len());

        for (entity, dim) in dimensions {
            projection.push(SqlNode::DimensionRef {
                entity: entity.to_string(),
                dimension: dim.to_string(),
            });
        }

        for (entity, measure) in measures {
            projection.push(SqlNode::MeasureRef {
                entity: entity.to_string(),
                measure: measure.to_string(),
            });
        }

        SqlNode::Select(projection)
    }

    /// Builds a GROUP BY node from dimension references.
    ///
    /// # Arguments
    /// * `dimensions` - A list of (entity_name, dimension_name) tuples.
    pub fn build_semantic_group_by(dimensions: &[(&str, &str)]) -> SqlNode {
        let mut cols = Vec::with_capacity(dimensions.len());
        for (entity, dim) in dimensions {
            cols.push(SqlNode::DimensionRef {
                entity: entity.to_string(),
                dimension: dim.to_string(),
            });
        }
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
}
