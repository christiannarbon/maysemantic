use async_trait::async_trait;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use may_auth::{
    error::AuthError,
    models::{Role, User},
    repository::UserRepository,
    token::TokenService,
};
use may_rest::{AppState, routes};
use serial_test::serial;
use std::sync::Arc;
use tower::ServiceExt;

// ---------------------------------------------------------------------------
// Mock repository with configurable behaviour
// ---------------------------------------------------------------------------

/// Controls what the mock `create` call returns.
pub enum CreateOutcome {
    /// Return a freshly minted User.
    Success,
    /// Simulate a DB error.
    Failure,
}

pub struct MockUserRepository {
    /// Used by `find_by_username`.
    pub valid_user: Option<User>,
    /// Controls `create` response.
    pub create_outcome: CreateOutcome,
    /// Controls `deactivate`: `None` means success, `Some(e)` means error.
    pub deactivate_error: Option<AuthError>,
}

impl MockUserRepository {
    fn success(valid_user: Option<User>) -> Self {
        Self {
            valid_user,
            create_outcome: CreateOutcome::Success,
            deactivate_error: None,
        }
    }

    fn with_missing_user() -> Self {
        Self {
            valid_user: None,
            create_outcome: CreateOutcome::Failure,
            deactivate_error: Some(AuthError::UserNotFound),
        }
    }
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
        username: &str,
        _password_hash: &str,
        role: Role,
    ) -> Result<User, AuthError> {
        match self.create_outcome {
            CreateOutcome::Success => Ok(User {
                id: uuid::Uuid::new_v4(),
                username: username.to_string(),
                password_hash: "[hashed]".to_string(),
                role,
                is_active: true,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            }),
            CreateOutcome::Failure => Err(AuthError::InvalidCredentials),
        }
    }

    async fn list(&self, _page: u32, _per_page: u32) -> Result<Vec<User>, AuthError> {
        match &self.valid_user {
            Some(u) => Ok(vec![u.clone()]),
            None => Ok(vec![]),
        }
    }

    async fn deactivate(&self, _id: uuid::Uuid) -> Result<(), AuthError> {
        match &self.deactivate_error {
            None => Ok(()),
            Some(AuthError::UserNotFound) => Err(AuthError::UserNotFound),
            Some(_) => Err(AuthError::InvalidCredentials),
        }
    }
}

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

fn build_app(mock_repo: MockUserRepository) -> axum::Router {
    #[allow(unsafe_code, reason = "set_var required to set secret for test app")]
    unsafe {
        std::env::set_var("MAY_JWT_SECRET", "test_secret_key_for_users_tests");
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

/// Mint a JWT with the given role claim using the shared test secret.
fn mint_token(role: &str) -> String {
    #[allow(unsafe_code, reason = "set_var required for test token minting")]
    unsafe {
        std::env::set_var("MAY_JWT_SECRET", "test_secret_key_for_users_tests");
    }
    // Construct a minimal User so TokenService::issue can sign it.
    let user = User {
        id: uuid::Uuid::new_v4(),
        username: "token_owner".to_string(),
        password_hash: String::new(),
        role: match role {
            "admin" => Role::Admin,
            _ => Role::Viewer,
        },
        is_active: true,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    let ts = TokenService::new().unwrap();
    ts.issue(&user).unwrap()
}

// ---------------------------------------------------------------------------
// POST /api/users
// ---------------------------------------------------------------------------

#[tokio::test]
#[serial]
async fn test_create_user_admin_succeeds() {
    let app = build_app(MockUserRepository::success(None));
    let token = mint_token("admin");

    let body = serde_json::json!({
        "username": "new_user",
        "password": "secure_pass_1!",
        "role": "viewer"
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/users")
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(body.to_string()))
        .unwrap();

    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["username"], "new_user");
    assert!(
        json.get("password_hash").is_none(),
        "password_hash must not be exposed"
    );
}

#[tokio::test]
#[serial]
async fn test_create_user_viewer_forbidden() {
    let app = build_app(MockUserRepository::success(None));
    let token = mint_token("viewer");

    let body = serde_json::json!({
        "username": "hacker",
        "password": "whatever",
        "role": "admin"
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/users")
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(body.to_string()))
        .unwrap();

    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
#[serial]
async fn test_create_user_missing_jwt_unauthorized() {
    let app = build_app(MockUserRepository::success(None));

    let body = serde_json::json!({
        "username": "new_user",
        "password": "secure_pass_1!",
        "role": "viewer"
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/users")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();

    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

// ---------------------------------------------------------------------------
// GET /api/users
// ---------------------------------------------------------------------------

#[tokio::test]
#[serial]
async fn test_list_users_admin_succeeds() {
    let user = User {
        id: uuid::Uuid::new_v4(),
        username: "alice".to_string(),
        password_hash: "[hashed]".to_string(),
        role: Role::Viewer,
        is_active: true,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    let app = build_app(MockUserRepository::success(Some(user)));
    let token = mint_token("admin");

    let req = Request::builder()
        .method("GET")
        .uri("/api/users?page=1&per_page=10")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(json.is_array());
    let arr = json.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["username"], "alice");
    assert!(
        arr[0].get("password_hash").is_none(),
        "password_hash must not be exposed"
    );
}

#[tokio::test]
#[serial]
async fn test_list_users_viewer_forbidden() {
    let app = build_app(MockUserRepository::success(None));
    let token = mint_token("viewer");

    let req = Request::builder()
        .method("GET")
        .uri("/api/users")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
#[serial]
async fn test_list_users_missing_jwt_unauthorized() {
    let app = build_app(MockUserRepository::success(None));

    let req = Request::builder()
        .method("GET")
        .uri("/api/users")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

// ---------------------------------------------------------------------------
// DELETE /api/users/{id}
// ---------------------------------------------------------------------------

#[tokio::test]
#[serial]
async fn test_deactivate_user_admin_succeeds() {
    let app = build_app(MockUserRepository::success(None));
    let token = mint_token("admin");
    let id = uuid::Uuid::new_v4();

    let req = Request::builder()
        .method("DELETE")
        .uri(format!("/api/users/{id}"))
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
#[serial]
async fn test_deactivate_user_not_found() {
    let app = build_app(MockUserRepository::with_missing_user());
    let token = mint_token("admin");
    let id = uuid::Uuid::new_v4();

    let req = Request::builder()
        .method("DELETE")
        .uri(format!("/api/users/{id}"))
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
#[serial]
async fn test_deactivate_user_viewer_forbidden() {
    let app = build_app(MockUserRepository::success(None));
    let token = mint_token("viewer");
    let id = uuid::Uuid::new_v4();

    let req = Request::builder()
        .method("DELETE")
        .uri(format!("/api/users/{id}"))
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
#[serial]
async fn test_deactivate_user_missing_jwt_unauthorized() {
    let app = build_app(MockUserRepository::success(None));
    let id = uuid::Uuid::new_v4();

    let req = Request::builder()
        .method("DELETE")
        .uri(format!("/api/users/{id}"))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
