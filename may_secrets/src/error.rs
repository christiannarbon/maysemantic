use thiserror::Error;

#[derive(Debug, Error)]
pub enum SecretsError {
    #[error("Secret not found: {0}")]
    SecretNotFound(String),

    #[error("Missing environment variable: {var_name}")]
    MissingVariable { var_name: String },

    #[error("Vault error (status {status}): {message}")]
    VaultError { status: u16, message: String },

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
