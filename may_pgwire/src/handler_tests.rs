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

struct MockClient {
    metadata: HashMap<String, String>,
}

impl pgwire::api::ClientInfo for MockClient {
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

impl futures::sink::Sink<pgwire::messages::PgWireBackendMessage> for MockClient {
    type Error = std::io::Error;

    fn poll_ready(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn start_send(
        self: std::pin::Pin<&mut Self>,
        _item: pgwire::messages::PgWireBackendMessage,
    ) -> Result<(), Self::Error> {
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

struct CapturingConnector {
    captured_sql: Arc<std::sync::Mutex<Option<String>>>,
}

#[async_trait]
impl may_connectors::WarehouseConnector for CapturingConnector {
    async fn execute(
        &self,
        sql: &str,
    ) -> Result<may_connectors::models::QueryResult, may_connectors::error::ConnectorError> {
        *self.captured_sql.lock().unwrap() = Some(sql.to_string());
        let row: may_connectors::models::Row = vec![may_connectors::models::ColumnValue::Int64(42)];
        Ok(Box::pin(futures::stream::iter(vec![Ok(row)])))
    }
}

#[tokio::test]
async fn test_invalid_semantic_metric_returns_pgwire_error() {
    use may_core::StateMgr;
    use pgwire::api::query::SimpleQueryHandler;

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

#[tokio::test]
async fn test_successful_semantic_query_routes_to_connector() {
    use may_core::StateMgr;
    use pgwire::api::query::SimpleQueryHandler;

    let state_mgr = Arc::new(StateMgr::new());

    let yaml = r#"
name: ecommerce_model
entities:
  - name: users
    table: public.users
    primary_key: user_id
    entity_type: dimension
    dimensions:
      - name: user_id
        type: number
        sql: id
      - name: email
        type: string
        sql: email
      - name: signup_date
        type: time
        sql: created_at
    measures:
      - name: user_count
        agg: count
        sql: id
  - name: orders
    table: public.orders
    primary_key: order_id
    entity_type: fact
    dimensions:
      - name: order_id
        type: number
        sql: id
      - name: status
        type: string
        sql: status
    measures:
      - name: total_revenue
        agg: sum
        sql: amount
      - name: order_count
        agg: count
        sql: id
joins: []
metrics:
  - name: daily_active_users
    measure: user_count
    dimensions: [signup_date]
  - name: revenue_by_status
    measure: total_revenue
    dimensions: [status]
"#;

    state_mgr
        .load_from_yaml(yaml)
        .expect("Failed to load fixture model");

    let captured_sql = Arc::new(std::sync::Mutex::new(None));
    let capturing_connector = CapturingConnector {
        captured_sql: captured_sql.clone(),
    };

    let mut connectors = may_connectors::ConnectorRegistry::new();
    connectors.register("ecommerce_model", Arc::new(capturing_connector));
    let connectors_ref = Arc::new(connectors);

    let processor = QueryProcessor::new(state_mgr, connectors_ref);

    let mut client = MockClient {
        metadata: HashMap::new(),
    };

    let query = r#"SEMANTIC {"metric_name":"revenue_by_status"}"#;
    let result = processor.do_query(&mut client, query).await;

    assert!(result.is_ok(), "Query execution failed: {:?}", result.err());

    let sql = captured_sql
        .lock()
        .unwrap()
        .clone()
        .expect("connector.execute was not called");
    assert!(!sql.trim().is_empty(), "Captured SQL should not be empty");

    // Guard REV-1.0.4 FN-1: Compiled SQL must preserve case and NOT be globally uppercased
    assert_ne!(
        sql,
        sql.to_uppercase(),
        "SQL should preserve case (not be globally uppercased)"
    );
    assert!(
        sql.contains("status"),
        "SQL should contain lowercase 'status' identifier, found: {}",
        sql
    );
    assert!(
        sql.contains("SUM(") || sql.contains("sum("),
        "SQL must contain aggregation SUM/sum, found: {}",
        sql
    );
    assert!(
        sql.contains("orders"),
        "SQL must contain base table orders, found: {}",
        sql
    );
}

#[test]
fn test_query_processor_dialect_config() {
    use may_core::StateMgr;
    let state_mgr = Arc::new(StateMgr::new());
    let connectors = Arc::new(may_connectors::ConnectorRegistry::new());
    let processor = QueryProcessor::new(state_mgr, connectors);

    // Verify default dialect is postgres
    assert_eq!(processor.dialect_kind(), "postgres");

    // Verify builder config changes dialect
    let processor_snowflake = processor.with_dialect("snowflake");
    assert_eq!(processor_snowflake.dialect_kind(), "snowflake");
}
