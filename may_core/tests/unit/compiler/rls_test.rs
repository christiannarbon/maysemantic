use may_core::UserContext;
use std::collections::HashMap;

#[test]
fn test_user_context_default() {
    let ctx = UserContext::default();
    assert!(ctx.claims.is_empty());
}

#[test]
fn test_user_context_clone_and_debug() {
    let mut claims = HashMap::new();
    claims.insert("region".to_string(), "EMEA".to_string());
    let ctx = UserContext { claims };
    let ctx_clone = ctx.clone();
    assert_eq!(ctx_clone.get_claim("region"), Some("EMEA"));

    let debug_str = format!("{:?}", ctx);
    assert!(debug_str.contains("region"));
    assert!(debug_str.contains("EMEA"));
}

#[test]
fn test_user_context_get_claim() {
    let mut claims = HashMap::new();
    claims.insert("region".to_string(), "EMEA".to_string());
    claims.insert("tenant_id".to_string(), "123".to_string());
    let ctx = UserContext { claims };

    assert_eq!(ctx.get_claim("region"), Some("EMEA"));
    assert_eq!(ctx.get_claim("tenant_id"), Some("123"));
    assert_eq!(ctx.get_claim("missing_key"), None);
}

#[test]
fn test_rls_policy_serde_roundtrip() {
    use may_core::RlsPolicy;

    let policy = RlsPolicy {
        claim_key: "region".to_string(),
        dimension: "region_name".to_string(),
    };
    let json = serde_json::to_string(&policy).expect("serialize RlsPolicy");
    let back: RlsPolicy = serde_json::from_str(&json).expect("deserialize RlsPolicy");
    assert_eq!(policy, back);
}

#[test]
fn test_entity_rls_policies_default() {
    use may_core::Entity;

    let yaml = r#"
name: orders
table: public.orders
primary_key: order_id
dimensions: []
measures: []
entity_type: fact
"#;
    let entity: Entity = serde_norway::from_str(yaml).expect("parse Entity YAML");
    assert!(entity.rls_policies.is_empty());
}

#[test]
fn test_entity_rls_policies_explicit() {
    use may_core::Entity;

    let yaml = r#"
name: orders
table: public.orders
primary_key: order_id
dimensions: []
measures: []
entity_type: fact
rls_policies:
  - claim_key: region
    dimension: region_name
"#;
    let entity: Entity = serde_norway::from_str(yaml).expect("parse Entity YAML with RLS policies");
    assert_eq!(entity.rls_policies.len(), 1);
    assert_eq!(entity.rls_policies[0].claim_key, "region");
    assert_eq!(entity.rls_policies[0].dimension, "region_name");
}
