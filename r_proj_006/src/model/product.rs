use std::fmt;

use super::Money;
use crate::error::{InventoryError, Result};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProductId(String);

impl ProductId {
    /// Validating constructor: an invalid `ProductId` can never exist.
    pub fn new(id: impl Into<String>) -> Result<Self> {
        let id = id.into();
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err(InventoryError::InvalidInput(format!(
                "bad product id '{id}'"
            )));
        }
        Ok(Self(id))
    }
}

impl fmt::Display for ProductId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Product {
    // Fields are private: invariants are enforced via methods.
    id: ProductId,
    name: String,
    price: Money,
    stock: u32,
}

impl Product {
    pub fn new(id: ProductId, name: impl Into<String>, price: Money) -> Self {
        Self {
            id,
            name: name.into(),
            price,
            stock: 0,
        }
    }

    pub fn id(&self) -> &ProductId {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn price(&self) -> Money {
        self.price
    }

    pub fn stock(&self) -> u32 {
        self.stock
    }

    pub fn stock_value(&self) -> Money {
        self.price * self.stock
    }

    pub fn restock(&mut self, qty: u32) {
        self.stock += qty;
    }

    pub fn take(&mut self, qty: u32) -> Result<()> {
        if qty > self.stock {
            return Err(InventoryError::InsufficientStock {
                id: self.id.clone(),
                requested: qty,
                available: self.stock,
            });
        }
        self.stock -= qty;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_id() {
        assert!(ProductId::new("").is_err());
        assert!(ProductId::new("has space").is_err());
        assert!(ProductId::new("SKU-42").is_ok());
    }

    #[test]
    fn cannot_take_more_than_stock() {
        let mut p = Product::new(ProductId::new("A").unwrap(), "Apple", Money::from_cents(50));
        p.restock(3);
        assert!(p.take(4).is_err());
        assert!(p.take(3).is_ok());
        assert_eq!(p.stock(), 0);
    }
}
