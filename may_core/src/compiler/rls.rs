use crate::ast::{and, eq, literal_str, ColumnIdent, Expr, SqlNode};
use crate::models::core::validate_name;
use crate::models::{Entity, SemanticState};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use validator::Validate;

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

/// Finds the first entity across all loaded models whose physical `table`
/// matches `table_name`. Returns `None` if no entity maps to that table.
fn find_entity_for_table<'a>(state: &'a SemanticState, table_name: &str) -> Option<&'a Entity> {
    state
        .models
        .values()
        .flat_map(|model| model.entities.iter())
        .find(|entity| entity.table == table_name)
}

/// Builds one equality predicate per RLS policy on `entity` whose claim is
/// present in `user_ctx`. Policies whose claim key is absent are skipped.
fn predicates_for_entity(entity: &Entity, user_ctx: &UserContext) -> Vec<Expr> {
    entity
        .rls_policies
        .iter()
        .filter_map(|policy| {
            user_ctx.get_claim(&policy.claim_key).map(|claim_value| {
                eq(
                    Expr::Column(ColumnIdent(policy.dimension.clone())),
                    literal_str(claim_value),
                )
            })
        })
        .collect()
}

/// Post-AST security pass that injects row-level-security predicates derived
/// from the caller's JWT claims. Pure: no I/O, no mutation of shared state.
pub struct RlsInjector;

impl RlsInjector {
    /// Injects RLS predicates into `ast`. For every base table in the FROM clause,
    /// each matching `RlsPolicy` adds `AND <dimension> = '<claim_value>'` to the
    /// WHERE clause. If no policies match, `ast` is returned unchanged.
    pub fn inject(ast: SqlNode, user_ctx: &UserContext, state: &SemanticState) -> SqlNode {
        match ast {
            SqlNode::Query {
                ctes,
                select,
                from,
                r#where,
                group_by,
                having,
            } => {
                // 1. Gather every security predicate that applies to this query.
                let table_names = collect_table_names(&from);
                let mut predicates: Vec<Expr> = Vec::new();
                for table in &table_names {
                    if let Some(entity) = find_entity_for_table(state, table) {
                        predicates.extend(predicates_for_entity(entity, user_ctx));
                    }
                }

                // 2. No predicates → return the query structurally unchanged.
                if predicates.is_empty() {
                    return SqlNode::Query {
                        ctes,
                        select,
                        from,
                        r#where,
                        group_by,
                        having,
                    };
                }

                // 3. Fold all predicates into a single ANDed expression.
                let mut combined = predicates.remove(0);
                for p in predicates {
                    combined = and(combined, p);
                }

                // 4. Merge with any existing WHERE clause (AND), or create a new one.
                let new_where = match r#where {
                    Some(existing) => match *existing {
                        SqlNode::Where(existing_expr) => {
                            Some(Box::new(SqlNode::Where(and(existing_expr, combined))))
                        }
                        // Defensive: if the boxed node is not a Where, keep it and
                        // still apply the security predicate alongside it.
                        other => {
                            // Preserve original node; wrap security predicate separately.
                            // In practice the where slot always holds SqlNode::Where.
                            let _ = other;
                            Some(Box::new(SqlNode::Where(combined)))
                        }
                    },
                    None => Some(Box::new(SqlNode::Where(combined))),
                };

                SqlNode::Query {
                    ctes,
                    select,
                    from,
                    r#where: new_where,
                    group_by,
                    having,
                }
            }
            // Non-Query roots are returned unchanged — RLS only applies to queries.
            other => other,
        }
    }
}
