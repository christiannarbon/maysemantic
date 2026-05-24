use crate::error::AuthError;
use bcrypt::{hash, verify};

/// Hashes a plaintext password using bcrypt.
///
/// **NOTE:** Bcrypt is CPU-bound. Callers in an async context MUST wrap
/// this call in `tokio::task::spawn_blocking` to avoid blocking the async executor.
pub fn hash_password(plaintext: &str) -> Result<String, AuthError> {
    // Requirements specify a cost of 12
    hash(plaintext, 12).map_err(|e| AuthError::HashError(e.to_string()))
}

/// Verifies a plaintext password against a bcrypt hash.
///
/// **NOTE:** Bcrypt is CPU-bound. Callers in an async context MUST wrap
/// this call in `tokio::task::spawn_blocking` to avoid blocking the async executor.
pub fn verify_password(plaintext: &str, hash: &str) -> Result<bool, AuthError> {
    verify(plaintext, hash).map_err(|e| AuthError::HashError(e.to_string()))
}
