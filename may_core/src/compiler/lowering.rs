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
