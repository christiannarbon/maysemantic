use crate::error::ConnectorError;
use crate::models::{ColumnValue, QueryResult};
use crate::traits::WarehouseConnector;
use async_stream::stream;
use async_trait::async_trait;
use futures::StreamExt;
use may_secrets::{DwhSecret, SecretsProvider};
use std::sync::Arc;
use tokio_postgres::types::Type;
use tokio_postgres_rustls::MakeRustlsConnect;

pub struct PostgresConnector {
    secret_name: String,
    secrets: Arc<dyn SecretsProvider>,
}

impl PostgresConnector {
    #[must_use]
    pub fn new(secret_name: impl Into<String>, secrets: Arc<dyn SecretsProvider>) -> Self {
        Self {
            secret_name: secret_name.into(),
            secrets,
        }
    }
}

fn map_column_value(row: &tokio_postgres::Row, i: usize) -> Result<ColumnValue, ConnectorError> {
    let column = &row.columns()[i];
    let col_type = column.type_();

    match *col_type {
        Type::INT2 => match row.try_get::<_, Option<i16>>(i) {
            Ok(Some(v)) => Ok(ColumnValue::Int64(i64::from(v))),
            Ok(None) => Ok(ColumnValue::Null),
            Err(e) => Err(ConnectorError::QueryFailed(format!(
                "Failed to decode column '{}' as INT2: {e}",
                column.name()
            ))),
        },
        Type::INT4 => match row.try_get::<_, Option<i32>>(i) {
            Ok(Some(v)) => Ok(ColumnValue::Int64(i64::from(v))),
            Ok(None) => Ok(ColumnValue::Null),
            Err(e) => Err(ConnectorError::QueryFailed(format!(
                "Failed to decode column '{}' as INT4: {e}",
                column.name()
            ))),
        },
        Type::INT8 => match row.try_get::<_, Option<i64>>(i) {
            Ok(Some(v)) => Ok(ColumnValue::Int64(v)),
            Ok(None) => Ok(ColumnValue::Null),
            Err(e) => Err(ConnectorError::QueryFailed(format!(
                "Failed to decode column '{}' as INT8: {e}",
                column.name()
            ))),
        },
        Type::FLOAT4 => match row.try_get::<_, Option<f32>>(i) {
            Ok(Some(v)) => Ok(ColumnValue::Float64(f64::from(v))),
            Ok(None) => Ok(ColumnValue::Null),
            Err(e) => Err(ConnectorError::QueryFailed(format!(
                "Failed to decode column '{}' as FLOAT4: {e}",
                column.name()
            ))),
        },
        Type::FLOAT8 => match row.try_get::<_, Option<f64>>(i) {
            Ok(Some(v)) => Ok(ColumnValue::Float64(v)),
            Ok(None) => Ok(ColumnValue::Null),
            Err(e) => Err(ConnectorError::QueryFailed(format!(
                "Failed to decode column '{}' as FLOAT8: {e}",
                column.name()
            ))),
        },
        Type::TEXT | Type::VARCHAR | Type::BPCHAR => match row.try_get::<_, Option<String>>(i) {
            Ok(Some(v)) => Ok(ColumnValue::Text(v)),
            Ok(None) => Ok(ColumnValue::Null),
            Err(e) => Err(ConnectorError::QueryFailed(format!(
                "Failed to decode column '{}' as string type: {e}",
                column.name()
            ))),
        },
        Type::BOOL => match row.try_get::<_, Option<bool>>(i) {
            Ok(Some(v)) => Ok(ColumnValue::Bool(v)),
            Ok(None) => Ok(ColumnValue::Null),
            Err(e) => Err(ConnectorError::QueryFailed(format!(
                "Failed to decode column '{}' as BOOL: {e}",
                column.name()
            ))),
        },
        Type::BYTEA => match row.try_get::<_, Option<Vec<u8>>>(i) {
            Ok(Some(v)) => Ok(ColumnValue::Bytes(v)),
            Ok(None) => Ok(ColumnValue::Null),
            Err(e) => Err(ConnectorError::QueryFailed(format!(
                "Failed to decode column '{}' as BYTEA: {e}",
                column.name()
            ))),
        },
        _ => Err(ConnectorError::UnsupportedType(col_type.name().to_string())),
    }
}

#[async_trait]
impl WarehouseConnector for PostgresConnector {
    async fn execute(&self, sql: &str) -> Result<QueryResult, ConnectorError> {
        let secret = self
            .secrets
            .get_secret(&self.secret_name)
            .await
            .map_err(|e| ConnectorError::ConnectionFailed(e.to_string()))?;

        let DwhSecret::UsernamePassword {
            host,
            port,
            database,
            username,
            password,
        } = secret
        else {
            return Err(ConnectorError::ConnectionFailed(
                "Expected UsernamePassword secret for Postgres".to_string(),
            ));
        };

        let mut config = tokio_postgres::Config::new();
        config.host(&host);
        config.port(port);
        config.dbname(&database);
        config.user(&username);
        config.password(&password);

        let mut root_store = rustls::RootCertStore::empty();
        let certs = rustls_native_certs::load_native_certs().certs;

        for cert in certs {
            root_store
                .add(cert)
                .map_err(|e| ConnectorError::ConnectionFailed(e.to_string()))?;
        }

        let tls_config = rustls::ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();

        let tls = MakeRustlsConnect::new(tls_config);

        let (client, connection) = config
            .connect(tls)
            .await
            .map_err(|e| ConnectorError::ConnectionFailed(e.to_string()))?;

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                tracing::error!("postgres connection task error: {e}");
            }
        });

        let owned_sql = sql.to_string();

        let stream = stream! {
            let row_stream = match client.query_raw(&owned_sql, std::iter::empty::<&i32>()).await {
                Ok(rs) => rs,
                Err(e) => {
                    yield Err(ConnectorError::QueryFailed(e.to_string()));
                    return;
                }
            };
            tokio::pin!(row_stream);

            while let Some(res) = row_stream.next().await {
                match res {
                    Ok(row) => {
                        let mut result_row = Vec::new();
                        for i in 0..row.columns().len() {
                            match map_column_value(&row, i) {
                                Ok(val) => result_row.push(val),
                                Err(e) => {
                                    yield Err(e);
                                    return;
                                }
                            }
                        }
                        yield Ok(result_row);
                    }
                    Err(e) => {
                        yield Err(ConnectorError::QueryFailed(e.to_string()));
                    }
                }
            }

            drop(client);
        };

        Ok(Box::pin(stream))
    }
}
