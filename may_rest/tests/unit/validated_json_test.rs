use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
    routing::post,
};
use http_body_util::BodyExt;
use may_rest::middleware::json::ValidatedJson;
use may_rest::routes::query::QueryRequest;
use tower::ServiceExt;

async fn echo(ValidatedJson(_): ValidatedJson<QueryRequest>) -> StatusCode {
    StatusCode::OK
}

fn app() -> Router {
    Router::new().route("/probe", post(echo))
}

async fn post_json(content_type: Option<&str>, body: &'static str) -> (StatusCode, String) {
    let mut builder = Request::builder().method("POST").uri("/probe");
    if let Some(content_type) = content_type {
        builder = builder.header("content-type", content_type);
    }
    let response = app()
        .oneshot(builder.body(Body::from(body)).expect("request builds"))
        .await
        .expect("router responds");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body collects")
        .to_bytes();
    (status, String::from_utf8_lossy(&bytes).to_string())
}

#[tokio::test]
async fn missing_field_is_400_json_not_422_text() {
    let (status, body) = post_json(Some("application/json"), "{}").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let parsed: serde_json::Value = serde_json::from_str(&body).expect("error body is json");
    assert!(parsed["error"].is_string());
}

#[tokio::test]
async fn malformed_syntax_is_400_json() {
    let (status, body) = post_json(Some("application/json"), "{").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let parsed: serde_json::Value = serde_json::from_str(&body).expect("error body is json");
    assert!(parsed["error"].is_string());
}

#[tokio::test]
async fn missing_content_type_is_400_json_not_415() {
    let (status, body) = post_json(None, r#"{"metrics":["revenue"]}"#).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let parsed: serde_json::Value = serde_json::from_str(&body).expect("error body is json");
    assert!(parsed["error"].is_string());
}

#[tokio::test]
async fn valid_payload_is_accepted() {
    let (status, _) = post_json(Some("application/json"), r#"{"metrics":["revenue"]}"#).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn oversized_body_is_413_not_400() {
    // axum's default body limit is 2 MiB; 3 MiB trips it.
    let big: &'static str =
        Box::leak(format!(r#"{{"metrics":["{}"]}}"#, "x".repeat(3 * 1024 * 1024)).into_boxed_str());
    let (status, body) = post_json(Some("application/json"), big).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    let parsed: serde_json::Value = serde_json::from_str(&body).expect("error body is json");
    assert!(parsed["error"].is_string());
}
