use crate::models::{Entity, EntityType};

/// The result of analysing a resolved join path for chasm trap risk.
///
/// Returned by `FanOutDetector::classify` and consumed by `ChasmTrapHandler`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathClassification {
    /// Exactly one fact table appears in the path — no fan-out risk.
    SingleFact,

    /// Two or more fact tables appear in the path — chasm trap risk present.
    /// `fact_tables` contains the entity names of each fact table, in path traversal order.
    MultiFactJoin { fact_tables: Vec<String> },

    /// No fact tables appear in the path (all entities are dimensions).
    PureDimension,
}

pub struct FanOutDetector;

impl FanOutDetector {
    /// Classify a join path given the ordered list of entity nodes in the path.
    ///
    /// `nodes` must be ordered from the base entity (first) to the final target entity (last).
    /// All `EntityType::Fact` entities are collected in traversal order.
    ///
    /// Returns:
    /// - `PureDimension`  — if no fact entities appear in the path
    /// - `SingleFact`     — if exactly one fact entity appears
    /// - `MultiFactJoin`  — if two or more fact entities appear (chasm trap risk)
    pub fn classify(nodes: &[&Entity]) -> PathClassification {
        let fact_tables: Vec<String> = nodes
            .iter()
            .filter(|e| e.entity_type == EntityType::Fact)
            .map(|e| e.name.clone())
            .collect();

        match fact_tables.len() {
            0 => PathClassification::PureDimension,
            1 => PathClassification::SingleFact,
            _ => PathClassification::MultiFactJoin { fact_tables },
        }
    }
}
