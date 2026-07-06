use async_trait::async_trait;
use futures::{Sink, SinkExt, stream};
use may_auth::repository::UserRepository;
use may_core::StateMgr;
use may_core::compiler::{SemanticCompiler, SemanticRequest};

use pgwire::api::PgWireConnectionState;
use pgwire::api::auth::{ServerParameterProvider, StartupHandler};
use pgwire::api::copy::NoopCopyHandler;
use pgwire::api::query::{PlaceholderExtendedQueryHandler, SimpleQueryHandler};
use pgwire::api::results::{DataRowEncoder, FieldFormat, FieldInfo, QueryResponse, Response, Tag};
use pgwire::api::{ClientInfo, PgWireHandlerFactory, Type};
use pgwire::error::{ErrorInfo, PgWireError, PgWireResult};
use pgwire::messages::PgWireBackendMessage;
use pgwire::messages::PgWireFrontendMessage;
use pgwire::messages::response::SslResponse;
use pgwire::messages::startup::Authentication;
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use tracing::{debug, info, warn};

pub struct QueryProcessor {
    state_mgr: Arc<StateMgr>,
    connectors: Arc<may_connectors::ConnectorRegistry>,
    dialect_kind: String,
}

impl QueryProcessor {
    pub fn new(
        state_mgr: Arc<StateMgr>,
        connectors: Arc<may_connectors::ConnectorRegistry>,
    ) -> Self {
        Self {
            state_mgr,
            connectors,
            dialect_kind: "postgres".to_string(),
        }
    }

    #[must_use]
    #[allow(dead_code)]
    pub fn with_dialect(mut self, dialect: &str) -> Self {
        self.dialect_kind = dialect.to_string();
        self
    }

    #[must_use]
    #[allow(dead_code)]
    pub fn dialect_kind(&self) -> &str {
        &self.dialect_kind
    }
}

pub struct PgWireAuthenticator {
    repository: Arc<dyn UserRepository + Send + Sync>,
}

impl PgWireAuthenticator {
    pub fn new(repository: Arc<dyn UserRepository + Send + Sync>) -> Self {
        Self { repository }
    }
}

const PG_COMPAT_VERSION: &str = "14.0";

impl ServerParameterProvider for PgWireAuthenticator {
    fn server_parameters<C>(&self, _client: &C) -> Option<HashMap<String, String>>
    where
        C: ClientInfo,
    {
        let mut params = HashMap::with_capacity(5);
        params.insert("server_version".to_owned(), PG_COMPAT_VERSION.to_owned());
        params.insert("server_encoding".to_owned(), "UTF8".to_owned());
        params.insert("client_encoding".to_owned(), "UTF8".to_owned());
        params.insert("DateStyle".to_owned(), "ISO, MDY".to_owned());
        params.insert("integer_datetimes".to_owned(), "on".to_owned());
        Some(params)
    }
}

