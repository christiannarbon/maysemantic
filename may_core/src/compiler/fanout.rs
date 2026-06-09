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
