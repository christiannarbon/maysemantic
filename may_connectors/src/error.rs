use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConnectorError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Query failed: {0}")]
    QueryFailed(String),

    #[error("Query timed out")]
    Timeout,

    #[error("Unsupported type: {0}")]
    UnsupportedType(String),
}
