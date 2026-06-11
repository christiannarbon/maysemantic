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
