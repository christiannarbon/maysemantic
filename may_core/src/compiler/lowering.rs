use crate::ast::{ColumnIdent, Expr, SqlNode};
use crate::models::SemanticState;
use std::collections::{HashMap, HashSet};
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum LoweringError {
    #[error("Cannot lower reference: entity '{entity}' was not found in the semantic state.")]
    EntityNotFound { entity: String },

    #[error("Cannot lower DimensionRef: dimension '{dimension}' not found in entity '{entity}'.")]
    DimensionNotFound { entity: String, dimension: String },

    #[error("Cannot lower MeasureRef: measure '{measure}' not found in entity '{entity}'.")]
    MeasureNotFound { entity: String, measure: String },

    #[error("Cannot lower DimensionRef: dimension '{dimension}' for entity '{entity}' is ambiguous; defined in multiple models {models:?}.")]
    AmbiguousDimension {
        entity: String,
        dimension: String,
        models: Vec<String>,
    },

    #[error("Cannot lower MeasureRef: measure '{measure}' for entity '{entity}' is ambiguous; defined in multiple models {models:?}.")]
    AmbiguousMeasure {
        entity: String,
        measure: String,
        models: Vec<String>,
    },
}

pub struct SemanticLowering<'a> {
    pub state: &'a SemanticState,
    // (entity_name, dimension_name) -> Vec<(model_name, dimension_sql)>
    dim_index: HashMap<(String, String), Vec<(String, String)>>,
    // (entity_name, measure_name) -> Vec<(model_name, lowered_measure_expr)>
    measure_index: HashMap<(String, String), Vec<(String, Expr)>>,
    // entity_name -> Set of model_names that define it
    entity_models: HashMap<String, HashSet<String>>,
}

impl<'a> SemanticLowering<'a> {
    pub fn new(state: &'a SemanticState) -> Self {
        let mut dim_index: HashMap<(String, String), Vec<(String, String)>> = HashMap::new();
        let mut measure_index: HashMap<(String, String), Vec<(String, Expr)>> = HashMap::new();
        let mut entity_models: HashMap<String, HashSet<String>> = HashMap::new();

        for (model_name, model) in &state.models {
            for entity in &model.entities {
                entity_models
                    .entry(entity.name.clone())
                    .or_default()
                    .insert(model_name.clone());

                for dim in &entity.dimensions {
                    dim_index
                        .entry((entity.name.clone(), dim.name.clone()))
                        .or_default()
                        .push((model_name.clone(), dim.sql.clone()));
                }

                for measure in &entity.measures {
                    let expr = measure.agg.to_expr(Expr::Column(ColumnIdent(measure.sql.clone())));
                    measure_index
                        .entry((entity.name.clone(), measure.name.clone()))
                        .or_default()
                        .push((model_name.clone(), expr));
                }
            }
        }

        Self {
            state,
            dim_index,
            measure_index,
            entity_models,
        }
    }

    pub fn lower_expr(&self, expr: Expr) -> Result<Expr, LoweringError> {
        match expr {
            Expr::DimensionRef {
                model,
                entity,
                dimension,
            } => {
                let entity_seen = if let Some(ref m_name) = model {
                    self.entity_models
                        .get(&entity)
                        .map(|models| models.contains(m_name))
                        .unwrap_or(false)
                } else {
                    self.entity_models.contains_key(&entity)
                };

                let mut hits = Vec::new();
                if let Some(all_hits) = self.dim_index.get(&(entity.clone(), dimension.clone())) {
                    for (m_name, sql) in all_hits {
                        if let Some(ref filter_m) = model {
                            if m_name == filter_m {
                                hits.push((m_name.clone(), sql.clone()));
                            }
                        } else {
                            hits.push((m_name.clone(), sql.clone()));
                        }
                    }
                }

                match hits.len() {
                    0 if !entity_seen => Err(LoweringError::EntityNotFound { entity }),
                    0 => Err(LoweringError::DimensionNotFound { entity, dimension }),
                    1 => Ok(Expr::Column(ColumnIdent(
                        hits.into_iter().next().expect("len==1").1,
                    ))),
                    _ => Err(LoweringError::AmbiguousDimension {
                        entity,
                        dimension,
                        models: hits.into_iter().map(|(m, _)| m).collect(),
                    }),
                }
            }
            Expr::MeasureRef {
                model,
                entity,
                measure,
            } => {
                let entity_seen = if let Some(ref m_name) = model {
                    self.entity_models
                        .get(&entity)
                        .map(|models| models.contains(m_name))
                        .unwrap_or(false)
                } else {
                    self.entity_models.contains_key(&entity)
                };

                let mut hits = Vec::new();
                if let Some(all_hits) = self.measure_index.get(&(entity.clone(), measure.clone())) {
                    for (m_name, expr) in all_hits {
                        if let Some(ref filter_m) = model {
                            if m_name == filter_m {
                                hits.push((m_name.clone(), expr.clone()));
                            }
                        } else {
                            hits.push((m_name.clone(), expr.clone()));
                        }
                    }
                }

                match hits.len() {
                    0 if !entity_seen => Err(LoweringError::EntityNotFound { entity }),
                    0 => Err(LoweringError::MeasureNotFound { entity, measure }),
                    1 => Ok(hits.into_iter().next().expect("len==1").1),
                    _ => Err(LoweringError::AmbiguousMeasure {
                        entity,
                        measure,
                        models: hits.into_iter().map(|(m, _)| m).collect(),
                    }),
                }
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
            Expr::Column(_) | Expr::Literal(_) | Expr::Raw(_) => Ok(expr),
            Expr::Aliased { expr, alias } => Ok(Expr::Aliased {
                expr: Box::new(self.lower_expr(*expr)?),
                alias,
            }),
            Expr::DateTrunc {
                granularity,
                column,
            } => Ok(Expr::DateTrunc {
                granularity,
                column: Box::new(self.lower_expr(*column)?),
            }),
            Expr::Cast { expr, target_type } => Ok(Expr::Cast {
                expr: Box::new(self.lower_expr(*expr)?),
                target_type,
            }),
            Expr::JsonAccess { column, path } => Ok(Expr::JsonAccess {
                column: Box::new(self.lower_expr(*column)?),
                path,
            }),
            Expr::Unnest { expr } => Ok(Expr::Unnest {
                expr: Box::new(self.lower_expr(*expr)?),
            }),
            Expr::CountDistinct(inner) => {
                Ok(Expr::CountDistinct(Box::new(self.lower_expr(*inner)?)))
            }
            Expr::CountStar => Ok(Expr::CountStar),
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
                order_by,
                limit,
                offset,
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
                let order_by = order_by
                    .into_iter()
                    .map(|o| {
                        Ok(crate::ast::OrderByExpr {
                            expr: self.lower_expr(o.expr)?,
                            direction: o.direction,
                        })
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?;
                Ok(SqlNode::Query {
                    ctes,
                    select,
                    from,
                    r#where,
                    group_by,
                    having,
                    order_by,
                    limit,
                    offset,
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
            SqlNode::Table(_) | SqlNode::AliasedTable { .. } | SqlNode::TimeSpine { .. } => {
                Ok(node)
            }
        }
    }
}
