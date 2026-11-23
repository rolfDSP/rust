# pyexc – Documentation

A complete guide to the `r_proj_080` example: a native Python extension module named
**`pyexc`**, written in Rust with **PyO3 0.30** and built with **maturin**. It covers
Python exceptions in both directions:

* **Rust → Python:** raising built-in exceptions, defining new exception classes, chaining
  exceptions, adding notes and raising exception groups.
* **Python → Rust:** catching exceptions raised by Python callbacks, inspecting them,
  handling some, re-raising others, and translating them into different exceptions.
* **Python side:** every pattern a Python script uses to handle these exceptions
  (`python/demo.py`).

---

## Table of contents

1. [Overview](#1-overview)
2. [How exceptions cross the Rust/Python boundary](#2-how-exceptions-cross-the-rustpython-boundary)
3. [Project layout](#3-project-layout)
4. [Setup, build and test](#4-setup-build-and-test)
5. [Configuration files explained](#5-configuration-files-explained)
6. [Module entry point (`lib.rs`)](#6-module-entry-point-librs)
7. [Built-in exceptions (`builtin.rs`)](#7-built-in-exceptions-builtinrs)
8. [Custom exception hierarchy and context manager (`bank.rs`)](#8-custom-exception-hierarchy-and-context-manager-bankrs)
9. [Rich exceptions, chaining, notes, groups (`validation.rs`)](#9-rich-exceptions-chaining-notes-groups-validationrs)
10. [Catching Python exceptions in Rust (`catching.rs`)](#10-catching-python-exceptions-in-rust-catchingrs)
11. [The demo script (`python/demo.py`)](#11-the-demo-script-pythondemopy)
12. [Python exception handling guide](#12-python-exception-handling-guide)
13. [Exception hierarchy reference](#13-exception-hierarchy-reference)
14. [Python API reference of `pyexc`](#14-python-api-reference-of-pyexc)
15. [PyO3 exception API cheat sheet](#15-pyo3-exception-api-cheat-sheet)
16. [Pitfalls and best practices](#16-pitfalls-and-best-practices)
17. [Extending the example](#17-extending-the-example)
18. [Troubleshooting](#18-troubleshooting)

---

## 1. Overview

In Rust, errors are values: a function returns `Result<T, E>`. In Python, errors are
exceptions: they are *raised* and *caught* with `try`/`except`. PyO3 connects the two:

```text
Rust                                      Python
────────────────────────────────────      ──────────────────────────────────
fn f() -> PyResult<T>                      f()
    Ok(value)                 ───────►     returns value
    Err(PyErr)                ───────►     raises the exception inside PyErr

obj.call0() -> PyResult<..>                def callback(): raise KeyError("x")
    Err(PyErr)                ◄───────     exception raised in Python
```

A short example of the result:

```python
import pyexc

try:
    pyexc.Account("Alice", 100).withdraw(500)
except pyexc.InsufficientFundsError as e:
    msg, balance, requested = e.args
    print(f"short by {requested - balance}")      # short by 400.0
except pyexc.BankError as e:
    print("other banking problem:", e)
```

The example is organised by topic, so each file can be read on its own:

| Topic | Rust file | Key PyO3 features |
|---|---|---|
| Module entry point | `src/lib.rs` | `#[pymodule]`, `m.add` |
| Built-in exceptions | `src/builtin.rs` | `PyResult`, `PyXxxError::new_err`, automatic `From<std error> for PyErr` |
| Custom hierarchy | `src/bank.rs` | `create_exception!`, `impl From<MyError> for PyErr`, `__enter__`/`__exit__` |
| Rich exceptions | `src/validation.rs` | `#[pyclass(extends = PyException)]`, `set_cause`, `add_note`, `ExceptionGroup` |
| Catching | `src/catching.rs` | `is_instance_of`, re-raise, `import_exception!`, `PyErr::warn`, `PanicException` |

---

## 2. How exceptions cross the Rust/Python boundary

### `PyResult<T>` and `PyErr`

```rust
pub type PyResult<T> = Result<T, PyErr>;
```

`PyErr` is PyO3's representation of a Python exception. A `#[pyfunction]` or
`#[pymethods]` method that returns `PyResult<T>` behaves like this:

* `Ok(v)`: `v` is converted to a Python object and returned.
* `Err(e)`: PyO3 hands the exception to the Python interpreter (in C:
  `PyErr_Restore`) and returns `NULL`. Python then raises it at the call site.

A function can return any `Result<T, E>` as long as `E: Into<PyErr>`. In practice you
use `PyResult<T>` and let `?` do the conversion.

### Lazy and normalised exceptions

`PyValueError::new_err("msg")` does **not** create a Python object right away. It
stores the exception *type* and the *arguments* (anything that implements
`PyErrArguments`, e.g. `&str`, `String`, tuples, numbers). This is the **lazy** state. It
is cheap, and it does not even need the GIL.

The exception object is only created ("**normalised**") when it is needed:

* when it is raised in Python;
* when Rust code calls `err.value(py)`, `err.traceback(py)`, `err.set_cause(...)`,
  `err.into_value(py)` and so on.

Normalising calls the exception class with the stored arguments, like
`ValueError("msg")`. For a class with a `#[new]` constructor, such as `ValidationError`,
that constructor runs at this point.

### The GIL token `py: Python<'_>`

Many `PyErr` methods (`value`, `is_instance_of`, `set_cause`, `cause`, `traceback`,
`get_type`) need a `Python<'py>` token. It proves that the current thread is attached to
the interpreter. A `#[pyfunction]` receives one when you add a `py: Python<'_>` parameter
(it is not visible to Python), or you get it from any bound object with `obj.py()`.

### Four ways to produce a `PyErr`

| Way | Example | Used in |
|---|---|---|
| built-in exception type | `PyKeyError::new_err(key)` | `builtin.rs` |
| `create_exception!` type | `InsufficientFundsError::new_err((msg, b, r))` | `bank.rs` |
| any Python exception class (`PyTypeInfo`) | `PyErr::new::<ValidationError, _>((field, reason))` | `validation.rs` |
| an existing exception *object* | `PyErr::from_value(group)` | `validation.rs` |

Plus **automatic conversions** through `?`: `From<ParseIntError> for PyErr`,
`From<io::Error> for PyErr`, … and your own `From<MyError> for PyErr`.

### What happens to Rust panics

A `panic!` must never unwind across the C boundary into the Python interpreter. PyO3
wraps every exported function in `catch_unwind` and turns a panic into
`pyo3_runtime.PanicException`. That class derives from **`BaseException`**, not from
`Exception`, because a panic signals a bug rather than an expected error (see
[§10.7](#107-panics--panicexception)).

---

## 3. Project layout

```text
r_proj_080/
├── Cargo.toml          Rust package, crate type cdylib + rlib, PyO3 dependency
├── pyproject.toml      Python package metadata, maturin as build backend
├── .gitignore
├── README.md           Short overview and cheat sheet
├── DOC.md              This document
├── src/
│   ├── lib.rs          #[pymodule] pyexc – registers all sub-parts
│   ├── builtin.rs      Built-in exceptions and automatic conversions
│   ├── bank.rs         create_exception! hierarchy, Account, Transaction
│   ├── validation.rs   ValidationError class, chaining, notes, ExceptionGroup
│   └── catching.rs     Catching Python exceptions in Rust, warnings, panics
└── python/
    └── demo.py         14 sections of Python exception handling, self-checking
```

---

## 4. Setup, build and test

### Prerequisites

* Rust (edition 2024, i.e. Rust ≥ 1.85)
* Python **≥ 3.11** (`ExceptionGroup`, `except*` and `BaseException.add_note()` are used)
* `maturin` (installed into the virtual environment)

### First-time setup

```bash
cd r_proj_080
python3 -m venv .venv
source .venv/bin/activate
pip install maturin
```

### Build and install for development

```bash
unset CONDA_PREFIX      # only if Anaconda is active as well (see Troubleshooting)
maturin develop         # debug build, installed into .venv
maturin develop --release   # optimised build
```

`maturin develop` compiles the crate as a shared library (`.so` on Linux) and installs
it into the active virtual environment, so `import pyexc` works right away. Run it again
after every Rust change.

### Run the demo

```bash
python python/demo.py
```

Each section prints what happens and checks the expected behaviour with `assert`. The
last line is `All exception examples passed.` The two panic examples in section 14 also
print Rust's panic message (`thread '<unnamed>' panicked at …`) to **stderr**. That is
normal: Rust's panic hook prints it before PyO3 converts the panic into an exception.

### Build a distributable wheel

```bash
maturin build --release
pip install target/wheels/pyexc-0.1.0-*.whl
```

### Rust unit tests

The pure Rust logic (`Ledger` in `bank.rs`) has unit tests. Because the crate links to
libpython, the test binary needs to find `libpython3.x.so` at run time. With an Anaconda
base interpreter:

```bash
PYO3_PYTHON=.venv/bin/python cargo test --lib --no-run      # build only
LD_LIBRARY_PATH=$HOME/anaconda3/lib target/debug/deps/pyexc-<hash>
```

Don't set `LD_LIBRARY_PATH` for the whole `cargo` command: Anaconda's `libssl` then
replaces the system one and cargo itself fails (`OPENSSL_3.2.0 not found`).
With a system Python (`/usr/bin/python3` plus `python3-dev`), plain
`PYO3_PYTHON=.venv/bin/python cargo test --lib` works.

### Interactive use

```bash
python -q
>>> import pyexc
>>> pyexc.divide(1, 0)
Traceback (most recent call last):
  File "<stdin>", line 1, in <module>
ZeroDivisionError: division by zero (raised in Rust)
```

---

## 5. Configuration files explained

### `Cargo.toml`

```toml
[lib]
name = "pyexc"                    # must match the #[pymodule] function name
crate-type = ["cdylib", "rlib"]

[dependencies]
pyo3 = "0.30"

[features]
extension-module = ["pyo3/extension-module"]
```

* **`name = "pyexc"`**: the shared library is called `libpyexc.so`. maturin installs it
  as `pyexc`, and Python looks for the init function `PyInit_pyexc`, which
  `#[pymodule] fn pyexc` generates.
* **`cdylib`**: a C-compatible shared library that Python can load.
* **`rlib`**: a normal Rust library as well, so `cargo test` can link the code.
* **`extension-module` feature**: tells PyO3 *not* to link against libpython (the
  interpreter that loads the module already provides the symbols). It is only enabled by
  maturin (see `pyproject.toml`), so `cargo test` still links libpython and can run.

### `pyproject.toml`

```toml
[build-system]
requires = ["maturin>=1.5,<2.0"]
build-backend = "maturin"

[project]
name = "pyexc"
requires-python = ">=3.11"

[tool.maturin]
features = ["extension-module"]
```

* `build-backend = "maturin"` lets `pip install .` and `maturin build` know how to compile.
* `requires-python = ">=3.11"`: the demo uses `except*`, and `validate_users` creates
  an `ExceptionGroup`.
* `features = ["extension-module"]` enables the feature only for maturin builds.

---

## 6. Module entry point (`lib.rs`)

```rust
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
```

Each topic file has a `pub fn register(m)` that adds its functions, classes and exception
types to the module. Everything ends up flat in the `pyexc` namespace
(`pyexc.divide`, `pyexc.BankError`, …).

The module function itself returns `PyResult<()>`. If it returned `Err`, `import pyexc`
would raise that exception (an `ImportError`, for example).

---

## 7. Built-in exceptions (`builtin.rs`)

### 7.1 Raising a built-in exception explicitly

Every Python built-in exception has a Rust counterpart in `pyo3::exceptions`, named with
a `Py` prefix: `PyValueError`, `PyTypeError`, `PyKeyError`, `PyIndexError`,
`PyZeroDivisionError`, `PyOverflowError`, `PyNotImplementedError`, `PyRuntimeError`,
`PyPermissionError`, `PyArithmeticError`, `PyLookupError`, `PyOSError`, …

```rust
#[pyfunction]
pub fn divide(a: f64, b: f64) -> PyResult<f64> {
    if b == 0.0 {
        return Err(PyZeroDivisionError::new_err("division by zero (raised in Rust)"));
    }
    Ok(a / b)
}
```

`new_err(args)` accepts anything that implements `PyErrArguments`:

| Rust argument | Python `e.args` |
|---|---|
| `"text"` / `String` | `("text",)` |
| `(msg, 1.0, 2.0)` | `(msg, 1.0, 2.0)` |
| `()` | `()` |

### 7.2 Functions in `builtin.rs`

| Function | Raises | Why |
|---|---|---|
| `divide(a, b)` | `ZeroDivisionError` | `b == 0` |
| `get_item(items, index)` | `IndexError` | index out of range (negative indices count from the end, like Python) |
| `lookup(table, key)` | `KeyError` | key missing; `e.args == (key,)`, like a Python `dict` |
| `ensure_str(obj)` | `TypeError` | `obj` is not a `str`; the message names the actual type |
| `not_ready()` | `NotImplementedError` | always |
| `multiply_i32(a, b)` | `OverflowError` | `checked_mul` returned `None` |
| `raise_builtin(kind, msg)` | the exception named `kind` | lets the demo test which `except` clause catches what |

**`KeyError` convention:** Python's `dict` raises `KeyError('missing')`, with the key as
the only argument, not a sentence. `lookup` does the same with
`PyKeyError::new_err(key.to_string())`, so `str(e)` prints `'missing'` (with quotes, as
`KeyError.__str__` uses `repr`).

**Checking a type by hand** (`ensure_str`):

```rust
if let Ok(s) = obj.extract::<String>() {
    return Ok(s);
}
let type_name = obj.get_type().name()?;
Err(PyTypeError::new_err(format!("expected str, got {type_name}")))
```

### 7.3 Automatic conversion of `std` errors with `?`

PyO3 implements `From<E> for PyErr` for many standard library error types, so `?` turns
them into Python exceptions without extra code:

```rust
#[pyfunction]
pub fn parse_int(text: &str) -> PyResult<i64> {
    Ok(text.trim().parse::<i64>()?)        // ParseIntError → ValueError
}
```

| Rust error | Python exception | Function in this project |
|---|---|---|
| `std::num::ParseIntError` | `ValueError` | `parse_int` |
| `std::num::ParseFloatError` | `ValueError` | `parse_float` |
| `std::num::TryFromIntError` | `ValueError` | `to_u8` |
| `std::io::Error` | `OSError` subclass chosen by `ErrorKind` | `read_text` |
| `std::string::FromUtf8Error` | `UnicodeDecodeError` | `decode_utf8` |
| `std::char::CharTryFromError` | `ValueError` | – |
| `std::net::AddrParseError` | `ValueError` | – |
| `std::ffi::NulError` | `ValueError` | – |
| `std::str::ParseBoolError`, `std::char::ParseCharError`, `std::array::TryFromSliceError` | `ValueError` | – |

`std::io::Error` mapping (by `io::ErrorKind`):

| `ErrorKind` | Python |
|---|---|
| `NotFound` | `FileNotFoundError` |
| `PermissionDenied` | `PermissionError` |
| `IsADirectory` | `IsADirectoryError` |
| `NotADirectory` | `NotADirectoryError` |
| `AlreadyExists` | `FileExistsError` |
| `ConnectionRefused` / `ConnectionReset` / `ConnectionAborted` | `ConnectionRefusedError` / `ConnectionResetError` / `ConnectionAbortedError` |
| `BrokenPipe` | `BrokenPipeError` |
| `WouldBlock` | `BlockingIOError` |
| `TimedOut` | `TimeoutError` |
| `Interrupted` | `InterruptedError` |
| `OutOfMemory` | `MemoryError` |
| other | `OSError` |

> **Note 1:** Only the message is passed to the `OSError` subclass, so `e.errno` is
> `None`. Check the class (`except FileNotFoundError`) and not `e.errno`.
>
> **Note 2:** `std::str::Utf8Error` (returned by `std::str::from_utf8`) has **no**
> conversion. `?` on it does not compile. Use `String::from_utf8`, which returns
> `FromUtf8Error`, or map the error yourself with `.map_err(...)`.

### 7.4 Exceptions raised by PyO3 itself

Before your function body runs, PyO3 converts the Python arguments. If that fails, PyO3
raises the exception, and your code never runs:

| Call | Exception | Reason |
|---|---|---|
| `divide("ten", 2)` | `TypeError: must be real number, not str` | cannot convert to `f64` |
| `to_u8(2**70)` | `OverflowError: Python int too large to convert to C long` | does not fit into `i64` |
| `divide(1.0)` | `TypeError: divide() missing 1 required positional argument: 'b'` | |
| `divide(1.0, 2.0, 3.0)` | `TypeError: divide() takes 2 positional arguments but 3 were given` | |

Note the difference between `to_u8(300)` and `to_u8(2**70)`: 300 fits into the `i64`
parameter, so the Rust body runs and `u8::try_from` fails (`ValueError`). `2**70` already
fails during argument conversion (`OverflowError`).

---

## 8. Custom exception hierarchy and context manager (`bank.rs`)

### 8.1 `create_exception!`

```rust
create_exception!(pyexc, BankError, PyException, "Base class of all banking errors.");
create_exception!(pyexc, InsufficientFundsError, BankError, "Balance too low for a withdrawal.");
create_exception!(pyexc, AccountLockedError, BankError, "The account is locked.");
create_exception!(pyexc, InvalidAmountError, BankError, "Amount is zero, negative or not finite.");
```

Arguments: `(module name, Rust/Python class name, base class, docstring)`. The macro
creates a Rust type that implements `PyTypeInfo`. It has `::new_err(args)` like the
built-in exceptions and can be used as a base for further `create_exception!` calls.

This is the equivalent of the following Python code:

```python
class BankError(Exception):
    """Base class of all banking errors."""
class InsufficientFundsError(BankError):
    """Balance too low for a withdrawal."""
class AccountLockedError(BankError): ...
class InvalidAmountError(BankError): ...
```

The types must be **added to the module**, or Python code can't name them in an `except`
clause:

```rust
m.add("BankError", py.get_type::<BankError>())?;
m.add("InsufficientFundsError", py.get_type::<InsufficientFundsError>())?;
```

Python sees them as normal exception classes. They can be raised from Python, subclassed
in Python and pickled:

```python
class OverdraftLimitError(pyexc.InsufficientFundsError): ...
raise OverdraftLimitError("limit reached")    # caught by `except pyexc.BankError`
```

**Limitation:** `create_exception!` creates classes with **one** base class and no
custom attributes or methods. For attributes, use `#[pyclass(extends = PyException)]`
([§9.1](#91-validationerror--an-exception-class-with-attributes)).

### 8.2 Keep business logic free of Python: Rust error enum + `From`

The design used here separates the pure Rust logic from the Python bindings:

```text
Ledger (pure Rust)  ──Result<_, BankFailure>──►  Account (#[pymethods])  ──PyResult──►  Python
                                                     uses `?` + From impl
```

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum BankFailure {
    InvalidAmount(f64),
    InsufficientFunds { balance: f64, requested: f64 },
    Locked(String),
    SameAccount,
}

impl fmt::Display for BankFailure { /* human readable messages */ }
impl std::error::Error for BankFailure {}

impl From<BankFailure> for PyErr {
    fn from(e: BankFailure) -> PyErr {
        let msg = e.to_string();
        match e {
            BankFailure::InvalidAmount(_) => InvalidAmountError::new_err(msg),
            BankFailure::InsufficientFunds { balance, requested } => {
                InsufficientFundsError::new_err((msg, balance, requested))
            }
            BankFailure::Locked(_) => AccountLockedError::new_err(msg),
            BankFailure::SameAccount => BankError::new_err(msg),
        }
    }
}
```

Advantages:

* `Ledger` can be used and unit-tested without Python (`cargo test`, see `mod tests`).
* There is one central place that decides which Python exception each error becomes.
* Extra data can be passed along: `InsufficientFundsError` gets the tuple
  `(msg, balance, requested)`, so Python can read `e.args`.

The methods then stay short:

```rust
fn withdraw(&mut self, amount: f64) -> PyResult<f64> {
    Ok(self.ledger.withdraw(amount)?)          // BankFailure → PyErr via From
}
```

For a constructor, convert explicitly with `.into()`:

```rust
return Err(BankFailure::InvalidAmount(balance).into());
```

> **Orphan rule:** `impl From<BankFailure> for PyErr` is allowed because `BankFailure` is
> defined in this crate. You cannot write `impl From<some_other_crate::Error> for PyErr`;
> wrap the foreign error in your own type or use `.map_err(...)`.

### 8.3 `Account` – the Python class

| Member | Description | May raise |
|---|---|---|
| `Account(owner, balance=0.0)` | constructor | `InvalidAmountError` (negative / NaN / inf balance) |
| `.owner`, `.balance`, `.locked` | read-only properties | – |
| `.deposit(amount)` | returns the new balance | `InvalidAmountError`, `AccountLockedError` |
| `.withdraw(amount)` | returns the new balance | `InvalidAmountError`, `AccountLockedError`, `InsufficientFundsError` |
| `.lock()` / `.unlock()` | lock state | – |
| `.transfer(other, amount)` | move money to another account | all of the above, and `BankError` for the same account |

### 8.4 Turning a PyO3 error into a domain error: `transfer`

```rust
fn transfer(&mut self, other: &Bound<'_, Account>, amount: f64) -> PyResult<()> {
    let mut target = other
        .try_borrow_mut()
        .map_err(|_| BankFailure::SameAccount)?;
    target.ledger.check(amount)?;
    self.ledger.withdraw(amount)?;
    target.ledger.deposit(amount)?;
    Ok(())
}
```

`&mut self` already holds a mutable borrow of the Python object. If Python calls
`acc.transfer(acc, 1.0)`, `other` is the same object and `try_borrow_mut()` fails with
`PyBorrowMutError`. Converted with `?` directly, that would become a generic
`RuntimeError: Already borrowed`. `map_err` turns it into a meaningful `BankError`
instead.

The order of checks matters too. The target is checked before anything changes, so a
failed transfer does not leave half-done changes behind.

### 8.5 `Transaction` – a context manager that sees the exception

Python's `with` statement calls `__exit__(exc_type, exc_value, traceback)` when the block
ends:

* on success, all three arguments are `None`;
* on an exception, they hold the class, the exception object and the traceback;
* if `__exit__` returns a true value, the exception is **suppressed**; otherwise it
  continues to propagate.

```rust
fn __enter__(&mut self, py: Python<'_>) -> Py<Account> {
    self.snapshot = Some(self.account.borrow(py).ledger.balance);
    self.rolled_back = false;
    self.error = None;
    self.account.clone_ref(py)                 // value bound by `as`
}

fn __exit__(
    &mut self,
    py: Python<'_>,
    exc_type: Option<&Bound<'_, PyType>>,
    exc_value: Option<&Bound<'_, PyAny>>,
    _traceback: Option<&Bound<'_, PyAny>>,
) -> PyResult<bool> {
    let (Some(exc_type), Some(exc_value)) = (exc_type, exc_value) else {
        return Ok(false);                      // no exception: commit
    };
    if let Some(balance) = self.snapshot.take() {
        self.account.borrow_mut(py).ledger.balance = balance;   // roll back
        self.rolled_back = true;
    }
    self.error = Some(format!("{}: {}", exc_type.name()?, exc_value.str()?));
    Ok(self.suppress && exc_value.is_instance_of::<BankError>())
}
```

Behaviour:

| Situation | Balance | Exception |
|---|---|---|
| block finishes | kept (commit) | – |
| block raises, `suppress=False` | restored | propagates |
| block raises a `BankError`, `suppress=True` | restored | swallowed; text stored in `tx.error` |
| block raises another exception (e.g. `KeyError`), `suppress=True` | restored | propagates |

`exc_value.is_instance_of::<BankError>()` respects inheritance, so all subclasses count.
Only suppress exceptions you expect. Swallowing everything hides bugs.

---

## 9. Rich exceptions, chaining, notes, groups (`validation.rs`)

### 9.1 `ValidationError` – an exception class with attributes

```rust
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
    fn new(field: String, reason: String) -> Self { Self { field, reason } }
    fn __str__(&self) -> String { format!("{}: {}", self.field, self.reason) }
    fn __repr__(&self) -> String { format!("ValidationError({:?}, {:?})", self.field, self.reason) }
}
```

* `extends = PyException` makes the class a real exception: it can be raised and caught,
  and it gets a traceback.
* `#[pyo3(get)]` exposes typed, read-only attributes (`e.field`, `e.reason`).
* `subclass` allows Python code to subclass it.
* `module = "pyexc"` makes tracebacks show `pyexc.ValidationError`.
* The constructor arguments also end up in `e.args` (`('age', 'bad')`), because the base
  `BaseException.__new__` stores them.

Raising it from Rust:

```rust
fn invalid(field: &str, reason: impl Into<String>) -> PyErr {
    PyErr::new::<ValidationError, _>((field.to_string(), reason.into()))
}
```

`PyErr::new::<T, A>(args)` works for **any** exception class `T: PyTypeInfo`. It is lazy;
when the exception is normalised, Python calls `ValidationError(field, reason)`, which
runs the `#[new]` method.

Python code can raise it as well: `raise pyexc.ValidationError("zip", "must have 5 digits")`.

**`create_exception!` or `#[pyclass(extends = PyException)]`?**

| | `create_exception!` | `#[pyclass(extends = PyException)]` |
|---|---|---|
| Code | one line | struct + `#[pymethods]` |
| Custom attributes / methods | no (only `e.args`) | yes |
| Custom `__str__` | no | yes |
| Base class | any exception type | a built-in exception type such as `PyException` or `PyValueError` |
| Raise from Rust | `T::new_err(args)` | `PyErr::new::<T, _>(args)` |

### 9.2 `validate_user` – first error only

```rust
#[pyfunction]
pub fn validate_user(name: &str, age: i64, email: &str) -> PyResult<String> {
    match check_user(name, age, email).into_iter().next() {
        Some(err) => Err(err),
        None => Ok(format!("user {name} ok")),
    }
}
```

`check_user` returns a `Vec<PyErr>` with **all** problems. Since `PyErr`s are lazy, it is
cheap to build several and use only one.

### 9.3 Exception chaining: `set_cause` (`raise … from …`)

In Python, `raise NewError(...) from original` stores `original` in
`NewError.__cause__`, and the traceback shows both exceptions with the line
*"The above exception was the direct cause of the following exception"*. In Rust:

```rust
let text = std::fs::read_to_string(path).map_err(|io_err| {
    let err = ConfigError::new_err(format!("cannot load config '{path}'"));
    err.set_cause(py, Some(PyErr::from(io_err)));   // io::Error → FileNotFoundError, …
    let _ = err
        .value(py)
        .call_method1("add_note", ("hint: check the path and file permissions",));
    err
})?;
```

* `err.set_cause(py, Some(cause))` sets `__cause__`. `None` removes it.
* `err.cause(py)` reads it back (`Option<PyErr>`).
* `set_cause` normalises `err` (creates the Python object), so it needs `py`.

A second case in `load_config` chains a `ValueError` to a `ConfigError` when a line has
no `=`.

Why chain? The caller gets a **high-level** exception (`ConfigError`) that is easy to
handle, and the **low-level** reason (`FileNotFoundError`) remains available for logging
and debugging.

### 9.4 Notes: `add_note` (`__notes__`)

Python 3.11 added `BaseException.add_note(text)`. Notes are stored in `e.__notes__` (a
list) and printed under the exception message in a traceback:

```text
pyexc.ConfigError: cannot load config '/tmp/pyexc-does-not-exist.txt'
hint: check the path and file permissions
```

PyO3 has no special API for this; call the Python method on the exception object:
`err.value(py).call_method1("add_note", (text,))`. Here the result is ignored with
`let _ =`, because a failed note should not replace the real error.

### 9.5 `ExceptionGroup` – reporting several errors at once

```rust
#[pyfunction]
pub fn validate_users(py: Python<'_>, users: Vec<(String, i64, String)>) -> PyResult<usize> {
    let mut all = Vec::new();
    for (i, (name, age, email)) in users.iter().enumerate() {
        for err in check_user(name, *age, email) {
            let exc = err.into_value(py);                       // PyErr → Py<PyBaseException>
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
```

Steps:

1. `err.into_value(py)` turns each lazy `PyErr` into a real exception object.
2. Each gets a note saying which record failed.
3. `ExceptionGroup(message, [exceptions])` is created by calling the built-in class.
4. `PyErr::from_value(obj)` turns any exception *object* into a `PyErr` to return.

Python handles it with `except*` ([§12.9](#129-exception-groups-and-except)).

---

## 10. Catching Python exceptions in Rust (`catching.rs`)

Every call into Python from Rust (`call0`, `call1`, `call_method1`, `getattr`,
`extract`, `try_iter`, `import`, …) returns `PyResult`. When the Python code raises, you
get `Err(PyErr)` and can decide what to do with it.

### 10.1 Handle some, re-raise others: `safe_call`

```rust
#[pyfunction]
#[pyo3(signature = (func, *args))]
pub fn safe_call(py: Python<'_>, func: &Bound<'_, PyAny>, args: &Bound<'_, PyTuple>)
    -> PyResult<Option<Py<PyAny>>>
{
    match func.call1(args) {
        Ok(value) => Ok(Some(value.unbind())),
        Err(err) if err.is_instance_of::<PyZeroDivisionError>(py) => Ok(None),
        Err(err) => Err(err),               // re-raise unchanged
    }
}
```

* `err.is_instance_of::<T>(py)` is the Rust version of `isinstance(e, T)` and respects
  subclasses. It works with built-in types, `create_exception!` types,
  `import_exception!` types and pyclass exceptions.
* Returning the **same** `PyErr` re-raises the original exception object. Python sees the
  same object (`e is original` is `True`), and the traceback still contains the frame of
  the Python callback.
* There is also `err.matches(py, exc_type)` (like `except` matching, which also accepts a
  tuple of types), and `err.get_type(py)` to get the class.

### 10.2 Inspecting an exception: `inspect_exception`

```rust
let Err(err) = func.call0() else { return Ok(None); };
let value = err.value(py);                               // &Bound<PyBaseException>
info.set_item("type", err.get_type(py).name()?)?;
info.set_item("message", value.str()?)?;
info.set_item("args", value.getattr("args")?)?;
info.set_item("is_lookup_error", err.is_instance_of::<PyLookupError>(py))?;
info.set_item("has_traceback", err.traceback(py).is_some())?;
info.set_item("cause", /* type name of err.cause(py) */)?;
info.set_item("exception", value)?;
```

It returns a `dict` that describes the exception, including the exception object itself.
This shows the main accessors on `PyErr`: `get_type`, `value`, `traceback` and `cause`.

### 10.3 Retry and translate: `retry`

```rust
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
```

* It retries only on `ConnectionError` and its subclasses (`ConnectionRefusedError`,
  `ConnectionResetError`, …).
* Any other exception is re-raised at once (e.g. `PermissionError`).
* When all attempts fail, it raises a new `RuntimeError` with the last
  `ConnectionError` as `__cause__`.

### 10.4 Wrapping a conversion error: `sum_numbers`

```rust
for (i, item) in values.try_iter()?.enumerate() {
    let item = item?;                         // errors raised by the iterator propagate
    total += item.extract::<f64>().map_err(|original| {
        let err = PyTypeError::new_err(format!("item #{i} ({}) is not a number", /* repr */));
        err.set_cause(py, Some(original.into()));
        err
    })?;
}
```

* `values.try_iter()?` raises `TypeError` if the object is not iterable.
* `item?` passes on exceptions raised *inside* a generator (the demo raises
  `ValueError("generator broke")` in a generator, and it arrives unchanged).
* `extract::<f64>()` returns a conversion error. It becomes the `__cause__` of a clearer
  `TypeError` that names the position.

### 10.5 Exception classes defined in Python modules: `import_exception!`

```rust
import_exception!(json, JSONDecodeError);
```

This creates a Rust type for `json.JSONDecodeError`. The module is imported the first
time the type is used. You can then **catch** it:

```rust
match py.import("json")?.call_method1("loads", (text,)) {
    Ok(v) => Ok(v),
    Err(e) if e.is_instance_of::<JSONDecodeError>(py) => Ok(default),
    Err(e) => Err(e),
}
```

…and **raise** it with the arguments the Python class expects
(`JSONDecodeError(msg, doc, pos)`):

```rust
Err(JSONDecodeError::new_err(("single quotes are not valid JSON", text.to_string(), pos)))
```

Python then sees a genuine `json.JSONDecodeError` with `.pos`, `.lineno` and `.colno`.

### 10.6 Warnings: `PyErr::warn`

```rust
PyErr::warn(
    py,
    &py.get_type::<PyDeprecationWarning>(),
    c"old_add() is deprecated, use the + operator",
    1,                       // stacklevel: 1 = report the Python caller's line
)?;
```

* The message is a C string literal (`c"..."`, type `&CStr`).
* Normally the warning is shown or recorded according to the active `warnings` filters,
  and the function continues.
* If the filter is `"error"` (`warnings.simplefilter("error")` or `python -W error`),
  `PyErr::warn` returns `Err`. The `?` then raises the warning **as an exception**
  (`DeprecationWarning` is an `Exception` subclass). Always propagate this `Err` with `?`;
  don't ignore it.

### 10.7 Panics → `PanicException`

```rust
#[pyfunction]
pub fn rust_panic(message: &str) -> i32 { panic!("{message}"); }

#[pyfunction]
pub fn buggy_index(items: Vec<i32>, index: usize) -> i32 { items[index] }   // may panic
```

* PyO3 catches the unwinding panic at the FFI boundary and raises
  `pyo3_runtime.PanicException` with the panic message.
* `PanicException` derives from **`BaseException`**, so `except Exception:` does **not**
  catch it, just like `KeyboardInterrupt` and `SystemExit`. A bug should not be hidden
  by generic error handling.
* The class is not importable by default; this module exposes it as
  `pyexc.PanicException`:

  ```rust
  m.add("PanicException", py.get_type::<pyo3::panic::PanicException>())?;
  ```
* The panic message is also printed to stderr by Rust's panic hook.
* With `panic = "abort"` in the Cargo profile, there is nothing to catch and the whole
  Python process terminates. Keep the default `unwind`.

**Rule:** use `PyResult` for errors a caller can expect and handle. A panic should only
ever mean a bug.

---

## 11. The demo script (`python/demo.py`)

The script is organised in 14 sections. Each prints the results and `assert`s the
expected behaviour.

| # | Section | Rust used | Python technique shown |
|---|---|---|---|
| 1 | Basic `try`/`except`/`else`/`finally` | `divide` | flow through all four blocks |
| 2 | Built-in exceptions raised in Rust | `get_item`, `lookup`, `ensure_str`, `not_ready`, `multiply_i32` | catching a specific type, `e.args` of `KeyError` |
| 3 | Automatic `std` error conversion | `parse_int`, `parse_float`, `to_u8`, `read_text`, `decode_utf8` | `FileNotFoundError` caught as `OSError` |
| 4 | Errors from argument conversion | `divide`, `to_u8` | `TypeError`/`OverflowError` raised before Rust runs |
| 5 | Exception hierarchy | `raise_builtin` | order of clauses, base classes, tuple `except (A, B)` |
| 6 | Custom hierarchy | `Account`, `create_exception!` types | `__mro__`, specific before general, unpacking `e.args`, Python subclass of a Rust exception |
| 7 | Context manager | `Transaction` | commit, rollback and propagate, rollback and suppress |
| 8 | Exception with attributes | `ValidationError`, `validate_user` | `e.field`, `e.reason`, `str`/`repr`, raising it from Python |
| 9 | Chaining and notes | `load_config`, `parse_int` | `__cause__`, `__notes__`, `traceback.format_exception`, `raise … from` |
| 10 | Exception groups | `validate_users` | `except*`, `group.exceptions` |
| 11 | Python exceptions through Rust | `safe_call`, `inspect_exception`, `retry`, `sum_numbers` | identity of re-raised exceptions, traceback frames, retry, generators |
| 12 | Python module exceptions | `parse_json_or`, `strict_json_check` | catching `json.JSONDecodeError` raised in Rust |
| 13 | Warnings | `old_add` | `warnings.catch_warnings(record=True)`, `simplefilter("error")` |
| 14 | Panics | `rust_panic`, `buggy_index` | `PanicException` is not caught by `except Exception` |

Shortened output:

```text
=== 6. Custom exception hierarchy defined in Rust (create_exception!) =
  MRO of InsufficientFundsError: InsufficientFundsError -> BankError -> Exception -> BaseException -> object
  withdraw 500 -> InsufficientFunds: short by 400.00
  deposit -5   -> InvalidAmountError: invalid amount -5: must be positive and finite
  locked       -> AccountLockedError: account of Alice is locked
  self-transfer -> BankError: cannot transfer money to the same account

=== 10. ExceptionGroup and except* (several errors at once) ===========
  3 validation error(s)
    - name   must not be empty                   ['record #1 ("")']
    - age    -3 is not between 0 and 150         ['record #2 ("Eve")']
    - email  'eve.example.com' has no '@'        ['record #2 ("Eve")']

=== 14. Rust panics -> PanicException (a BaseException!) ==============
  PanicException MRO: PanicException -> BaseException -> object
  caught PanicException: something went badly wrong
  caught PanicException: index out of bounds: the len is 3 but the index is 7
```

---

## 12. Python exception handling guide

For a Python script, exceptions raised in Rust are ordinary exceptions. This section
summarises the handling techniques the demo uses.

### 12.1 The full `try` statement

```python
try:
    result = pyexc.divide(10.0, b)
except ZeroDivisionError as e:      # runs if this type (or a subclass) was raised
    print("error:", e)
else:                               # runs only if no exception was raised
    print("result:", result)
finally:                            # always runs – cleanup goes here
    print("done")
```

Keep the `try` block small. Code in `else` is not protected, so an unexpected exception
there is not silently caught by the `except` clause.

### 12.2 Order of `except` clauses

Python checks the clauses from top to bottom and runs the **first** that matches
(`isinstance(e, cls)`). Put specific classes first:

```python
try:
    acc.withdraw(500)
except pyexc.InsufficientFundsError:   # specific
    ...
except pyexc.BankError:                # general – catches the other subclasses
    ...
```

If `BankError` came first, the `InsufficientFundsError` clause would never run.

### 12.3 Catching a family via the base class

| `except` | catches |
|---|---|
| `LookupError` | `KeyError`, `IndexError` |
| `ArithmeticError` | `ZeroDivisionError`, `OverflowError`, `FloatingPointError` |
| `OSError` | `FileNotFoundError`, `PermissionError`, `IsADirectoryError`, `ConnectionError`, … |
| `ValueError` | `UnicodeDecodeError`, `json.JSONDecodeError`, … |
| `pyexc.BankError` | `InsufficientFundsError`, `AccountLockedError`, `InvalidAmountError` |
| `Exception` | almost everything, but **not** `PanicException`, `KeyboardInterrupt`, `SystemExit` |
| `BaseException` | everything |

### 12.4 Several types in one clause

```python
except (TypeError, ValueError) as e:
    print(type(e).__name__, e)
```

### 12.5 Getting information out of the exception

```python
except pyexc.InsufficientFundsError as e:
    str(e)                       # message (str(e) == str(e.args[0]) for one argument)
    e.args                       # all constructor arguments: (msg, balance, requested)
    type(e).__name__             # 'InsufficientFundsError'
    e.__traceback__              # traceback object

except pyexc.ValidationError as e:
    e.field, e.reason            # typed attributes from the Rust struct
```

Note that the `as e` variable is deleted at the end of the `except` block. Assign it to
another name if you need it later.

### 12.6 Re-raising and translating

```python
try:
    pyexc.parse_int(text)
except ValueError as e:
    log.warning("bad input %r", text)
    raise                               # re-raise the same exception, traceback intact

try:
    pyexc.parse_int(text)
except ValueError as e:
    raise AppError("invalid user input") from e     # chain: AppError.__cause__ is e

try:
    pyexc.parse_int(text)
except ValueError:
    raise AppError("invalid user input") from None  # hide the original (no chain shown)
```

### 12.7 Chains and notes

```python
except pyexc.ConfigError as e:
    e.__cause__        # FileNotFoundError (set in Rust with set_cause)
    e.__context__      # exception that was being handled when this one was raised
    e.__notes__        # ['hint: check the path and file permissions']
```

Use `__cause__` for explicit chains (`raise … from`, `set_cause`). `__context__` is set
automatically when an exception is raised inside an `except` block.

### 12.8 Printing or logging the traceback

```python
import traceback, logging

except Exception as e:
    traceback.print_exc()                          # print current exception to stderr
    text = "".join(traceback.format_exception(e))  # as a string (3.10+ signature)
    logging.exception("operation failed")          # log with traceback
```

### 12.9 Exception groups and `except*`

```python
try:
    pyexc.validate_users(users)
except* pyexc.ValidationError as group:     # group is an ExceptionGroup
    for exc in group.exceptions:            # only the matching exceptions
        print(exc.field, exc.reason, exc.__notes__)
except* TypeError as group:                 # several except* clauses can run
    ...
```

* Each `except*` clause receives a **sub-group** with only the matching exceptions.
* Several `except*` clauses can run for the same group.
* Unmatched exceptions are re-raised in a new group.
* `except ExceptionGroup as g:` (without `*`) catches the whole group at once.

### 12.10 Context managers

```python
with pyexc.Transaction(acc) as a:       # __enter__ returns the account
    a.withdraw(70)
    a.withdraw(70)                      # raises → __exit__ rolls back, exception propagates
```

`with` gives you guaranteed cleanup like `try`/`finally`. The context manager can also
decide to suppress the exception. The Python standard library has
`contextlib.suppress(SomeError)` for simple cases.

### 12.11 Warnings

```python
import warnings

with warnings.catch_warnings(record=True) as caught:
    warnings.simplefilter("always")
    pyexc.old_add(2, 3)
caught[0].category, caught[0].message, caught[0].lineno

with warnings.catch_warnings():
    warnings.simplefilter("error")      # warnings become exceptions
    pyexc.old_add(2, 3)                 # raises DeprecationWarning
```

From the command line: `python -W error python/demo.py` or
`python -W error::DeprecationWarning …`.

### 12.12 Panics

```python
try:
    pyexc.buggy_index([1, 2, 3], 7)
except Exception:
    ...                                  # NOT reached
except pyexc.PanicException as e:        # or: except BaseException
    print("bug in the extension:", e)
```

Catch `PanicException` only at the top level of an application (to log it and exit
cleanly) or in tests. Treat it as a bug to fix, not as an error to handle.

---

## 13. Exception hierarchy reference

Exceptions used in this project (`*` = defined in `pyexc`):

```text
BaseException
├── PanicException *            (pyo3_runtime, raised for Rust panics)
├── Exception
│   ├── ArithmeticError
│   │   ├── OverflowError
│   │   └── ZeroDivisionError
│   ├── LookupError
│   │   ├── IndexError
│   │   └── KeyError
│   ├── OSError
│   │   ├── ConnectionError
│   │   │   ├── ConnectionRefusedError
│   │   │   └── ConnectionResetError
│   │   ├── FileNotFoundError
│   │   ├── IsADirectoryError
│   │   └── PermissionError
│   ├── RuntimeError
│   │   └── NotImplementedError
│   ├── TypeError
│   ├── ValueError
│   │   ├── UnicodeError
│   │   │   └── UnicodeDecodeError
│   │   └── json.JSONDecodeError
│   ├── Warning
│   │   └── DeprecationWarning
│   ├── BankError *
│   │   ├── InsufficientFundsError *
│   │   ├── AccountLockedError *
│   │   └── InvalidAmountError *
│   ├── ConfigError *
│   ├── ValidationError *        (#[pyclass(extends = PyException)])
│   └── ExceptionGroup           (also a BaseExceptionGroup)
├── KeyboardInterrupt
└── SystemExit
```

---

## 14. Python API reference of `pyexc`

### Module attributes

| Name | Value |
|---|---|
| `__version__` | `"0.1.0"` (from `Cargo.toml`) |

### Exception classes

| Class | Base | Defined with | Notes |
|---|---|---|---|
| `BankError` | `Exception` | `create_exception!` | base of all banking errors |
| `InsufficientFundsError` | `BankError` | `create_exception!` | `args == (msg, balance, requested)` |
| `AccountLockedError` | `BankError` | `create_exception!` | |
| `InvalidAmountError` | `BankError` | `create_exception!` | |
| `ConfigError` | `Exception` | `create_exception!` | usually has `__cause__` |
| `ValidationError(field, reason)` | `Exception` | `#[pyclass(extends = PyException)]` | attributes `.field`, `.reason` |
| `PanicException` | `BaseException` | PyO3 | Rust panic |

### Functions – built-in exceptions (`builtin.rs`)

| Signature | Returns | Raises |
|---|---|---|
| `divide(a: float, b: float)` | `float` | `ZeroDivisionError` |
| `get_item(items: list[int], index: int)` | `int` | `IndexError` |
| `lookup(table: dict[str, int], key: str)` | `int` | `KeyError(key)` |
| `ensure_str(obj)` | `str` | `TypeError` |
| `not_ready()` | – | `NotImplementedError` |
| `raise_builtin(kind: str, message: str)` | – | the named exception; `ValueError` for unknown kinds |
| `parse_int(text: str)` | `int` | `ValueError` |
| `parse_float(text: str)` | `float` | `ValueError` |
| `to_u8(value: int)` | `int` (0–255) | `ValueError`, `OverflowError` (beyond i64) |
| `read_text(path: str)` | `str` | `FileNotFoundError`, `PermissionError`, `IsADirectoryError`, … |
| `decode_utf8(data: bytes)` | `str` | `UnicodeDecodeError` |
| `multiply_i32(a: int, b: int)` | `int` | `OverflowError` |

### Classes – banking (`bank.rs`)

| Member | Returns | Raises |
|---|---|---|
| `Account(owner: str, balance: float = 0.0)` | `Account` | `InvalidAmountError` |
| `.owner` / `.balance` / `.locked` | `str` / `float` / `bool` | – |
| `.deposit(amount: float)` | new balance | `InvalidAmountError`, `AccountLockedError` |
| `.withdraw(amount: float)` | new balance | `InvalidAmountError`, `AccountLockedError`, `InsufficientFundsError` |
| `.transfer(other: Account, amount: float)` | `None` | any `BankError` |
| `.lock()` / `.unlock()` | `None` | – |
| `Transaction(account: Account, suppress: bool = False)` | context manager | – |
| `.rolled_back` | `bool` – whether the last `with` block was rolled back | – |
| `.error` | `str \| None` – `"Type: message"` of the exception seen by `__exit__` | – |

### Functions – validation (`validation.rs`)

| Signature | Returns | Raises |
|---|---|---|
| `validate_user(name: str, age: int, email: str)` | `str` | `ValidationError` (first problem) |
| `validate_users(users: list[tuple[str, int, str]])` | number of users | `ExceptionGroup` of `ValidationError`s, each with a note |
| `load_config(path: str)` | `list[tuple[str, str]]` | `ConfigError` with `__cause__` (`OSError` subclass or `ValueError`) |

Rules of `validate_user(s)`: the name must not be empty or only whitespace, the age must
be between 0 and 150, and the email must contain `@`.

`load_config` file format: `key = value` per line; empty lines and lines starting with `#`
are skipped.

### Functions – catching (`catching.rs`)

| Signature | Returns | Raises |
|---|---|---|
| `safe_call(func, *args)` | `func(*args)`, or `None` if it raised `ZeroDivisionError` | any other exception from `func`, unchanged |
| `inspect_exception(func)` | `dict` (`type`, `message`, `args`, `is_lookup_error`, `has_traceback`, `cause`, `exception`) or `None` | – |
| `retry(func, attempts: int)` | `func()` result | `RuntimeError` (cause: last `ConnectionError`), or any non-connection error at once |
| `sum_numbers(iterable)` | `float` | `TypeError` (cause: original), or errors raised by the iterator |
| `parse_json_or(text: str, default=None)` | parsed JSON or `default` | – |
| `strict_json_check(text: str)` | `None` | `json.JSONDecodeError` if `text` contains `'` |
| `old_add(a: int, b: int)` | `a + b` | warns `DeprecationWarning`; raises it under filter `"error"` |
| `rust_panic(message: str)` | – | `PanicException` |
| `buggy_index(items: list[int], index: int)` | `int` | `PanicException` if out of range |

---

## 15. PyO3 exception API cheat sheet

### Creating and raising

| Goal | Code |
|---|---|
| raise built-in | `Err(PyValueError::new_err("msg"))` |
| several args | `PyValueError::new_err((msg, 1, 2))` |
| no args | `PyStopIteration::new_err(())` |
| any exception class | `PyErr::new::<T, _>(args)` |
| from an exception object | `PyErr::from_value(obj)` |
| define simple class | `create_exception!(module, Name, Base, "doc");` |
| class with attributes | `#[pyclass(extends = PyException)] struct …` |
| Python-defined class | `import_exception!(module, Name);` |
| own error type with `?` | `impl From<MyError> for PyErr { … }` |
| inline conversion | `.map_err(\|e\| PyValueError::new_err(e.to_string()))?` |
| `Option` → exception | `.ok_or_else(\|\| PyKeyError::new_err(key))?` |
| warning | `PyErr::warn(py, &py.get_type::<PyUserWarning>(), c"msg", 1)?` |
| expose class to Python | `m.add("Name", py.get_type::<Name>())?` |

### Inspecting a `PyErr`

| Goal | Code |
|---|---|
| type check (with subclasses) | `err.is_instance_of::<PyKeyError>(py)` |
| `except`-style match (type or tuple) | `err.matches(py, exc_type)?` |
| exception class | `err.get_type(py)` → `Bound<PyType>` |
| exception object | `err.value(py)` → `&Bound<PyBaseException>` |
| take ownership of object | `err.into_value(py)` → `Py<PyBaseException>` |
| traceback | `err.traceback(py)` → `Option<Bound<PyTraceback>>` |
| traceback as text | `err.traceback(py).map(\|t\| t.format())` (needs `PyTracebackMethods`, in the prelude) |
| cause | `err.cause(py)` / `err.set_cause(py, Some(other))` |
| print like Python | `err.print(py)` |
| message | `err.value(py).str()?` or `err.to_string()` |

### Lower-level (rarely needed)

| Goal | Code |
|---|---|
| take the currently set error | `PyErr::take(py)` → `Option<PyErr>` |
| set it as current error | `err.restore(py)` |
| check whether an error is set | `PyErr::occurred(py)` |

---

## 16. Pitfalls and best practices

1. **Use `PyResult` for expected errors and never panic on user input.** `unwrap()`,
   `expect()` and slice indexing like `v[i]` panic → `PanicException`, which `except
   Exception` does not catch. Use `get()`, `checked_*`, `ok_or_else`, `?`.
2. **Use the right exception class.** Use `TypeError` for a wrong type, `ValueError`
   for a right type with a wrong value, `KeyError`/`IndexError` for missing entries, and
   `OSError` subclasses for the system. Python users then know what to catch.
3. **Give each library a base exception** (`BankError`), so users can catch all your
   errors with one clause and still be specific when they need to.
4. **Keep business logic Python-free.** Return your own error enum and convert in one
   `From<MyError> for PyErr`. This makes the logic testable with `cargo test`.
5. **Chain instead of losing information.** Translating a low-level exception into a
   high-level one? Use `set_cause` so the original stays in the traceback.
6. **Re-raise with `Err(err)`, not with a new exception.** That keeps the original object
   and its traceback.
7. **Register the exception types in the module.** A `create_exception!` type that is not
   added with `m.add(...)` can be raised, but Python code cannot name it in `except`.
8. **Use `try_borrow_mut` where aliasing is possible.** A `#[pyclass]` argument can be
   the same object as `self`. `borrow_mut()` would panic; `try_borrow_mut()` returns an
   error you can map.
9. **Don't swallow exceptions in `__exit__`** unless they are expected; return
   `Ok(false)` for everything else.
10. **Propagate `PyErr::warn` with `?`.** If it returns `Err`, a filter turned the warning
    into an exception, and you have to pass it on.
11. **`errno` is not set** on `OSError`s converted from `std::io::Error`. Check the
    exception class instead.
12. **No conversion for `std::str::Utf8Error`.** Use `String::from_utf8` or `map_err`.
13. **Error messages:** start with a lower-case letter, no trailing period, and include
    the offending value (`invalid amount -5: must be positive`), following Python's own
    style.

---

## 17. Extending the example

### Add another exception to the bank hierarchy

```rust
create_exception!(pyexc, DailyLimitError, BankError, "Daily withdrawal limit exceeded.");

// in BankFailure:     DailyLimit { limit: f64 },
// in Display:         Self::DailyLimit { limit } => write!(f, "daily limit of {limit:.2} exceeded"),
// in From<..>:        BankFailure::DailyLimit { limit } => DailyLimitError::new_err((msg, limit)),
// in register():      m.add("DailyLimitError", py.get_type::<DailyLimitError>())?;
```

### Inherit from a built-in exception other than `Exception`

```rust
create_exception!(pyexc, NegativeValueError, pyo3::exceptions::PyValueError, "…");
```

Then `except ValueError` catches it as well. This is useful when your error is a special
case of a standard one.

### Use `thiserror` for the Rust error enum

```toml
[dependencies]
thiserror = "2"
```

```rust
#[derive(Debug, thiserror::Error)]
pub enum BankFailure {
    #[error("invalid amount {0}: must be positive and finite")]
    InvalidAmount(f64),
    // …
}
```

You still write the `From<BankFailure> for PyErr` impl yourself.

### Use `anyhow` in application-style code

PyO3's optional feature `anyhow` adds `From<anyhow::Error> for PyErr`. It creates a
`RuntimeError`, unless the `anyhow::Error` just wraps a `PyErr`, which is passed through. There is a similar `eyre` feature. This is convenient but loses the
exception type, so prefer your own enum for library APIs.

### Exception attributes with `create_exception!` style simplicity

If you only need a few extra values, pass a tuple (`new_err((msg, a, b))`) and read
`e.args` in Python, as `InsufficientFundsError` does. Use a pyclass exception once you
want named attributes.

---

## 18. Troubleshooting

| Problem | Cause | Fix |
|---|---|---|
| `maturin develop` fails with *"Both VIRTUAL_ENV and CONDA_PREFIX are set"* or installs to the wrong environment | Anaconda base environment is active | `unset CONDA_PREFIX` before `maturin develop` |
| `cargo build` fails in `pyo3-ffi` build script | PyO3 cannot find a suitable Python | `PYO3_PYTHON=$PWD/.venv/bin/python cargo build`, or use `maturin develop` |
| test binary: `libpython3.12.so.1.0: cannot open shared object file` | libpython not on the library path | `LD_LIBRARY_PATH=$HOME/anaconda3/lib` when **running** the test binary |
| `cargo: … libssl.so.3: version OPENSSL_3.2.0 not found` | `LD_LIBRARY_PATH` set for the `cargo` command itself | set it only for the test binary (see §4) |
| `ModuleNotFoundError: No module named 'pyexc'` | not built into the active venv | `source .venv/bin/activate && maturin develop` |
| Changes in Rust have no effect | old build still installed | run `maturin develop` again |
| `SyntaxError` at `except*` | Python < 3.11 | use Python ≥ 3.11 |
| `AttributeError: module 'pyexc' has no attribute 'XError'` | exception type not added to the module | `m.add("XError", py.get_type::<XError>())?` |
| `` `?` couldn't convert the error to `PyErr` `` | no `From<E> for PyErr` for that error type | `map_err(...)` or implement `From` for your own type |
| `RuntimeError: Already borrowed` | a `#[pyclass]` borrowed twice (e.g. `self` passed as an argument) | `try_borrow_mut()` and map the error (see `transfer`) |
| `except Exception` does not catch an error from the module | it is a `PanicException` (a Rust panic) | fix the bug; catch `BaseException` only at top level |
| stderr shows `thread '<unnamed>' panicked at …` | Rust's panic hook prints the panic before PyO3 converts it | expected for panics; fix the panic or set a custom hook |
