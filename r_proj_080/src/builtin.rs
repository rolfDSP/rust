//! Built-in Python exceptions raised from Rust.
//!
//! * A `#[pyfunction]` returning `PyResult<T>` raises the exception held in `Err(PyErr)`.
//! * Every built-in exception has a Rust type in `pyo3::exceptions` (`PyValueError`, …);
//!   `PyXxxError::new_err(args)` creates the `PyErr` (lazily – the Python object is only
//!   built when Python actually needs it).
//! * Many `std` error types implement `From<E> for PyErr`, so `?` converts them
//!   automatically (`ParseIntError` → `ValueError`, `io::Error` → `OSError` subclasses, …).
//! * Argument conversion errors are raised by PyO3 itself (`TypeError`, `OverflowError`).

use pyo3::exceptions::{
    PyArithmeticError, PyIndexError, PyKeyError, PyNotImplementedError, PyOverflowError,
    PyPermissionError, PyRuntimeError, PyTypeError, PyValueError, PyZeroDivisionError,
};
use pyo3::prelude::*;
use std::collections::HashMap;

/// `ZeroDivisionError` (subclass of `ArithmeticError`).
#[pyfunction]
pub fn divide(a: f64, b: f64) -> PyResult<f64> {
    if b == 0.0 {
        return Err(PyZeroDivisionError::new_err("division by zero (raised in Rust)"));
    }
    Ok(a / b)
}

/// `IndexError` (subclass of `LookupError`). Negative indices work like in Python.
#[pyfunction]
pub fn get_item(items: Vec<i64>, index: isize) -> PyResult<i64> {
    let len = items.len() as isize;
    let i = if index < 0 { index + len } else { index };
    if i < 0 || i >= len {
        return Err(PyIndexError::new_err(format!(
            "index {index} out of range for list of length {len}"
        )));
    }
    Ok(items[i as usize])
}

/// `KeyError` (subclass of `LookupError`). Passing the key itself – not a sentence –
/// mirrors what Python's `dict` does: `KeyError('missing')`.
#[pyfunction]
pub fn lookup(table: HashMap<String, i64>, key: &str) -> PyResult<i64> {
    table
        .get(key)
        .copied()
        .ok_or_else(|| PyKeyError::new_err(key.to_string()))
}

/// `TypeError` raised by an explicit type check on an arbitrary object.
#[pyfunction]
pub fn ensure_str(obj: &Bound<'_, PyAny>) -> PyResult<String> {
    if let Ok(s) = obj.extract::<String>() {
        return Ok(s);
    }
    let type_name = obj.get_type().name()?;
    Err(PyTypeError::new_err(format!("expected str, got {type_name}")))
}

/// `NotImplementedError` – a placeholder for future functionality.
#[pyfunction]
pub fn not_ready() -> PyResult<()> {
    Err(PyNotImplementedError::new_err("this feature is not implemented yet"))
}

/// Raises the exception named `kind` with `message` – used by the demo to show how
/// Python's exception *hierarchy* decides which `except` clause catches what.
#[pyfunction]
pub fn raise_builtin(kind: &str, message: String) -> PyResult<()> {
    Err(match kind {
        "ValueError" => PyValueError::new_err(message),
        "TypeError" => PyTypeError::new_err(message),
        "KeyError" => PyKeyError::new_err(message),
        "IndexError" => PyIndexError::new_err(message),
        "ZeroDivisionError" => PyZeroDivisionError::new_err(message),
        "OverflowError" => PyOverflowError::new_err(message),
        "ArithmeticError" => PyArithmeticError::new_err(message),
        "PermissionError" => PyPermissionError::new_err(message),
        "RuntimeError" => PyRuntimeError::new_err(message),
        other => PyValueError::new_err(format!("unknown exception kind '{other}'")),
    })
}

// ------------------------------------------------------------------------------------
// Automatic conversions: just use `?` – PyO3 implements `From<std error> for PyErr`.
// ------------------------------------------------------------------------------------

/// `std::num::ParseIntError` → `ValueError`.
#[pyfunction]
pub fn parse_int(text: &str) -> PyResult<i64> {
    Ok(text.trim().parse::<i64>()?)
}

/// `std::num::ParseFloatError` → `ValueError`.
#[pyfunction]
pub fn parse_float(text: &str) -> PyResult<f64> {
    Ok(text.trim().parse::<f64>()?)
}

/// `std::num::TryFromIntError` → `ValueError`.
#[pyfunction]
pub fn to_u8(value: i64) -> PyResult<u8> {
    Ok(u8::try_from(value)?)
}

/// `std::io::Error` → the matching `OSError` subclass, chosen by `ErrorKind`:
/// `FileNotFoundError`, `PermissionError`, `IsADirectoryError`, … (only the message is
/// passed, so `errno` stays `None`).
#[pyfunction]
pub fn read_text(path: &str) -> PyResult<String> {
    Ok(std::fs::read_to_string(path)?)
}

/// `std::string::FromUtf8Error` → `UnicodeDecodeError`.
/// (Note: `std::str::Utf8Error` has *no* automatic conversion – use `String::from_utf8`.)
#[pyfunction]
pub fn decode_utf8(data: Vec<u8>) -> PyResult<String> {
    Ok(String::from_utf8(data)?)
}

/// Checked arithmetic: `None` from `checked_mul` turned into `OverflowError` by hand.
#[pyfunction]
pub fn multiply_i32(a: i32, b: i32) -> PyResult<i32> {
    a.checked_mul(b)
        .ok_or_else(|| PyOverflowError::new_err(format!("{a} * {b} overflows i32")))
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(divide, m)?)?;
    m.add_function(wrap_pyfunction!(get_item, m)?)?;
    m.add_function(wrap_pyfunction!(lookup, m)?)?;
    m.add_function(wrap_pyfunction!(ensure_str, m)?)?;
    m.add_function(wrap_pyfunction!(not_ready, m)?)?;
    m.add_function(wrap_pyfunction!(raise_builtin, m)?)?;
    m.add_function(wrap_pyfunction!(parse_int, m)?)?;
    m.add_function(wrap_pyfunction!(parse_float, m)?)?;
    m.add_function(wrap_pyfunction!(to_u8, m)?)?;
    m.add_function(wrap_pyfunction!(read_text, m)?)?;
    m.add_function(wrap_pyfunction!(decode_utf8, m)?)?;
    m.add_function(wrap_pyfunction!(multiply_i32, m)?)?;
    Ok(())
}
