use thiserror::Error;

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum ConnectorError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Query failed: {0}")]
    QueryFailed(String),

    // TODO: implement query timeout logic
    #[error("Query timed out")]
    Timeout,

    #[error("Unsupported type: {0}")]
    UnsupportedType(String),
}
