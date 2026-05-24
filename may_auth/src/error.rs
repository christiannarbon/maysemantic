use thiserror::Error;

#[derive(Error, Debug)]
pub enum AuthError {
    #[error("User not found")]
    UserNotFound,
    #[error("Invalid credentials")]
    InvalidCredentials,
    #[error("Hash error: {0}")]
    HashError(String),
    #[error("Token expired")]
    TokenExpired,
    #[error("Invalid token")]
    InvalidToken,
    #[error("Missing config: {0}")]
    MissingConfig(String),
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}
