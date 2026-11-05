//! The HTTP layer: routes, handlers, and middleware.

use std::time::Duration;

use axum::extract::rejection::{JsonRejection, PathRejection};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{Value, json};
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

use crate::error::AppError;
use crate::models::{CreateTask, Task, UpdateTask};
use crate::store::TaskStore;

/// Builds the complete service.
pub fn app(store: TaskStore, request_timeout: Duration) -> Router {
    with_middleware(api_routes(store), request_timeout)
}

fn api_routes(store: TaskStore) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/tasks", get(list_tasks).post(create_task))
        .route(
            "/tasks/{id}",
            get(get_task).patch(update_task).delete(delete_task),
        )
        .with_state(store)
}

/// Adds the middleware. The layer added last runs first, so the trace layer also logs
/// the responses of requests that timed out.
pub fn with_middleware(router: Router, request_timeout: Duration) -> Router {
    router
        .layer(TimeoutLayer::with_status_code(
            StatusCode::GATEWAY_TIMEOUT,
            request_timeout,
        ))
        .layer(TraceLayer::new_for_http())
}

/// For load balancers, Docker, and Kubernetes, to check that the service is up.
async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

async fn list_tasks(State(store): State<TaskStore>) -> Json<Vec<Task>> {
    Json(store.list())
}

// Handlers take `Result<Path<_>, _>` and `Result<Json<_>, _>` to receive parsing failures
// themselves. Otherwise axum would answer with its plain text message, not our JSON error.

async fn create_task(
    State(store): State<TaskStore>,
    body: Result<Json<CreateTask>, JsonRejection>,
) -> Result<(StatusCode, Json<Task>), AppError> {
    let Json(input) = body?;
    let task = store.create(validate_title(&input.title)?);
    Ok((StatusCode::CREATED, Json(task)))
}

async fn get_task(
    State(store): State<TaskStore>,
    id: Result<Path<u64>, PathRejection>,
) -> Result<Json<Task>, AppError> {
    let Path(id) = id?;
    store.get(id).map(Json).ok_or(AppError::TaskNotFound(id))
}

async fn update_task(
    State(store): State<TaskStore>,
    id: Result<Path<u64>, PathRejection>,
    body: Result<Json<UpdateTask>, JsonRejection>,
) -> Result<Json<Task>, AppError> {
    let Path(id) = id?;
    let Json(input) = body?;
    let title = input.title.as_deref().map(validate_title).transpose()?;
    store
        .update(id, title, input.completed)
        .map(Json)
        .ok_or(AppError::TaskNotFound(id))
}

async fn delete_task(
    State(store): State<TaskStore>,
    id: Result<Path<u64>, PathRejection>,
) -> Result<StatusCode, AppError> {
    let Path(id) = id?;
    if store.delete(id) {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::TaskNotFound(id))
    }
}

fn validate_title(title: &str) -> Result<String, AppError> {
    let title = title.trim();
    if title.is_empty() {
        return Err(AppError::Validation("title must not be empty".to_string()));
    }
    Ok(title.to_string())
}
