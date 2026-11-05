//! The JSON data types of the API.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Task {
    pub id: u64,
    pub title: String,
    pub completed: bool,
}

/// Request body of `POST /tasks`.
#[derive(Debug, Deserialize)]
pub struct CreateTask {
    pub title: String,
}

/// Request body of `PATCH /tasks/{id}`. Fields left out keep their current value.
#[derive(Debug, Deserialize)]
pub struct UpdateTask {
    pub title: Option<String>,
    pub completed: Option<bool>,
}
