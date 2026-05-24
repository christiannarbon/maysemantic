use may_auth::models::Role;
use may_auth::repository::{PgUserRepository, UserRepository};
use sqlx::PgPool;
use std::env;

#[tokio::test]
async fn test_user_repository_integration() -> Result<(), Box<dyn std::error::Error>> {
    if env::var("AUTH_TESTS").is_err() {
        println!("Skipping auth integration tests because AUTH_TESTS is not set");
        return Ok(());
    }

    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5434/postgres".to_string());

    let pool = PgPool::connect(&database_url).await?;
    let repo = PgUserRepository::new(pool);

    let test_username = format!("test_user_{}", uuid::Uuid::new_v4());

    // Create user
    let created_user = repo
        .create(&test_username, "password_hash", Role::Viewer)
        .await?;
    assert_eq!(created_user.username, test_username);
    assert_eq!(created_user.role, Role::Viewer);

    // Retrieve user
    let retrieved_user = repo.find_by_username(&test_username).await?;
    assert_eq!(retrieved_user.id, created_user.id);
    assert_eq!(retrieved_user.username, test_username);
    assert_eq!(retrieved_user.role, Role::Viewer);

    // Test unknown user returns expected error
    let unknown_user_res = repo.find_by_username("does_not_exist_ever").await;
    assert!(matches!(
        unknown_user_res,
        Err(may_auth::error::AuthError::UserNotFound)
    ));

    Ok(())
}
