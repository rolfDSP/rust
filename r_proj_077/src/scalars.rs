//! Scalar (single value) conversions between Python and Rust.
//!
//! PyO3 converts automatically at the function boundary:
//!
//! | Python            | Rust                                   |
//! |-------------------|----------------------------------------|
//! | `int`             | `i8..i128`, `u8..u128`, `isize`, `usize` (range checked → `OverflowError`) |
//! | `float`           | `f32`, `f64`                           |
//! | `bool`            | `bool`                                 |
//! | `str`             | `String`, `&str`, `char` (1-char str)  |
//! | `bytes`           | `&[u8]`, `Vec<u8>` (in) / `Cow<[u8]>`, `PyBytes` (out) |
//! | `complex`         | `num_complex::Complex<f64>` (feature `num-complex`) |
//! | `None`            | `Option<T>::None`, `()`                |

use num_complex::Complex64;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use std::borrow::Cow;

// ---------------------------------------------------------------- integers

/// Add two signed 64-bit integers. Overflow raises `OverflowError`.
#[pyfunction]
pub fn add_i64(a: i64, b: i64) -> PyResult<i64> {
    a.checked_add(b)
        .ok_or_else(|| pyo3::exceptions::PyOverflowError::new_err("i64 overflow"))
}

/// An `u8` argument: Python raises `OverflowError` if the value is not in 0..=255.
#[pyfunction]
pub fn double_u8(x: u8) -> u16 {
    x as u16 * 2
}

/// 128-bit integers are supported too – Python ints have arbitrary precision.
#[pyfunction]
pub fn factorial_u128(n: u32) -> PyResult<u128> {
    (1..=n as u128).try_fold(1u128, |acc, x| acc.checked_mul(x)).ok_or_else(|| {
        pyo3::exceptions::PyOverflowError::new_err(format!("{n}! does not fit into u128"))
    })
}

// ------------------------------------------------------------------ floats

/// `f64` in and out. Python `int` arguments are accepted and converted.
#[pyfunction]
pub fn hypot(x: f64, y: f64) -> f64 {
    x.hypot(y)
}

/// `f32` loses precision – useful to show the conversion.
#[pyfunction]
pub fn to_f32(x: f64) -> f32 {
    x as f32
}

// ------------------------------------------------------------------- bool

/// Note: PyO3 only accepts a real `bool` here (not `0`/`1`).
#[pyfunction]
pub fn logical_xor(a: bool, b: bool) -> bool {
    a ^ b
}

// ---------------------------------------------------------------- strings

/// Borrowing `&str` avoids a copy on input; returning `String` creates a new `str`.
#[pyfunction]
pub fn reverse_string(s: &str) -> String {
    s.chars().rev().collect()
}

/// Unicode aware: counts characters (code points), bytes and words.
#[pyfunction]
pub fn string_stats(s: &str) -> (usize, usize, usize) {
    (s.chars().count(), s.len(), s.split_whitespace().count())
}

/// `char` maps to a Python `str` of length 1.
#[pyfunction]
pub fn next_char(c: char) -> Option<char> {
    char::from_u32(c as u32 + 1)
}

// ------------------------------------------------------------------ bytes

/// Takes `bytes` as a borrowed slice (zero-copy) and returns new `bytes`.
///
/// Returning `Cow<[u8]>` produces Python `bytes` (a `Vec<u8>` would become a `list[int]`!).
#[pyfunction]
pub fn xor_bytes<'a>(data: &[u8], key: u8) -> Cow<'a, [u8]> {
    Cow::Owned(data.iter().map(|b| b ^ key).collect())
}

/// Builds `bytes` directly in Python memory using `PyBytes::new_with` (no extra copy).
#[pyfunction]
pub fn byte_range(py: Python<'_>, n: usize) -> PyResult<Bound<'_, PyBytes>> {
    PyBytes::new_with(py, n, |buf| {
        for (i, b) in buf.iter_mut().enumerate() {
            *b = (i % 256) as u8;
        }
        Ok(())
    })
}

/// Simple checksum over any bytes-like object (`bytes`, `bytearray`, ...) given as `Vec<u8>`.
#[pyfunction]
pub fn checksum(data: Vec<u8>) -> u32 {
    data.iter().map(|&b| b as u32).sum()
}

// ---------------------------------------------------------------- complex

/// Python `complex` ↔ `num_complex::Complex64`.
#[pyfunction]
pub fn complex_mul(a: Complex64, b: Complex64) -> Complex64 {
    a * b
}

/// Returns magnitude and phase of a complex number as a tuple.
#[pyfunction]
pub fn complex_polar(z: Complex64) -> (f64, f64) {
    z.to_polar()
}

// ----------------------------------------------------------- Option / None

/// `Option<T>` parameter: Python may pass `None`. Returns `None` for an empty input.
#[pyfunction]
#[pyo3(signature = (value=None))]
pub fn maybe_square(value: Option<i64>) -> Option<i64> {
    value.map(|v| v * v)
}

/// Returning `()` becomes Python `None`.
#[pyfunction]
pub fn do_nothing() {}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(add_i64, m)?)?;
    m.add_function(wrap_pyfunction!(double_u8, m)?)?;
    m.add_function(wrap_pyfunction!(factorial_u128, m)?)?;
    m.add_function(wrap_pyfunction!(hypot, m)?)?;
    m.add_function(wrap_pyfunction!(to_f32, m)?)?;
    m.add_function(wrap_pyfunction!(logical_xor, m)?)?;
    m.add_function(wrap_pyfunction!(reverse_string, m)?)?;
    m.add_function(wrap_pyfunction!(string_stats, m)?)?;
    m.add_function(wrap_pyfunction!(next_char, m)?)?;
    m.add_function(wrap_pyfunction!(xor_bytes, m)?)?;
    m.add_function(wrap_pyfunction!(byte_range, m)?)?;
    m.add_function(wrap_pyfunction!(checksum, m)?)?;
    m.add_function(wrap_pyfunction!(complex_mul, m)?)?;
    m.add_function(wrap_pyfunction!(complex_polar, m)?)?;
    m.add_function(wrap_pyfunction!(maybe_square, m)?)?;
    m.add_function(wrap_pyfunction!(do_nothing, m)?)?;
    Ok(())
}
