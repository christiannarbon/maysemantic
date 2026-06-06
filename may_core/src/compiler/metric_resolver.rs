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
            let mut found = None;
            for entity in &self.model.entities {
                if let Some(m) = entity.measures.iter().find(|m| m.name == metric.measure) {
                    found = Some((entity.clone(), m.clone()));
                    break;
                }
            }
            found.ok_or_else(|| {
                MetricResolutionError::MeasureNotFound(metric.measure.clone(), metric.name.clone())
            })?
        };

        let mut resolved_dimensions = Vec::new();

        for dim_name in &metric.dimensions {
            let mut found = None;
            for entity in &self.model.entities {
                if let Some(d) = entity.dimensions.iter().find(|d| d.name == *dim_name) {
                    found = Some((entity.clone(), d.clone()));
                    break;
                }
            }

            let (dim_entity, dimension) = found.ok_or_else(|| {
                MetricResolutionError::DimensionNotFound(dim_name.clone(), metric.name.clone())
            })?;

            resolved_dimensions.push((dim_entity, dimension));
        }

        Ok(ResolvedMetric {
            metric: metric.clone(),
            measure_entity,
            measure,
            dimensions: resolved_dimensions,
        })
    }
}
