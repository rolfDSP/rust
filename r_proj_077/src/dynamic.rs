//! Dynamic Python features from Rust.
//!
//! * Default and keyword-only arguments, `*args`, `**kwargs` via `#[pyo3(signature)]`
//! * Accepting "any" Python object (`Bound<PyAny>`) and inspecting its type
//! * Accepting one of several types with `#[derive(FromPyObject)]`
//! * Returning one of several types with `#[derive(IntoPyObject)]`
//! * Calling Python callbacks from Rust
//! * Calling into the Python standard library (`import math`)
//! * Releasing the GIL for CPU-heavy pure Rust work (`py.detach`)

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyFloat, PyInt, PyList, PyString, PyTuple};

// ---------------------------------------------------- flexible signatures

/// `greet(name, /, greeting="Hello", *, punctuation="!", times=1)`
///
/// * `name` is positional-only (`/`)
/// * `greeting` has a default
/// * `punctuation` and `times` are keyword-only (`*`)
#[pyfunction]
#[pyo3(signature = (name, /, greeting="Hello", *, punctuation="!", times=1))]
pub fn greet(name: &str, greeting: &str, punctuation: &str, times: usize) -> String {
    vec![format!("{greeting}, {name}{punctuation}"); times].join(" ")
}

/// `summarize(*args, **kwargs)` – describes everything that was passed in.
#[pyfunction]
#[pyo3(signature = (*args, **kwargs))]
pub fn summarize(args: &Bound<'_, PyTuple>, kwargs: Option<&Bound<'_, PyDict>>) -> PyResult<String> {
    let mut parts = vec![format!("{} positional", args.len())];
    for a in args.iter() {
        parts.push(format!("  {} ({})", a.repr()?, a.get_type().name()?));
    }
    if let Some(kw) = kwargs {
        parts.push(format!("{} keyword", kw.len()));
        for (k, v) in kw.iter() {
            parts.push(format!("  {k}={} ({})", v.repr()?, v.get_type().name()?));
        }
    }
    Ok(parts.join("\n"))
}

// ------------------------------------------------------------ duck typing

/// Takes *any* Python object and reports what it is, by checking types at runtime.
#[pyfunction]
pub fn type_of(obj: &Bound<'_, PyAny>) -> PyResult<String> {
    // Note: bool is a subclass of int in Python, so check it first.
    let kind = if obj.is_none() {
        "none".to_string()
    } else if obj.is_instance_of::<pyo3::types::PyBool>() {
        format!("bool: {}", obj.extract::<bool>()?)
    } else if obj.is_instance_of::<PyInt>() {
        format!("int: {}", obj.extract::<i128>().map(|v| v.to_string()).unwrap_or("<huge>".into()))
    } else if obj.is_instance_of::<PyFloat>() {
        format!("float: {}", obj.extract::<f64>()?)
    } else if let Ok(s) = obj.cast::<PyString>() {
        format!("str of length {}", s.to_str()?.chars().count())
    } else if let Ok(l) = obj.cast::<PyList>() {
        format!("list with {} items", l.len())
    } else if let Ok(d) = obj.cast::<PyDict>() {
        format!("dict with {} keys", d.len())
    } else if obj.is_callable() {
        "callable".to_string()
    } else if obj.hasattr("__len__")? {
        format!("sized {} (len {})", obj.get_type().name()?, obj.len()?)
    } else {
        format!("other: {}", obj.get_type().name()?)
    };
    Ok(kind)
}

/// Accepts one of several Python types. PyO3 tries the variants in order.
#[derive(FromPyObject, Debug)]
pub enum Number {
    Int(i64),
    Float(f64),
    /// A numeric string like "3.5"
    Text(String),
    /// A `(numerator, denominator)` tuple
    Fraction(i64, i64),
}

/// A struct extracted from any object having `.name` and `.value` attributes
/// (e.g. a dataclass, namedtuple or SimpleNamespace)...
#[derive(FromPyObject, Debug)]
pub struct NamedValue {
    pub name: String,
    pub value: f64,
}

/// ...or from a dict with the keys "name" and "value".
#[derive(FromPyObject, Debug)]
pub struct DictNamedValue {
    #[pyo3(item)]
    pub name: String,
    #[pyo3(item)]
    pub value: f64,
}

/// Returns one of several types depending on the input.
#[derive(IntoPyObject)]
pub enum Parsed {
    Int(i64),
    Float(f64),
    Bool(bool),
    Text(String),
    List(Vec<Parsed>),
    Nothing(Option<()>),
}

