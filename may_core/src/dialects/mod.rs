pub mod bigquery;
pub mod core;
pub(crate) mod dummy;
pub mod postgres;
pub mod snowflake;

pub use bigquery::BigQueryDialect;
pub use core::{DialectError, SqlDialect};
#[doc(hidden)]
pub use dummy::test_support::DummyDialect;
pub use postgres::PostgresDialect;
pub use snowflake::SnowflakeDialect;

pub fn dialect_for(kind: &str) -> Box<dyn SqlDialect + Send + Sync> {
    match kind.to_ascii_lowercase().as_str() {
        "snowflake" => Box::new(SnowflakeDialect),
        "bigquery" => Box::new(BigQueryDialect),
        _ => Box::new(PostgresDialect), // default
    }
}
