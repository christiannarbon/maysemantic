use may_auth::error::AuthError;

#[test]
fn test_error_display() {
    let err = AuthError::UserNotFound;
    assert_eq!(err.to_string(), "User not found");
    
    let err = AuthError::HashError("test error".to_string());
    assert_eq!(err.to_string(), "Hash error: test error");
}
