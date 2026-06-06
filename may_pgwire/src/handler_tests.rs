use crate::handler::{PgWireAuthenticator, QueryProcessor};
use async_trait::async_trait;
use may_auth::error::AuthError;
use may_auth::models::{Role, User};
use may_auth::repository::UserRepository;
use pgwire::api::DefaultClient;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;

struct MockUserRepository;

#[async_trait]
impl UserRepository for MockUserRepository {
    async fn find_by_username(&self, _username: &str) -> Result<User, AuthError> {
        Err(AuthError::UserNotFound)
    }
    async fn create(&self, _u: &str, _p: &str, _r: Role) -> Result<User, AuthError> {
        Err(AuthError::InvalidCredentials)
    }
    async fn list(&self, _page: u32, _per_page: u32) -> Result<Vec<User>, AuthError> {
        Err(AuthError::InvalidCredentials)
    }
    async fn deactivate(&self, _id: uuid::Uuid) -> Result<(), AuthError> {
        Err(AuthError::UserNotFound)
    }
    async fn update(
        &self,
        _id: uuid::Uuid,
        _role: Option<Role>,
        _password_hash: Option<String>,
    ) -> Result<User, AuthError> {
        Err(AuthError::UserNotFound)
    }
}

#[test]
fn test_pgwire_server_parameters() {
    use pgwire::api::auth::ServerParameterProvider;

    let repo = Arc::new(MockUserRepository);
    let auth = PgWireAuthenticator::new(repo);

    let client = DefaultClient::<()>::new(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 5432),
        false,
    );

    let params = auth
        .server_parameters(&client)
        .expect("Parameters should be returned");
    assert_eq!(params.get("server_version").unwrap(), "14.0");
    assert_eq!(params.get("client_encoding").unwrap(), "UTF8");
    assert_eq!(params.get("DateStyle").unwrap(), "ISO, MDY");
}

#[test]
fn test_non_semantic_prefix_bypasses_routing() {
    assert!(!"SELECT 1".starts_with("SEMANTIC "));
}

#[test]
fn test_semantic_prefix_detected() {
    assert!("SEMANTIC {\"metric_name\":\"revenue\"}".starts_with("SEMANTIC "));
}

#[tokio::test]
async fn test_invalid_semantic_metric_returns_pgwire_error() {
    use may_core::StateMgr;
    use pgwire::api::ClientInfo;
    use pgwire::api::query::SimpleQueryHandler;
    use pgwire::messages::PgWireBackendMessage;
    use futures::sink::Sink;

    struct MockClient {
        metadata: HashMap<String, String>,
    }

    impl ClientInfo for MockClient {
        fn socket_addr(&self) -> SocketAddr {
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 5432)
        }
        fn is_secure(&self) -> bool {
            false
        }
        fn state(&self) -> pgwire::api::PgWireConnectionState {
            pgwire::api::PgWireConnectionState::ReadyForQuery
        }
        fn set_state(&mut self, _new_state: pgwire::api::PgWireConnectionState) {}
        fn transaction_status(&self) -> pgwire::messages::response::TransactionStatus {
            pgwire::messages::response::TransactionStatus::Idle
        }
        fn set_transaction_status(
            &mut self,
            _new_status: pgwire::messages::response::TransactionStatus,
        ) {
        }
        fn metadata(&self) -> &HashMap<String, String> {
            &self.metadata
        }
        fn metadata_mut(&mut self) -> &mut HashMap<String, String> {
            &mut self.metadata
        }
    }

    impl Sink<PgWireBackendMessage> for MockClient {
        type Error = std::io::Error;

        fn poll_ready(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Result<(), Self::Error>> {
            std::task::Poll::Ready(Ok(()))
        }

        fn start_send(self: std::pin::Pin<&mut Self>, _item: PgWireBackendMessage) -> Result<(), Self::Error> {
            Ok(())
        }

        fn poll_flush(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Result<(), Self::Error>> {
            std::task::Poll::Ready(Ok(()))
        }

        fn poll_close(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Result<(), Self::Error>> {
            std::task::Poll::Ready(Ok(()))
        }
    }

    let state_mgr = Arc::new(StateMgr::new()); // empty — no models loaded
    let connectors = Arc::new(may_connectors::ConnectorRegistry::new());
    let processor = QueryProcessor::new(state_mgr, connectors);

    let mut client = MockClient {
        metadata: HashMap::new(),
    };

    let query = r#"SEMANTIC {"metric_name":"nonexistent_metric","dimensions":[]}"#;
    let result = processor.do_query(&mut client, query).await;

    let err = match result {
        Err(e) => e,
        Ok(_) => panic!("Expected an error for unknown metric"),
    };
    let err_msg = err.to_string();
    assert!(
        err_msg.contains("nonexistent_metric"),
        "Error message should mention the metric name, got: {err_msg}"
    );
}
