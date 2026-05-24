use crate::error::AuthError;

pub fn hash_password(plaintext: &str) -> Result<String, AuthError> {
    // Dummy implementation for now, will be implemented in TASK-AUTH-1.2.3
    Ok(plaintext.to_string())
}
