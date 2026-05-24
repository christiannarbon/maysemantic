use may_auth::password::{hash_password, verify_password};

#[test]
fn test_hash_and_verify_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
    let password = "my_secure_password_123";

    // Hash the password
    let hashed = hash_password(password)?;

    // Verify it matches
    let is_valid = verify_password(password, &hashed)?;
    assert!(is_valid, "Password should verify successfully");

    Ok(())
}

#[test]
fn test_verify_wrong_password() -> Result<(), Box<dyn std::error::Error>> {
    let password = "my_secure_password_123";

    // Hash the password
    let hashed = hash_password(password)?;

    // Verify against a wrong password
    let is_valid = verify_password("wrong_password", &hashed)?;
    assert!(!is_valid, "Wrong password should fail verification");

    Ok(())
}
