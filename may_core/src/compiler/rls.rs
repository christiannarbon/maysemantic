use crate::ast::{and, eq, literal_bool, literal_num, literal_str, ColumnIdent, Expr, SqlNode};
use crate::models::core::validate_name;
use crate::models::{DimensionType, Entity, SemanticState};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use validator::Validate;

/// Errors raised while injecting row-level-security predicates.
#[derive(Debug, Error)]
pub enum RlsError {
    /// An `RlsPolicy` names a dimension that does not exist on its entity —
    /// a configuration error, surfaced loudly rather than silently ignored.
    #[error("RLS policy on entity '{entity}' references unknown dimension '{dimension}'")]
    UnknownDimension { entity: String, dimension: String },
}

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

/// A never-true predicate (`1 = 0`) used to fail closed at RUNTIME when RLS
/// cannot be satisfied (missing claim, or a claim value invalid for its
/// column type). NOTE: a missing *dimension* is a config error (see RlsError),
/// not a runtime condition, and must NOT use this.
fn deny_all() -> Expr {
    eq(
        Expr::Literal("1".to_string()),
        Expr::Literal("0".to_string()),
    )
}

/// Builds one predicate per RLS policy on `entity`:
/// - Resolves the policy's logical dimension to its physical `sql` column and
///   type (mirroring SemanticLowering::lower_expr's DimensionRef handling). A
///   policy naming a non-existent dimension is a misconfiguration and returns
///   `Err(RlsError::UnknownDimension)`.
/// - Emits a type-correct literal: unquoted numbers/booleans, quoted+escaped
///   strings (default). Numeric/boolean claim values are validated; an invalid
///   value fails closed (`1 = 0`).
/// - A policy whose claim is ABSENT from the context fails closed (`1 = 0`).
fn predicates_for_entity(entity: &Entity, user_ctx: &UserContext) -> Result<Vec<Expr>, RlsError> {
    entity
        .rls_policies
        .iter()
        .map(|policy| {
            // Resolve the logical dimension to its physical column + type.
            // A policy that names a non-existent dimension fails loudly.
            let dimension = entity
                .dimensions
                .iter()
                .find(|d| d.name == policy.dimension)
                .ok_or_else(|| RlsError::UnknownDimension {
                    entity: entity.name.clone(),
                    dimension: policy.dimension.clone(),
                })?;
            let column = dimension.sql.clone();

            let claim_value = match user_ctx.get_claim(&policy.claim_key) {
                Some(v) => v,
                None => {
                    tracing::warn!(
                        claim_key = %policy.claim_key,
                        entity = %entity.name,
                        "RLS policy claim missing from user context; denying all rows"
                    );
                    return Ok(deny_all());
                }
            };

            // Type-aware literal. Numeric/boolean constructors validate the
            // untrusted claim value and return None when it is not valid for
            // that type, so we fail closed instead of emitting unsafe SQL.
            let value_expr = match &dimension.dimension_type {
                DimensionType::Number => literal_num(claim_value),
                DimensionType::Boolean => literal_bool(claim_value),
                // String or Time -> quoted, escaped string literal.
                _ => Some(literal_str(claim_value)),
            };

            match value_expr {
                Some(v) => Ok(eq(Expr::Column(ColumnIdent(column)), v)),
                None => {
                    tracing::warn!(
                        dimension = %policy.dimension,
                        entity = %entity.name,
                        "RLS claim value invalid for dimension type; denying all rows"
                    );
                    Ok(deny_all())
                }
            }
        })
        .collect()
}

/// Finds every entity across all loaded models whose physical `table`
/// matches `table_name`, sorted by entity name for deterministic output.
/// Returns an empty vec if no entity maps to that table.
fn find_entities_for_table<'a>(state: &'a SemanticState, table_name: &str) -> Vec<&'a Entity> {
    let mut entities: Vec<&Entity> = state
        .models
        .values()
        .flat_map(|model| model.entities.iter())
        .filter(|entity| entity.table == table_name)
        .collect();
    entities.sort_by(|a, b| a.name.cmp(&b.name));
    entities
}

/// Post-AST security pass that injects row-level-security predicates derived
/// from the caller's JWT claims. Pure: no I/O, no mutation of shared state.
pub struct RlsInjector;

