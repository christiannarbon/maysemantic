use crate::ast::{Expr, JoinType};
use crate::compiler::rls::RlsPolicy;
use once_cell::sync::Lazy;
use regex::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError};

static NAME_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^[a-z_][a-z0-9_]*$")
        .expect("NAME_REGEX is a compile-time constant pattern and should never fail to compile")
});

pub(crate) fn validate_name(name: &str) -> Result<(), ValidationError> {
    if NAME_REGEX.is_match(name) {
        Ok(())
    } else {
        Err(ValidationError::new(
            "Invalid name format: must match ^[a-z_][a-z0-9_]*$",
        ))
    }
}

pub(crate) fn validate_bare_sql(sql: &str) -> Result<(), ValidationError> {
    let parts: Vec<&str> = sql.split('.').collect();
    if parts.len() == 2 {
        let is_ident = |s: &str| {
            !s.is_empty()
                && s.chars().enumerate().all(|(i, c)| {
                    if i == 0 {
                        c.is_ascii_alphabetic() || c == '_'
                    } else {
                        c.is_ascii_alphanumeric() || c == '_'
                    }
                })
        };
        if is_ident(parts[0].trim()) && is_ident(parts[1].trim()) {
            return Err(ValidationError::new(
                "qualified_sql_forbidden: Do NOT table-qualify columns (e.g. 'table.column') in the 'sql' field. Use bare column names instead. The compiler/aliaser handles qualification dynamically.",
            ));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
pub struct Dimension {
    #[validate(custom(function = "validate_name"))]
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub dimension_type: DimensionType,
    /// The bare physical column name. Do NOT table-qualify here — the compiler adds the correct
    /// table/alias prefix during join construction (see aliasing).
    #[validate(custom(function = "validate_bare_sql"))]
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
    /// The bare physical column name. Do NOT table-qualify here — the compiler adds the correct
    /// table/alias prefix during join construction (see aliasing).
    #[validate(custom(function = "validate_bare_sql"))]
    pub sql: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum AggregationType {
    Sum,
    Count,
    Average,
    Min,
    Max,
    #[serde(rename = "count_distinct")]
    CountDistinct,
}

impl AggregationType {
    pub fn to_expr(&self, column: Expr) -> Expr {
        match self {
            AggregationType::CountDistinct => Expr::CountDistinct(Box::new(column)),
            _ => {
                let name = match self {
                    AggregationType::Sum => "SUM",
                    AggregationType::Count => "COUNT",
                    AggregationType::Average => "AVG",
                    AggregationType::Min => "MIN",
                    AggregationType::Max => "MAX",
                    AggregationType::CountDistinct => unreachable!(),
                };
                Expr::Function {
                    name: name.to_string(),
                    args: vec![column],
                }
            }
        }
    }
}

/// Classifies whether an entity is a fact table (transactional, additive rows)
/// or a dimension table (descriptive lookup rows).
/// Used by `FanOutDetector` to identify chasm-trap risk in join paths.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum EntityType {
    #[default]
    Fact,
    Dimension,
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
    #[serde(default)]
    pub entity_type: EntityType,
    #[serde(default)]
    #[validate(nested)]
    pub rls_policies: Vec<RlsPolicy>,
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
