use tokio_postgres::NoTls;

#[tokio::test]
async fn test_pagila_connection_and_basic_query() {
    let connect_str = "host=localhost port=5433 user=postgres password=may_password dbname=pagila";
    
    // Connect to the database.
    let (client, connection) = tokio_postgres::connect(connect_str, NoTls)
        .await
        .expect("Failed to connect to Pagila Postgres database on port 5433. Is docker-compose up?");

    // The connection object performs the actual communication with the database,
    // so spawn it off to run on its own.
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });

    // Run a basic query against a well-known Pagila table
    let rows = client
        .query("SELECT first_name, last_name FROM actor ORDER BY actor_id LIMIT 1", &[])
        .await
        .expect("Failed to execute SELECT query against Pagila");

    assert_eq!(rows.len(), 1, "Expected exactly 1 row from the limit query");
    
    let first_name: &str = rows[0].get("first_name");
    let last_name: &str = rows[0].get("last_name");
    
    assert_eq!(first_name, "PENELOPE");
    assert_eq!(last_name, "GUINESS");
}
