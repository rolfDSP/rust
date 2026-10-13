//! Storage abstraction. The service depends on the `Repository` *trait*,
//! not on a concrete database, so implementations are interchangeable
//! (in-memory for tests, file/SQL for production, ...).

mod memory;

pub use memory::InMemoryRepository;

use crate::error::Result;
use crate::model::{Product, ProductId};

pub trait Repository {
    fn get(&self, id: &ProductId) -> Result<Product>;
    fn insert(&mut self, product: Product) -> Result<()>;
    fn update(&mut self, product: Product) -> Result<()>;
    fn all(&self) -> Vec<Product>;

    /// Default method: implementors get it for free but may override it.
    fn contains(&self, id: &ProductId) -> bool {
        self.get(id).is_ok()
    }
}
