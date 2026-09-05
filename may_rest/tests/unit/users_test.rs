use crate::support::{MockUserRepository, mint_token};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use may_auth::models::{Role, User};
use serial_test::serial;
use tower::ServiceExt;

// ---------------------------------------------------------------------------
// POST /api/users
// ---------------------------------------------------------------------------

#[tokio::test]
#[serial]
async fn test_create_user_admin_succeeds() {
    let app = crate::support::test_app_with_repo(MockUserRepository::success(None));
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
    let app = crate::support::test_app_with_repo(MockUserRepository::success(None));
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
    let app = crate::support::test_app_with_repo(MockUserRepository::success(None));

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
    let app = crate::support::test_app_with_repo(MockUserRepository::success(Some(user)));
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
    let app = crate::support::test_app_with_repo(MockUserRepository::success(None));
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
    let app = crate::support::test_app_with_repo(MockUserRepository::success(None));

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
    let app = crate::support::test_app_with_repo(MockUserRepository::success(None));
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
    let app = crate::support::test_app_with_repo(MockUserRepository::with_missing_user());
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
    let app = crate::support::test_app_with_repo(MockUserRepository::success(None));
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
    let app = crate::support::test_app_with_repo(MockUserRepository::success(None));
    let id = uuid::Uuid::new_v4();

    let req = Request::builder()
        .method("DELETE")
        .uri(format!("/api/users/{id}"))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
