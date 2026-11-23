//! Error handling: Rust `Result` → Python exceptions.
//!
//! * Built-in exceptions: `PyValueError::new_err(...)` etc.
//! * Custom exception hierarchy with `create_exception!`.
//! * Converting your own Rust error type via `impl From<MyError> for PyErr`.
//! * Catching Python exceptions in Rust.

use pyo3::create_exception;
use pyo3::exceptions::{PyException, PyValueError};
use pyo3::prelude::*;
use std::fmt;

// Python: class RustyError(Exception), class ParseError(RustyError), class LimitError(RustyError)
create_exception!(rustypy, RustyError, PyException, "Base class for all rustypy errors.");
create_exception!(rustypy, ParseError, RustyError, "Input could not be parsed.");
create_exception!(rustypy, LimitError, RustyError, "A value exceeded a limit.");

/// An ordinary Rust error type, independent of Python.
#[derive(Debug)]
pub enum ConfigError {
    MissingKey(String),
    BadValue { key: String, value: String },
    TooLarge(u64),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::MissingKey(k) => write!(f, "missing key '{k}'"),
            ConfigError::BadValue { key, value } => write!(f, "bad value '{value}' for '{key}'"),
            ConfigError::TooLarge(v) => write!(f, "value {v} exceeds the limit of 1000"),
        }
    }
}

impl std::error::Error for ConfigError {}

/// The bridge: lets `?` turn a `ConfigError` into the matching Python exception.
impl From<ConfigError> for PyErr {
    fn from(e: ConfigError) -> PyErr {
        match e {
            ConfigError::MissingKey(_) | ConfigError::BadValue { .. } => {
                ParseError::new_err(e.to_string())
            }
            ConfigError::TooLarge(_) => LimitError::new_err(e.to_string()),
        }
    }
}

/// Pure Rust logic returning a Rust error.
fn parse_config_impl(text: &str) -> Result<(String, u64), ConfigError> {
    let mut name = None;
    let mut size = None;
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let (k, v) = line.split_once('=').ok_or_else(|| ConfigError::BadValue {
            key: line.to_string(),
            value: String::new(),
        })?;
        match k.trim() {
            "name" => name = Some(v.trim().to_string()),
            "size" => {
                let n: u64 = v.trim().parse().map_err(|_| ConfigError::BadValue {
                    key: "size".into(),
                    value: v.trim().into(),
                })?;
                if n > 1000 {
                    return Err(ConfigError::TooLarge(n));
                }
                size = Some(n);
            }
            _ => {}
        }
    }
    Ok((
        name.ok_or_else(|| ConfigError::MissingKey("name".into()))?,
        size.ok_or_else(|| ConfigError::MissingKey("size".into()))?,
    ))
}

/// Parses `"name = foo\nsize = 10"`. Raises `ParseError` or `LimitError`.
#[pyfunction]
pub fn parse_config(text: &str) -> PyResult<(String, u64)> {
    Ok(parse_config_impl(text)?) // `?` uses the From impl above
}

/// Built-in exception example.
#[pyfunction]
pub fn safe_sqrt(x: f64) -> PyResult<f64> {
    if x < 0.0 {
        Err(PyValueError::new_err(format!("cannot take sqrt of negative number {x}")))
    } else {
        Ok(x.sqrt())
    }
}

/// `std::num::ParseIntError` already converts to `ValueError` automatically.
#[pyfunction]
pub fn parse_int(s: &str) -> PyResult<i64> {
    Ok(s.trim().parse::<i64>()?)
}

/// Calls a Python callable and *catches* any exception it raises, returning
/// `(True, result)` or `(False, "ExceptionType: message")`.
#[pyfunction]
pub fn try_call(py: Python<'_>, func: &Bound<'_, PyAny>) -> (bool, Py<PyAny>) {
    match func.call0() {
        Ok(v) => (true, v.unbind()),
        Err(e) => {
            let type_name = e
                .get_type(py)
                .name()
                .map(|n| n.to_string())
                .unwrap_or_else(|_| "?".into());
            let msg = format!("{type_name}: {}", e.value(py));
            (false, msg.into_pyobject(py).unwrap().into_any().unbind())
        }
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = m.py();
    m.add("RustyError", py.get_type::<RustyError>())?;
    m.add("ParseError", py.get_type::<ParseError>())?;
    m.add("LimitError", py.get_type::<LimitError>())?;
    m.add_function(wrap_pyfunction!(parse_config, m)?)?;
    m.add_function(wrap_pyfunction!(safe_sqrt, m)?)?;
    m.add_function(wrap_pyfunction!(parse_int, m)?)?;
    m.add_function(wrap_pyfunction!(try_call, m)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_config() {
        assert_eq!(
            parse_config_impl("name = abc\nsize = 5").unwrap(),
            ("abc".to_string(), 5)
        );
    }

    #[test]
    fn rejects_large_size() {
        assert!(matches!(
            parse_config_impl("name=a\nsize=5000"),
            Err(ConfigError::TooLarge(5000))
        ));
    }
}
