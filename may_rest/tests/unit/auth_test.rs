use crate::support::MockUserRepository;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use may_auth::{
    models::{Role, User},
    password::hash_password,
};
use may_rest::routes::auth::LoginResponse;
use serial_test::serial;
use tower::ServiceExt; // for `oneshot` and `ready`

fn build_app(mock_repo: MockUserRepository) -> axum::Router {
    may_rest::build_router(crate::support::test_state_with_repo(mock_repo))
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
        is_active: true,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    let mock_repo = MockUserRepository::success(Some(valid_user.clone()));
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
        is_active: true,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    let mock_repo = MockUserRepository::success(Some(valid_user.clone()));
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
    let mock_repo = MockUserRepository::success(None);
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
