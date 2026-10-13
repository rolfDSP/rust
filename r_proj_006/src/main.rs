//! Thin binary: wires the library together and handles I/O.
//! All logic lives in the library (`src/lib.rs`), so it is reusable and testable.

use std::error::Error;

use r_proj_006::report::formatter_for;
use r_proj_006::{InMemoryRepository, InventoryService};

fn main() -> Result<(), Box<dyn Error>> {
    let format = std::env::args().nth(1).unwrap_or_else(|| "text".into());
    let formatter = formatter_for(&format).ok_or(format!("unknown format '{format}'"))?;

    let mut shop = InventoryService::new(InMemoryRepository::new());

    let apple = shop.add_product("A1", "Apple", 40)?;
    let pear = shop.add_product("P1", "Pear", 55)?;
    shop.restock(&apple, 20)?;
    shop.restock(&pear, 8)?;

    let paid = shop.sell(&apple, 5)?;
    println!("Sold 5 apples for {paid}\n");

    // Errors are values: handle them where it makes sense.
    if let Err(e) = shop.sell(&pear, 100) {
        println!("Expected error: {e}\n");
    }

    print!(
        "{}",
        formatter.render(&shop.products(), shop.total_stock_value())
    );
    Ok(())
}
