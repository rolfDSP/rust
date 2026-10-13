//! One error type for the whole crate, plus a `Result` alias.

use std::fmt;

use crate::model::ProductId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InventoryError {
    NotFound(ProductId),
    AlreadyExists(ProductId),
    InsufficientStock {
        id: ProductId,
        requested: u32,
        available: u32,
    },
    InvalidInput(String),
}

impl fmt::Display for InventoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(id) => write!(f, "product {id} not found"),
            Self::AlreadyExists(id) => write!(f, "product {id} already exists"),
            Self::InsufficientStock {
                id,
                requested,
                available,
            } => write!(
                f,
                "product {id}: requested {requested}, only {available} in stock"
            ),
            Self::InvalidInput(msg) => write!(f, "invalid input: {msg}"),
        }
    }
}

// Lets the error work with `Box<dyn Error>`, `?` in `main`, `anyhow`, etc.
impl std::error::Error for InventoryError {}

/// Crate-wide shorthand, like `std::io::Result`.
pub type Result<T> = std::result::Result<T, InventoryError>;
