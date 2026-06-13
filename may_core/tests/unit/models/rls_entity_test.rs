#[cfg(test)]
mod rls_entity_tests {
    use may_core::{Entity, RlsPolicy, models::EntityType};
    use validator::Validate;

    fn make_entity(name: &str, rls_policies: Vec<RlsPolicy>) -> Entity {
        Entity {
            name: name.to_string(),
            description: None,
            table: format!("{}_tbl", name),
            primary_key: "id".to_string(),
            dimensions: vec![],
            measures: vec![],
            entity_type: EntityType::Fact,
            rls_policies,
        }
    }

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

    #[test]
    fn test_entity_with_valid_rls_policy_passes_validation() {
        let policy = RlsPolicy {
            claim_key: "region".to_string(),
            dimension: "user_region".to_string(),
        };
        let entity = make_entity("orders", vec![policy]);
        assert!(entity.validate().is_ok());
    }

    #[test]
    fn test_entity_with_invalid_rls_policy_fails_validation() {
        let bad_policy_1 = RlsPolicy {
            claim_key: "region".to_string(),
            dimension: "1bad".to_string(),
        };
        let entity_1 = make_entity("orders", vec![bad_policy_1]);
        assert!(entity_1.validate().is_err());

        let bad_policy_2 = RlsPolicy {
            claim_key: "region".to_string(),
            dimension: "a; DROP".to_string(),
        };
        let entity_2 = make_entity("orders", vec![bad_policy_2]);
        assert!(entity_2.validate().is_err());
    }
}
