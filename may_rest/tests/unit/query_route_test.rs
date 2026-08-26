use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
};
use http_body_util::BodyExt;
use tower::ServiceExt;

fn build_query_app() -> Router {
    may_rest::build_router(crate::support::test_state())
}

#[tokio::test]
async fn valid_body_routes_and_returns_200() {
    let app = build_query_app();
    let body = r#"{"metrics":["revenue_by_status"],"dimensions":["status"]}"#;
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/query")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .expect("request builds"),
        )
        .await
        .expect("router responds");

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    assert_eq!(json["metric"], "revenue_by_status");
}

#[tokio::test]
async fn malformed_json_returns_400() {
    let app = build_query_app();
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/query")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("{ not valid json "))
                .expect("request builds"),
        )
        .await
        .expect("router responds");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn empty_metrics_returns_400() {
    let app = build_query_app();
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/query")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"metrics":[]}"#))
                .expect("request builds"),
        )
        .await
        .expect("router responds");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
