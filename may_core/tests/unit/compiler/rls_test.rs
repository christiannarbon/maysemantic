#[cfg(test)]
mod user_context_test {
    use may_core::UserContext;
    use std::collections::HashMap;

    fn make_ctx(pairs: &[(&str, &str)]) -> UserContext {
        UserContext {
            claims: pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect::<HashMap<_, _>>(),
        }
    }

    #[test]
    fn test_get_claim_returns_some_for_known_key() {
        let ctx = make_ctx(&[("region", "EMEA")]);
        assert_eq!(ctx.get_claim("region"), Some("EMEA"));
    }

    #[test]
    fn test_get_claim_returns_none_for_unknown_key() {
        let ctx = make_ctx(&[("region", "EMEA")]);
        assert_eq!(ctx.get_claim("tenant_id"), None);
    }

    #[test]
    fn test_get_claim_on_empty_context_returns_none() {
        let ctx = UserContext::default();
        assert_eq!(ctx.get_claim("anything"), None);
    }

    #[test]
    fn test_user_context_clone_is_independent() {
        let mut ctx = make_ctx(&[("region", "EMEA")]);
        let cloned = ctx.clone();
        ctx.claims.insert("tenant_id".to_string(), "acme".to_string());
        assert_eq!(cloned.get_claim("tenant_id"), None);
    }
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
