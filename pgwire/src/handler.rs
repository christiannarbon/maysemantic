use async_trait::async_trait;
use futures::{Sink, SinkExt, stream};
use maysemantic::StateMgr;
use pgwire::api::auth::{ServerParameterProvider, StartupHandler};
use pgwire::api::copy::NoopCopyHandler;
use pgwire::api::query::{PlaceholderExtendedQueryHandler, SimpleQueryHandler};
use pgwire::api::results::{DataRowEncoder, FieldFormat, FieldInfo, QueryResponse, Response, Tag};
use pgwire::api::{ClientInfo, PgWireHandlerFactory, Type};
use pgwire::error::{ErrorInfo, PgWireError, PgWireResult};
use pgwire::messages::PgWireBackendMessage;
use pgwire::messages::PgWireFrontendMessage;
use pgwire::messages::response::SslResponse;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use tracing::{debug, info};

pub struct SemanticProcessor {
    state_mgr: Arc<StateMgr>,
}

impl SemanticProcessor {
    pub fn new(state_mgr: Arc<StateMgr>) -> Self {
        Self { state_mgr }
    }
}

impl ServerParameterProvider for SemanticProcessor {
    fn server_parameters<C>(&self, _client: &C) -> Option<HashMap<String, String>>
    where
        C: ClientInfo,
    {
        let mut params = HashMap::with_capacity(5);
        params.insert("server_version".to_owned(), "13.0".to_owned());
        params.insert("server_encoding".to_owned(), "UTF8".to_owned());
        params.insert("client_encoding".to_owned(), "UTF8".to_owned());
        params.insert("DateStyle".to_owned(), "ISO YMD".to_owned());
        params.insert("integer_datetimes".to_owned(), "on".to_owned());
        Some(params)
    }
}

#[async_trait]
impl StartupHandler for SemanticProcessor {
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
impl SimpleQueryHandler for SemanticProcessor {
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

        let upper_query = query_trimmed.to_uppercase();
        if upper_query.starts_with("SET ")
            || upper_query.starts_with("SHOW ")
            || upper_query.starts_with("BEGIN")
            || upper_query.starts_with("COMMIT")
        {
            debug!("Acknowledging handshake/setup command: {query_trimmed}");
            return Ok(vec![Response::Execution(Tag::new("OK"))]);
        }

        let is_version_query = upper_query.contains("VERSION()");
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

        let stats = self.state_mgr.get_stats().map_err(|e| {
            PgWireError::UserError(Box::new(ErrorInfo::new(
                "ERROR".to_owned(),
                "XX000".to_owned(),
                e.to_string(),
            )))
        })?;

        let model_count = stats.model_count;
        info!(
            "Semantic state holds {model_count} models. Parsing real queries coming in MAY-2.0.0."
        );

        let field_info = FieldInfo::new("status".into(), None, None, Type::TEXT, FieldFormat::Text);
        let schema = Arc::new(vec![field_info]);

        let mut encoder = DataRowEncoder::new(schema.clone());
        let msg =
            format!("Query received but execution is pending MAY-2.0.0. Models: {model_count}");
        encoder.encode_field(&Some(msg.as_str()))?;
        let row = encoder.finish();

        Ok(vec![Response::Query(QueryResponse::new(
            schema,
            stream::iter(vec![row]),
        ))])
    }
}

pub struct SemanticProcessorFactory {
    handler: Arc<SemanticProcessor>,
}

impl SemanticProcessorFactory {
    pub fn new(handler: Arc<SemanticProcessor>) -> Self {
        Self { handler }
    }
}

impl PgWireHandlerFactory for SemanticProcessorFactory {
    type StartupHandler = SemanticProcessor;
    type SimpleQueryHandler = SemanticProcessor;
    type ExtendedQueryHandler = PlaceholderExtendedQueryHandler;
    type CopyHandler = NoopCopyHandler;

    fn simple_query_handler(&self) -> Arc<Self::SimpleQueryHandler> {
        self.handler.clone()
    }

    fn extended_query_handler(&self) -> Arc<Self::ExtendedQueryHandler> {
        Arc::new(PlaceholderExtendedQueryHandler)
    }

    fn startup_handler(&self) -> Arc<Self::StartupHandler> {
        self.handler.clone()
    }

    fn copy_handler(&self) -> Arc<Self::CopyHandler> {
        Arc::new(NoopCopyHandler)
    }
}
