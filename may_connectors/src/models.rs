use crate::error::ConnectorError;
use futures::Stream;
use std::pin::Pin;

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ColumnValue {
    Null,
    Int64(i64),
    Float64(f64),
    Text(String),
    Bool(bool),
    Bytes(Vec<u8>),
}

pub type Row = Vec<ColumnValue>;

pub type QueryResult = Pin<Box<dyn Stream<Item = Result<Row, ConnectorError>> + Send>>;
