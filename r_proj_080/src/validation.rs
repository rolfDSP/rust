//! Richer exceptions.
//!
//! * `ValidationError` is a `#[pyclass(extends = PyException)]`: a real exception class
//!   with typed attributes (`e.field`, `e.reason`) and its own `__str__`.
//! * Exception chaining: `PyErr::set_cause` gives the same result as Python's
//!   `raise ConfigError(...) from original_error` (`__cause__`).
//! * Notes: `BaseException.add_note()` (Python 3.11+) attaches extra lines shown in the
//!   traceback (`__notes__`).
//! * `ExceptionGroup` (Python 3.11+) reports several errors at once; Python handles it
//!   with `except*`.

use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;

create_exception!(pyexc, ConfigError, PyException, "A configuration file could not be loaded.");

/// Exception with structured data. Python can also raise it itself:
/// `raise pyexc.ValidationError("age", "must be positive")`.
#[pyclass(extends = PyException, module = "pyexc", subclass)]
pub struct ValidationError {
    #[pyo3(get)]
    field: String,
    #[pyo3(get)]
    reason: String,
}

#[pymethods]
impl ValidationError {
    #[new]
    fn new(field: String, reason: String) -> Self {
        Self { field, reason }
    }

    fn __str__(&self) -> String {
        format!("{}: {}", self.field, self.reason)
    }

    fn __repr__(&self) -> String {
        format!("ValidationError({:?}, {:?})", self.field, self.reason)
    }
}

/// Builds a lazy `PyErr`; Python calls `ValidationError(field, reason)` when it is raised.
fn invalid(field: &str, reason: impl Into<String>) -> PyErr {
    PyErr::new::<ValidationError, _>((field.to_string(), reason.into()))
}

/// Pure checks, returning *all* problems instead of stopping at the first one.
fn check_user(name: &str, age: i64, email: &str) -> Vec<PyErr> {
    let mut errors = Vec::new();
    if name.trim().is_empty() {
        errors.push(invalid("name", "must not be empty"));
    }
    if !(0..=150).contains(&age) {
        errors.push(invalid("age", format!("{age} is not between 0 and 150")));
    }
    if !email.contains('@') {
        errors.push(invalid("email", format!("'{email}' has no '@'")));
    }
    errors
}

/// Raises the *first* `ValidationError`, or returns a greeting.
#[pyfunction]
pub fn validate_user(name: &str, age: i64, email: &str) -> PyResult<String> {
    match check_user(name, age, email).into_iter().next() {
        Some(err) => Err(err),
        None => Ok(format!("user {name} ok")),
    }
}

/// Validates every `(name, age, email)` tuple and raises **one** `ExceptionGroup`
/// holding all `ValidationError`s (each with a note saying which record failed).
#[pyfunction]
pub fn validate_users(py: Python<'_>, users: Vec<(String, i64, String)>) -> PyResult<usize> {
    let mut all = Vec::new();
    for (i, (name, age, email)) in users.iter().enumerate() {
        for err in check_user(name, *age, email) {
            let exc = err.into_value(py); // force creation of the exception object
            exc.bind(py).call_method1("add_note", (format!("record #{i} ({name:?})"),))?;
            all.push(exc);
        }
    }
    if all.is_empty() {
        return Ok(users.len());
    }
    let group_type = py.import("builtins")?.getattr("ExceptionGroup")?;
    let group = group_type.call1((format!("{} validation error(s)", all.len()), all))?;
    Err(PyErr::from_value(group))
}

/// Reads a `key=value` config file.
///
/// * a missing/unreadable file → `ConfigError` whose `__cause__` is the original
///   `FileNotFoundError`/`PermissionError` (like `raise ... from err`) plus a note;
/// * a malformed line → `ConfigError` whose `__cause__` is a `ValueError`.
#[pyfunction]
pub fn load_config(py: Python<'_>, path: &str) -> PyResult<Vec<(String, String)>> {
    let text = std::fs::read_to_string(path).map_err(|io_err| {
        let err = ConfigError::new_err(format!("cannot load config '{path}'"));
        err.set_cause(py, Some(PyErr::from(io_err))); // io::Error → OSError subclass
        let _ = err
            .value(py)
            .call_method1("add_note", ("hint: check the path and file permissions",));
        err
    })?;

    let mut entries = Vec::new();
    for (no, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            let err = ConfigError::new_err(format!("{path}:{}: syntax error", no + 1));
            err.set_cause(
                py,
                Some(pyo3::exceptions::PyValueError::new_err(format!(
                    "expected 'key=value', got {line:?}"
                ))),
            );
            return Err(err);
        };
        entries.push((k.trim().to_string(), v.trim().to_string()));
    }
    Ok(entries)
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = m.py();
    m.add("ConfigError", py.get_type::<ConfigError>())?;
    m.add_class::<ValidationError>()?;
    m.add_function(wrap_pyfunction!(validate_user, m)?)?;
    m.add_function(wrap_pyfunction!(validate_users, m)?)?;
    m.add_function(wrap_pyfunction!(load_config, m)?)?;
    Ok(())
}
