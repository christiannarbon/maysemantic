use crate::ast::{ColumnIdent, Expr, SqlNode, TableIdent};
use crate::compiler::fanout::PathClassification;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ChasmTrapError {
    #[error("inject_ctes requires a SqlNode::Query root, but received a different variant")]
    NotAQueryNode,

    #[error("MultiFactJoin classification contains an empty fact_tables list")]
    EmptyFactTableList,
}

pub struct ChasmTrapHandler;

impl ChasmTrapHandler {
    /// Apply pre-aggregation CTE injection if the path classification requires it.
    ///
    /// - `SingleFact` and `PureDimension` → returns `query` unchanged (zero cost)
    /// - `MultiFactJoin` → delegates to `build_cte_query` (implemented in SQL-ENGINE-3.3.T2)
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

    fn build_cte_query(
        query: SqlNode,
        fact_tables: &[String],
        link_key: &str,
    ) -> Result<SqlNode, ChasmTrapError> {
        if let SqlNode::Query {
            ctes: _,
            select,
            from,
            r#where,
            group_by,
            having,
        } = query
        {
            let mut ctes_vec = Vec::new();
            for fact_name in fact_tables {
                let alias = TableIdent(format!("{}_agg", fact_name));
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
