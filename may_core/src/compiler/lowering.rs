use crate::ast::{ColumnIdent, Expr, SqlNode};
use crate::models::SemanticState;
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum LoweringError {
    #[error("Cannot lower reference: entity '{entity}' was not found in the semantic state.")]
    EntityNotFound { entity: String },

    #[error("Cannot lower DimensionRef: dimension '{dimension}' not found in entity '{entity}'.")]
    DimensionNotFound { entity: String, dimension: String },

    #[error("Cannot lower MeasureRef: measure '{measure}' not found in entity '{entity}'.")]
    MeasureNotFound { entity: String, measure: String },
}

pub struct SemanticLowering<'a> {
    pub state: &'a SemanticState,
}

impl<'a> SemanticLowering<'a> {
    pub fn new(state: &'a SemanticState) -> Self {
        Self { state }
    }

    pub fn lower_expr(&self, expr: Expr) -> Result<Expr, LoweringError> {
        match expr {
            Expr::DimensionRef { entity, dimension } => {
                for model in self.state.models.values() {
                    if let Some(e) = model.entities.iter().find(|e| e.name == entity) {
                        if let Some(d) = e.dimensions.iter().find(|d| d.name == dimension) {
                            return Ok(Expr::Column(ColumnIdent(format!("{}.{}", e.table, d.sql))));
                        }
                        return Err(LoweringError::DimensionNotFound { entity, dimension });
                    }
                }
                Err(LoweringError::EntityNotFound { entity })
            }
            Expr::MeasureRef { entity, measure } => {
                for model in self.state.models.values() {
                    if let Some(e) = model.entities.iter().find(|e| e.name == entity) {
                        if let Some(m) = e.measures.iter().find(|m| m.name == measure) {
                            return Ok(m.agg.to_expr(Expr::Column(ColumnIdent(format!(
                                "{}.{}",
                                e.table, m.sql
                            )))));
                        }
                        return Err(LoweringError::MeasureNotFound { entity, measure });
                    }
                }
                Err(LoweringError::EntityNotFound { entity })
            }
            Expr::BinaryOp { left, op, right } => Ok(Expr::BinaryOp {
                left: Box::new(self.lower_expr(*left)?),
                op,
                right: Box::new(self.lower_expr(*right)?),
            }),
            Expr::Function { name, args } => {
                let lowered_args = args
                    .into_iter()
                    .map(|arg| self.lower_expr(arg))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Expr::Function {
                    name,
                    args: lowered_args,
                })
            }
            other => Ok(other),
        }
    }

    pub fn lower_node(&self, node: SqlNode) -> Result<SqlNode, LoweringError> {
        match node {
            SqlNode::Query {
                ctes,
                select,
                from,
                r#where,
                group_by,
                having,
            } => {
                let ctes = ctes
                    .map(|c| {
                        c.into_iter()
                            .map(|n| self.lower_node(n))
                            .collect::<Result<Vec<_>, _>>()
                    })
                    .transpose()?;
                let select = Box::new(self.lower_node(*select)?);
                let from = Box::new(self.lower_node(*from)?);
                let r#where = r#where
                    .map(|n| self.lower_node(*n))
                    .transpose()?
                    .map(Box::new);
                let group_by = group_by
                    .map(|n| self.lower_node(*n))
                    .transpose()?
                    .map(Box::new);
                let having = having
                    .map(|n| self.lower_node(*n))
                    .transpose()?
                    .map(Box::new);
                Ok(SqlNode::Query {
                    ctes,
                    select,
                    from,
                    r#where,
                    group_by,
                    having,
                })
            }
            SqlNode::CTE { alias, query } => {
                let query = Box::new(self.lower_node(*query)?);
                Ok(SqlNode::CTE { alias, query })
            }
            SqlNode::Select(exprs) => {
                let lowered = exprs
                    .into_iter()
                    .map(|e| self.lower_expr(e))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(SqlNode::Select(lowered))
            }
            SqlNode::From { source, joins } => {
                let source = Box::new(self.lower_node(*source)?);
                let joins = joins
                    .into_iter()
                    .map(|j| self.lower_node(j))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(SqlNode::From { source, joins })
            }
            SqlNode::Join {
                join_type,
                relation,
                on,
            } => {
                let relation = Box::new(self.lower_node(*relation)?);
                let on = self.lower_expr(on)?;
                Ok(SqlNode::Join {
                    join_type,
                    relation,
                    on,
                })
            }
            SqlNode::Where(expr) => Ok(SqlNode::Where(self.lower_expr(expr)?)),
            SqlNode::GroupBy(exprs) => {
                let lowered = exprs
                    .into_iter()
                    .map(|e| self.lower_expr(e))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(SqlNode::GroupBy(lowered))
            }
            SqlNode::Having(expr) => Ok(SqlNode::Having(self.lower_expr(expr)?)),
            other => Ok(other),
        }
    }
}
