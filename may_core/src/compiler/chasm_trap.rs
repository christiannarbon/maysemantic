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
                let alias_str = format!("{}_agg", fact.table);
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
                rewritten_from =
                    Self::rewrite_from_and_joins(rewritten_from, &fact.table, &fact.group_key);
            }

            // 2. SELECT measure substitution
            let mut select_exprs = match *select {
                SqlNode::Select(exprs) => exprs,
                _ => return Err(ChasmTrapError::NotAQueryNode),
            };

            for expr in &mut select_exprs {
                Self::substitute_select_measure(expr, facts);
            }

            Ok(SqlNode::Query {
                ctes,
                select: Box::new(SqlNode::Select(select_exprs)),
                from: Box::new(rewritten_from),
                r#where,
                group_by,
                having,
            })
        } else {
            Err(ChasmTrapError::NotAQueryNode)
        }
    }

    fn rewrite_from_and_joins(from_node: SqlNode, table: &str, group_key: &str) -> SqlNode {
        match from_node {
            SqlNode::From { source, joins } => {
                let new_source = match *source {
                    SqlNode::Table(TableIdent(ref name)) if name == table => {
                        SqlNode::Table(TableIdent(format!("{}_agg", table)))
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
                                SqlNode::Table(TableIdent(ref name)) if name == table => {
                                    SqlNode::Table(TableIdent(format!("{}_agg", table)))
                                }
                                other => other,
                            };
                            let new_on = Self::rewrite_expr(on, table, group_key);
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

    fn rewrite_expr(expr: Expr, table: &str, group_key: &str) -> Expr {
        match expr {
            Expr::Column(ColumnIdent(col_name)) => {
                if col_name == table {
                    Expr::Column(ColumnIdent(format!("{}_agg", table)))
                } else if col_name.starts_with(&format!("{}.", table)) {
                    Expr::Column(ColumnIdent(format!("{}_agg.{}", table, group_key)))
                } else {
                    Expr::Column(ColumnIdent(col_name))
                }
            }
            Expr::BinaryOp { left, op, right } => Expr::BinaryOp {
                left: Box::new(Self::rewrite_expr(*left, table, group_key)),
                op,
                right: Box::new(Self::rewrite_expr(*right, table, group_key)),
            },
            Expr::Function { name, args } => Expr::Function {
                name,
                args: args
                    .into_iter()
                    .map(|arg| Self::rewrite_expr(arg, table, group_key))
                    .collect(),
            },
            Expr::Aliased { expr, alias } => Expr::Aliased {
                expr: Box::new(Self::rewrite_expr(*expr, table, group_key)),
                alias,
            },
            other => other,
        }
    }

    fn substitute_select_measure(expr: &mut Expr, facts: &[FactPreAgg]) {
        match expr {
            Expr::MeasureRef { entity, measure } => {
                for fact in facts {
                    if &fact.entity == entity {
                        if let Some(m) = fact.measures.iter().find(|m| &m.name == measure) {
                            *expr =
                                Expr::Column(ColumnIdent(format!("{}_agg.{}", fact.table, m.sql)));
                            break;
                        }
                    }
                }
            }
            Expr::BinaryOp { left, right, .. } => {
                Self::substitute_select_measure(left, facts);
                Self::substitute_select_measure(right, facts);
            }
            Expr::Function { args, .. } => {
                for arg in args {
                    Self::substitute_select_measure(arg, facts);
                }
            }
            Expr::Aliased { expr: inner, .. } => {
                Self::substitute_select_measure(inner, facts);
            }
            _ => {}
        }
    }
}
