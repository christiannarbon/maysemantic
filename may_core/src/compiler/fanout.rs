#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathClassification {
    SingleFact,
    PureDimension,
    MultiFactJoin { fact_tables: Vec<String> },
}
