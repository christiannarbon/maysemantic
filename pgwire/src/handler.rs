use async_trait::async_trait;
use futures::{Sink, stream};
use maysemantic::StateMgr;
use pgwire::api::auth::noop::NoopStartupHandler;
use pgwire::api::copy::NoopCopyHandler;
use pgwire::api::query::{PlaceholderExtendedQueryHandler, SimpleQueryHandler};
use pgwire::api::results::{DataRowEncoder, FieldFormat, FieldInfo, QueryResponse, Response, Tag};
use pgwire::api::{ClientInfo, PgWireHandlerFactory, Type};
use pgwire::error::{ErrorInfo, PgWireError, PgWireResult};
use pgwire::messages::PgWireBackendMessage;
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

impl NoopStartupHandler for SemanticProcessor {}

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
        info!("Received query: {}", query_trimmed);

        let upper_query = query_trimmed.to_uppercase();
        if upper_query.starts_with("SET ")
            || upper_query.starts_with("SHOW ")
            || upper_query.starts_with("BEGIN")
            || upper_query.starts_with("COMMIT")
        {
            debug!("Acknowledging handshake/setup command: {}", query_trimmed);
            return Ok(vec![Response::Execution(Tag::new("OK"))]);
        }

        if upper_query.contains("VERSION()") {
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

        info!(
            "Semantic state holds {} models. Parsing real queries coming in MAY-2.0.0.",
            stats.model_count
        );

        let field_info = FieldInfo::new("status".into(), None, None, Type::TEXT, FieldFormat::Text);
        let schema = Arc::new(vec![field_info]);

        let mut encoder = DataRowEncoder::new(schema.clone());
        let msg = format!(
            "Query received but execution is pending MAY-2.0.0. Models: {}",
            stats.model_count
        );
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
