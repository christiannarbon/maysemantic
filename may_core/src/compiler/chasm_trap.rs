use thiserror::Error;
use crate::ast::SqlNode;
use crate::compiler::fanout::PathClassification;

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
        // To be implemented in SQL-ENGINE-3.3.T2
        let _ = (query, fact_tables, link_key);
        todo!("CTE injection — implement in SQL-ENGINE-3.3.T2")
    }
}