/// `to_float(5)`, `to_float(2.5)`, `to_float("7.25")`, `to_float((1, 4))`
#[pyfunction]
pub fn to_float(n: Number) -> PyResult<f64> {
    Ok(match n {
        Number::Int(i) => i as f64,
        Number::Float(f) => f,
        Number::Text(s) => s
            .trim()
            .parse()
            .map_err(|_| pyo3::exceptions::PyValueError::new_err(format!("not a number: {s:?}")))?,
        Number::Fraction(_, 0) => {
            return Err(pyo3::exceptions::PyZeroDivisionError::new_err("denominator is 0"));
        }
        Number::Fraction(a, b) => a as f64 / b as f64,
    })
}

/// Reads `.name` / `.value` attributes from arbitrary objects.
#[pyfunction]
pub fn format_named(items: Vec<NamedValue>) -> Vec<String> {
    items.iter().map(|i| format!("{} = {:.2}", i.name, i.value)).collect()
}

/// Same, but from dicts.
#[pyfunction]
pub fn format_dicts(items: Vec<DictNamedValue>) -> Vec<String> {
    items.iter().map(|i| format!("{} = {:.2}", i.name, i.value)).collect()
}

/// Very small literal parser returning a value of a type chosen at runtime.
#[pyfunction]
pub fn parse_literal(s: &str) -> Parsed {
    let s = s.trim();
    if s.is_empty() || s == "null" {
        Parsed::Nothing(None)
    } else if let Ok(i) = s.parse::<i64>() {
        Parsed::Int(i)
    } else if let Ok(f) = s.parse::<f64>() {
        Parsed::Float(f)
    } else if let Ok(b) = s.parse::<bool>() {
        Parsed::Bool(b)
    } else if let Some(inner) = s.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
        Parsed::List(inner.split(',').map(parse_literal).collect())
    } else {
        Parsed::Text(s.to_string())
    }
}

// --------------------------------------------------------------- callbacks

/// Applies a Python function to each element: `apply(lambda x: x * 2, [1, 2, 3])`.
#[pyfunction]
pub fn apply<'py>(func: &Bound<'py, PyAny>, items: Vec<Bound<'py, PyAny>>) -> PyResult<Vec<Bound<'py, PyAny>>> {
    items.into_iter().map(|x| func.call1((x,))).collect()
}

/// Calls `callback(i, total)` for progress reporting, with keyword args too.
#[pyfunction]
#[pyo3(signature = (n, callback))]
pub fn run_with_progress(n: u32, callback: &Bound<'_, PyAny>) -> PyResult<u64> {
    let mut sum = 0u64;
    for i in 1..=n {
        sum += (i as u64).pow(2);
        let kwargs = PyDict::new(callback.py());
        kwargs.set_item("total", n)?;
        // callback(i, total=n); returning False aborts the loop
        let keep_going = callback.call((i,), Some(&kwargs))?;
        if !keep_going.is_none() && !keep_going.is_truthy()? {
            break;
        }
    }
    Ok(sum)
}

/// Uses the Python standard library from Rust: `math.gcd` and `json.dumps`.
#[pyfunction]
pub fn use_stdlib(py: Python<'_>, a: i64, b: i64) -> PyResult<String> {
    let math = py.import("math")?;
    let gcd: i64 = math.getattr("gcd")?.call1((a, b))?.extract()?;

    let data = PyDict::new(py);
    data.set_item("a", a)?;
    data.set_item("b", b)?;
    data.set_item("gcd", gcd)?;
    let json = py.import("json")?;
    let kwargs = PyDict::new(py);
    kwargs.set_item("sort_keys", true)?;
    json.call_method("dumps", (data,), Some(&kwargs))?.extract()
}

// --------------------------------------------------------------------- GIL

/// Counts primes below `limit`. The GIL is released during the computation,
/// so other Python threads can run in parallel.
#[pyfunction]
pub fn count_primes(py: Python<'_>, limit: usize) -> usize {
    py.detach(|| {
        if limit < 2 {
            return 0;
        }
        let mut sieve = vec![true; limit];
        sieve[0] = false;
        sieve[1] = false;
        let mut i = 2;
        while i * i < limit {
            if sieve[i] {
                (i * i..limit).step_by(i).for_each(|j| sieve[j] = false);
            }
            i += 1;
        }
        sieve.iter().filter(|&&p| p).count()
    })
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(greet, m)?)?;
    m.add_function(wrap_pyfunction!(summarize, m)?)?;
    m.add_function(wrap_pyfunction!(type_of, m)?)?;
    m.add_function(wrap_pyfunction!(to_float, m)?)?;
    m.add_function(wrap_pyfunction!(format_named, m)?)?;
    m.add_function(wrap_pyfunction!(format_dicts, m)?)?;
    m.add_function(wrap_pyfunction!(parse_literal, m)?)?;
    m.add_function(wrap_pyfunction!(apply, m)?)?;
    m.add_function(wrap_pyfunction!(run_with_progress, m)?)?;
    m.add_function(wrap_pyfunction!(use_stdlib, m)?)?;
    m.add_function(wrap_pyfunction!(count_primes, m)?)?;
    Ok(())
}