impl RlsInjector {
    /// Injects RLS predicates into `ast`. For every base table in each query's
    /// FROM clause, each matching `RlsPolicy` adds `AND <column> = <value>` to
    /// that query's WHERE clause. Recurses into CTE bodies and FROM subqueries
    /// so nested queries are protected independently. Non-`Query` roots are
    /// returned unchanged. Returns `Err` if any applicable policy references an
    /// unknown dimension.
    pub fn inject(
        ast: SqlNode,
        user_ctx: &UserContext,
        state: &SemanticState,
    ) -> Result<SqlNode, RlsError> {
        match ast {
            SqlNode::Query {
                ctes,
                select,
                from,
                r#where,
                group_by,
                having,
                order_by,
                limit,
                offset,
            } => {
                // 0. Recurse into nested queries first so every Query level is
                //    independently protected (CTE bodies and FROM subqueries).
                let ctes = ctes
                    .map(|cte_list| {
                        cte_list
                            .into_iter()
                            .map(|cte| match cte {
                                SqlNode::CTE { alias, query } => Ok(SqlNode::CTE {
                                    alias,
                                    query: Box::new(Self::inject(*query, user_ctx, state)?),
                                }),
                                other => Ok(other),
                            })
                            .collect::<Result<Vec<_>, RlsError>>()
                    })
                    .transpose()?;
                let from = Box::new(Self::inject_into_relations(*from, user_ctx, state)?);

                // 1. Gather every predicate for THIS query's base tables, from
                //    ALL entities mapping to each table, de-duplicated.
                let table_names = collect_table_names(&from);
                let mut predicates: Vec<Expr> = Vec::new();
                for table in &table_names {
                    for entity in find_entities_for_table(state, table) {
                        for pred in predicates_for_entity(entity, user_ctx)? {
                            if !predicates.contains(&pred) {
                                predicates.push(pred);
                            }
                        }
                    }
                }

                // 2. No predicates at this level -> return with recursed children.
                if predicates.is_empty() {
                    return Ok(SqlNode::Query {
                        ctes,
                        select,
                        from,
                        r#where,
                        group_by,
                        having,
                        order_by,
                        limit,
                        offset,
                    });
                }

                // 3. Fold all predicates into a single ANDed expression.
                let mut combined = predicates.remove(0);
                for p in predicates {
                    combined = and(combined, p);
                }

                // 4. Merge with any existing WHERE clause (AND), or create one.
                let new_where = match r#where {
                    Some(existing) => match *existing {
                        SqlNode::Where(existing_expr) => {
                            Some(Box::new(SqlNode::Where(and(existing_expr, combined))))
                        }
                        // Invariant: a Query's `where` slot always holds
                        // SqlNode::Where. Anything else is a malformed AST;
                        // assert in debug, fall back to the predicate alone.
                        other => {
                            debug_assert!(
                                false,
                                "RlsInjector: Query.where held a non-Where node: {other:?}"
                            );
                            Some(Box::new(SqlNode::Where(combined)))
                        }
                    },
                    None => Some(Box::new(SqlNode::Where(combined))),
                };

                Ok(SqlNode::Query {
                    ctes,
                    select,
                    from,
                    r#where: new_where,
                    group_by,
                    having,
                    order_by,
                    limit,
                    offset,
                })
            }
            // Non-Query roots are returned unchanged — RLS only applies to queries.
            other => Ok(other),
        }
    }

    /// Recurses RLS injection into the subquery source and join relations of a
    /// FROM clause. Base `Table` nodes pass through `inject` unchanged; subquery
    /// `Query` nodes receive their own predicates.
    fn inject_into_relations(
        from: SqlNode,
        user_ctx: &UserContext,
        state: &SemanticState,
    ) -> Result<SqlNode, RlsError> {
        match from {
            SqlNode::From { source, joins } => {
                let source = Box::new(Self::inject(*source, user_ctx, state)?);
                let joins = joins
                    .into_iter()
                    .map(|join| match join {
                        SqlNode::Join {
                            join_type,
                            relation,
                            on,
                        } => Ok(SqlNode::Join {
                            join_type,
                            relation: Box::new(Self::inject(*relation, user_ctx, state)?),
                            on,
                        }),
                        other => Ok(other),
                    })
                    .collect::<Result<Vec<_>, RlsError>>()?;
                Ok(SqlNode::From { source, joins })
            }
            other => Ok(other),
        }
    }
}
