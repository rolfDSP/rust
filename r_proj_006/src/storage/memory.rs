use std::collections::BTreeMap;

use super::Repository;
use crate::error::{InventoryError, Result};
use crate::model::{Product, ProductId};

#[derive(Debug, Default)]
pub struct InMemoryRepository {
    items: BTreeMap<ProductId, Product>,
}

impl InMemoryRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Repository for InMemoryRepository {
    fn get(&self, id: &ProductId) -> Result<Product> {
        self.items
            .get(id)
            .cloned()
            .ok_or_else(|| InventoryError::NotFound(id.clone()))
    }

    fn insert(&mut self, product: Product) -> Result<()> {
        if self.items.contains_key(product.id()) {
            return Err(InventoryError::AlreadyExists(product.id().clone()));
        }
        self.items.insert(product.id().clone(), product);
        Ok(())
    }

    fn update(&mut self, product: Product) -> Result<()> {
        match self.items.get_mut(product.id()) {
            Some(slot) => {
                *slot = product;
                Ok(())
            }
            None => Err(InventoryError::NotFound(product.id().clone())),
        }
    }

    fn all(&self) -> Vec<Product> {
        self.items.values().cloned().collect()
    }

    // Override the default with a cheaper version.
    fn contains(&self, id: &ProductId) -> bool {
        self.items.contains_key(id)
    }
}
