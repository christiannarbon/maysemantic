use axum::{
    Router,
    body::Body,
    http::{Method, Request, StatusCode},
};
use http_body_util::BodyExt;
use tower::ServiceExt; // for `oneshot`

fn health_app() -> Router {
    may_rest::build_router(crate::support::test_state())
}

#[tokio::test]
async fn health_returns_200_ok() {
    let app = health_app();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("router responds");

    assert_eq!(response.status(), StatusCode::OK);

    let body = response
        .into_body()
        .collect()
        .await
        .expect("body collects")
        .to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).expect("valid json");
    assert_eq!(json["status"], "ok");
}

#[tokio::test]
async fn health_cors_preflight_has_allow_origin() {
    let app = health_app();
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::OPTIONS)
                .uri("/health")
                .header("Origin", "http://example.com")
                .header("Access-Control-Request-Method", "GET")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("router responds");

    assert!(response.status().is_success());
    assert!(
        response
            .headers()
            .contains_key("access-control-allow-origin")
    );
}

#[tokio::test]
async fn health_is_not_mounted_under_api() {
    let app = health_app();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/health")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("router responds");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
