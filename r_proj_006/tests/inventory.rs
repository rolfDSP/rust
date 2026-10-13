//! Integration test: uses only the crate's *public* API, like an external user.

use r_proj_006::{InMemoryRepository, InventoryError, InventoryService, Money};

#[test]
fn full_workflow() {
    let mut shop = InventoryService::new(InMemoryRepository::new());
    let id = shop.add_product("X-1", "Widget", 1_000).unwrap();

    shop.restock(&id, 2).unwrap();
    assert_eq!(shop.sell(&id, 2).unwrap(), Money::from_cents(2_000));

    let err = shop.sell(&id, 1).unwrap_err();
    assert!(matches!(
        err,
        InventoryError::InsufficientStock { available: 0, .. }
    ));
}
