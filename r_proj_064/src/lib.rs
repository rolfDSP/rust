//! A small REST microservice built with axum, managing a list of tasks.
//!
//! The library holds the whole service, and `main.rs` only starts it. That split lets
//! the tests in `tests/api.rs` call the router directly, without opening a network port.

pub mod config;
pub mod error;
pub mod models;
pub mod routes;
pub mod store;
