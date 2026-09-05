use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use http_body_util::BodyExt;
use serial_test::serial;
use tower::ServiceExt;

#[tokio::test]
#[serial]
async fn valid_body_routes_and_returns_200() {
    let app = crate::support::test_app();
    let body = r#"{"metrics":["revenue_by_status"],"dimensions":["status"]}"#;
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/query")
                .header(header::CONTENT_TYPE, "application/json")
                .header(
                    header::AUTHORIZATION,
                    format!("Bearer {}", crate::support::mint_token("admin")),
                )
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
    let sql = json["sql"].as_str().expect("sql is a string");
    assert!(!sql.is_empty(), "compiled SQL should be non-empty");
    assert_eq!(json["columns"], serde_json::json!(["revenue_by_status"]));
    assert_eq!(json["rows"], serde_json::json!([[serde_json::Value::Null]]));
    assert_eq!(json["row_count"], 1);
}

#[tokio::test]
#[serial]
async fn malformed_json_returns_400() {
    let app = crate::support::test_app();
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/query")
                .header(header::CONTENT_TYPE, "application/json")
                .header(
                    header::AUTHORIZATION,
                    format!("Bearer {}", crate::support::mint_token("admin")),
                )
                .body(Body::from("{ not valid json "))
                .expect("request builds"),
        )
        .await
        .expect("router responds");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
#[serial]
async fn empty_metrics_returns_400() {
    let app = crate::support::test_app();
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/query")
                .header(header::CONTENT_TYPE, "application/json")
                .header(
                    header::AUTHORIZATION,
                    format!("Bearer {}", crate::support::mint_token("admin")),
                )
                .body(Body::from(r#"{"metrics":[]}"#))
                .expect("request builds"),
        )
        .await
        .expect("router responds");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    assert!(json["error"].is_string());
}

#[tokio::test]
#[serial]
async fn time_range_bounds_return_400() {
    let app = crate::support::test_app();
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/query")
                .header(header::CONTENT_TYPE, "application/json")
                .header(
                    header::AUTHORIZATION,
                    format!("Bearer {}", crate::support::mint_token("admin")),
                )
                .body(Body::from(
                    r#"{"metrics":["revenue"],"time_range":{"start":"2024-01-01"}}"#,
                ))
                .expect("request builds"),
        )
        .await
        .expect("router responds");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    assert!(json["error"].is_string());
}

#[tokio::test]
#[serial]
async fn unknown_field_returns_400() {
    let app = crate::support::test_app();
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/query")
                .header(header::CONTENT_TYPE, "application/json")
                .header(
                    header::AUTHORIZATION,
                    format!("Bearer {}", crate::support::mint_token("admin")),
                )
                .body(Body::from(
                    r#"{"metrics":["revenue"],"dimension":["status"]}"#,
                ))
                .expect("request builds"),
        )
        .await
        .expect("router responds");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    assert!(json["error"].is_string());
}

#[tokio::test]
#[serial]
async fn missing_token_returns_401() {
    let app = crate::support::test_app();
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/query")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"metrics":["revenue"]}"#))
                .expect("request builds"),
        )
        .await
        .expect("router responds");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    assert!(json["error"].is_string(), "401 must use the error envelope");
}
