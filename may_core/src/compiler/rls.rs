use crate::models::core::validate_name;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use validator::Validate;
use crate::ast::SqlNode;

/// Represents the identity and data-access claims of the caller decoded from a JWT token.
/// Passed through the compiler pipeline so that the `RlsInjector` can apply row-level
/// security predicates without performing any I/O.
#[derive(Debug, Clone, Default)]
pub struct UserContext {
    /// Raw decoded JWT claims keyed by claim name. Values are stored as strings,
    /// so non-string JWT claims (numbers, booleans, arrays) must be stringified by the caller.
    pub claims: HashMap<String, String>,
}

impl UserContext {
    /// Returns the value of the given claim key, or `None` if it is absent.
    pub fn get_claim(&self, key: &str) -> Option<&str> {
        self.claims.get(key).map(String::as_str)
    }
}

/// Declares a row-level security policy on an `Entity`.
/// When `claim_key` is present in the caller's `UserContext`, the compiler injects an
/// equality predicate `<dimension> = <value>` into the query AST. The claim value is
/// caller-controlled (decoded from a JWT) and MUST be emitted as a bound/escaped literal
/// node — never interpolated into raw SQL text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Validate)]
pub struct RlsPolicy {
    /// The JWT claim key to look up (e.g. `"region"`).
    #[validate(custom(function = "validate_name"))]
    pub claim_key: String,
    /// The dimension/column the claim value filters (e.g. `"region_name"`).
    #[validate(custom(function = "validate_name"))]
    pub dimension: String,
}

#[allow(dead_code)]
/// Collects the names of all base tables referenced by a FROM clause,
/// including the primary source and every joined relation.
/// Subqueries (non-`Table` relations) are skipped — RLS applies to base tables only.
fn collect_table_names(from: &SqlNode) -> Vec<String> {
    let mut tables = Vec::new();
    if let SqlNode::From { source, joins } = from {
        if let SqlNode::Table(crate::ast::TableIdent(name)) = source.as_ref() {
            tables.push(name.clone());
        }
        for join in joins {
            if let SqlNode::Join { relation, .. } = join {
                if let SqlNode::Table(crate::ast::TableIdent(name)) = relation.as_ref() {
                    tables.push(name.clone());
                }
            }
        }
    }
    tables
}
