//! The other direction: Python exceptions arriving in Rust.
//!
//! * Calling Python code (`call0`, `call1`, `extract`, `getattr`, …) returns `PyResult`;
//!   `Err(PyErr)` holds the Python exception that was raised.
//! * `err.is_instance_of::<T>(py)` checks the type (respecting inheritance),
//!   `err.value(py)` gives the exception object, `err.traceback(py)` the traceback.
//! * Returning the same `Err(err)` re-raises the original exception unchanged.
//! * `import_exception!` makes exception classes from Python modules usable in Rust.
//! * `PyErr::warn` issues a warning – which becomes an exception under `-W error`.
//! * A Rust `panic!` becomes `pyo3_runtime.PanicException` (a `BaseException`!).

use pyo3::exceptions::{
    PyConnectionError, PyDeprecationWarning, PyRuntimeError, PyTypeError, PyZeroDivisionError,
};
use pyo3::import_exception;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyTuple};

// Python: from json import JSONDecodeError
import_exception!(json, JSONDecodeError);

/// Calls `func(*args)`. A `ZeroDivisionError` is *handled* in Rust (returns `None`),
/// every other exception is re-raised unchanged.
#[pyfunction]
#[pyo3(signature = (func, *args))]
pub fn safe_call(
    py: Python<'_>,
    func: &Bound<'_, PyAny>,
    args: &Bound<'_, PyTuple>,
) -> PyResult<Option<Py<PyAny>>> {
    match func.call1(args) {
        Ok(value) => Ok(Some(value.unbind())),
        Err(err) if err.is_instance_of::<PyZeroDivisionError>(py) => Ok(None),
        Err(err) => Err(err), // re-raise: same object, same traceback
    }
}

/// Calls `func()` and describes the exception it raises as a dict
/// (or returns `None` if it did not raise).
#[pyfunction]
pub fn inspect_exception<'py>(
    py: Python<'py>,
    func: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyDict>>> {
    let Err(err) = func.call0() else {
        return Ok(None);
    };
    let info = PyDict::new(py);
    let value = err.value(py);
    info.set_item("type", err.get_type(py).name()?)?;
    info.set_item("message", value.str()?)?;
    info.set_item("args", value.getattr("args")?)?;
    info.set_item("is_lookup_error", err.is_instance_of::<pyo3::exceptions::PyLookupError>(py))?;
    info.set_item("has_traceback", err.traceback(py).is_some())?;
    info.set_item(
        "cause",
        err.cause(py).map(|c| c.get_type(py).name().map(|n| n.to_string())).transpose()?,
    )?;
    info.set_item("exception", value)?; // the exception object itself
    Ok(Some(info))
}

/// Calls `func()` up to `attempts` times while it raises `ConnectionError`
/// (including subclasses like `ConnectionRefusedError`). Any other exception is
/// re-raised immediately. When all attempts fail, raises `RuntimeError` with the last
/// `ConnectionError` as `__cause__`.
#[pyfunction]
pub fn retry(py: Python<'_>, func: &Bound<'_, PyAny>, attempts: u32) -> PyResult<Py<PyAny>> {
    let mut last_error = None;
    for _ in 0..attempts {
        match func.call0() {
            Ok(v) => return Ok(v.unbind()),
            Err(e) if e.is_instance_of::<PyConnectionError>(py) => last_error = Some(e),
            Err(e) => return Err(e),
        }
    }
    let err = PyRuntimeError::new_err(format!("giving up after {attempts} attempt(s)"));
    err.set_cause(py, last_error);
    Err(err)
}

/// Sums any Python iterable of numbers. `extract::<f64>()` raises `TypeError` for a
/// non-number; we replace it by a clearer `TypeError` that names the position and keeps
/// the original as `__cause__`.
#[pyfunction]
pub fn sum_numbers(py: Python<'_>, values: &Bound<'_, PyAny>) -> PyResult<f64> {
    let mut total = 0.0;
    for (i, item) in values.try_iter()?.enumerate() {
        let item = item?; // an exception raised by the iterator itself propagates
        total += item.extract::<f64>().map_err(|original| {
            let err = PyTypeError::new_err(format!("item #{i} ({}) is not a number", item.repr().map(|r| r.to_string()).unwrap_or_default()));
            err.set_cause(py, Some(original.into()));
            err
        })?;
    }
    Ok(total)
}

/// Parses JSON with Python's `json` module and turns a decode error into a
/// default value; shows `import_exception!` for a class defined in Python.
#[pyfunction]
#[pyo3(signature = (text, default = None))]
pub fn parse_json_or<'py>(
    py: Python<'py>,
    text: &str,
    default: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    match py.import("json")?.call_method1("loads", (text,)) {
        Ok(v) => Ok(v),
        Err(e) if e.is_instance_of::<JSONDecodeError>(py) => {
            Ok(default.unwrap_or_else(|| py.None().into_bound(py)))
        }
        Err(e) => Err(e),
    }
}

/// Raises Python's own `json.JSONDecodeError(msg, doc, pos)` from Rust.
#[pyfunction]
pub fn strict_json_check(text: &str) -> PyResult<()> {
    match text.find('\'') {
        Some(pos) => Err(JSONDecodeError::new_err((
            "single quotes are not valid JSON",
            text.to_string(),
            pos,
        ))),
        None => Ok(()),
    }
}

/// Issues a `DeprecationWarning`. `PyErr::warn` returns `Err` when the warning filter
/// is set to "error" – the `?` then raises the warning as an exception.
#[pyfunction]
pub fn old_add(py: Python<'_>, a: i64, b: i64) -> PyResult<i64> {
    PyErr::warn(
        py,
        &py.get_type::<PyDeprecationWarning>(),
        c"old_add() is deprecated, use the + operator",
        1, // stacklevel 1 = point to the Python caller
    )?;
    Ok(a + b)
}

/// A Rust panic is caught by PyO3 at the FFI boundary and raised as `PanicException`.
#[pyfunction]
pub fn rust_panic(message: &str) -> i32 {
    panic!("{message}");
}

/// Indexing a slice out of bounds panics too – a bug, not an expected error.
#[pyfunction]
pub fn buggy_index(items: Vec<i32>, index: usize) -> i32 {
    items[index]
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = m.py();
    // Make the otherwise hidden PanicException catchable by name.
    m.add("PanicException", py.get_type::<pyo3::panic::PanicException>())?;
    m.add_function(wrap_pyfunction!(safe_call, m)?)?;
    m.add_function(wrap_pyfunction!(inspect_exception, m)?)?;
    m.add_function(wrap_pyfunction!(retry, m)?)?;
    m.add_function(wrap_pyfunction!(sum_numbers, m)?)?;
    m.add_function(wrap_pyfunction!(parse_json_or, m)?)?;
    m.add_function(wrap_pyfunction!(strict_json_check, m)?)?;
    m.add_function(wrap_pyfunction!(old_add, m)?)?;
    m.add_function(wrap_pyfunction!(rust_panic, m)?)?;
    m.add_function(wrap_pyfunction!(buggy_index, m)?)?;
    Ok(())
}
