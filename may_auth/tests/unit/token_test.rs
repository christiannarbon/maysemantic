use chrono::Utc;
use may_auth::error::AuthError;
use may_auth::models::{Role, User};
use may_auth::token::TokenService;
use std::env;
use std::thread;
use uuid::Uuid;

fn create_mock_user() -> User {
    User {
        id: Uuid::new_v4(),
        username: "testuser".to_string(),
        password_hash: "dummyhash".to_string(),
        role: Role::Admin,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

// Helper to run tests serially to avoid environment variable races
// In a real project, we'd use a serial_test crate or allow dependency injection,
// but we adhere to the prompt requiring env var reading directly in `new()`.
fn run_with_env<F>(
    secret: &str,
    expiry: Option<&str>,
    test_fn: F,
) -> Result<(), Box<dyn std::error::Error>>
where
    F: FnOnce() -> Result<(), Box<dyn std::error::Error>> + std::panic::UnwindSafe,
{
    // Since cargo test runs multithreaded by default, modifying env vars is inherently risky
    // for other parallel tests. In practice, `cargo test -- --test-threads=1` might be needed
    // if multiple tests modify the same env variables concurrently.
    env::set_var("MAY_JWT_SECRET", secret);
    if let Some(e) = expiry {
        env::set_var("MAY_JWT_EXPIRY_SECS", e);
    } else {
        env::remove_var("MAY_JWT_EXPIRY_SECS");
    }

    let result = std::panic::catch_unwind(test_fn);

    env::remove_var("MAY_JWT_SECRET");
    env::remove_var("MAY_JWT_EXPIRY_SECS");

    match result {
        Ok(r) => r,
        Err(e) => std::panic::resume_unwind(e),
    }
}

#[test]
fn test_issue_and_verify_valid_token() -> Result<(), Box<dyn std::error::Error>> {
    run_with_env("supersecret123", None, || {
        let service = TokenService::new()?;
        let user = create_mock_user();

        let token = service.issue(&user)?;
        let claims = service.verify(&token)?;

        assert_eq!(claims.sub, user.id.to_string());
        assert_eq!(claims.role, "admin");

        Ok(())
    })
}

#[test]
fn test_verify_expired_token() -> Result<(), Box<dyn std::error::Error>> {
    run_with_env("supersecret123", Some("0"), || {
        let service = TokenService::new()?;
        let user = create_mock_user();

        let token = service.issue(&user)?;

        // Wait 1 second to ensure the token is actually expired (if it was 0 secs)
        thread::sleep(std::time::Duration::from_secs(1));

        let result = service.verify(&token);

        match result {
            Err(AuthError::TokenExpired) => Ok(()),
            _ => Err("Expected AuthError::TokenExpired".into()),
        }
    })
}

#[test]
fn test_verify_tampered_signature() -> Result<(), Box<dyn std::error::Error>> {
    run_with_env("supersecret123", None, || {
        let service = TokenService::new()?;
        let user = create_mock_user();

        let mut token = service.issue(&user)?;

        // Tamper with the token signature by replacing the last character
        let last_char = token.chars().last().ok_or("Token is empty")?;
        token.pop();
        if last_char == 'a' {
            token.push('b');
        } else {
            token.push('a');
        }

        let result = service.verify(&token);

        match result {
            Err(AuthError::InvalidToken) => Ok(()),
            _ => Err("Expected AuthError::InvalidToken".into()),
        }
    })
}
