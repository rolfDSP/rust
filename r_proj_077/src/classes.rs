//! Rust structs exposed as Python classes with `#[pyclass]`.
//!
//! * [`Point`]     – value type: constructor, properties, operators, `__repr__`, hashing,
//!                   `@staticmethod`, `@classmethod`, pickling support.
//! * [`Inventory`] – container type: `__len__`, `__getitem__`, `__setitem__`,
//!                   `__delitem__`, `__contains__`, `__iter__` (with a separate iterator class).
//! * [`Counter`]   – mutable state shared between Python references.
//! * [`Animal`] / [`Dog`] – inheritance (`extends = ...`), subclassable from Python.

use pyo3::exceptions::{PyIndexError, PyKeyError, PyZeroDivisionError};
use pyo3::prelude::*;
use pyo3::types::PyType;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

// ====================================================================== Point

/// A 2D point. `#[pyclass(get_all, set_all)]` would expose all fields at once;
/// here `#[pyo3(get, set)]` is used per field for illustration.
#[pyclass(module = "rustypy", eq, from_py_object)]
#[derive(Clone, Debug, PartialEq)]
pub struct Point {
    #[pyo3(get, set)]
    pub x: f64,
    #[pyo3(get, set)]
    pub y: f64,
}

#[pymethods]
impl Point {
    /// `Point(x=0.0, y=0.0)`
    #[new]
    #[pyo3(signature = (x=0.0, y=0.0))]
    fn new(x: f64, y: f64) -> Self {
        Point { x, y }
    }

    /// Computed read-only property.
    #[getter]
    fn length(&self) -> f64 {
        self.x.hypot(self.y)
    }

    /// Regular method taking another `Point` by reference.
    fn distance_to(&self, other: &Point) -> f64 {
        (self.x - other.x).hypot(self.y - other.y)
    }

    /// Mutating method (`&mut self`).
    fn translate(&mut self, dx: f64, dy: f64) {
        self.x += dx;
        self.y += dy;
    }

    /// `@staticmethod`
    #[staticmethod]
    fn origin() -> Self {
        Point { x: 0.0, y: 0.0 }
    }

    /// `@classmethod` – `cls` is the (possibly Python-subclassed) type.
    #[classmethod]
    fn from_polar(_cls: &Bound<'_, PyType>, r: f64, phi: f64) -> Self {
        Point { x: r * phi.cos(), y: r * phi.sin() }
    }

    /// Returns a tuple, enabling `x, y = p.as_tuple()`.
    fn as_tuple(&self) -> (f64, f64) {
        (self.x, self.y)
    }

    // ---- dunder / magic methods ----

    fn __repr__(&self) -> String {
        format!("Point(x={:?}, y={:?})", self.x, self.y)
    }

    fn __add__(&self, other: &Point) -> Point {
        Point { x: self.x + other.x, y: self.y + other.y }
    }

    fn __sub__(&self, other: &Point) -> Point {
        Point { x: self.x - other.x, y: self.y - other.y }
    }

    /// `p * 2.0`
    fn __mul__(&self, k: f64) -> Point {
        Point { x: self.x * k, y: self.y * k }
    }

    /// `2.0 * p`
    fn __rmul__(&self, k: f64) -> Point {
        self.__mul__(k)
    }

    fn __truediv__(&self, k: f64) -> PyResult<Point> {
        if k == 0.0 {
            return Err(PyZeroDivisionError::new_err("division of Point by zero"));
        }
        Ok(Point { x: self.x / k, y: self.y / k })
    }

    fn __neg__(&self) -> Point {
        Point { x: -self.x, y: -self.y }
    }

    fn __abs__(&self) -> f64 {
        self.length()
    }

    fn __bool__(&self) -> bool {
        self.x != 0.0 || self.y != 0.0
    }

    /// Makes `Point` usable in sets / as dict keys (`eq` above provides `__eq__`).
    fn __hash__(&self) -> u64 {
        let mut h = DefaultHasher::new();
        self.x.to_bits().hash(&mut h);
        self.y.to_bits().hash(&mut h);
        h.finish()
    }

    /// Pickle / copy support: `Point(*args)` recreates the object.
    fn __getnewargs__(&self) -> (f64, f64) {
        (self.x, self.y)
    }
}

// ================================================================== Inventory

/// A dictionary-like container mapping item names to quantities.
#[pyclass(module = "rustypy")]
#[derive(Default)]
pub struct Inventory {
    items: HashMap<String, u32>,
}

#[pymethods]
impl Inventory {
    /// `Inventory()` or `Inventory({"apple": 3})`
    #[new]
    #[pyo3(signature = (items=None))]
    fn new(items: Option<HashMap<String, u32>>) -> Self {
        Inventory { items: items.unwrap_or_default() }
    }

    /// Adds `qty` units; returns the new quantity.
    #[pyo3(signature = (name, qty=1))]
    fn add(&mut self, name: String, qty: u32) -> u32 {
        let e = self.items.entry(name).or_insert(0);
        *e += qty;
        *e
    }

    /// Removes up to `qty` units. Raises `KeyError` for unknown items.
    fn remove(&mut self, name: &str, qty: u32) -> PyResult<u32> {
        let e = self
            .items
            .get_mut(name)
            .ok_or_else(|| PyKeyError::new_err(name.to_string()))?;
        *e = e.saturating_sub(qty);
        Ok(*e)
    }

