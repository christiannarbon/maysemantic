use crate::models::{Dimension, Entity, Measure, Metric, SemanticModel};

#[derive(Debug, Clone)]
pub struct ResolvedMetric {
    pub metric: Metric,
    pub measure_entity: Entity,
    pub measure: Measure,
    pub dimensions: Vec<(Entity, Dimension)>,
}

#[derive(Debug, thiserror::Error)]
pub enum MetricResolutionError {
    #[error("Metric not found: {0}")]
    MetricNotFound(String),
    #[error("Measure not found for metric {1}: {0}")]
    MeasureNotFound(String, String),
    #[error("Dimension not found for metric {1}: {0}")]
    DimensionNotFound(String, String),
    #[error("Ambiguous measure '{measure}' for metric '{metric}': found in entities {entities:?}")]
    AmbiguousMeasure {
        measure: String,
        metric: String,
        entities: Vec<String>,
    },
    #[error(
        "Ambiguous dimension '{dimension}' for metric '{metric}': found in entities {entities:?}"
    )]
    AmbiguousDimension {
        dimension: String,
        metric: String,
        entities: Vec<String>,
    },
}

pub struct MetricResolver<'a> {
    model: &'a SemanticModel,
}

impl<'a> MetricResolver<'a> {
    pub fn new(model: &'a SemanticModel) -> Self {
        Self { model }
    }

    pub fn resolve(&self, metric_name: &str) -> Result<ResolvedMetric, MetricResolutionError> {
        let metric = self
            .model
            .metrics
            .iter()
            .find(|m| m.name == metric_name)
            .ok_or_else(|| MetricResolutionError::MetricNotFound(metric_name.to_string()))?;

        let (measure_entity, measure) = {
            let mut matches: Vec<(Entity, Measure)> = Vec::new();
            for entity in &self.model.entities {
                if let Some(m) = entity.measures.iter().find(|m| m.name == metric.measure) {
                    matches.push((entity.clone(), m.clone()));
                }
            }
            match matches.len() {
                0 => {
                    return Err(MetricResolutionError::MeasureNotFound(
                        metric.measure.clone(),
                        metric.name.clone(),
                    ))
                }
                1 => matches.into_iter().next().expect("len checked == 1"),
                _ => {
                    return Err(MetricResolutionError::AmbiguousMeasure {
                        measure: metric.measure.clone(),
                        metric: metric.name.clone(),
                        entities: matches.into_iter().map(|(e, _)| e.name).collect(),
                    })
                }
            }
        };

        let mut resolved_dimensions = Vec::new();

        for dim_name in &metric.dimensions {
            let mut matches: Vec<(Entity, Dimension)> = Vec::new();
            for entity in &self.model.entities {
                if let Some(d) = entity.dimensions.iter().find(|d| d.name == *dim_name) {
                    matches.push((entity.clone(), d.clone()));
                }
            }

            let (dim_entity, dimension) = match matches.len() {
                0 => {
                    return Err(MetricResolutionError::DimensionNotFound(
                        dim_name.clone(),
                        metric.name.clone(),
                    ))
                }
                1 => matches.into_iter().next().expect("len checked == 1"),
                _ => {
                    return Err(MetricResolutionError::AmbiguousDimension {
                        dimension: dim_name.clone(),
                        metric: metric.name.clone(),
                        entities: matches.into_iter().map(|(e, _)| e.name).collect(),
                    })
                }
            };

            resolved_dimensions.push((dim_entity, dimension));
        }

        Ok(ResolvedMetric {
            metric: metric.clone(),
            measure_entity,
            measure,
            dimensions: resolved_dimensions,
        })
    }

    pub fn resolve_custom_dimensions(
        &self,
        dim_names: &[String],
        metric_name: &str,
    ) -> Result<Vec<(Entity, Dimension)>, MetricResolutionError> {
        let mut resolved_dimensions = Vec::new();

        for dim_name in dim_names {
            let mut matches: Vec<(Entity, Dimension)> = Vec::new();
            for entity in &self.model.entities {
                if let Some(d) = entity.dimensions.iter().find(|d| d.name == *dim_name) {
                    matches.push((entity.clone(), d.clone()));
                }
            }

            let (dim_entity, dimension) = match matches.len() {
                0 => {
                    return Err(MetricResolutionError::DimensionNotFound(
                        dim_name.clone(),
                        metric_name.to_string(),
                    ))
                }
                1 => matches.into_iter().next().expect("len checked == 1"),
                _ => {
                    return Err(MetricResolutionError::AmbiguousDimension {
                        dimension: dim_name.clone(),
                        metric: metric_name.to_string(),
                        entities: matches.into_iter().map(|(e, _)| e.name).collect(),
                    })
                }
            };

            resolved_dimensions.push((dim_entity, dimension));
        }
        Ok(resolved_dimensions)
    }
}
