//! `pyexc` – a native Python module showing how Python exceptions work with PyO3.
//!
//! | Module           | Topic |
//! |------------------|-------|
//! | [`builtin`]      | raising built-in exceptions, automatic `std` error → exception conversion |
//! | [`bank`]         | custom exception hierarchy, Rust error enum → `PyErr`, context manager `__exit__` |
//! | [`validation`]   | exception class with extra attributes, chaining (`__cause__`), notes, `ExceptionGroup` |
//! | [`catching`]     | catching Python exceptions in Rust, re-raising, retry, warnings, panics |

use pyo3::prelude::*;

pub mod bank;
pub mod builtin;
pub mod catching;
pub mod validation;

#[pymodule]
fn pyexc(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    builtin::register(m)?;
    bank::register(m)?;
    validation::register(m)?;
    catching::register(m)?;
    Ok(())
}
