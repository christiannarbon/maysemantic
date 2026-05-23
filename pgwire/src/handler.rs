use async_trait::async_trait;
use futures::{Sink, SinkExt, stream};
use maysemantic::StateMgr;
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
}

impl QueryProcessor {
    pub fn new(state_mgr: Arc<StateMgr>) -> Self {
        Self { state_mgr }
    }
}

pub struct MockAuthenticator;

impl ServerParameterProvider for MockAuthenticator {
    fn server_parameters<C>(&self, _client: &C) -> Option<HashMap<String, String>>
    where
        C: ClientInfo,
    {
        let mut params = HashMap::with_capacity(5);
        params.insert("server_version".to_owned(), "13.0".to_owned());
        params.insert("server_encoding".to_owned(), "UTF8".to_owned());
        params.insert("client_encoding".to_owned(), "UTF8".to_owned());
        params.insert("DateStyle".to_owned(), "ISO, MDY".to_owned());
        params.insert("integer_datetimes".to_owned(), "on".to_owned());
        Some(params)
    }
}

#[async_trait]
impl StartupHandler for MockAuthenticator {
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
            PgWireFrontendMessage::PasswordMessageFamily(_) => {
                info!("MockAuthenticator explicitly accepted provided credentials.");
                pgwire::api::auth::finish_authentication(client, self).await?;
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

        let upper_query = query_trimmed.to_uppercase();
        if upper_query.starts_with("SET ")
            || upper_query.starts_with("SHOW ")
            || upper_query.starts_with("BEGIN")
            || upper_query.starts_with("COMMIT")
        {
            debug!("Acknowledging handshake/setup command: {query_trimmed}");
            return Ok(vec![Response::Execution(Tag::new("OK"))]);
        }

        match Parser::parse_sql(&PostgreSqlDialect {}, query_trimmed) {
            Ok(ast) => info!("Successfully parsed AST: {:#?}", ast),
            Err(e) => warn!(
                "Failed to parse query into AST: {:?}. Query was: {}",
                e, query_trimmed
            ),
        }

        let is_version_query = upper_query.contains("VERSION()");
        let is_select_1 = upper_query == "SELECT 1;" || upper_query == "SELECT 1";
        drop(upper_query);

        if is_version_query {
            let field_info =
                FieldInfo::new("version".into(), None, None, Type::TEXT, FieldFormat::Text);
            let schema = Arc::new(vec![field_info]);

            let mut encoder = DataRowEncoder::new(schema.clone());
            encoder.encode_field(&Some("May Semantic Layer (PostgreSQL 14.0 compatible)"))?;
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

        let _stats = self.state_mgr.get_stats().map_err(|e| {
            PgWireError::UserError(Box::new(ErrorInfo::new(
                "ERROR".to_owned(),
                "XX000".to_owned(),
                e.to_string(),
            )))
        })?;

        info!("Returning a generic mock tabular response for query execution.");

        let field_id = FieldInfo::new("id".into(), None, None, Type::INT8, FieldFormat::Text);
        let field_name =
            FieldInfo::new("name".into(), None, None, Type::VARCHAR, FieldFormat::Text);
        let field_active = FieldInfo::new(
            "is_active".into(),
            None,
            None,
            Type::BOOL,
            FieldFormat::Text,
        );
        let schema = Arc::new(vec![field_id, field_name, field_active]);

        let mut encoder1 = DataRowEncoder::new(schema.clone());
        encoder1.encode_field(&Some(1_i64))?;
        encoder1.encode_field(&Some("Alice"))?;
        encoder1.encode_field(&Some(true))?;
        let row1 = encoder1.finish();

        let mut encoder2 = DataRowEncoder::new(schema.clone());
        encoder2.encode_field(&Some(2_i64))?;
        encoder2.encode_field(&Some("Bob"))?;
        encoder2.encode_field(&Some(false))?;
        let row2 = encoder2.finish();

        Ok(vec![Response::Query(QueryResponse::new(
            schema,
            stream::iter(vec![row1, row2]),
        ))])
    }
}

pub struct QueryProcessorFactory {
    handler: Arc<QueryProcessor>,
    authenticator: Arc<MockAuthenticator>,
}

impl QueryProcessorFactory {
    pub fn new(handler: Arc<QueryProcessor>) -> Self {
        Self {
            handler,
            authenticator: Arc::new(MockAuthenticator),
        }
    }
}

impl PgWireHandlerFactory for QueryProcessorFactory {
    type StartupHandler = MockAuthenticator;
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
