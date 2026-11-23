//! Container conversions.
//!
//! | Python                 | Rust (owned, copied)                    | Rust (borrowed, no copy) |
//! |------------------------|-----------------------------------------|--------------------------|
//! | `list` / any sequence  | `Vec<T>`, `[T; N]`                      | `Bound<PyList>`          |
//! | `tuple`                | `(A, B, ...)`                           | `Bound<PyTuple>`         |
//! | `dict`                 | `HashMap<K, V>`, `BTreeMap<K, V>`       | `Bound<PyDict>`          |
//! | `set` / `frozenset`    | `HashSet<T>`, `BTreeSet<T>`             | `Bound<PySet>`           |
//!
//! Owned conversions copy the whole container (simple, safe). The `Bound<...>`
//! variants work directly on the Python object, so mutations are visible to the caller.

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};
use std::collections::{BTreeMap, HashMap, HashSet};

// ------------------------------------------------------------------- lists

/// `list[float]` → `Vec<f64>` → statistics tuple `(min, max, mean)`.
#[pyfunction]
pub fn stats(values: Vec<f64>) -> Option<(f64, f64, f64)> {
    if values.is_empty() {
        return None;
    }
    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    Some((min, max, mean))
}

/// Returns a new sorted and de-duplicated list. Works with any iterable of ints.
#[pyfunction]
pub fn sorted_unique(mut values: Vec<i64>) -> Vec<i64> {
    values.sort_unstable();
    values.dedup();
    values
}

/// Fixed size arrays: Python must pass exactly 3 elements.
#[pyfunction]
pub fn cross_product(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Nested containers: `list[list[float]]` (a matrix) is transposed.
#[pyfunction]
pub fn transpose(matrix: Vec<Vec<f64>>) -> PyResult<Vec<Vec<f64>>> {
    let rows = matrix.len();
    let cols = matrix.first().map_or(0, Vec::len);
    if matrix.iter().any(|r| r.len() != cols) {
        return Err(pyo3::exceptions::PyValueError::new_err("ragged matrix"));
    }
    Ok((0..cols)
        .map(|c| (0..rows).map(|r| matrix[r][c]).collect())
        .collect())
}

/// Works **in place** on the caller's list (no copy): every element is doubled.
/// Elements may be any Python object supporting `* 2`.
#[pyfunction]
pub fn double_in_place(list: &Bound<'_, PyList>) -> PyResult<()> {
    for i in 0..list.len() {
        let item = list.get_item(i)?;
        list.set_item(i, item.mul(2)?)?;
    }
    Ok(())
}

// ------------------------------------------------------------------ tuples

/// Tuples with mixed types are mapped to Rust tuples.
#[pyfunction]
pub fn describe_person(person: (String, u8, bool)) -> String {
    let (name, age, active) = person;
    format!(
        "{name} is {age} years old and {}",
        if active { "active" } else { "inactive" }
    )
}

/// Returns a variable-length tuple built at runtime.
#[pyfunction]
pub fn make_tuple<'py>(py: Python<'py>, n: usize) -> PyResult<Bound<'py, PyTuple>> {
    PyTuple::new(py, (0..n).map(|i| i * i))
}

// -------------------------------------------------------------------- dicts

/// `dict[str, int]` → `HashMap<String, i64>`; returns a word frequency dict.
#[pyfunction]
pub fn word_count(text: &str) -> HashMap<String, usize> {
    let mut map = HashMap::new();
    for w in text.split_whitespace() {
        *map.entry(w.to_lowercase()).or_insert(0) += 1;
    }
    map
}

/// `BTreeMap` gives a deterministic (sorted) key order in the resulting dict.
#[pyfunction]
pub fn invert_dict(d: HashMap<String, i64>) -> BTreeMap<i64, Vec<String>> {
    let mut out: BTreeMap<i64, Vec<String>> = BTreeMap::new();
    for (k, v) in d {
        out.entry(v).or_default().push(k);
    }
    out.values_mut().for_each(|v| v.sort());
    out
}

/// Builds a heterogeneous dict (values of different Python types) with `PyDict`.
#[pyfunction]
pub fn system_info(py: Python<'_>) -> PyResult<Bound<'_, PyDict>> {
    let d = PyDict::new(py);
    d.set_item("language", "Rust")?;
    d.set_item("pointer_width", usize::BITS)?;
    d.set_item("debug_build", cfg!(debug_assertions))?;
    d.set_item("os", std::env::consts::OS)?;
    d.set_item("features", vec!["pyclass", "pyfunction", "pymodule"])?;
    d.set_item("nothing", py.None())?;
    Ok(d)
}

/// Reads an arbitrary dict without converting it first, summing all numeric values.
#[pyfunction]
pub fn sum_numeric_values(d: &Bound<'_, PyDict>) -> f64 {
    d.iter()
        .filter_map(|(_k, v)| v.extract::<f64>().ok())
        .sum()
}

// --------------------------------------------------------------------- sets

/// Set operations: returns `(intersection, union, difference)`.
#[pyfunction]
pub fn set_ops(a: HashSet<i64>, b: HashSet<i64>) -> (HashSet<i64>, HashSet<i64>, HashSet<i64>) {
    (
        a.intersection(&b).copied().collect(),
        a.union(&b).copied().collect(),
        a.difference(&b).copied().collect(),
    )
}

/// Characters occurring in a string, as a `set[str]`.
#[pyfunction]
pub fn unique_chars(s: &str) -> HashSet<char> {
    s.chars().collect()
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(stats, m)?)?;
    m.add_function(wrap_pyfunction!(sorted_unique, m)?)?;
    m.add_function(wrap_pyfunction!(cross_product, m)?)?;
    m.add_function(wrap_pyfunction!(transpose, m)?)?;
    m.add_function(wrap_pyfunction!(double_in_place, m)?)?;
    m.add_function(wrap_pyfunction!(describe_person, m)?)?;
    m.add_function(wrap_pyfunction!(make_tuple, m)?)?;
    m.add_function(wrap_pyfunction!(word_count, m)?)?;
    m.add_function(wrap_pyfunction!(invert_dict, m)?)?;
    m.add_function(wrap_pyfunction!(system_info, m)?)?;
    m.add_function(wrap_pyfunction!(sum_numeric_values, m)?)?;
    m.add_function(wrap_pyfunction!(set_ops, m)?)?;
    m.add_function(wrap_pyfunction!(unique_chars, m)?)?;
    Ok(())
}
