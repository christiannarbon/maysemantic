use crate::compiler::request::SemanticRequest;
use crate::models::SemanticState;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RequestParseError {
    #[error("Metric name must not be empty.")]
    EmptyMetricName,

    #[error("Metric '{0}' was not found in any loaded semantic model.")]
    MetricNotFound(String),

    #[error("Dimension '{0}' was not found in any entity across all loaded models.")]
    DimensionNotFound(String),
}

pub struct RequestParser<'a> {
    state: &'a SemanticState,
}

impl<'a> RequestParser<'a> {
    pub fn new(state: &'a SemanticState) -> Self {
        Self { state }
    }

    pub fn validate(&self, request: &SemanticRequest) -> Result<(), RequestParseError> {
        // Step 1: reject empty/whitespace metric_name → EmptyMetricName
        if request.metric_name.trim().is_empty() {
            return Err(RequestParseError::EmptyMetricName);
        }

        // Step 2: search state.models.values() for matching metric → MetricNotFound
        let metric_found = self
            .state
            .models
            .values()
            .any(|model| model.metrics.iter().any(|m| m.name == request.metric_name));
        if !metric_found {
            return Err(RequestParseError::MetricNotFound(
                request.metric_name.clone(),
            ));
        }

        // Step 3: for each dim in request.dimensions, search all entity dimensions → DimensionNotFound
        for dim_name in &request.dimensions {
            let dim_found = self.state.models.values().any(|model| {
                model.entities.iter().any(|entity| {
                    entity
                        .dimensions
                        .iter()
                        .any(|dimension| &dimension.name == dim_name)
                })
            });
            if !dim_found {
                return Err(RequestParseError::DimensionNotFound(dim_name.clone()));
            }
        }

        Ok(())
    }
}
