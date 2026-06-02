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
    pub fn new(secret_name: String, secrets: Arc<dyn SecretsProvider>) -> Self {
        Self {
            secret_name,
            secrets,
        }
    }
}

#[async_trait]
#[allow(clippy::too_many_lines, reason = "extensive match block for postgres types")]
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
                eprintln!("connection error: {e}");
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
                        for (i, column) in row.columns().iter().enumerate() {
                            let col_type = column.type_();

                            let val = match *col_type {
                                Type::INT2 => match row.try_get::<_, Option<i16>>(i) {
                                    Ok(Some(v)) => ColumnValue::Int64(i64::from(v)),
                                    Ok(None) => ColumnValue::Null,
                                    Err(e) => ColumnValue::Text(e.to_string()),
                                },
                                Type::INT4 => match row.try_get::<_, Option<i32>>(i) {
                                    Ok(Some(v)) => ColumnValue::Int64(i64::from(v)),
                                    Ok(None) => ColumnValue::Null,
                                    Err(e) => ColumnValue::Text(e.to_string()),
                                },
                                Type::INT8 => match row.try_get::<_, Option<i64>>(i) {
                                    Ok(Some(v)) => ColumnValue::Int64(v),
                                    Ok(None) => ColumnValue::Null,
                                    Err(e) => ColumnValue::Text(e.to_string()),
                                },
                                Type::FLOAT4 => match row.try_get::<_, Option<f32>>(i) {
                                    Ok(Some(v)) => ColumnValue::Float64(f64::from(v)),
                                    Ok(None) => ColumnValue::Null,
                                    Err(e) => ColumnValue::Text(e.to_string()),
                                },
                                Type::FLOAT8 => match row.try_get::<_, Option<f64>>(i) {
                                    Ok(Some(v)) => ColumnValue::Float64(v),
                                    Ok(None) => ColumnValue::Null,
                                    Err(e) => ColumnValue::Text(e.to_string()),
                                },
                                Type::TEXT | Type::VARCHAR | Type::BPCHAR => {
                                    match row.try_get::<_, Option<String>>(i) {
                                        Ok(Some(v)) => ColumnValue::Text(v),
                                        Ok(None) => ColumnValue::Null,
                                        Err(e) => ColumnValue::Text(e.to_string()),
                                    }
                                }
                                Type::BOOL => match row.try_get::<_, Option<bool>>(i) {
                                    Ok(Some(v)) => ColumnValue::Bool(v),
                                    Ok(None) => ColumnValue::Null,
                                    Err(e) => ColumnValue::Text(e.to_string()),
                                },
                                Type::BYTEA => match row.try_get::<_, Option<Vec<u8>>>(i) {
                                    Ok(Some(v)) => ColumnValue::Bytes(v),
                                    Ok(None) => ColumnValue::Null,
                                    Err(e) => ColumnValue::Text(e.to_string()),
                                },
                                _ => {
                                    ColumnValue::Text(format!("Unsupported type: {}", col_type.name()))
                                }
                            };
                            result_row.push(val);
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
