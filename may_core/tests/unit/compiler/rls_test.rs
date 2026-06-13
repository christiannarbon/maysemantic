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
        ctx.claims
            .insert("tenant_id".to_string(), "acme".to_string());
        assert_eq!(cloned.get_claim("tenant_id"), None);
    }
}
