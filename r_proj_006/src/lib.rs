//! `r_proj_006` – a small inventory manager that demonstrates how to
//! structure a Rust project into reusable, modular pieces.
//!
//! Layering (each layer only depends on the layers below it):
//!
//! ```text
//!   main.rs        (binary: wiring, I/O, no business logic)
//!      │
//!   service        (business rules, generic over storage)
//!      │
//!   storage        (trait `Repository` + implementations)
//!      │
//!   model, error   (plain data types, shared error type)
//! ```

// Module declarations: each `mod` maps to a file (`error.rs`) or a
// directory with a same-named file (`model.rs` + `model/`).
pub mod error;
pub mod model;
pub mod report;
pub mod service;
pub mod storage;

// Re-exports: give users of the crate a flat, convenient API
// (`r_proj_006::Product` instead of `r_proj_006::model::product::Product`).
pub use error::{InventoryError, Result};
pub use model::{Money, Product, ProductId};
pub use service::InventoryService;
pub use storage::{InMemoryRepository, Repository};
