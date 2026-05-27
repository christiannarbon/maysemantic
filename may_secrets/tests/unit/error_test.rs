use may_secrets::SecretsError;
use std::io;

#[test]
fn test_error_formatting() {
    let err = SecretsError::SecretNotFound("my_secret".to_string());
    assert_eq!(err.to_string(), "Secret not found: my_secret");

    let err = SecretsError::MissingVariable {
        var_name: "MY_VAR".to_string(),
    };
    assert_eq!(err.to_string(), "Missing environment variable: MY_VAR");

    let err = SecretsError::VaultError {
        status: 403,
        message: "Forbidden".to_string(),
    };
    assert_eq!(err.to_string(), "Vault error (status 403): Forbidden");

    let err = SecretsError::InvalidConfig("bad json".to_string());
    assert_eq!(err.to_string(), "Invalid configuration: bad json");

    let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
    let err = SecretsError::Io(io_err);
    assert_eq!(err.to_string(), "I/O error: file not found");
}
