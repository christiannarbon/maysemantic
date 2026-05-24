use crate::ast::JoinType;
use once_cell::sync::Lazy;
use regex::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError};

static NAME_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^[a-z_][a-z0-9_]*$")
        .expect("NAME_REGEX is a compile-time constant pattern and should never fail to compile")
});

fn validate_name(name: &str) -> Result<(), ValidationError> {
    if NAME_REGEX.is_match(name) {
        Ok(())
    } else {
        Err(ValidationError::new(
            "Invalid name format: must match ^[a-z_][a-z0-9_]*$",
        ))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
pub struct Dimension {
    #[validate(custom(function = "validate_name"))]
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub dimension_type: DimensionType,
    pub sql: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum DimensionType {
    String,
    Number,
    Boolean,
    Time,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
pub struct Measure {
    #[validate(custom(function = "validate_name"))]
    pub name: String,
    pub description: Option<String>,
    pub agg: AggregationType,
    pub sql: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum AggregationType {
    Sum,
    Count,
    Average,
    Min,
    Max,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
pub struct Entity {
    #[validate(custom(function = "validate_name"))]
    pub name: String,
    pub description: Option<String>,
    pub table: String,
    #[validate(custom(function = "validate_name"))]
    pub primary_key: String,
    #[validate(nested)]
    pub dimensions: Vec<Dimension>,
    #[validate(nested)]
    pub measures: Vec<Measure>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
pub struct Metric {
    #[validate(custom(function = "validate_name"))]
    pub name: String,
    pub description: Option<String>,
    pub measure: String,
    pub dimensions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
pub struct SemanticModel {
    #[validate(custom(function = "validate_name"))]
    pub name: String,
    #[validate(nested)]
    pub entities: Vec<Entity>,
    #[validate(nested)]
    pub metrics: Vec<Metric>,
    #[serde(default)]
    #[validate(nested)]
    pub joins: Vec<JoinDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
pub struct JoinDefinition {
    #[validate(custom(function = "validate_name"))]
    pub left_entity: String,
    #[validate(custom(function = "validate_name"))]
    pub left_column: String,
    #[validate(custom(function = "validate_name"))]
    pub right_entity: String,
    #[validate(custom(function = "validate_name"))]
    pub right_column: String,
    #[serde(default = "default_join_type")]
    pub join_type: JoinType,
}

fn default_join_type() -> JoinType {
    JoinType::Left
}
