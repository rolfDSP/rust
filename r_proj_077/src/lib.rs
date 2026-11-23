//! # rustypy – a comprehensive PyO3 example
//!
//! This crate is compiled into a native Python extension module called
//! `rustypy`. Every sub-module below focuses on one topic:
//!
//! | Rust module      | What it shows                                              |
//! |------------------|------------------------------------------------------------|
//! | [`scalars`]      | int / float / bool / str / bytes / complex / Option / char |
//! | [`collections`]  | list, tuple, dict, set, nested containers                  |
//! | [`classes`]      | `#[pyclass]` structs, properties, dunder methods, iterators|
//! | [`enums`]        | simple (C-like) and data-carrying enums                    |
//! | [`errors`]       | `Result` → exceptions, custom exception types              |
//! | [`dynamic`]      | `*args`, `**kwargs`, defaults, callbacks, duck typing, GIL |
//!
//! Build & install into the active virtualenv with `maturin develop`.

use pyo3::prelude::*;

pub mod classes;
pub mod collections;
pub mod dynamic;
pub mod enums;
pub mod errors;
pub mod scalars;

/// The module initialiser. Its name **must** match `[lib] name` in Cargo.toml.
#[pymodule]
fn rustypy(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    // Module level constants of different types
    m.add("PI", std::f64::consts::PI)?;
    m.add("MAX_U32", u32::MAX)?;
    m.add("GREETING", "Hello from Rust")?;

    scalars::register(m)?;
    collections::register(m)?;
    classes::register(m)?;
    enums::register(m)?;
    errors::register(m)?;
    dynamic::register(m)?;
    Ok(())
}
