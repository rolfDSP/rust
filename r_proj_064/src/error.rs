//! The service's error type. Implementing `IntoResponse` lets handlers return
//! `Result<_, AppError>` and use `?`, and every error reaches the client as JSON.

use axum::Json;
use axum::extract::rejection::{JsonRejection, PathRejection};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("task {0} not found")]
    TaskNotFound(u64),

    #[error("{0}")]
    Validation(String),

    /// The body was not valid JSON, or did not match the expected type.
    #[error(transparent)]
    InvalidJson(#[from] JsonRejection),

    /// A path parameter could not be parsed, for example `/tasks/abc`.
    #[error(transparent)]
    InvalidPath(#[from] PathRejection),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self {
            AppError::TaskNotFound(_) => StatusCode::NOT_FOUND,
            AppError::Validation(_) => StatusCode::UNPROCESSABLE_ENTITY,
            AppError::InvalidJson(rejection) => rejection.status(),
            AppError::InvalidPath(rejection) => rejection.status(),
        };
        (status, Json(json!({ "error": self.to_string() }))).into_response()
    }
}
