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
        link_key: &str,
    ) -> Result<SqlNode, ChasmTrapError> {
        match classification {
            PathClassification::SingleFact | PathClassification::PureDimension => Ok(query),
            PathClassification::MultiFactJoin { fact_tables } => {
                if fact_tables.is_empty() {
                    return Err(ChasmTrapError::EmptyFactTableList);
                }
                Self::build_cte_query(query, fact_tables, link_key)
            }
        }
    }

    /// Builds and injects pre-aggregation CTEs for the specified fact tables.
    ///
    /// // TODO(SQL-ENGINE-REV-1.0.11, orig REV-1.0.5/F1): The generated CTE currently projects/groups by the link key ONLY and does not yet aggregate the fact tables' measures.
    /// // TODO(SQL-ENGINE-REV-1.0.12, orig REV-1.0.5/F2): The outer query's FROM/JOIN chain is NOT yet rewritten to reference the `_agg` CTEs, so the injected CTEs do not yet affect the emitted SQL.
    /// // TODO(SQL-ENGINE-REV-1.0.10, orig REV-1.0.5/F4): A single `link_key` is applied to all fact tables; per-fact join keys are not yet supported.
    fn build_cte_query(
        query: SqlNode,
        fact_tables: &[String],
        link_key: &str,
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
            let mut ctes_vec = ctes.unwrap_or_default();

            // Collect existing CTE aliases to prevent duplicate emission.
            let mut existing_aliases = HashSet::new();
            for cte in &ctes_vec {
                if let SqlNode::CTE { alias, .. } = cte {
                    existing_aliases.insert(alias.0.clone());
                }
            }

            for fact_name in fact_tables {
                let alias_str = format!("{}_agg", fact_name);
                if !existing_aliases.insert(alias_str.clone()) {
                    continue; // Skip generating a duplicate CTE alias
                }
                let alias = TableIdent(alias_str);
                let sub_query = SqlNode::Query {
                    ctes: None,
                    select: Box::new(SqlNode::Select(vec![Expr::Column(ColumnIdent(
                        link_key.to_string(),
                    ))])),
                    from: Box::new(SqlNode::From {
                        source: Box::new(SqlNode::Table(TableIdent(fact_name.clone()))),
                        joins: vec![],
                    }),
                    r#where: None,
                    group_by: Some(Box::new(SqlNode::GroupBy(vec![Expr::Column(ColumnIdent(
                        link_key.to_string(),
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
}
