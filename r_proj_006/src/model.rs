//! Domain types. They know nothing about storage or presentation.

mod money;
mod product;

pub use money::Money;
pub use product::{Product, ProductId};
