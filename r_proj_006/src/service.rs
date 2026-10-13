//! Business logic. Generic over `R: Repository` (static dispatch): the
//! compiler generates specialised code per storage type, no runtime cost.

use crate::error::Result;
use crate::model::{Money, Product, ProductId};
use crate::storage::Repository;

pub struct InventoryService<R: Repository> {
    repo: R,
}

impl<R: Repository> InventoryService<R> {
    /// Dependency injection: the caller decides which storage to use.
    pub fn new(repo: R) -> Self {
        Self { repo }
    }

    pub fn add_product(&mut self, id: &str, name: &str, price_cents: u64) -> Result<ProductId> {
        let id = ProductId::new(id)?; // `?` propagates errors upwards
        self.repo.insert(Product::new(
            id.clone(),
            name,
            Money::from_cents(price_cents),
        ))?;
        Ok(id)
    }

    pub fn restock(&mut self, id: &ProductId, qty: u32) -> Result<()> {
        let mut product = self.repo.get(id)?;
        product.restock(qty);
        self.repo.update(product)
    }

    /// Sells `qty` items and returns the price to pay.
    pub fn sell(&mut self, id: &ProductId, qty: u32) -> Result<Money> {
        let mut product = self.repo.get(id)?;
        product.take(qty)?;
        let total = product.price() * qty;
        self.repo.update(product)?;
        Ok(total)
    }

    pub fn total_stock_value(&self) -> Money {
        self.repo.all().iter().map(Product::stock_value).sum()
    }

    pub fn products(&self) -> Vec<Product> {
        self.repo.all()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::InventoryError;
    use crate::storage::InMemoryRepository;

    fn service() -> InventoryService<InMemoryRepository> {
        InventoryService::new(InMemoryRepository::new())
    }

    #[test]
    fn sell_reduces_stock_and_returns_price() {
        let mut s = service();
        let id = s.add_product("A1", "Apple", 40).unwrap();
        s.restock(&id, 10).unwrap();

        assert_eq!(s.sell(&id, 3).unwrap(), Money::from_cents(120));
        assert_eq!(s.total_stock_value(), Money::from_cents(280));
    }

    #[test]
    fn duplicate_product_is_rejected() {
        let mut s = service();
        s.add_product("A1", "Apple", 40).unwrap();
        let err = s.add_product("A1", "Apple", 40).unwrap_err();
        assert!(matches!(err, InventoryError::AlreadyExists(_)));
    }
}
