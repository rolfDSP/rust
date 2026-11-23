# rustypy – Documentation

A complete guide to the `r_proj_077` example: a native Python extension module
named **`rustypy`**, written in Rust with **PyO3 0.30** and built with **maturin**.

---

## Table of contents

1. [Overview](#1-overview)
2. [How it works](#2-how-it-works)
3. [Project layout](#3-project-layout)
4. [Setup, build and test](#4-setup-build-and-test)
5. [Configuration files explained](#5-configuration-files-explained)
6. [Module entry point (`lib.rs`)](#6-module-entry-point-librs)
7. [Scalar types (`scalars.rs`)](#7-scalar-types-scalarsrs)
8. [Collections (`collections.rs`)](#8-collections-collectionsrs)
9. [Classes (`classes.rs`)](#9-classes-classesrs)
10. [Enums (`enums.rs`)](#10-enums-enumsrs)
11. [Error handling (`errors.rs`)](#11-error-handling-errorsrs)
12. [Dynamic features (`dynamic.rs`)](#12-dynamic-features-dynamicrs)
13. [The demo script (`python/demo.py`)](#13-the-demo-script-pythondemopy)
14. [Type mapping reference](#14-type-mapping-reference)
15. [Python API reference](#15-python-api-reference)
16. [Pitfalls and tips](#16-pitfalls-and-tips)
17. [Extending the example](#17-extending-the-example)
18. [Troubleshooting](#18-troubleshooting)

---

## 1. Overview

Python is convenient but slow for CPU-heavy work; Rust is fast and memory-safe.
PyO3 lets you write functions and classes in Rust and call them from Python as if
they were ordinary Python code:

```python
import rustypy as rp

rp.factorial_u128(30)           # 265252859812191058636308480000000
p = rp.Point(3, 4) + rp.Point(1, 1)
rp.count_primes(50_000_000)     # runs without holding the GIL
```

The example is organised by topic so each file can be read on its own:

| Topic | Rust file | Key PyO3 features |
|---|---|---|
| Module & constants | `src/lib.rs` | `#[pymodule]`, `m.add` |
| Scalars | `src/scalars.rs` | `#[pyfunction]`, automatic conversion, `PyBytes` |
| Collections | `src/collections.rs` | `Vec`, `HashMap`, `HashSet`, `PyList`, `PyDict`, `PyTuple` |
| Classes | `src/classes.rs` | `#[pyclass]`, `#[pymethods]`, dunder methods, inheritance |
| Enums | `src/enums.rs` | simple and data-carrying `#[pyclass]` enums |
| Errors | `src/errors.rs` | `PyResult`, `create_exception!`, `From<E> for PyErr` |
| Dynamic | `src/dynamic.rs` | `signature`, `*args/**kwargs`, `FromPyObject`, `IntoPyObject`, callbacks, `py.detach` |

---

## 2. How it works

```
 ┌──────────────┐  cargo + maturin  ┌───────────────────────────────┐
 │ src/*.rs     │ ────────────────▶ │ rustypy.cpython-312-*.so      │
 │ (Rust code)  │                   │ (shared library, cdylib)      │
 └──────────────┘                   └──────────────┬────────────────┘
                                                   │ import rustypy
                                                   ▼
                                    ┌───────────────────────────────┐
                                    │ Python interpreter calls      │
                                    │ PyInit_rustypy() and gets a   │
                                    │ module object                 │
                                    └───────────────────────────────┘
```

1. The crate is compiled as a **`cdylib`** (C-compatible shared library).
2. `#[pymodule] fn rustypy(...)` makes PyO3 generate the C symbol
   `PyInit_rustypy`, which CPython looks for when executing `import rustypy`.
3. Each `#[pyfunction]` / `#[pyclass]` gets wrapper code that
   - converts Python arguments to Rust types (`FromPyObject`),
   - calls your Rust code,
   - converts the result back (`IntoPyObject`),
   - turns a returned `Err(PyErr)` into a raised Python exception.
4. **maturin** drives `cargo`, names the library correctly for the current
   Python version and platform, and installs it into the virtualenv (or packs it
   into a wheel).

### Key PyO3 concepts

| Concept | Meaning |
|---|---|
| `Python<'py>` | A token proving the current thread holds the GIL (Global Interpreter Lock). Needed to create Python objects. |
| `Bound<'py, T>` | A reference to a Python object of type `T`, valid while the GIL is held. The main way to work with Python objects. |
| `Py<T>` | An owned reference independent of the GIL lifetime; can be stored in structs and sent between threads. |
| `PyResult<T>` | Alias for `Result<T, PyErr>`. An `Err` becomes a Python exception. |
| `PyRef<T>` / `PyRefMut<T>` | Borrowed (immutable / mutable) access to a `#[pyclass]` instance; borrow rules are checked at runtime. |
| `py.detach(\|\| ...)` | Releases the GIL while the closure runs (called `allow_threads` in older PyO3 versions). |

---

## 3. Project layout

```
r_proj_077/
├── Cargo.toml          Rust package; lib name "rustypy", crate-type cdylib + rlib
├── pyproject.toml      Python packaging metadata, build backend = maturin
├── README.md           Short introduction and cheat sheet
├── doc.md              This document
├── .gitignore
├── src/
│   ├── lib.rs          #[pymodule] – registers everything
│   ├── scalars.rs      int, float, bool, str, char, bytes, complex, Option
│   ├── collections.rs  list, tuple, dict, set, nested containers
│   ├── classes.rs      Point, Inventory, InventoryIter, Counter, Animal, Dog
│   ├── enums.rs        Color, Shape
│   ├── errors.rs       RustyError, ParseError, LimitError, ConfigError
│   ├── dynamic.rs      signatures, duck typing, callbacks, GIL release
│   └── main.rs         leftover "Hello, world!" binary (not used by Python)
├── python/
│   └── demo.py         Uses and checks every exported item
└── .venv/              Virtualenv with maturin (not in git)
```

---

## 4. Setup, build and test

### Prerequisites

- Rust toolchain (tested with cargo 1.99, edition 2024)
- Python ≥ 3.9 (tested with 3.12)
- maturin ≥ 1.5 (tested with 1.16)

### First time setup

```bash
cd ~/rust/r_proj_077
python3 -m venv .venv
source .venv/bin/activate
pip install maturin
```

### Build and install for development

```bash
source .venv/bin/activate
unset CONDA_PREFIX            # required if Anaconda's base env is also active
maturin develop --release
```

`maturin develop` compiles the crate and installs it into the active virtualenv
(editable install). Re-run it after every Rust change. Omit `--release` for faster
compilation but much slower code.

### Run the demo

```bash
python python/demo.py
```

The script prints each example and uses `assert` / `expect_error` to verify
results. It ends with `All checks passed ✔`.

### Build a distributable wheel

```bash
maturin build --release
pip install target/wheels/rustypy-0.1.0-cp312-cp312-linux_x86_64.whl
```

A wheel is specific to the Python version and platform. For Windows, build on
Windows (or cross-compile with `maturin build --target x86_64-pc-windows-msvc`
plus a suitable toolchain).

### Rust unit tests

```bash
PYO3_PYTHON=$PWD/.venv/bin/python cargo test --lib
```

Pure Rust logic (e.g. `parse_config_impl` in `errors.rs`) can be tested without
Python. `PYO3_PYTHON` tells PyO3 which interpreter to link against.

### Interactive use

```bash
python -i -c "import rustypy as rp"
>>> help(rp.Point)          # Rust doc comments become Python docstrings
>>> rp.greet.__text_signature__
```

---

## 5. Configuration files explained

### `Cargo.toml`

```toml
[lib]
name = "rustypy"                 # must equal the #[pymodule] function name
crate-type = ["cdylib", "rlib"]  # cdylib for Python, rlib so `cargo test` works

[dependencies]
pyo3 = { version = "0.30", features = ["num-complex"] }  # complex ↔ Complex64
num-complex = "0.4"

[features]
extension-module = ["pyo3/extension-module"]
```

The `extension-module` feature tells PyO3 **not** to link against `libpython`
(the interpreter that imports the module already provides those symbols). It is
enabled only by maturin (see below); otherwise `cargo test` would fail to link.

### `pyproject.toml`

```toml
[build-system]
requires = ["maturin>=1.5,<2.0"]
build-backend = "maturin"        # makes `pip install .` work too

[project]
name = "rustypy"

[tool.maturin]
features = ["extension-module"]
```

---

## 6. Module entry point (`lib.rs`)

```rust
#[pymodule]
fn rustypy(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add("PI", std::f64::consts::PI)?;
    m.add("MAX_U32", u32::MAX)?;
    m.add("GREETING", "Hello from Rust")?;

    scalars::register(m)?;
    collections::register(m)?;
    ...
}
```

- `m.add(name, value)` creates module-level attributes of any convertible type.
- Each sub-module has a `register(m)` function that calls
  `m.add_function(wrap_pyfunction!(f, m)?)` and `m.add_class::<T>()`.
  This keeps `lib.rs` short and groups registration with the code it belongs to.
- All items appear flat in Python: `rustypy.Point`, `rustypy.greet`, …

```python
>>> rp.__version__, rp.PI, rp.MAX_U32, rp.GREETING
('0.1.0', 3.141592653589793, 4294967295, 'Hello from Rust')
```

---

## 7. Scalar types (`scalars.rs`)

### Integers

```rust
#[pyfunction]
pub fn add_i64(a: i64, b: i64) -> PyResult<i64> {
    a.checked_add(b).ok_or_else(|| PyOverflowError::new_err("i64 overflow"))
}

#[pyfunction]
pub fn double_u8(x: u8) -> u16 { x as u16 * 2 }

#[pyfunction]
pub fn factorial_u128(n: u32) -> PyResult<u128> { ... }
```

- Python `int` has unlimited precision; Rust integers have fixed width.
  PyO3 checks the range **on input**: `double_u8(256)` raises
  `OverflowError: number too large to fit in target type`.
- Arithmetic overflow **inside** Rust is your responsibility – use `checked_*`
  and return a `PyOverflowError` (as in `add_i64`), otherwise a release build
  silently wraps around.
- `u128`/`i128` are supported: `factorial_u128(30)` equals `math.factorial(30)`.
- There is no implicit conversion from `str`: `add_i64("1", 2)` → `TypeError`.

### Floats

```rust
pub fn hypot(x: f64, y: f64) -> f64
pub fn to_f32(x: f64) -> f32
```

- Python `float` is a C `double` → `f64` is lossless.
- `int` arguments are accepted for `f64` parameters (`hypot(3, 4)` → `5.0`).
- Returning `f32` shows precision loss: `to_f32(0.1)` → `0.10000000149011612`.

### Booleans

```rust
pub fn logical_xor(a: bool, b: bool) -> bool
```

Only real `True`/`False` are accepted; `logical_xor(1, 0)` raises `TypeError`.

### Strings and characters

```rust
pub fn reverse_string(s: &str) -> String
pub fn string_stats(s: &str) -> (usize, usize, usize)   // (chars, UTF-8 bytes, words)
pub fn next_char(c: char) -> Option<char>
```

- `&str` borrows the Python string's UTF-8 data (no copy on input in most cases);
  `String` returns a new Python `str`.
- Rust strings are UTF-8, Python strings are sequences of code points.
  `string_stats("Grüße aus Rust 🦀")` → `(16, 21, 4)`: 16 characters, 21 bytes.
- `char` accepts exactly one character; `next_char("ab")` → `ValueError`.

### Bytes

```rust
pub fn xor_bytes<'a>(data: &[u8], key: u8) -> Cow<'a, [u8]>
pub fn byte_range(py: Python<'_>, n: usize) -> PyResult<Bound<'_, PyBytes>>
pub fn checksum(data: Vec<u8>) -> u32
```

| Approach | Notes |
|---|---|
| `&[u8]` parameter | Zero-copy view of a `bytes` object (not `bytearray`). |
| `Vec<u8>` parameter | Copies; accepts `bytes`, `bytearray` and lists of ints. |
| `Cow<[u8]>` return | Produces a Python **`bytes`** object. |
| `PyBytes::new_with(py, n, \|buf\| ...)` | Allocates the `bytes` object first and fills it in place – no intermediate `Vec`. |
| `Vec<u8>` return | ⚠ Produces a **`list[int]`**, not `bytes`. |

### Complex numbers

```rust
pub fn complex_mul(a: Complex64, b: Complex64) -> Complex64
pub fn complex_polar(z: Complex64) -> (f64, f64)
```

Requires the PyO3 feature `num-complex`. `complex_mul(1+2j, 3-1j)` → `(5+5j)`.

### `Option` and `None`

```rust
#[pyfunction]
#[pyo3(signature = (value=None))]
pub fn maybe_square(value: Option<i64>) -> Option<i64>

#[pyfunction]
pub fn do_nothing() {}
```

- `Option<T>` parameters accept `None`; `Option<T>` returns give `None` for `None`.
- `#[pyo3(signature = (value=None))]` makes the argument optional
  (`maybe_square()` works).
- A function returning `()` returns `None` in Python.

---

## 8. Collections (`collections.rs`)

There are two ways to receive a container:

| Style | Example | Behaviour |
|---|---|---|
| **Owned Rust type** | `Vec<f64>`, `HashMap<String, i64>` | The whole container is converted (copied). Simple and type-checked, but changes are not visible to the caller. |
| **Python object** | `&Bound<'_, PyList>`, `&Bound<'_, PyDict>` | Works directly on the caller's object. No copy; mutations are visible; elements are untyped (`Bound<PyAny>`). |

### Lists

```rust
pub fn stats(values: Vec<f64>) -> Option<(f64, f64, f64)>   // (min, max, mean)
pub fn sorted_unique(mut values: Vec<i64>) -> Vec<i64>
pub fn cross_product(a: [f64; 3], b: [f64; 3]) -> [f64; 3]
pub fn transpose(matrix: Vec<Vec<f64>>) -> PyResult<Vec<Vec<f64>>>
pub fn double_in_place(list: &Bound<'_, PyList>) -> PyResult<()>
```

- `Vec<T>` accepts any sequence (list, tuple, …) but **not** `str`
  (PyO3 refuses to treat a string as a sequence of characters).
- Fixed arrays `[T; N]` check the length:
  `cross_product([1, 2], [3, 4, 5])` → `ValueError: expected a sequence of length 3 (got 2)`.
- Nested containers (`Vec<Vec<f64>>`) convert recursively.
- `double_in_place` uses `list.get_item(i)` / `list.set_item(i, item.mul(2)?)` –
  Python's own `*` operator, so it works for numbers, strings and lists alike:

```python
lst = [1, 2.5, "ab", [0]]
rp.double_in_place(lst)
lst   # [2, 5.0, 'abab', [0, 0]]
```

### Tuples

```rust
pub fn describe_person(person: (String, u8, bool)) -> String
pub fn make_tuple<'py>(py: Python<'py>, n: usize) -> PyResult<Bound<'py, PyTuple>>
```

- Rust tuples map to Python tuples with a fixed number of elements of
  (possibly) different types.
- `PyTuple::new(py, iterator)` builds a tuple of length known only at runtime.

### Dictionaries

```rust
pub fn word_count(text: &str) -> HashMap<String, usize>
pub fn invert_dict(d: HashMap<String, i64>) -> BTreeMap<i64, Vec<String>>
pub fn system_info(py: Python<'_>) -> PyResult<Bound<'_, PyDict>>
pub fn sum_numeric_values(d: &Bound<'_, PyDict>) -> f64
```

- `HashMap` gives an arbitrary key order; `BTreeMap` gives sorted keys in the
  resulting dict (deterministic output).
- `PyDict::new(py)` + `set_item` builds a dict with **heterogeneous** values
  (str, int, bool, list, None) – impossible with a single `HashMap<K, V>` type.
- `sum_numeric_values` iterates a dict without converting it and uses
  `v.extract::<f64>().ok()` to skip non-numeric values. Note that `True`
  counts as `1.0` because `bool` is a subclass of `int` in Python.

### Sets

```rust
pub fn set_ops(a: HashSet<i64>, b: HashSet<i64>)
    -> (HashSet<i64>, HashSet<i64>, HashSet<i64>)   // (intersection, union, difference)
pub fn unique_chars(s: &str) -> HashSet<char>
```

Both `set` and `frozenset` are accepted. Returning `HashSet` creates a `set`.

---

## 9. Classes (`classes.rs`)

A Rust struct becomes a Python class with `#[pyclass]`; its methods go into a
`#[pymethods]` impl block.

### Common attributes

| Attribute | Effect |
|---|---|
| `#[pyclass(module = "rustypy")]` | Sets `__module__` (needed for pickling and nice reprs). |
| `#[pyclass(eq)]` | Generates `__eq__`/`__ne__` from Rust `PartialEq`. |
| `#[pyclass(from_py_object)]` | Lets functions take the class **by value** (`Point`, `Vec<Point>`); requires `Clone`. |
| `#[pyclass(subclass)]` | Allows inheritance (from Rust or Python). |
| `#[pyclass(extends = Base)]` | Rust-level inheritance. |
| `#[pyo3(get, set)]` on a field | Generates a property with getter and setter. |
| `#[new]` | The constructor (`__new__`). |
| `#[getter]` / `#[setter]` | Computed properties. |
| `#[staticmethod]` / `#[classmethod]` | Same as in Python. |

### `Point` – a value type

```rust
#[pyclass(module = "rustypy", eq, from_py_object)]
#[derive(Clone, Debug, PartialEq)]
pub struct Point {
    #[pyo3(get, set)] pub x: f64,
    #[pyo3(get, set)] pub y: f64,
}
```

| Python | Rust method |
|---|---|
| `Point(x=0.0, y=0.0)` | `#[new] #[pyo3(signature = (x=0.0, y=0.0))] fn new` |
| `p.x`, `p.x = 5` | field attribute `#[pyo3(get, set)]` |
| `p.length` (read-only) | `#[getter] fn length` |
| `p.distance_to(q)` | `fn distance_to(&self, other: &Point)` |
| `p.translate(dx, dy)` | `fn translate(&mut self, ...)` |
| `Point.origin()` | `#[staticmethod]` |
| `Point.from_polar(r, phi)` | `#[classmethod]` (receives `cls`) |
| `x, y = p.as_tuple()` | returns `(f64, f64)` |
| `repr(p)` | `__repr__` → `Point(x=3.0, y=4.0)` |
| `p + q`, `p - q` | `__add__`, `__sub__` |
| `p * 2` / `2 * p` | `__mul__` / `__rmul__` |
| `p / 2` (raises `ZeroDivisionError` for 0) | `__truediv__` |
| `-p`, `abs(p)`, `bool(p)` | `__neg__`, `__abs__`, `__bool__` |
| `p == q` | `eq` attribute + `PartialEq` |
| `{p, q}`, dict keys | `__hash__` |
| `pickle.dumps(p)`, `copy.copy(p)` | `__getnewargs__` |

Functions can take and return the class like any other type:

```rust
#[pyfunction]
pub fn centroid(points: Vec<Point>) -> Option<Point>
#[pyfunction]
pub fn nth_point(points: Vec<Point>, n: usize) -> PyResult<Point>   // IndexError
```

> Taking `Point` by value or `Vec<Point>` **clones** the Rust data. To modify the
> caller's object use `PyRefMut<Point>` or `&mut self` methods instead.

### `Inventory` – the container protocol

```python
inv = rp.Inventory({"apple": 3})
inv.add("banana", qty=5)     # add(name, qty=1) -> new quantity
inv["cherry"] = 12           # __setitem__
inv["banana"]                # __getitem__ (KeyError if missing)
"apple" in inv               # __contains__
len(inv)                     # __len__
del inv["cherry"]            # __delitem__
for name in inv: ...         # __iter__ -> InventoryIter (sorted names)
inv.total                    # getter: sum of quantities
inv.to_dict()                # copy as a Python dict
inv.remove("banana", 2)      # saturating subtraction, KeyError if missing
```

Iteration uses a **separate iterator class** `InventoryIter` holding a
`std::vec::IntoIter<String>`. Its `__next__` returns `Option<String>`;
returning `None` makes PyO3 raise `StopIteration`.

### `Counter` – reference semantics and self-iteration

```python
c = rp.Counter(start=10, step=5)   # Counter(start=0, step=1, limit=None)
alias = c
alias.increment()
c.value                            # 15 – both names refer to the same object
list(rp.Counter(0, 3, limit=15))   # [0, 3, 6, 9, 12]
c.value = 1                        # AttributeError – only #[pyo3(get)]
```

A pyclass instance lives on the Python heap; Python variables hold references to
it, just like normal Python objects. `__iter__` returns `self` (`PyRef<Self>`),
so the object is its own iterator.

### `Animal` / `Dog` – inheritance

```rust
#[pyclass(module = "rustypy", subclass)]
pub struct Animal { #[pyo3(get)] name: String }

#[pyclass(module = "rustypy", extends = Animal)]
pub struct Dog { #[pyo3(get, set)] tricks: Vec<String> }

#[new]
fn new(name: String, tricks: Vec<String>) -> PyClassInitializer<Self> {
    PyClassInitializer::from(Animal::new(name)).add_subclass(Dog { tricks })
}
```

- A subclass constructor initialises the parent part first through
  `PyClassInitializer`, then adds the child part.
- Inside a child method, `self_.as_super()` gives access to the parent struct.
- `Animal::introduce` calls `slf.call_method0("speak")` – a **Python-level** call –
  so overrides defined in Python subclasses are honoured:

```python
class Cat(rp.Animal):
    def speak(self):
        return f"{self.name} says Meow"

Cat("Tom").introduce()   # 'Let me introduce myself: Tom says Meow'
rp.Dog("Rex").speak()    # 'Rex says Woof!'
isinstance(rp.Dog("Rex"), rp.Animal)   # True
```

---

## 10. Enums (`enums.rs`)

### Simple enum: `Color`

```rust
#[pyclass(module = "rustypy", eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Color { Red = 1, Green = 2, Blue = 3 }
```

- Variants are class attributes: `rp.Color.Red`.
- `eq` enables `==`; `eq_int` also allows comparison with integers and `int(Color.Blue) == 3`.
- Methods work like on structs: `Color.Green.hex()` → `'#00ff00'`,
  `Color.parse("BLUE")` (static method, `ValueError` for unknown names).
- Enums are normal argument types: `mix(Color.Red, Color.Blue)` → `'Red-Blue mix'`.

### Data-carrying enum: `Shape`

```rust
#[pyclass(module = "rustypy", from_py_object)]
#[derive(Clone, Debug)]
pub enum Shape {
    Circle { radius: f64 },
    Rectangle { width: f64, height: f64 },
    Triangle { a: f64, b: f64, c: f64 },
    Polygon(Vec<(f64, f64)>),
}
```

- Every variant becomes a **subclass** of `Shape`: `Shape.Circle(radius=1.0)`.
- Struct variant fields are attributes (`s.radius`); tuple variant fields are
  accessed by index (`polygon[0]`).
- Works with `isinstance` and structural pattern matching:

```python
match shape:
    case rp.Shape.Circle(radius=r):            ...
    case rp.Shape.Rectangle(width=w, height=h): ...
    case rp.Shape.Polygon():                   ...
```

- `area()` uses a Rust `match` over all variants (Heron's formula for
  triangles, the shoelace formula for polygons).
- `largest(shapes: Vec<Shape>) -> Option<Shape>` returns the biggest shape.

---

## 11. Error handling (`errors.rs`)

### Principle

A function returning `PyResult<T>` (= `Result<T, PyErr>`) raises a Python
exception when it returns `Err`. A Rust **panic** is also caught at the boundary
and raised as `pyo3_runtime.PanicException` – but panics should be reserved for
bugs, not expected errors.

### Built-in exceptions

```rust
use pyo3::exceptions::PyValueError;
Err(PyValueError::new_err(format!("cannot take sqrt of negative number {x}")))
```

Available: `PyValueError`, `PyTypeError`, `PyKeyError`, `PyIndexError`,
`PyOverflowError`, `PyZeroDivisionError`, `PyRuntimeError`, `PyIOError`, …

Many standard Rust errors convert automatically with `?`:
`ParseIntError`/`ParseFloatError` → `ValueError`, `std::io::Error` → `OSError`
(subclass chosen by error kind), etc.

```rust
pub fn parse_int(s: &str) -> PyResult<i64> {
    Ok(s.trim().parse::<i64>()?)       // "12x" -> ValueError
}
```

### Custom exception hierarchy

```rust
create_exception!(rustypy, RustyError, PyException, "Base class for all rustypy errors.");
create_exception!(rustypy, ParseError, RustyError,  "Input could not be parsed.");
create_exception!(rustypy, LimitError, RustyError,  "A value exceeded a limit.");

// in register():
m.add("RustyError", py.get_type::<RustyError>())?;
```

```python
try:
    rp.parse_config("name=a\nsize=5000")
except rp.RustyError as e:       # catches ParseError and LimitError
    print(type(e).__name__, e)   # LimitError value 5000 exceeds the limit of 1000
```

### Bridging a Rust error type

The recommended architecture: keep the core logic free of PyO3 and convert at
the boundary.

```rust
pub enum ConfigError { MissingKey(String), BadValue { key, value }, TooLarge(u64) }

impl From<ConfigError> for PyErr {
    fn from(e: ConfigError) -> PyErr {
        match e {
            ConfigError::MissingKey(_) | ConfigError::BadValue { .. } => ParseError::new_err(e.to_string()),
            ConfigError::TooLarge(_) => LimitError::new_err(e.to_string()),
        }
    }
}

fn parse_config_impl(text: &str) -> Result<(String, u64), ConfigError> { ... }   // pure Rust

#[pyfunction]
pub fn parse_config(text: &str) -> PyResult<(String, u64)> {
    Ok(parse_config_impl(text)?)    // ? uses the From impl
}
```

`parse_config_impl` has ordinary Rust unit tests (`cargo test --lib`).

| Input | Result |
|---|---|
| `"name = demo\nsize = 42"` | `('demo', 42)` |
| `"size = 1"` | `ParseError: missing key 'name'` |
| `"name=a\nsize=abc"` | `ParseError: bad value 'abc' for 'size'` |
| `"name=a\nsize=5000"` | `LimitError: value 5000 exceeds the limit of 1000` |

### Catching Python exceptions in Rust

```rust
pub fn try_call(py: Python<'_>, func: &Bound<'_, PyAny>) -> (bool, Py<PyAny>) {
    match func.call0() {
        Ok(v) => (true, v.unbind()),
        Err(e) => (false, /* "TypeName: message" */),
    }
}
```

```python
rp.try_call(lambda: 1 / 0)   # (False, 'ZeroDivisionError: division by zero')
rp.try_call(lambda: "ok")    # (True, 'ok')
```

Every call into Python returns `PyResult`, so exceptions raised by Python code
can be inspected (`e.get_type(py)`, `e.value(py)`, `e.is_instance_of::<T>(py)`),
handled, or propagated with `?`.

---

## 12. Dynamic features (`dynamic.rs`)

### Signatures: defaults, positional-only, keyword-only

```rust
#[pyfunction]
#[pyo3(signature = (name, /, greeting="Hello", *, punctuation="!", times=1))]
pub fn greet(name: &str, greeting: &str, punctuation: &str, times: usize) -> String
```

Equivalent Python signature: `greet(name, /, greeting="Hello", *, punctuation="!", times=1)`.

```python
rp.greet("Rolf")                                     # 'Hello, Rolf!'
rp.greet("World", "Hi", punctuation="?", times=2)    # 'Hi, World? Hi, World?'
rp.greet(name="x")         # TypeError – positional-only
rp.greet("x", "Hi", "!")   # TypeError – punctuation is keyword-only
```

### `*args` and `**kwargs`

```rust
#[pyo3(signature = (*args, **kwargs))]
pub fn summarize(args: &Bound<'_, PyTuple>, kwargs: Option<&Bound<'_, PyDict>>) -> PyResult<String>
```

`args` is always a tuple; `kwargs` is `None` when no keyword arguments are passed.

### Duck typing with `Bound<PyAny>`

`type_of(obj)` accepts anything and inspects it at runtime:

| Check | API |
|---|---|
| `None` | `obj.is_none()` |
| exact Python type | `obj.is_instance_of::<PyBool>()` |
| cast to concrete type | `obj.cast::<PyString>()` → `Result<&Bound<PyString>, _>` |
| try converting | `obj.extract::<i128>()` |
| protocols | `obj.is_callable()`, `obj.hasattr("__len__")`, `obj.len()` |
| type name | `obj.get_type().name()` |

> Check `PyBool` before `PyInt` – in Python `bool` is a subclass of `int`.

### Union input types: `#[derive(FromPyObject)]` on enums

```rust
#[derive(FromPyObject)]
pub enum Number {
    Int(i64),
    Float(f64),
    Text(String),
    Fraction(i64, i64),    // a 2-tuple
}
```

PyO3 tries the variants **in order** and takes the first that succeeds:

```python
rp.to_float(5)          # 5.0
rp.to_float(2.5)        # 2.5
rp.to_float(" 7.25 ")   # 7.25
rp.to_float((1, 4))     # 0.25
rp.to_float([1])        # TypeError listing why each variant failed
```

### Struct extraction from attributes or dict items

```rust
#[derive(FromPyObject)]
pub struct NamedValue { pub name: String, pub value: f64 }   // reads obj.name, obj.value

#[derive(FromPyObject)]
pub struct DictNamedValue {
    #[pyo3(item)] pub name: String,                            // reads obj["name"]
    #[pyo3(item)] pub value: f64,
}
```

```python
@dataclass
class Measurement:
    name: str
    value: float

rp.format_named([Measurement("temp", 21.456), SimpleNamespace(name="hum", value=55)])
# ['temp = 21.46', 'hum = 55.00']
rp.format_dicts([{"name": "pressure", "value": 1013.25}])
# ['pressure = 1013.25']
```

Any object with the right attributes works – no Python base class is needed.

### Union output types: `#[derive(IntoPyObject)]`

```rust
#[derive(IntoPyObject)]
pub enum Parsed { Int(i64), Float(f64), Bool(bool), Text(String), List(Vec<Parsed>), Nothing(Option<()>) }
```

Each variant converts to its inner value, so a function can return values of
different Python types:

```python
rp.parse_literal("42")           # 42
rp.parse_literal("3.14")         # 3.14
rp.parse_literal("true")         # True
rp.parse_literal("")             # None
rp.parse_literal("[1, 2.5, x]")  # [1, 2.5, 'x']
```

### Calling Python from Rust

```rust
// callbacks
pub fn apply<'py>(func: &Bound<'py, PyAny>, items: Vec<Bound<'py, PyAny>>) -> PyResult<Vec<Bound<'py, PyAny>>> {
    items.into_iter().map(|x| func.call1((x,))).collect()
}

// positional + keyword args: callback(i, total=n)
let kwargs = PyDict::new(py);
kwargs.set_item("total", n)?;
callback.call((i,), Some(&kwargs))?;

// importing the standard library
let math = py.import("math")?;
let gcd: i64 = math.getattr("gcd")?.call1((a, b))?.extract()?;
json.call_method("dumps", (data,), Some(&kwargs))?.extract()
```

| Method | Python equivalent |
|---|---|
| `f.call0()` | `f()` |
| `f.call1((a, b))` | `f(a, b)` |
| `f.call((a,), Some(&kw))` | `f(a, **kw)` |
| `o.call_method0("m")` | `o.m()` |
| `o.call_method("m", args, kw)` | `o.m(*args, **kw)` |
| `o.getattr("x")` | `o.x` |
| `py.import("math")` | `import math` |

`run_with_progress(n, callback)` stops early when the callback returns a falsy
value (but not `None`):

```python
def progress(i, total):
    return i < 3
rp.run_with_progress(10, progress)   # 14 = 1² + 2² + 3²
```

### Releasing the GIL

```rust
#[pyfunction]
pub fn count_primes(py: Python<'_>, limit: usize) -> usize {
    py.detach(|| { /* sieve of Eratosthenes, pure Rust */ })
}
```

While the closure runs, other Python threads may execute, so several threads
calling `count_primes` really run in parallel. Rules inside `detach`:

- do not touch Python objects (`Bound`, `Python` token) – the compiler enforces
  this because the closure must be `Send`/`Ungil`;
- copy any needed data into Rust types before entering the closure.

The demo measures the effect with 4 threads. The speed-up is about 1.7× instead
of 4× because the 50 MB sieve is limited by memory bandwidth, not by the CPU.
Pure arithmetic workloads scale closer to the number of cores.

---

## 13. The demo script (`python/demo.py`)

Structure:

- `section(title)` prints a header.
- `expect_error(exc_type, func, *args)` calls a function, verifies that it raises
  the expected exception and prints the message.
- Sections in order: module constants, integers, floats/bool, strings, bytes,
  complex, Option/None, lists, tuples, dicts, sets, `Point`, `Inventory`,
  `Counter`, inheritance, enums, errors, signatures, duck typing, callbacks,
  GIL release.
- `assert` statements verify results against Python equivalents
  (`math.factorial`, complex multiplication, `math.pi`, …).

Excerpt of the output:

```
=== Class: Point ================================================
  p=Point(x=3.0, y=4.0)  q=Point(x=0.0, y=1.0)  p.length=5.0  abs(p)=5.0
  p + q = Point(x=3.0, y=5.0)  p - q = Point(x=3.0, y=3.0)  p * 2 = Point(x=6.0, y=8.0) ...
=== Errors / exceptions =========================================
  parse_config('name=a\nsize=5000',) -> LimitError: value 5000 exceeds the limit of 1000
  LimitError is a RustyError: True  and an Exception: True
...
All checks passed ✔
```

---

## 14. Type mapping reference

| Python | Rust argument | Rust return value | Notes |
|---|---|---|---|
| `int` | `i8`…`i128`, `u8`…`u128`, `isize`, `usize` | same | range-checked → `OverflowError` |
| `float` | `f32`, `f64` | same | `int` accepted |
| `bool` | `bool` | `bool` | strict |
| `str` | `&str`, `String`, `Cow<str>`, `char` | `String`, `&str`, `char` | `char` = 1 code point |
| `bytes` | `&[u8]`, `Vec<u8>` | `Cow<[u8]>`, `Bound<PyBytes>` | `Vec<u8>` returns `list` |
| `complex` | `Complex64` | `Complex64` | feature `num-complex` |
| `None` | `Option<T>` | `Option<T>`, `()` | |
| `list` | `Vec<T>`, `[T; N]`, `Bound<PyList>` | `Vec<T>`, `[T; N]` | `str` not accepted as `Vec` |
| `tuple` | `(A, B, …)`, `Bound<PyTuple>` | `(A, B, …)` | |
| `dict` | `HashMap`, `BTreeMap`, `Bound<PyDict>` | same | |
| `set`, `frozenset` | `HashSet`, `BTreeSet`, `Bound<PySet>` | `HashSet`, `BTreeSet` | |
| any object | `Bound<PyAny>` | `Bound<PyAny>`, `Py<PyAny>` | |
| callable | `Bound<PyAny>` (call with `.call*`) | | |
| own `#[pyclass]` | `&T`, `&mut T`, `PyRef<T>`, `PyRefMut<T>`, `T` (Clone + `from_py_object`) | `T` | |
| union of types | `#[derive(FromPyObject)] enum` | `#[derive(IntoPyObject)] enum` | |

Optional PyO3 features add more mappings: `chrono`/`jiff` (datetime),
`num-bigint` (big ints), `rust_decimal` (Decimal), `uuid`, `serde`, …

---

## 15. Python API reference

### Constants

| Name | Value |
|---|---|
| `__version__` | `"0.1.0"` |
| `PI` | `3.141592653589793` |
| `MAX_U32` | `4294967295` |
| `GREETING` | `"Hello from Rust"` |

### Functions

| Function | Returns | Raises |
|---|---|---|
| `add_i64(a, b)` | `int` | `OverflowError` |
| `double_u8(x)` | `int` | `OverflowError` if x ∉ 0..255 |
| `factorial_u128(n)` | `int` | `OverflowError` if n > 34 |
| `hypot(x, y)` | `float` | |
| `to_f32(x)` | `float` (f32 precision) | |
| `logical_xor(a, b)` | `bool` | |
| `reverse_string(s)` | `str` | |
| `string_stats(s)` | `(chars, bytes, words)` | |
| `next_char(c)` | `str` or `None` | `ValueError` if len ≠ 1 |
| `xor_bytes(data, key)` | `bytes` | |
| `byte_range(n)` | `bytes` 0,1,…,255,0,… | |
| `checksum(data)` | `int` | |
| `complex_mul(a, b)` | `complex` | |
| `complex_polar(z)` | `(r, phi)` | |
| `maybe_square(value=None)` | `int` or `None` | |
| `do_nothing()` | `None` | |
| `stats(values)` | `(min, max, mean)` or `None` | |
| `sorted_unique(values)` | `list[int]` | |
| `cross_product(a, b)` | `list[float]` (3) | `ValueError` wrong length |
| `transpose(matrix)` | `list[list[float]]` | `ValueError` ragged |
| `double_in_place(list)` | `None` (modifies list) | |
| `describe_person((name, age, active))` | `str` | |
| `make_tuple(n)` | `tuple` of squares | |
| `word_count(text)` | `dict[str, int]` | |
| `invert_dict(d)` | `dict[int, list[str]]` | |
| `system_info()` | `dict` | |
| `sum_numeric_values(d)` | `float` | |
| `set_ops(a, b)` | `(intersection, union, difference)` | |
| `unique_chars(s)` | `set[str]` | |
| `centroid(points)` | `Point` or `None` | |
| `nth_point(points, n)` | `Point` | `IndexError` |
| `mix(a, b)` | `str` | |
| `largest(shapes)` | `Shape` or `None` | |
| `parse_config(text)` | `(name, size)` | `ParseError`, `LimitError` |
| `safe_sqrt(x)` | `float` | `ValueError` |
| `parse_int(s)` | `int` | `ValueError` |
| `try_call(func)` | `(ok, result_or_message)` | |
| `greet(name, /, greeting="Hello", *, punctuation="!", times=1)` | `str` | |
| `summarize(*args, **kwargs)` | `str` | |
| `type_of(obj)` | `str` | |
| `to_float(n)` | `float` | `TypeError`, `ValueError`, `ZeroDivisionError` |
| `format_named(items)` | `list[str]` | |
| `format_dicts(items)` | `list[str]` | |
| `parse_literal(s)` | `int`/`float`/`bool`/`str`/`list`/`None` | |
| `apply(func, items)` | `list` | whatever `func` raises |
| `run_with_progress(n, callback)` | `int` | whatever `callback` raises |
| `use_stdlib(a, b)` | JSON `str` | |
| `count_primes(limit)` | `int` | |

### Classes

| Class | Constructor | Members |
|---|---|---|
| `Point` | `Point(x=0.0, y=0.0)` | `x`, `y`, `length`, `distance_to`, `translate`, `as_tuple`, `origin()`, `from_polar(r, phi)`, operators, hash, pickle |
| `Inventory` | `Inventory(items=None)` | `add(name, qty=1)`, `remove(name, qty)`, `total`, `to_dict()`, `len`, `[]`, `in`, `del`, iteration |
| `InventoryIter` | (from `iter(inventory)`) | iterator protocol |
| `Counter` | `Counter(start=0, step=1, limit=None)` | `value` (read-only), `increment()`, `reset()`, iteration |
| `Animal` | `Animal(name)` | `name`, `speak()`, `introduce()`; subclassable |
| `Dog(Animal)` | `Dog(name, tricks=[])` | `tricks`, `learn(trick)`, `speak()` |
| `Color` | `Color.Red/Green/Blue` | `hex()`, `parse(name)`, `int()` |
| `Shape` | `Shape.Circle(radius)`, `Shape.Rectangle(width, height)`, `Shape.Triangle(a, b, c)`, `Shape.Polygon(points)` | `area()`, `kind()` |

### Exceptions

```
Exception
└── RustyError
    ├── ParseError
    └── LimitError
```

---

## 16. Pitfalls and tips

1. **`Vec<u8>` returns a list.** Use `Cow<[u8]>` or `PyBytes` for `bytes`.
2. **Integer overflow inside Rust** wraps silently in release builds – use
   `checked_*` operations.
3. **`bool` is an `int` in Python.** Check `PyBool` before `PyInt`;
   `extract::<f64>()` succeeds for `True`.
4. **Owned arguments copy.** `Vec<T>`/`HashMap` convert the whole container on
   every call. For very large data prefer `Bound<PyList>`, the buffer protocol,
   or the `numpy` crate (`PyReadonlyArray1<f64>`) for zero-copy arrays.
5. **Runtime borrow checking.** A `#[pyclass]` is borrowed at runtime; calling a
   `&mut self` method while another borrow is active (e.g. from a callback)
   raises `RuntimeError: Already borrowed`.
6. **Long-running Rust code blocks other threads** unless you release the GIL
   with `py.detach`.
7. **Conda + venv conflict.** maturin refuses to run if both `VIRTUAL_ENV` and
   `CONDA_PREFIX` are set – `unset CONDA_PREFIX` or deactivate conda.
8. **Rebuild after changes.** Python keeps using the old `.so` until you run
   `maturin develop` again and restart the interpreter.
9. **The lib name must match** the `#[pymodule]` function name, otherwise
   `ImportError: dynamic module does not define module export function`.
10. **Doc comments are docstrings.** `///` comments appear in `help()` – write them.

---

## 17. Extending the example

### Adding a function

1. Write it in the appropriate file:
   ```rust
   /// Clamp a value into [lo, hi].
   #[pyfunction]
   #[pyo3(signature = (x, lo=0.0, hi=1.0))]
   pub fn clamp(x: f64, lo: f64, hi: f64) -> f64 { x.clamp(lo, hi) }
   ```
2. Register it in that file's `register()`:
   ```rust
   m.add_function(wrap_pyfunction!(clamp, m)?)?;
   ```
3. `maturin develop --release`, then `rp.clamp(1.7)` → `1.0`.

### Adding a class

```rust
#[pyclass(module = "rustypy")]
pub struct Stack { items: Vec<Py<PyAny>> }

#[pymethods]
impl Stack {
    #[new] fn new() -> Self { Stack { items: vec![] } }
    fn push(&mut self, item: Py<PyAny>) { self.items.push(item) }
    fn pop(&mut self) -> Option<Py<PyAny>> { self.items.pop() }
    fn __len__(&self) -> usize { self.items.len() }
}
// register: m.add_class::<Stack>()?;
```

`Py<PyAny>` stores arbitrary Python objects inside a Rust struct.

### Further ideas

- **Type stubs** (`rustypy.pyi`) for IDE autocompletion and mypy.
- **NumPy arrays** with the `numpy` crate.
- **Async** functions with `pyo3-async-runtimes`.
- **Sub-modules** (`rustypy.geometry`) via `#[pymodule] mod geometry { ... }`.
- **Datetime** support with the `chrono` feature.
- **Serde**: convert arbitrary JSON-like data with `pythonize`.

---

## 18. Troubleshooting

| Symptom | Cause / fix |
|---|---|
| `ModuleNotFoundError: No module named 'rustypy'` | Not built or wrong interpreter. Activate `.venv`, run `maturin develop`. |
| `Both VIRTUAL_ENV and CONDA_PREFIX are set` | `unset CONDA_PREFIX` |
| `cargo test` linker errors (`undefined reference to Py...`) | Don't enable `extension-module` for tests; set `PYO3_PYTHON=.venv/bin/python`. |
| `ImportError: ... does not define module export function` | `[lib] name` ≠ `#[pymodule]` function name. |
| Changes not visible | Re-run `maturin develop` and restart Python. |
| `RuntimeError: Already borrowed` | Overlapping `PyRef`/`PyRefMut` borrows of the same object. |
| `TypeError: argument 'x': ...` | Python value doesn't convert to the Rust parameter type – check the type mapping table. |
| rust-analyzer shows errors but `cargo build` works | Stale analysis of macro-generated code; restart rust-analyzer. |

---

*Further reading:* [PyO3 user guide](https://pyo3.rs) ·
[PyO3 API docs](https://docs.rs/pyo3) · [maturin user guide](https://www.maturin.rs)
