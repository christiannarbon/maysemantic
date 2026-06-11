use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents the identity and data-access claims of the caller decoded from a JWT token.
/// Passed through the compiler pipeline so that the `RlsInjector` can apply row-level
/// security predicates without performing any I/O.
#[derive(Debug, Clone, Default)]
pub struct UserContext {
    /// Raw decoded JWT claims keyed by claim name.
    pub claims: HashMap<String, String>,
}

impl UserContext {
    /// Returns the value of the given claim key, or `None` if it is absent.
    pub fn get_claim(&self, key: &str) -> Option<&str> {
        self.claims.get(key).map(String::as_str)
    }
}

/// Declares a row-level security policy on an `Entity`.
/// When `claim_key` is present in the caller's `UserContext`, the compiler
/// will inject `WHERE <dimension> = '<claim_value>'` into the query AST.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RlsPolicy {
    /// The JWT claim key to look up (e.g. `"region"`).
    pub claim_key: String,
    /// The dimension/column the claim value filters (e.g. `"region_name"`).
    pub dimension: String,
}