#[async_trait]
impl StartupHandler for PgWireAuthenticator {
    async fn on_startup<C>(
        &self,
        client: &mut C,
        message: PgWireFrontendMessage,
    ) -> PgWireResult<()>
    where
        C: ClientInfo + futures::sink::Sink<PgWireBackendMessage> + Unpin + Send,
        C::Error: Debug,
        PgWireError: From<<C as futures::sink::Sink<PgWireBackendMessage>>::Error>,
    {
        match message {
            PgWireFrontendMessage::SslRequest(_) => {
                // Reject SSL requests to force fallback to plaintext
                client
                    .send(PgWireBackendMessage::SslResponse(SslResponse::Refuse))
                    .await?;
            }
            PgWireFrontendMessage::Startup(ref startup) => {
                // Extract and log connection parameters
                pgwire::api::auth::save_startup_parameters_to_metadata(client, startup);

                info!("PGWire Client Handshake Parameters:");
                for (k, v) in &startup.parameters {
                    info!("  {k} = {v}");
                }

                client.set_state(PgWireConnectionState::AuthenticationInProgress);
                client
                    .send(PgWireBackendMessage::Authentication(
                        Authentication::CleartextPassword,
                    ))
                    .await?;
            }
            PgWireFrontendMessage::PasswordMessageFamily(ref password_msg) => {
                let password_string = match password_msg {
                    pgwire::messages::startup::PasswordMessageFamily::Password(p) => {
                        p.password.clone()
                    }
                    pgwire::messages::startup::PasswordMessageFamily::Raw(bytes) => {
                        let len = if bytes.ends_with(&[0]) {
                            bytes.len() - 1
                        } else {
                            bytes.len()
                        };
                        match std::str::from_utf8(&bytes[..len]) {
                            Ok(s) => s.to_owned(),
                            Err(_) => {
                                let error_info = ErrorInfo::new(
                                    "FATAL".to_owned(),
                                    "28P01".to_owned(),
                                    "invalid utf8 in password".to_owned(),
                                );
                                return Err(PgWireError::UserError(Box::new(error_info)));
                            }
                        }
                    }
                    other => {
                        warn!("unsupported password message type: {:?}", other);
                        let error_info = ErrorInfo::new(
                            "FATAL".to_owned(),
                            "28P01".to_owned(),
                            "unsupported password message type".to_owned(),
                        );
                        return Err(PgWireError::UserError(Box::new(error_info)));
                    }
                };

                let user_name = match client.metadata().get("user") {
                    Some(u) => u.clone(),
                    None => {
                        let error_info = ErrorInfo::new(
                            "FATAL".to_owned(),
                            "28P01".to_owned(),
                            "password authentication failed for user \"\"".to_owned(),
                        );
                        return Err(PgWireError::UserError(Box::new(error_info)));
                    }
                };

                let user_res = self.repository.find_by_username(&user_name).await;
                let is_valid = match user_res {
                    Ok(user) => {
                        let verify_task = tokio::task::spawn_blocking(move || {
                            may_auth::password::verify_password(
                                &password_string,
                                &user.password_hash,
                            )
                        })
                        .await;
                        matches!(verify_task, Ok(Ok(true)))
                    }
                    Err(_) => false,
                };

                if is_valid {
                    info!(
                        "PgWireAuthenticator successfully authenticated user: {}",
                        user_name
                    );
                    pgwire::api::auth::finish_authentication(client, self).await?;
                } else {
                    warn!("Authentication failed for user: {}", user_name);
                    let error_info = ErrorInfo::new(
                        "FATAL".to_owned(),
                        "28P01".to_owned(),
                        format!("password authentication failed for user \"{}\"", user_name),
                    );
                    return Err(PgWireError::UserError(Box::new(error_info)));
                }
            }
            _ => {
                debug!(
                    "Ignoring unexpected frontend message during startup phase: {:?}",
                    message
                );
            }
        }
        Ok(())
    }
}

