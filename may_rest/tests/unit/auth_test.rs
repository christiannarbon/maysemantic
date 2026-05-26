use async_trait::async_trait;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use may_auth::{
    error::AuthError,
    models::{Role, User},
    password::hash_password,
    repository::UserRepository,
    token::TokenService,
};
use may_rest::routes::auth::LoginResponse;
use may_rest::{AppState, routes};
use serial_test::serial;
use std::sync::Arc;
use tower::ServiceExt; // for `oneshot` and `ready`

pub struct MockUserRepository {
    pub valid_user: Option<User>,
}

#[async_trait]
impl UserRepository for MockUserRepository {
    async fn find_by_username(&self, username: &str) -> Result<User, AuthError> {
        self.valid_user
            .as_ref()
            .filter(|u| u.username == username)
            .cloned()
            .ok_or(AuthError::UserNotFound)
    }

    async fn create(
        &self,
        _username: &str,
        _password_hash: &str,
        _role: Role,
    ) -> Result<User, AuthError> {
        Err(AuthError::InvalidCredentials)
    }

    async fn list(&self) -> Result<Vec<User>, AuthError> {
        Err(AuthError::InvalidCredentials)
    }
}

fn build_app(mock_repo: MockUserRepository) -> axum::Router {
    #[allow(unsafe_code, reason = "set_var required to set secret for test app")]
    unsafe {
        std::env::set_var("MAY_JWT_SECRET", "test_secret_key");
    }
    let token_service = Arc::new(TokenService::new().unwrap());
    let user_repository = Arc::new(mock_repo);

    let state = AppState {
        user_repository,
        token_service,
    };

    axum::Router::new()
        .nest("/api", routes::router())
        .with_state(state)
}

#[tokio::test]
#[serial]
async fn test_login_valid_credentials() {
    let password = "correct_password";
    let hashed_password = tokio::task::spawn_blocking(move || hash_password(password).unwrap())
        .await
        .unwrap();

    let valid_user = User {
        id: uuid::Uuid::new_v4(),
        username: "test_user".to_string(),
        password_hash: hashed_password,
        role: Role::Viewer,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    let mock_repo = MockUserRepository {
        valid_user: Some(valid_user.clone()),
    };
    let app = build_app(mock_repo);

    let body = serde_json::json!({
        "username": "test_user",
        "password": "correct_password"
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/auth/login")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();

    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let login_resp: LoginResponse = serde_json::from_slice(&body).unwrap();
    assert!(!login_resp.token.is_empty());
}

#[tokio::test]
#[serial]
async fn test_login_invalid_password() {
    let password = "correct_password";
    let hashed_password = tokio::task::spawn_blocking(move || hash_password(password).unwrap())
        .await
        .unwrap();

    let valid_user = User {
        id: uuid::Uuid::new_v4(),
        username: "test_user".to_string(),
        password_hash: hashed_password,
        role: Role::Viewer,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    let mock_repo = MockUserRepository {
        valid_user: Some(valid_user.clone()),
    };
    let app = build_app(mock_repo);

    let body = serde_json::json!({
        "username": "test_user",
        "password": "wrong_password"
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/auth/login")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();

    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[serial]
async fn test_login_non_existent_user() {
    let mock_repo = MockUserRepository { valid_user: None };
    let app = build_app(mock_repo);

    let body = serde_json::json!({
        "username": "nobody",
        "password": "any_password"
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/auth/login")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();

    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
