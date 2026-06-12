#[cfg(test)]
mod rls_entity_tests {
    use may_core::{Entity, RlsPolicy};

    #[test]
    fn test_entity_without_rls_policies_defaults_to_empty_vec() {
        let yaml = r#"
name: orders
table: orders_tbl
primary_key: order_id
dimensions: []
measures: []
"#;
        let entity: Entity = serde_norway::from_str(yaml).expect("should deserialise");
        assert!(entity.rls_policies.is_empty());
    }

    #[test]
    fn test_entity_with_rls_policies_deserialises_correctly() {
        let yaml = r#"
name: orders
table: orders_tbl
primary_key: order_id
dimensions: []
measures: []
rls_policies:
  - claim_key: region
    dimension: user_region
"#;
        let entity: Entity = serde_norway::from_str(yaml).expect("should deserialise");
        assert_eq!(entity.rls_policies.len(), 1);
        assert_eq!(entity.rls_policies[0].claim_key, "region");
        assert_eq!(entity.rls_policies[0].dimension, "user_region");
    }

    #[test]
    fn test_rls_policy_round_trips_through_json() {
        let policy = RlsPolicy {
            claim_key: "region".to_string(),
            dimension: "user_region".to_string(),
        };
        let json = serde_json::to_string(&policy).expect("serialise");
        let restored: RlsPolicy = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(policy, restored);
    }
}
