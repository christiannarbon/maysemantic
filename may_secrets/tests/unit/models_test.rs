use may_secrets::DwhSecret;

#[test]
fn test_dwh_secret_serialization() -> Result<(), Box<dyn std::error::Error>> {
    let secret = DwhSecret::UsernamePassword {
        host: "localhost".to_string(),
        port: 5432,
        database: "postgres".to_string(),
        username: "admin".to_string(),
        password: "password".to_string(),
    };

    let json = serde_json::to_string(&secret)?;
    assert!(json.contains("username_password"));
    assert!(json.contains("localhost"));

    let deserialized: DwhSecret = serde_json::from_str(&json)?;
    match deserialized {
        DwhSecret::UsernamePassword { host, .. } => assert_eq!(host, "localhost"),
        _ => return Err("Expected UsernamePassword variant".into()),
    }

    Ok(())
}
