use crate::compiler::{LoweringError, MetricResolutionError, RequestParseError};
use crate::dialects::SqlDialect;
use crate::graph::JoinResolutionError;
use crate::models::SemanticState;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CompilerError {
    #[error("Request validation failed: {0}")]
    RequestParsing(#[from] RequestParseError),

    #[error("Metric resolution failed: {0}")]
    MetricResolution(#[from] MetricResolutionError),

    #[error("Join path resolution failed: {0}")]
    JoinResolution(#[from] JoinResolutionError),

    #[error("AST lowering failed: {0}")]
    Lowering(#[from] LoweringError),

    #[error("SQL code generation failed: {0}")]
    CodeGeneration(String),
}

#[allow(dead_code)]
pub struct SemanticCompiler {
    state: Arc<SemanticState>,
    dialect: Box<dyn SqlDialect + Send + Sync>,
}

impl SemanticCompiler {
    pub fn new(
        state: Arc<SemanticState>,
        dialect: Box<dyn SqlDialect + Send + Sync>,
    ) -> Self {
        Self { state, dialect }
    }
}
