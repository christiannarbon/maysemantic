use may_auth::models::Role;
use may_auth::repository::{PgUserRepository, UserRepository};
use may_pgwire::server::run_server;
use sqlx::PgPool;
use std::env;
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time::sleep;
use tokio_postgres::NoTls;
use uuid::Uuid;

static NEXT_PORT: AtomicU16 = AtomicU16::new(15432);

async fn setup_db_and_server() -> (String, String, broadcast::Sender<()>, u16) {
    let database_url = "postgres://postgres:may_password@localhost:5433/pagila";
    let pool_res = PgPool::connect(database_url).await;
    assert!(pool_res.is_ok(), "Failed to connect to integration DB");
    let pool = match pool_res {
        Ok(p) => p,
        Err(_) => unreachable!(),
    };

    let migrate_res = sqlx::migrate!("../may_auth/migrations").run(&pool).await;
    assert!(migrate_res.is_ok(), "Failed to run auth migrations");

    let repo = PgUserRepository::new(pool);
    let test_user = format!("pgwire_test_user_{}", Uuid::new_v4());
    let test_pass = "secret123";

    let hash_res = may_auth::password::hash_password(test_pass);
    assert!(hash_res.is_ok(), "Password hashing failed");
    let hashed = match hash_res {
        Ok(h) => h,
        Err(_) => unreachable!(),
    };

    let create_res = repo.create(&test_user, &hashed, Role::Viewer).await;
    assert!(create_res.is_ok(), "Failed to seed user");

    let port = NEXT_PORT.fetch_add(1, Ordering::SeqCst);

    // Set up the repository connection pool (so the server uses this DB instead of reading env var)
    // Wait, the server still uses `env::var("DATABASE_URL")`. We must pass it via env,
    // OR we can change `run_server` to take `Option<String>` or use a temporary `unsafe` block just for `DATABASE_URL` which doesn't race across different ports.
    // Actually, `PAGILA_TESTS` sets the database, so the connection string is the same for ALL tests.
    // Setting `DATABASE_URL` safely using `std::env::set_var` is impossible without `unsafe`. But it's constant for all tests, so it's safe to set it.
    unsafe {
        env::set_var("DATABASE_URL", database_url);
    }

    let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

    tokio::spawn(async move {
        let _ = tracing_subscriber::fmt::try_init();
        if let Err(e) = run_server(port, shutdown_rx).await {
            eprintln!("Server crashed: {:?}", e);
        }
    });

    // Wait for the server to bind to the port
    let mut ready = false;
    for _ in 0..50 {
        if tokio::net::TcpStream::connect(format!("127.0.0.1:{}", port))
            .await
            .is_ok()
        {
            ready = true;
            break;
        }
        sleep(Duration::from_millis(100)).await;
    }
    assert!(ready, "Server failed to start in time");

    (test_user, test_pass.to_string(), shutdown_tx, port)
}

#[tokio::test]
async fn test_pgwire_auth_accepts_valid_credentials() {
    if env::var("PAGILA_TESTS").is_err() {
        println!("Skipping auth integration tests because PAGILA_TESTS is not set");
        return;
    }

    let (test_user, test_pass, shutdown_tx, port) = setup_db_and_server().await;

    let valid_conn_str = format!(
        "host=127.0.0.1 port={} user={} password={} sslmode=disable",
        port, test_user, test_pass
    );
    let connect_res = tokio_postgres::connect(&valid_conn_str, NoTls).await;

    assert!(
        connect_res.is_ok(),
        "Valid credentials should be accepted, but connection failed: {:?}",
        connect_res.err()
    );

    let (client, connection) = match connect_res {
        Ok(c) => c,
        Err(_) => unreachable!(),
    };

    tokio::spawn(async move {
        let _ = connection.await;
    });

    let query_res = client.simple_query("SELECT 1").await;
    assert!(query_res.is_ok(), "Query execution should succeed");

    let messages = match query_res {
        Ok(m) => m,
        Err(_) => unreachable!(),
    };

    let rows: Vec<_> = messages
        .into_iter()
        .filter(|m| matches!(m, tokio_postgres::SimpleQueryMessage::Row(_)))
        .collect();
    assert_eq!(
        rows.len(),
        1,
        "Expected exactly 1 row message from SELECT 1"
    );

    let _ = shutdown_tx.send(());
}

#[tokio::test]
async fn test_pgwire_auth_rejects_invalid_credentials() {
    if env::var("PAGILA_TESTS").is_err() {
        println!("Skipping auth integration tests because PAGILA_TESTS is not set");
        return;
    }

    let (test_user, _, shutdown_tx, port) = setup_db_and_server().await;

    let invalid_conn_str = format!(
        "host=127.0.0.1 port={} user={} password=wrongpass sslmode=disable",
        port, test_user
    );
    let connect_res = tokio_postgres::connect(&invalid_conn_str, NoTls).await;

    assert!(
        connect_res.is_err(),
        "Invalid credentials should be rejected, but connection succeeded"
    );

    if let Err(e) = connect_res {
        let err_msg = e.to_string();
        assert!(
            err_msg.contains("password authentication failed") || err_msg.contains("db error"),
            "Expected password auth failure message, got: {}",
            err_msg
        );
    }

    let _ = shutdown_tx.send(());
}
