use crate::ast::{ColumnIdent, Expr, SqlNode, TableIdent};
use crate::compiler::fanout::PathClassification;
use std::collections::HashSet;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ChasmTrapError {
    #[error("inject_ctes requires a SqlNode::Query root, but received a different variant")]
    NotAQueryNode,

    #[error("MultiFactJoin classification contains an empty fact_tables list")]
    EmptyFactTableList,

    #[error("No conformed dimension table found in the join path for a MultiFactJoin")]
    LinkDimensionNotFound,

    #[error("Average aggregation in chasm-trap joins is not yet supported")]
    UnsupportedAverageAggregation,

    #[error("dimension '{dimension}' (entity '{entity}') is finer than the conformed chasm-trap grain '{conformed_grain}' and cannot be selected through a pre-aggregated join")]
    FinerThanConformedGrain {
        dimension: String,
        entity: String,
        conformed_grain: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasureProjection {
    pub name: String,
    pub agg: crate::models::AggregationType,
    pub sql: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactPreAgg {
    pub entity: String,
    pub table: String,
    pub group_key: String,
    pub measures: Vec<MeasureProjection>,
}

pub struct ChasmTrapHandler;

impl ChasmTrapHandler {
    /// The pre-aggregation CTE alias for a fact table, as a single unqualified,
    /// identifier-safe token (e.g. `public.orders` -> `orders_agg`).
    fn agg_alias(table: &str) -> String {
        let base = table.rsplit('.').next().unwrap_or(table);
        format!("{}_agg", base)
    }

    /// Apply pre-aggregation CTE injection if the path classification requires it.
    ///
    /// - `SingleFact` and `PureDimension` → returns `query` unchanged (zero cost)
    /// - `MultiFactJoin` → delegates to `build_cte_query` (implemented in SQL-ENGINE-3.3.T2)
    ///
    /// Note: CTE injection has temporary limitations (see TODO(SQL-ENGINE-REV-1.0.5/*) on `build_cte_query`).
    /// Completion is tracked by SQL-ENGINE-REV-1.0.10 (F4) → REV-1.0.11 (F1) → REV-1.0.12 (F2).
    pub fn inject_ctes(
        query: SqlNode,
        classification: &PathClassification,
        facts: &[FactPreAgg],
    ) -> Result<SqlNode, ChasmTrapError> {
        match classification {
            PathClassification::SingleFact | PathClassification::PureDimension => Ok(query),
            PathClassification::MultiFactJoin { .. } => {
                if facts.is_empty() {
                    return Err(ChasmTrapError::EmptyFactTableList);
                }
                let cte_query = Self::build_cte_query(query, facts)?;
                Self::rewrite_outer_query(cte_query, facts)
            }
        }
    }

    /// Builds and injects pre-aggregation CTEs for the specified fact tables.
    ///
    /// // TODO(SQL-ENGINE-REV-1.0.11, orig REV-1.0.5/F1): The generated CTE currently projects/groups by the link key ONLY and does not yet aggregate the fact tables' measures.
    /// // TODO(SQL-ENGINE-REV-1.0.12, orig REV-1.0.5/F2): The outer query's FROM/JOIN chain is NOT yet rewritten to reference the `_agg` CTEs, so the injected CTEs do not yet affect the emitted SQL.
    /// // TODO(SQL-ENGINE-REV-1.0.10, orig REV-1.0.5/F4): A single `link_key` is applied to all fact tables; per-fact join keys are not yet supported.
    fn build_cte_query(query: SqlNode, facts: &[FactPreAgg]) -> Result<SqlNode, ChasmTrapError> {
        if let SqlNode::Query {
            ctes,
            select,
            from,
            r#where,
            group_by,
            having,
        } = query
        {
            let mut ctes_vec = ctes.unwrap_or_default();

            // Collect existing CTE aliases to prevent duplicate emission.
            let mut existing_aliases = HashSet::new();
            for cte in &ctes_vec {
                if let SqlNode::CTE { alias, .. } = cte {
                    existing_aliases.insert(alias.0.clone());
                }
            }

            for fact in facts {
                let alias_str = Self::agg_alias(&fact.table);
                if !existing_aliases.insert(alias_str.clone()) {
                    continue; // Skip generating a duplicate CTE alias
                }
                let alias = TableIdent(alias_str);

                let mut select_exprs = vec![Expr::Column(ColumnIdent(fact.group_key.clone()))];
                for m in &fact.measures {
                    select_exprs.push(Expr::Aliased {
                        expr: Box::new(m.agg.to_expr(Expr::Column(ColumnIdent(m.sql.clone())))),
                        alias: m.sql.clone(),
                    });
                }

                let sub_query = SqlNode::Query {
                    ctes: None,
                    select: Box::new(SqlNode::Select(select_exprs)),
                    from: Box::new(SqlNode::From {
                        source: Box::new(SqlNode::Table(TableIdent(fact.table.clone()))),
                        joins: vec![],
                    }),
                    r#where: None,
                    group_by: Some(Box::new(SqlNode::GroupBy(vec![Expr::Column(ColumnIdent(
                        fact.group_key.clone(),
                    ))]))),
                    having: None,
                };
                ctes_vec.push(SqlNode::CTE {
                    alias,
                    query: Box::new(sub_query),
                });
            }

            Ok(SqlNode::Query {
                ctes: Some(ctes_vec),
                select,
                from,
                r#where,
                group_by,
                having,
            })
        } else {
            Err(ChasmTrapError::NotAQueryNode)
        }
    }

    fn rewrite_outer_query(
        query: SqlNode,
        facts: &[FactPreAgg],
    ) -> Result<SqlNode, ChasmTrapError> {
        if let SqlNode::Query {
            ctes,
            select,
            from,
            r#where,
            group_by,
            having,
        } = query
        {
            // 1. Swap table name in FROM / JOIN and rewrite JOIN ON
            let mut rewritten_from = *from;
            for fact in facts {
                rewritten_from = Self::rewrite_from_and_joins(rewritten_from, fact);
            }

            // 2. SELECT measure/dimension substitution
            let mut select_exprs = match *select {
                SqlNode::Select(exprs) => exprs,
                _ => return Err(ChasmTrapError::NotAQueryNode),
            };

            for expr in &mut select_exprs {
                Self::rewrite_outer_expr(expr, facts);
            }

            // 3. GROUP BY dimension/key substitution
            let rewritten_group_by = if let Some(gb) = group_by {
                let mut gb_node = *gb;
                if let SqlNode::GroupBy(ref mut exprs) = gb_node {
                    for expr in exprs {
                        Self::rewrite_outer_expr(expr, facts);
                    }
                }
                Some(Box::new(gb_node))
            } else {
                None
            };

            // 4. HAVING clause substitution
            let rewritten_having = if let Some(hv) = having {
                let mut hv_node = *hv;
                if let SqlNode::Having(ref mut expr) = hv_node {
                    Self::rewrite_outer_expr(expr, facts);
                }
                Some(Box::new(hv_node))
            } else {
                None
            };

            // 5. WHERE clause substitution
            let rewritten_where = if let Some(wh) = r#where {
                let mut wh_node = *wh;
                if let SqlNode::Where(ref mut expr) = wh_node {
                    Self::rewrite_outer_expr(expr, facts);
                }
                Some(Box::new(wh_node))
            } else {
                None
            };

            Ok(SqlNode::Query {
                ctes,
                select: Box::new(SqlNode::Select(select_exprs)),
                from: Box::new(rewritten_from),
                r#where: rewritten_where,
                group_by: rewritten_group_by,
                having: rewritten_having,
            })
        } else {
            Err(ChasmTrapError::NotAQueryNode)
        }
    }

    fn rewrite_from_and_joins(from_node: SqlNode, fact: &FactPreAgg) -> SqlNode {
        match from_node {
            SqlNode::From { source, joins } => {
                let new_source = match *source {
                    SqlNode::Table(TableIdent(ref name)) if name == &fact.table => {
                        SqlNode::Table(TableIdent(Self::agg_alias(&fact.table)))
                    }
                    other => other,
                };
                let new_joins = joins
                    .into_iter()
                    .map(|join| match join {
                        SqlNode::Join {
                            join_type,
                            relation,
                            on,
                        } => {
                            let new_relation = match *relation {
                                SqlNode::Table(TableIdent(ref name)) if name == &fact.table => {
                                    SqlNode::Table(TableIdent(Self::agg_alias(&fact.table)))
                                }
                                other => other,
                            };
                            let new_on = Self::rewrite_expr(on, fact);
                            SqlNode::Join {
                                join_type,
                                relation: Box::new(new_relation),
                                on: new_on,
                            }
                        }
                        other => other,
                    })
                    .collect();
                SqlNode::From {
                    source: Box::new(new_source),
                    joins: new_joins,
                }
            }
            other => other,
        }
    }

    pub fn rewrite_expr(expr: Expr, fact: &FactPreAgg) -> Expr {
        match expr {
            Expr::Column(ColumnIdent(col_name)) => {
                let prefix = format!("{}.", fact.table);
                if let Some(suffix) = col_name.strip_prefix(&prefix) {
                    Expr::Column(ColumnIdent(format!(
                        "{}.{}",
                        Self::agg_alias(&fact.table),
                        suffix
                    )))
                } else {
                    Expr::Column(ColumnIdent(col_name))
                }
            }
            Expr::DimensionRef { entity, dimension } => {
                if entity == fact.entity {
                    Expr::Column(ColumnIdent(format!(
                        "{}.{}",
                        Self::agg_alias(&fact.table),
                        fact.group_key
                    )))
                } else {
                    Expr::DimensionRef { entity, dimension }
                }
            }
            Expr::BinaryOp { left, op, right } => Expr::BinaryOp {
                left: Box::new(Self::rewrite_expr(*left, fact)),
                op,
                right: Box::new(Self::rewrite_expr(*right, fact)),
            },
            Expr::Function { name, args } => Expr::Function {
                name,
                args: args
                    .into_iter()
                    .map(|arg| Self::rewrite_expr(arg, fact))
                    .collect(),
            },
            Expr::Aliased { expr, alias } => Expr::Aliased {
                expr: Box::new(Self::rewrite_expr(*expr, fact)),
                alias,
            },
            other => other,
        }
    }

    fn reaggregate(agg: &crate::models::AggregationType) -> crate::models::AggregationType {
        match agg {
            crate::models::AggregationType::Sum => crate::models::AggregationType::Sum,
            crate::models::AggregationType::Count => crate::models::AggregationType::Sum,
            crate::models::AggregationType::Min => crate::models::AggregationType::Min,
            crate::models::AggregationType::Max => crate::models::AggregationType::Max,
            crate::models::AggregationType::Average => {
                debug_assert!(
                    false,
                    "Average aggregation should have been rejected upstream"
                );
                crate::models::AggregationType::Sum
            }
        }
    }

    fn rewrite_outer_expr(expr: &mut Expr, facts: &[FactPreAgg]) {
        match expr {
            Expr::MeasureRef { entity, measure } => {
                for fact in facts {
                    if &fact.entity == entity {
                        if let Some(m) = fact.measures.iter().find(|m| &m.name == measure) {
                            *expr = Self::reaggregate(&m.agg).to_expr(Expr::Column(ColumnIdent(
                                format!("{}.{}", Self::agg_alias(&fact.table), m.sql),
                            )));
                            break;
                        }
                    }
                }
            }
            Expr::DimensionRef {
                entity,
                dimension: _,
            } => {
                for fact in facts {
                    if &fact.entity == entity {
                        *expr = Expr::Column(ColumnIdent(format!(
                            "{}.{}",
                            Self::agg_alias(&fact.table),
                            fact.group_key
                        )));
                        break;
                    }
                }
            }
            Expr::BinaryOp { left, right, .. } => {
                Self::rewrite_outer_expr(left, facts);
                Self::rewrite_outer_expr(right, facts);
            }
            Expr::Function { args, .. } => {
                for arg in args {
                    Self::rewrite_outer_expr(arg, facts);
                }
            }
            Expr::Aliased { expr: inner, .. } => {
                Self::rewrite_outer_expr(inner, facts);
            }
            _ => {}
        }
    }
}
