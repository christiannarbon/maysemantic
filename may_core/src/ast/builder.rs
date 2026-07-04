//! Helper module for constructing the Abstract Syntax Tree (AST).
//!
//! Provides ergonomic builder functions to abstract away memory allocations (e.g., `Box::new()`)
//! and raw variant instantiation when constructing semantic queries.

use crate::ast::{ColumnIdent, Expr, OrderByExpr, SqlNode, TableIdent};
use crate::compiler::ResolvedJoin;
use crate::graph::GraphNode;
use std::collections::HashMap;

/// Builds a SELECT node containing dimension and measure references.
///
/// # Arguments
/// * `model` - Optional model name to qualify references.
/// * `dimensions` - A list of (entity_name, dimension_name) tuples.
/// * `measures` - A list of (entity_name, measure_name) tuples.
pub fn build_semantic_select(
    model: Option<&str>,
    dimensions: &[(&str, &str)],
    measures: &[(&str, &str)],
) -> SqlNode {
    let model_str = model.map(|m| m.to_string());
    // Iterator chains eliminate manual Vec::with_capacity bookkeeping and
    // express the pure transformation intent more clearly.
    let projection: Vec<Expr> = dimensions
        .iter()
        .map(|(entity, dim)| Expr::DimensionRef {
            model: model_str.clone(),
            entity: entity.to_string(),
            dimension: dim.to_string(),
        })
        .chain(measures.iter().map(|(entity, measure)| Expr::MeasureRef {
            model: model_str.clone(),
            entity: entity.to_string(),
            measure: measure.to_string(),
        }))
        .collect();

    SqlNode::Select(projection)
}

/// Builds a GROUP BY node from dimension references.
///
/// # Arguments
/// * `model` - Optional model name to qualify references.
/// * `dimensions` - A list of (entity_name, dimension_name) tuples.
pub fn build_semantic_group_by(model: Option<&str>, dimensions: &[(&str, &str)]) -> SqlNode {
    let model_str = model.map(|m| m.to_string());
    let cols: Vec<Expr> = dimensions
        .iter()
        .map(|(entity, dim)| Expr::DimensionRef {
            model: model_str.clone(),
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
        order_by: vec![],
        limit: None,
        offset: None,
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

    let mut table_counts: HashMap<String, usize> = HashMap::new();
    let mut entity_aliases: HashMap<String, String> = HashMap::new();

    // Base entity
    let base_table = base_entity.table_name.clone();
    let base_count = table_counts.entry(base_table.clone()).or_insert(0);
    let base_alias = if *base_count == 0 {
        base_table.clone()
    } else {
        format!("{}_{}", base_table, base_count)
    };
    *base_count += 1;
    entity_aliases.insert(base_entity.entity_name.clone(), base_alias.clone());

    // Join hops
    for join in joins {
        let right_table = join.right_table.table_name.clone();
        let right_count = table_counts.entry(right_table.clone()).or_insert(0);
        let right_alias = if *right_count == 0 {
            right_table.clone()
        } else {
            format!("{}_{}", right_table, right_count)
        };
        *right_count += 1;
        entity_aliases.insert(join.right_table.entity_name.clone(), right_alias);
    }

    let mut joins_nodes = Vec::new();
    for join in joins {
        let left_alias = entity_aliases
            .get(&join.left_table.entity_name)
            .cloned()
            .unwrap_or_else(|| join.left_table.table_name.clone());
        let right_alias = entity_aliases
            .get(&join.right_table.entity_name)
            .cloned()
            .unwrap_or_else(|| join.right_table.table_name.clone());

        let left_col = format!("{}.{}", left_alias, join.edge.left_column);
        let right_col = format!("{}.{}", right_alias, join.edge.right_column);

        let relation = if right_alias != join.right_table.table_name {
            SqlNode::AliasedTable {
                table: TableIdent(join.right_table.table_name.clone()),
                alias: TableIdent(right_alias),
            }
        } else {
            SqlNode::Table(TableIdent(join.right_table.table_name.clone()))
        };

        joins_nodes.push(SqlNode::Join {
            join_type: join.edge.join_type,
            relation: Box::new(relation),
            on: Expr::BinaryOp {
                left: Box::new(Expr::Column(ColumnIdent(left_col))),
                op: "=".to_string(),
                right: Box::new(Expr::Column(ColumnIdent(right_col))),
            },
        });
    }

    let base_relation = if base_alias != base_entity.table_name {
        SqlNode::AliasedTable {
            table: TableIdent(base_entity.table_name.clone()),
            alias: TableIdent(base_alias),
        }
    } else {
        SqlNode::Table(TableIdent(base_entity.table_name.clone()))
    };

    SqlNode::From {
        source: Box::new(base_relation),
        joins: joins_nodes,
    }
}

/// Attaches ORDER BY / LIMIT / OFFSET to an existing root `Query` node.
///
/// Returns the node unchanged if it is not a `SqlNode::Query` (callers should
/// only pass root query nodes here).
pub fn with_pagination(
    query: SqlNode,
    order_by: Vec<OrderByExpr>,
    limit: Option<u64>,
    offset: Option<u64>,
) -> SqlNode {
    if let SqlNode::Query {
        ctes,
        select,
        from,
        r#where,
        group_by,
        having,
        ..
    } = query
    {
        SqlNode::Query {
            ctes,
            select,
            from,
            r#where,
            group_by,
            having,
            order_by,
            limit,
            offset,
        }
    } else {
        query
    }
}
