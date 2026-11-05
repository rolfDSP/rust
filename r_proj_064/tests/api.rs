//! Endpoint tests. They send requests straight to the router, with no network involved.

use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::routing::get;
use http_body_util::BodyExt;
use r_proj_064::routes;
use r_proj_064::store::TaskStore;
use serde_json::{Value, json};
use tower::ServiceExt;

fn test_app() -> Router {
    routes::app(TaskStore::new(), Duration::from_secs(5))
}

/// Sends one request and returns the status and the body parsed as JSON (`Null` if empty).
async fn send(app: &Router, method: &str, uri: &str, body: Option<&str>) -> (StatusCode, Value) {
    let mut request = Request::builder().method(method).uri(uri);
    if body.is_some() {
        request = request.header(header::CONTENT_TYPE, "application/json");
    }
    let request = request
        .body(Body::from(body.unwrap_or_default().to_owned()))
        .unwrap();

    // A Router is cheap to clone, and `oneshot` consumes the clone.
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, json)
}

#[tokio::test]
async fn health_reports_ok() {
    let (status, body) = send(&test_app(), "GET", "/health", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({ "status": "ok" }));
}

#[tokio::test]
async fn task_lifecycle() {
    let app = test_app();

    let (status, body) = send(&app, "POST", "/tasks", Some(r#"{"title":"  Learn axum "}"#)).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(
        body,
        json!({ "id": 1, "title": "Learn axum", "completed": false })
    );

    let (status, body) = send(&app, "PATCH", "/tasks/1", Some(r#"{"completed":true}"#)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({ "id": 1, "title": "Learn axum", "completed": true })
    );

    let (status, body) = send(&app, "GET", "/tasks/1", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["completed"], json!(true));

    let (status, body) = send(&app, "DELETE", "/tasks/1", None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(body, Value::Null);

    let (status, body) = send(&app, "GET", "/tasks", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!([]));
}

#[tokio::test]
async fn tasks_are_listed_by_id() {
    let app = test_app();
    for title in ["first", "second", "third"] {
        let body = json!({ "title": title }).to_string();
        send(&app, "POST", "/tasks", Some(&body)).await;
    }

    let (_, body) = send(&app, "GET", "/tasks", None).await;
    let ids: Vec<u64> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|task| task["id"].as_u64().unwrap())
        .collect();
    assert_eq!(ids, [1, 2, 3]);
}

#[tokio::test]
async fn deleted_ids_are_not_reused() {
    let app = test_app();
    send(&app, "POST", "/tasks", Some(r#"{"title":"a"}"#)).await;
    send(&app, "DELETE", "/tasks/1", None).await;

    let (_, body) = send(&app, "POST", "/tasks", Some(r#"{"title":"b"}"#)).await;
    assert_eq!(body["id"], json!(2));
}

#[tokio::test]
async fn unknown_task_gives_404_on_every_method() {
    let app = test_app();
    for (method, body) in [
        ("GET", None),
        ("PATCH", Some(r#"{"completed":true}"#)),
        ("DELETE", None),
    ] {
        let (status, json) = send(&app, method, "/tasks/99", body).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{method}");
        assert_eq!(json, json!({ "error": "task 99 not found" }), "{method}");
    }
}

#[tokio::test]
async fn blank_title_is_rejected_and_changes_nothing() {
    let app = test_app();
    let (status, body) = send(&app, "POST", "/tasks", Some(r#"{"title":"   "}"#)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body, json!({ "error": "title must not be empty" }));

    send(&app, "POST", "/tasks", Some(r#"{"title":"keep"}"#)).await;
    let (status, _) = send(&app, "PATCH", "/tasks/1", Some(r#"{"title":""}"#)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let (_, body) = send(&app, "GET", "/tasks", None).await;
    assert_eq!(
        body,
        json!([{ "id": 1, "title": "keep", "completed": false }])
    );
}

#[tokio::test]
async fn malformed_requests_get_json_errors() {
    let app = test_app();

    let (status, body) = send(&app, "POST", "/tasks", Some("{not json")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].is_string());

    let (status, body) = send(&app, "POST", "/tasks", Some(r#"{"name":"x"}"#)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(body["error"].is_string());

    let (status, body) = send(&app, "GET", "/tasks/abc", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].is_string());
}

#[tokio::test]
async fn slow_requests_time_out() {
    let slow = Router::new().route(
        "/slow",
        get(|| async {
            tokio::time::sleep(Duration::from_secs(5)).await;
        }),
    );
    let app = routes::with_middleware(slow, Duration::from_millis(10));

    let request = Request::builder().uri("/slow").body(Body::empty()).unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
}