#[async_trait]
impl SimpleQueryHandler for QueryProcessor {
    async fn do_query<'a, C>(
        &self,
        _client: &mut C,
        query: &'a str,
    ) -> PgWireResult<Vec<Response<'a>>>
    where
        C: ClientInfo + Sink<PgWireBackendMessage> + Unpin + Send + Sync,
        C::Error: Debug,
        PgWireError: From<<C as Sink<PgWireBackendMessage>>::Error>,
    {
        let query_trimmed = query.trim();
        info!("Received query: {query_trimmed}");

        if query_trimmed.is_empty() {
            return Ok(vec![Response::EmptyQuery]);
        }

        const SEMANTIC_PREFIX: &str = "SEMANTIC ";

        let mut actual_query = query_trimmed.to_string();

        if let Some(json_body) = query_trimmed.strip_prefix(SEMANTIC_PREFIX) {
            let request: SemanticRequest = match serde_json::from_str(json_body) {
                Ok(r) => r,
                Err(e) => {
                    return Err(PgWireError::UserError(Box::new(ErrorInfo::new(
                        "ERROR".to_string(),
                        "22000".to_string(),
                        format!("Invalid semantic request JSON: {e}"),
                    ))));
                }
            };

            let compiled_sql = {
                let state_lock = self.state_mgr.get_state();
                let state_guard = state_lock.read().map_err(|_| {
                    PgWireError::UserError(Box::new(ErrorInfo::new(
                        "ERROR".to_string(),
                        "XX000".to_string(),
                        "Failed to acquire read lock on semantic state".to_string(),
                    )))
                })?;
                info!("Semantic request validated: metric={}", request.metric_name);

                let state_arc = Arc::new(state_guard.clone());
                // The dialect is dynamically resolved based on QueryProcessor configuration.
                let compiler = SemanticCompiler::new(state_arc, may_core::dialect_for(&self.dialect_kind));

                // RLS context wiring is deferred; pass None until per-connection JWT
                // extraction is implemented (tracked separately, out of scope for SQL-ENGINE-4).
                compiler.compile(request, None).map_err(|e| {
                    PgWireError::UserError(Box::new(ErrorInfo::new(
                        "ERROR".to_string(),
                        "42000".to_string(),
                        e.to_string(),
                    )))
                })?
            };
            tracing::debug!("Compiled semantic SQL: {}", compiled_sql);
            actual_query = compiled_sql;
        }

        let upper_query = actual_query.to_uppercase();
        if upper_query.starts_with("SET ")
            || upper_query.starts_with("SHOW ")
            || upper_query.starts_with("BEGIN")
            || upper_query.starts_with("COMMIT")
        {
            debug!("Acknowledging handshake/setup command: {actual_query}");
            return Ok(vec![Response::Execution(Tag::new("OK"))]);
        }

        match Parser::parse_sql(&PostgreSqlDialect {}, &actual_query) {
            Ok(ast) => info!("Successfully parsed AST: {:#?}", ast),
            Err(e) => warn!(
                "Failed to parse query into AST: {:?}. Query was: {}",
                e, actual_query
            ),
        }

        let is_version_query = upper_query.contains("VERSION()");
        let is_select_1 = upper_query == "SELECT 1;" || upper_query == "SELECT 1";

        if is_version_query {
            let field_info =
                FieldInfo::new("version".into(), None, None, Type::TEXT, FieldFormat::Text);
            let schema = Arc::new(vec![field_info]);

            let mut encoder = DataRowEncoder::new(schema.clone());
            let version_str = format!(
                "May Semantic Layer (PostgreSQL {} compatible)",
                PG_COMPAT_VERSION
            );
            encoder.encode_field(&Some(version_str.as_str()))?;
            let row = encoder.finish();

            return Ok(vec![Response::Query(QueryResponse::new(
                schema,
                stream::iter(vec![row]),
            ))]);
        }

        if is_select_1 {
            let field_col =
                FieldInfo::new("?column?".into(), None, None, Type::INT4, FieldFormat::Text);
            let schema = Arc::new(vec![field_col]);

            let mut encoder = DataRowEncoder::new(schema.clone());
            encoder.encode_field(&Some(1_i32))?;
            let row = encoder.finish();

            return Ok(vec![Response::Query(QueryResponse::new(
                schema,
                stream::iter(vec![row]),
            ))]);
        }

        let data_source_name = match self.state_mgr.get_default_model() {
            Ok(Some(model)) => model.name,
            _ => {
                let error_info = ErrorInfo::new(
                    "ERROR".to_owned(),
                    "XX000".to_owned(),
                    "No semantic model loaded, cannot determine data source.".to_owned(),
                );
                return Err(PgWireError::UserError(Box::new(error_info)));
            }
        };

        let connector = match self.connectors.get(&data_source_name) {
            Some(c) => c,
            None => {
                let error_info = ErrorInfo::new(
                    "ERROR".to_owned(),
                    "XX000".to_owned(),
                    format!(
                        "No connector registered for data source: {}",
                        data_source_name
                    ),
                );
                return Err(PgWireError::UserError(Box::new(error_info)));
            }
        };

        // Execute the resolved SQL — either the pass-through query or the compiled semantic SQL.
        let mut query_result = connector.execute(&actual_query).await.map_err(|e| {
            PgWireError::UserError(Box::new(ErrorInfo::new(
                "ERROR".to_owned(),
                "XX000".to_owned(),
                format!("Connector error: {:?}", e),
            )))
        })?;

        use futures::StreamExt;

        // Peek at the first row to determine column count and build a schema
        let first_row = match query_result.next().await {
            Some(Ok(row)) => row,
            Some(Err(e)) => {
                return Err(PgWireError::UserError(Box::new(ErrorInfo::new(
                    "ERROR".to_owned(),
                    "XX000".to_owned(),
                    format!("Query error: {:?}", e),
                ))));
            }
            None => {
                // Empty result, return empty schema
                return Ok(vec![Response::Query(QueryResponse::new(
                    Arc::new(vec![]),
                    stream::empty(),
                ))]);
            }
        };

        let field_infos: Vec<FieldInfo> = (0..first_row.len())
            .map(|i| {
                FieldInfo::new(
                    format!("col_{}", i),
                    None,
                    None,
                    Type::TEXT,
                    FieldFormat::Text,
                )
            })
            .collect();

        let schema_ref = Arc::new(field_infos);
        let mut data_rows = Vec::new();

        // Process first row
        let mut encoder = DataRowEncoder::new(schema_ref.clone());
        for val in &first_row {
            let s = match val {
                may_connectors::models::ColumnValue::Null => "NULL".to_owned(),
                may_connectors::models::ColumnValue::Int64(i) => i.to_string(),
                may_connectors::models::ColumnValue::Float64(f) => f.to_string(),
                may_connectors::models::ColumnValue::Text(t) => t.clone(),
                may_connectors::models::ColumnValue::Bool(b) => b.to_string(),
                may_connectors::models::ColumnValue::Bytes(_) => "<bytes>".to_owned(),
                _ => "UNKNOWN".to_owned(),
            };
            encoder.encode_field(&Some(s)).map_err(|e| {
                PgWireError::UserError(Box::new(ErrorInfo::new(
                    "ERROR".to_owned(),
                    "XX000".to_owned(),
                    format!("Encoding error: {:?}", e),
                )))
            })?;
        }
        data_rows.push(encoder.finish());

        // Process remaining rows
        while let Some(res) = query_result.next().await {
            let row = res.map_err(|e| {
                PgWireError::UserError(Box::new(ErrorInfo::new(
                    "ERROR".to_owned(),
                    "XX000".to_owned(),
                    format!("Query error: {:?}", e),
                )))
            })?;
            let mut encoder = DataRowEncoder::new(schema_ref.clone());
            for val in &row {
                let s = match val {
                    may_connectors::models::ColumnValue::Null => "NULL".to_owned(),
                    may_connectors::models::ColumnValue::Int64(i) => i.to_string(),
                    may_connectors::models::ColumnValue::Float64(f) => f.to_string(),
                    may_connectors::models::ColumnValue::Text(t) => t.clone(),
                    may_connectors::models::ColumnValue::Bool(b) => b.to_string(),
                    may_connectors::models::ColumnValue::Bytes(_) => "<bytes>".to_owned(),
                    _ => "UNKNOWN".to_owned(),
                };
                encoder.encode_field(&Some(s)).map_err(|e| {
                    PgWireError::UserError(Box::new(ErrorInfo::new(
                        "ERROR".to_owned(),
                        "XX000".to_owned(),
                        format!("Encoding error: {:?}", e),
                    )))
                })?;
            }
            data_rows.push(encoder.finish());
        }

        Ok(vec![Response::Query(QueryResponse::new(
            schema_ref,
            stream::iter(data_rows),
        ))])
    }
}

pub struct QueryProcessorFactory {
    handler: Arc<QueryProcessor>,
    authenticator: Arc<PgWireAuthenticator>,
}

impl QueryProcessorFactory {
    pub fn new(handler: Arc<QueryProcessor>, authenticator: Arc<PgWireAuthenticator>) -> Self {
        Self {
            handler,
            authenticator,
        }
    }
}

impl PgWireHandlerFactory for QueryProcessorFactory {
    type StartupHandler = PgWireAuthenticator;
    type SimpleQueryHandler = QueryProcessor;
    type ExtendedQueryHandler = PlaceholderExtendedQueryHandler;
    type CopyHandler = NoopCopyHandler;

    fn simple_query_handler(&self) -> Arc<Self::SimpleQueryHandler> {
        self.handler.clone()
    }

    fn extended_query_handler(&self) -> Arc<Self::ExtendedQueryHandler> {
        Arc::new(PlaceholderExtendedQueryHandler)
    }

    fn startup_handler(&self) -> Arc<Self::StartupHandler> {
        self.authenticator.clone()
    }

    fn copy_handler(&self) -> Arc<Self::CopyHandler> {
        Arc::new(NoopCopyHandler)
    }
}