    #[getter]
    fn total(&self) -> u32 {
        self.items.values().sum()
    }

    /// Returns a *copy* of the content as a regular Python dict.
    fn to_dict(&self) -> HashMap<String, u32> {
        self.items.clone()
    }

    fn __len__(&self) -> usize {
        self.items.len()
    }

    fn __contains__(&self, name: &str) -> bool {
        self.items.contains_key(name)
    }

    fn __getitem__(&self, name: &str) -> PyResult<u32> {
        self.items
            .get(name)
            .copied()
            .ok_or_else(|| PyKeyError::new_err(name.to_string()))
    }

    fn __setitem__(&mut self, name: String, qty: u32) {
        self.items.insert(name, qty);
    }

    fn __delitem__(&mut self, name: &str) -> PyResult<()> {
        self.items
            .remove(name)
            .map(|_| ())
            .ok_or_else(|| PyKeyError::new_err(name.to_string()))
    }

    /// `for name in inventory:` – returns a separate iterator object (sorted names).
    fn __iter__(&self) -> InventoryIter {
        let mut names: Vec<String> = self.items.keys().cloned().collect();
        names.sort();
        InventoryIter { names: names.into_iter() }
    }

    fn __repr__(&self) -> String {
        let mut v: Vec<_> = self.items.iter().collect();
        v.sort();
        format!("Inventory({v:?})")
    }
}

/// Iterator returned by `iter(Inventory)`.
#[pyclass(module = "rustypy")]
pub struct InventoryIter {
    names: std::vec::IntoIter<String>,
}

#[pymethods]
impl InventoryIter {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// Returning `None` raises `StopIteration`.
    fn __next__(&mut self) -> Option<String> {
        self.names.next()
    }
}

// ==================================================================== Counter

/// Shows that a pyclass is a *reference* type in Python: two names pointing to the
/// same object see the same state. Also a generator-like `__next__` with an upper bound.
#[pyclass(module = "rustypy")]
pub struct Counter {
    #[pyo3(get)]
    value: i64,
    step: i64,
    limit: Option<i64>,
}

#[pymethods]
impl Counter {
    #[new]
    #[pyo3(signature = (start=0, step=1, limit=None))]
    fn new(start: i64, step: i64, limit: Option<i64>) -> Self {
        Counter { value: start, step, limit }
    }

    fn increment(&mut self) -> i64 {
        self.value += self.step;
        self.value
    }

    fn reset(&mut self) {
        self.value = 0;
    }

    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self) -> Option<i64> {
        let current = self.value;
        if self.limit.is_some_and(|l| current >= l) {
            return None;
        }
        self.value += self.step;
        Some(current)
    }
}

// ============================================================ Inheritance

/// Base class. `subclass` allows Python classes to inherit from it.
#[pyclass(module = "rustypy", subclass)]
pub struct Animal {
    #[pyo3(get)]
    name: String,
}

#[pymethods]
impl Animal {
    #[new]
    fn new(name: String) -> Self {
        Animal { name }
    }

    /// Can be overridden in a Python subclass.
    fn speak(&self) -> String {
        format!("{} makes a sound", self.name)
    }

    /// Calls `self.speak()` through Python, so Python overrides are honoured.
    fn introduce(slf: &Bound<'_, Self>) -> PyResult<String> {
        let sound: String = slf.call_method0("speak")?.extract()?;
        Ok(format!("Let me introduce myself: {sound}"))
    }
}

/// Rust subclass of `Animal`.
#[pyclass(module = "rustypy", extends = Animal)]
pub struct Dog {
    #[pyo3(get, set)]
    tricks: Vec<String>,
}

#[pymethods]
impl Dog {
    /// The constructor initialises the parent part first, then adds the child part.
    #[new]
    #[pyo3(signature = (name, tricks=Vec::new()))]
    fn new(name: String, tricks: Vec<String>) -> PyClassInitializer<Self> {
        PyClassInitializer::from(Animal::new(name)).add_subclass(Dog { tricks })
    }

    fn speak(self_: PyRef<'_, Self>) -> String {
        // Access the parent part through `as_super()` / `as_ref()`.
        let parent: &Animal = self_.as_super();
        format!("{} says Woof!", parent.name)
    }

    fn learn(&mut self, trick: String) {
        self.tricks.push(trick);
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Point>()?;
    m.add_class::<Inventory>()?;
    m.add_class::<InventoryIter>()?;
    m.add_class::<Counter>()?;
    m.add_class::<Animal>()?;
    m.add_class::<Dog>()?;
    m.add_function(wrap_pyfunction!(centroid, m)?)?;
    m.add_function(wrap_pyfunction!(nth_point, m)?)?;
    Ok(())
}

/// Functions can take and return pyclasses, including `Vec<Point>`.
#[pyfunction]
pub fn centroid(points: Vec<Point>) -> Option<Point> {
    if points.is_empty() {
        return None;
    }
    let n = points.len() as f64;
    let (sx, sy) = points.iter().fold((0.0, 0.0), |(a, b), p| (a + p.x, b + p.y));
    Some(Point { x: sx / n, y: sy / n })
}

/// Raises `IndexError` – the Python-native way to signal a bad index.
#[pyfunction]
pub fn nth_point(points: Vec<Point>, n: usize) -> PyResult<Point> {
    points
        .get(n)
        .cloned()
        .ok_or_else(|| PyIndexError::new_err(format!("index {n} out of range")))
}
