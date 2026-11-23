# embedpy – Embedding a Python Script into a Rust Program with PyO3

This project (`r_proj_081`) shows how to **ship a Python script inside a Rust executable**
and run it with the CPython interpreter through **PyO3 0.30**.

* The Python source (`python/analytics.py`) is compiled **into the binary** with
  `include_str!`. The `.py` file is not needed at run time.
* At run time the source is turned into a real Python module with
  `PyModule::from_code` and cached, so it runs only once.
* Small, typed Rust functions wrap the Python functions and the Python class. Callers
  only see Rust types (`&str`, `&[f64]`, `Vec<(String, usize)>`, `Option<f64>`, a Rust
  `struct`) and `PyResult`.
* A second variant (`src/inline.rs`, [section 16](#16-python-code-embedded-as-a-string-srcinliners))
  writes the Python code **directly as a string literal in the Rust source** and runs
  it with `py.run`. Data goes in and out through a `globals` dictionary.
* 30 Rust unit tests check every wrapper, including error handling and threading.

This is the opposite of `r_proj_080`: there Rust code was compiled into a Python
**extension module** that Python imports. Here a Rust **program** embeds Python.

---

## Table of contents

1. [Quick start](#1-quick-start)
2. [Project layout](#2-project-layout)
3. [How the embedding works](#3-how-the-embedding-works)
4. [Configuration (`Cargo.toml`)](#4-configuration-cargotoml)
5. [The embedded script (`python/analytics.py`)](#5-the-embedded-script-pythonanalyticspy)
6. [The Rust library (`src/lib.rs`)](#6-the-rust-library-srclibrs)
7. [The demo program (`src/main.rs`)](#7-the-demo-program-srcmainrs)
8. [Type conversions between Rust and Python](#8-type-conversions-between-rust-and-python)
9. [Error handling](#9-error-handling)
10. [Tests](#10-tests)
11. [Ways to run Python code: comparison](#11-ways-to-run-python-code-comparison)
12. [Pitfalls and best practices](#12-pitfalls-and-best-practices)
13. [Extending the example](#13-extending-the-example)
14. [Troubleshooting](#14-troubleshooting)
15. [PyO3 API cheat sheet](#15-pyo3-api-cheat-sheet)
16. [Python code embedded as a string (`src/inline.rs`)](#16-python-code-embedded-as-a-string-srcinliners)

---

## 1. Quick start

Prerequisites:

* Rust (edition 2024, tested with cargo 1.99)
* Python 3 with its shared library (`libpython3.x.so`) installed, tested with Python 3.12.
  On Debian/Ubuntu this is `python3-dev`. With pyenv, build Python with
  `PYTHON_CONFIGURE_OPTS="--enable-shared"`.

```bash
cargo run      # run the demo
cargo test     # run the 30 unit tests
cargo clippy --all-targets
```

Demo output:

```text
Embedded analytics.py version 1.0.0

Top words: [("the", 4), ("dog", 2), ("fox", 2)]

Summary {
    count: 5,
    mean: 15.1,
    median: 15.75,
    stdev: Some(
        4.467241878385365,
    ),
    min: 9.25,
    max: 21.0,
}

Statistics
======================
mean             15.10
median           15.75
max              21.00

TemperatureLog(location='Stuttgart', readings=3) -> average Some(7.25) °C, in °F [38.3, 44.6, 52.25]
Area of a circle with r = 2: 12.5664

Expected error: StatisticsError: describe() requires at least one value
Expected error: ValueError: -500.0 °C is below absolute zero

--- Inline Python scripts ---
  2 x Coffee            7.00
  1 x Cake              4.25
subtotal 11.25, tax 2.14, total 13.39
Fibonacci: [0, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89]
Captured output: Hello from Python 3.12.3
```

To choose which Python PyO3 links against, set `PYO3_PYTHON` before building:

```bash
PYO3_PYTHON=/usr/bin/python3.12 cargo build
```

---

## 2. Project layout

```text
r_proj_081/
├── Cargo.toml            # package, lib + bin targets, pyo3 dependency
├── DOC.md                # this document
├── python/
│   └── analytics.py      # the Python script that gets embedded
└── src/
    ├── lib.rs            # library "embedpy": loading + typed wrappers + tests
    ├── inline.rs         # module embedpy::inline: Python code as Rust string literals + tests
    └── main.rs           # demo binary that uses the library
```

The code is split into a **library** (`embedpy`) and a **binary** so that:

* the wrappers can be tested with `cargo test` (unit tests live in `lib.rs`),
* other Rust crates could reuse the wrappers,
* `main.rs` only demonstrates how to use them.

---

## 3. How the embedding works

```text
 compile time                                   run time
 ────────────                                   ────────
 python/analytics.py                            first call of analytics_module(py)
        │ include_str!                                   │
        ▼                                                ▼
 &'static str ──c_str!──► SCRIPT_SOURCE: &CStr ──► PyModule::from_code(...)
        (stored in the binary)                           │  compile + execute
                                                         ▼
                                              Bound<PyModule> "analytics"
                                                         │ unbind()
                                                         ▼
                                     static ANALYTICS: PyOnceLock<Py<PyModule>>
                                                         │ later calls: bind(py)
                                                         ▼
                              getattr("describe")?.call1((...))?.extract()?
```

Step by step:

1. **Include the script.** `include_str!("../python/analytics.py")` puts the file
   contents into the binary as a `&'static str`. The path is relative to the source
   file that contains the macro (`src/lib.rs`). Cargo rebuilds the crate whenever the
   script changes.
2. **Make it a C string.** CPython's API takes NUL-terminated strings, so PyO3 0.30's
   `from_code`, `run` and `eval` take `&CStr`. `pyo3::ffi::c_str!` appends `"\0"` with
   `concat!` and checks **at compile time** that the text has no interior NUL byte.
3. **Start the interpreter.** The `auto-initialize` feature makes the first
   `Python::attach` start CPython automatically. `Python::attach` (called `with_gil`
   in older PyO3 versions) attaches the current thread to the interpreter and gives a
   `Python<'py>` token that proves it is attached.
4. **Create the module.** `PyModule::from_code(py, code, file_name, module_name)`
   compiles the source and executes it in a new module namespace. That runs
   imports and defines functions, classes and constants. The `file_name` shows up in
   tracebacks, and the module is registered in `sys.modules["analytics"]`.
5. **Cache it.** The resulting `Bound<'py, PyModule>` only lives while the thread is
   attached. `unbind()` turns it into a `Py<PyModule>` that is independent of the
   interpreter lock and can be stored in a `static PyOnceLock`. Later calls `bind(py)`
   it again.
6. **Call into Python.** Every wrapper does
   `getattr(name)` → `call1(args)` → `extract::<RustType>()`.

---

## 4. Configuration (`Cargo.toml`)

```toml
[package]
name = "r_proj_081"
version = "0.1.0"
edition = "2024"

[lib]
name = "embedpy"          # crate name used in main.rs: `use embedpy::...`
path = "src/lib.rs"

[[bin]]
name = "r_proj_081"
path = "src/main.rs"

[dependencies.pyo3]
version = "0.30.1"
features = ["auto-initialize"]
```

| Setting | Why |
|---|---|
| `[lib] name = "embedpy"` | Gives the library a short name, separate from the package name. |
| `features = ["auto-initialize"]` | Starts the interpreter on the first `Python::attach`. Without it you would call `Python::initialize()` yourself. |
| **no** `extension-module` feature | That feature is only for Python extension modules (`cdylib`). It stops PyO3 from linking `libpython`, so an embedding program would fail to link. |

---

## 5. The embedded script (`python/analytics.py`)

The script is ordinary Python and can be run or tested on its own
(`python3 -c "import sys; sys.path.insert(0,'python'); import analytics"`). It contains:

| Python object | Kind | Shows |
|---|---|---|
| `SCRIPT_VERSION = "1.0.0"` | module constant | reading a Python attribute |
| `word_frequencies(text, top_n=5)` | function | `str`/`int` args → `list[tuple[str, int]]` |
| `describe(numbers)` | function | `list[float]` → `dict`; raises `StatisticsError` |
| `render_report(title, rows)` | function | list of tuples → formatted `str` |
| `TemperatureLog(location)` | class | stateful object owned by Rust, methods, `__len__`, `__repr__` |

Details:

* `word_frequencies` sorts with key `(-count, word)`, so ties come out alphabetically
  and the result is always the same, which the tests rely on.
* `describe` uses the standard library `statistics` module. An empty sequence raises
  `statistics.StatisticsError`, and `stdev` is `None` for fewer than two values (Python's
  `statistics.stdev` needs at least two).
* `TemperatureLog.add` raises `ValueError` for values below −273.15 °C, so you can see a
  Python exception coming back from a method call.

> The script imports only the standard library. If you import third-party packages
> (e.g. `numpy`), they have to be installed in the Python environment the program runs
> with, because they are **not** embedded. See [Troubleshooting](#14-troubleshooting).

---

## 6. The Rust library (`src/lib.rs`)

### 6.1 Embedding constants

```rust
pub const SCRIPT_SOURCE: &CStr = c_str!(include_str!("../python/analytics.py"));
const SCRIPT_FILE: &CStr = c"analytics.py";
const MODULE_NAME: &CStr = c"analytics";
```

`c"..."` is Rust's C-string literal (since Rust 1.77). It cannot be combined with
`include_str!`, which is why `c_str!` is used for the source.

### 6.2 Loading and caching the module

```rust
static ANALYTICS: PyOnceLock<Py<PyModule>> = PyOnceLock::new();

pub fn analytics_module(py: Python<'_>) -> PyResult<&Bound<'_, PyModule>> {
    ANALYTICS
        .get_or_try_init(py, || {
            PyModule::from_code(py, SCRIPT_SOURCE, SCRIPT_FILE, MODULE_NAME).map(Bound::unbind)
        })
        .map(|module| module.bind(py))
}
```

* `PyOnceLock` (called `GILOnceCell` before PyO3 0.26) is a one-time cell designed for
  Python objects. It avoids deadlocks between Rust's own locks and the interpreter.
* `get_or_try_init` runs the closure only on the first successful call. If the script
  raises while loading, the error is returned and the next call tries again.
* Without the cache, every `from_code` call would run the whole script again and
  create a new module object, so state like class identity would not survive between
  calls.

### 6.3 Function wrappers

All wrappers follow the same pattern:

```rust
pub fn word_frequencies(text: &str, top_n: usize) -> PyResult<Vec<(String, usize)>> {
    Python::attach(|py| {
        analytics_module(py)?               // &Bound<PyModule>
            .getattr("word_frequencies")?   // Bound<PyAny> (the function object)
            .call1((text, top_n))?          // positional args as a Rust tuple
            .extract()                      // Python list[tuple] -> Vec<(String, usize)>
    })
}
```

* Arguments are passed as a **Rust tuple**. A single argument needs a trailing comma:
  `call1((numbers.to_vec(),))`.
* `call1` passes positional arguments only. For keyword arguments use
  `call((args...), Some(&kwargs_dict))`. `call0` takes no arguments.
* `extract()` checks the type at run time. If Python returns something else, you get a
  `TypeError` as a `PyErr` instead of undefined behaviour.

| Rust function | Python call | Rust return type |
|---|---|---|
| `script_version()` | `analytics.SCRIPT_VERSION` | `PyResult<String>` |
| `word_frequencies(text, top_n)` | `analytics.word_frequencies(text, top_n)` | `PyResult<Vec<(String, usize)>>` |
| `describe(numbers)` | `analytics.describe(list)` | `PyResult<Summary>` |
| `render_report(title, rows)` | `analytics.render_report(title, rows)` | `PyResult<String>` |
| `evaluate_expression(expr, vars)` | `eval(expr, globals)` | `PyResult<f64>` |

### 6.4 Converting a `dict` into a Rust struct

`describe` returns a Python `dict`, which becomes a Rust `Summary`:

```rust
let result = analytics_module(py)?.getattr("describe")?.call1((numbers.to_vec(),))?;
let dict = result.cast::<PyDict>()?;          // type check: is it really a dict?

fn field<'py, T: for<'a> FromPyObject<'a, 'py>>(dict: &Bound<'py, PyDict>, key: &str)
    -> PyResult<T>
{
    dict.get_item(key)?                                    // PyResult<Option<Bound<PyAny>>>
        .ok_or_else(|| PyValueError::new_err(format!("missing key '{key}'")))?
        .extract()
        .map_err(Into::into)
}

Ok(Summary { count: field(dict, "count")?, stdev: field(dict, "stdev")?, /* ... */ })
```

* `cast::<PyDict>()` (called `downcast` before PyO3 0.27) checks the Python type.
* `get_item` returns `Ok(None)` for a missing key, and the helper turns that into a
  `ValueError` with a clear message.
* `stdev: Option<f64>` extracts Python `None` as Rust `None`.
* `FromPyObject` has two lifetimes in PyO3 0.30 (`'a` for the borrowed input, `'py` for
  the interpreter). `map_err(Into::into)` turns the extraction error into a `PyErr`.

You could also use `#[derive(FromPyObject)]` on `Summary` with
`#[pyo3(item("count"))]` field attributes. The manual version here shows every step
explicitly.

### 6.5 Evaluating an expression with Rust variables

```rust
pub fn evaluate_expression(expression: &str, variables: &[(&str, f64)]) -> PyResult<f64> {
    let code = CString::new(expression)
        .map_err(|e| PyValueError::new_err(format!("expression contains NUL byte: {e}")))?;
    Python::attach(|py| {
        let globals = PyDict::new(py);
        globals.set_item("analytics", analytics_module(py)?)?;
        globals.set_item("math", py.import("math")?)?;
        for (name, value) in variables {
            globals.set_item(*name, *value)?;
        }
        py.eval(&code, Some(&globals), None)?.extract()
    })
}
```

* A string built at run time has to be turned into a `CString` first, which fails if it
  contains `\0`. That failure becomes a `ValueError`.
* A fresh `globals` dict sandboxes the **namespace** (each call starts clean and does not
  touch `__main__`). It is **not** a security sandbox: `__import__('os').system(...)`
  still works. Never evaluate untrusted input.
* `py.eval` accepts one expression only. For statements use `py.run`, see
  [section 11](#11-ways-to-run-python-code-comparison).

### 6.6 Owning a Python object from Rust: `TemperatureLog`

```rust
pub struct TemperatureLog {
    inner: Py<PyAny>,
}

impl TemperatureLog {
    pub fn new(location: &str) -> PyResult<Self> {
        Python::attach(|py| {
            let class = analytics_module(py)?.getattr("TemperatureLog")?;
            let instance = class.call1((location,))?;     // calling the class = constructor
            Ok(Self { inner: instance.unbind() })
        })
    }

    pub fn add(&self, celsius: f64) -> PyResult<()> {
        Python::attach(|py| self.inner.bind(py).call_method1("add", (celsius,)).map(drop))
    }

    pub fn len(&self) -> PyResult<usize> {
        Python::attach(|py| self.inner.bind(py).len())   // Python len() -> __len__
    }
    // average(), to_fahrenheit(), location(), repr(), is_empty() ...
}
```

* `Py<T>` is a reference-counted pointer to a Python object that does not need the
  thread to be attached. It is `Send + Sync`, so `TemperatureLog` can be moved to
  another thread (see the test `temperature_log_can_be_used_from_another_thread`).
* `bind(py)` borrows it as a `Bound<'py, PyAny>` for the duration of a call.
* When the Rust struct is dropped, the Python reference count goes down. If the thread
  is not attached at that moment, PyO3 queues the decrement and does it later.
* `call_method0/1` call methods, `getattr` reads attributes (`location`), and
  `len()`/`repr()` use Python's protocols.

---

## 7. The demo program (`src/main.rs`)

`main` returns `PyResult<()>`, so `?` works directly on every wrapper. It runs the
wrappers in this order:

1. `word_frequencies` on a short sentence,
2. `describe` → `Summary` printed with `{:#?}`,
3. `render_report` with values from the summary,
4. a `TemperatureLog` with three readings,
5. `evaluate_expression("math.pi * r ** 2", r = 2)`,
6. two expected errors (`StatisticsError`, `ValueError`) caught and printed,
7. the inline scripts from `src/inline.rs`: invoice, Fibonacci, captured `print` output
   (see [section 16](#16-python-code-embedded-as-a-string-srcinliners)).

Each wrapper attaches to the interpreter on its own. Attaching again on the same thread
is cheap, and nested `Python::attach` calls are allowed.

---

## 8. Type conversions between Rust and Python

Conversions used in this project:

| Rust → Python (`IntoPyObject`) | Python → Rust (`FromPyObject`) |
|---|---|
| `&str`, `String` → `str` | `str` → `String` |
| `usize`, `i64` → `int` | `int` → `usize` (fails with `OverflowError` if negative) |
| `f64` → `float` | `float` *or* `int` → `f64` |
| `Vec<f64>` → `list[float]` | `list`/`tuple` → `Vec<T>` |
| `Vec<(&str, f64)>` → `list[tuple[str, float]]` | `tuple` → `(A, B)` |
| `(a, b)` → `tuple` (used as call arguments) | `None` → `Option::None`, value → `Some(v)` |
| `&Bound<PyModule>` → the module itself | `dict` → `Bound<PyDict>` via `cast` |

Notes:

* Extracting a Python `str` into `Vec<T>` is deliberately refused by PyO3, so a string
  is not silently split into characters.
* A `&[f64]` slice would also become a `list` (only `&[u8]` becomes `bytes`).
  `numbers.to_vec()` gives an owned `Vec<f64>`, so the argument tuple does not borrow
  from the caller.
* Every conversion makes a **copy**. For large numeric arrays use the `numpy` crate
  (`rust-numpy`) to share buffers instead.

---

## 9. Error handling

Any Python exception (raised while loading the module, in a function or method, during
`eval`, or during `extract`) becomes a `pyo3::PyErr` inside `PyResult`.

Inspecting an error:

```rust
let err = describe(&[]).unwrap_err();
Python::attach(|py| {
    err.is_instance_of::<PyValueError>(py);       // isinstance check, includes subclasses
    err.get_type(py).name()?;                     // "StatisticsError"
    err.value(py);                                // the exception object
    err.traceback(py);                            // Option<Bound<PyTraceback>>
    err.display(py);                              // print it like Python does (to stderr)
    Ok::<_, PyErr>(())
});
println!("{err}");                                // "StatisticsError: describe() requires ..."
```

* `StatisticsError` has no built-in PyO3 type, but it is a subclass of `ValueError`, so
  `is_instance_of::<PyValueError>` works. Compare `get_type(py).name()` for the exact
  class.
* `SyntaxError` in an `eval` string is reported only when the code runs, not at compile
  time.
* Errors created in Rust (`PyValueError::new_err(...)`) look the same as Python errors
  to the caller.
* Tracebacks point to `analytics.py`, line N, because `SCRIPT_FILE` was passed to
  `from_code`.

To use the wrappers in an application that uses `anyhow`/`thiserror`, convert at the
boundary. `PyErr` implements `std::error::Error`, so `?` into `anyhow::Result` works.

---

## 10. Tests

The tests are in `src/lib.rs` (19 tests, listed below) and `src/inline.rs` (11 tests,
listed in [section 16.7](#167-tests-of-the-inline-module)), each under
`#[cfg(test)] mod tests`. Run them with `cargo test`.

| Test | Checks |
|---|---|
| `embedded_source_is_not_empty_and_contains_functions` | `include_str!` really embedded the script |
| `module_is_loaded_once_and_cached` | same object pointer on repeated calls; `__name__ == "analytics"` |
| `script_version_is_read` | reading a module constant |
| `word_frequencies_counts_case_insensitively` | counting, case folding, ordering |
| `word_frequencies_breaks_ties_alphabetically` | deterministic tie ordering |
| `word_frequencies_respects_top_n_and_empty_input` | `top_n = 0`, empty text |
| `describe_computes_statistics` | mean / median / stdev / min / max values |
| `describe_single_value_has_no_stdev` | Python `None` → Rust `None` |
| `describe_empty_input_raises_statistics_error` | exception type, base class, message |
| `render_report_formats_table` | text layout returned by Python |
| `render_report_without_rows` | edge case |
| `evaluate_expression_uses_variables_and_math` | variables injected via `globals` |
| `evaluate_expression_can_call_embedded_module` | embedded module usable inside `eval` |
| `evaluate_expression_propagates_python_errors` | `ZeroDivisionError`, `NameError`, `SyntaxError`, `TypeError` on extract |
| `evaluate_expression_rejects_nul_bytes` | `CString` failure → `ValueError` |
| `temperature_log_keeps_state_between_calls` | object state, methods, attributes, `repr` |
| `temperature_log_rejects_values_below_absolute_zero` | exception from a method; state unchanged |
| `temperature_logs_are_independent_instances` | two Rust handles = two Python objects |
| `temperature_log_can_be_used_from_another_thread` | `Py<T>` is `Send`; attach from another thread |

Notes on testing with an embedded interpreter:

* `cargo test` runs tests in **parallel threads of one process**, so they all share
  one interpreter. Each test attaches through `Python::attach`, which handles this
  safely. Tests must not rely on global Python state that other tests change (e.g.
  `sys.path`).
* The interpreter cannot be shut down and restarted within a process, so the module
  cache stays alive for the whole test run, which is what we want.
* Exact float comparisons use a small tolerance (`approx`) except where the values are
  exactly representable (`50.0, 68.0, 86.0`).

Useful commands:

```bash
cargo test                          # all tests
cargo test describe                 # tests whose name contains "describe"
cargo test -- --nocapture           # show println! output
cargo test -- --test-threads=1      # run tests one after another
```

---

## 11. Ways to run Python code: comparison

| API | Input | Returns | Use for |
|---|---|---|---|
| `py.eval(code, globals, locals)` | one **expression** | `Bound<PyAny>` (the value) | small calculations, `evaluate_expression` |
| `py.run(code, globals, locals)` | **statements** | `()`; results are written into `globals`/`locals` | inline scripts that assign variables (`src/inline.rs`, section 16) |
| `PyModule::from_code(py, code, file, name)` | a whole **module** | `Bound<PyModule>` | embedded scripts with functions and classes (`analytics.py`) |
| `py.import("name")` | module on `sys.path` | `Bound<PyModule>` | installed packages, stdlib, `.py` files on disk |

Example with `py.run` getting a value back from statements:

```rust
Python::attach(|py| -> PyResult<()> {
    let globals = PyDict::new(py);
    py.run(c"total = sum(range(10))\nmsg = f'total={total}'", Some(&globals), None)?;
    let msg: String = globals.get_item("msg")?.unwrap().extract()?;
    assert_eq!(msg, "total=45");
    Ok(())
})
```

**Embed vs. load from disk:**

| | string literal in `.rs` + `run` (section 16) | `include_str!` + `from_code` (embedded) | `py.import` / read file at run time |
|---|---|---|---|
| Deployment | single binary | single binary, script cannot go missing | `.py` file has to be shipped next to the binary |
| Changing the script | rebuild needed | rebuild needed | edit and restart, no rebuild |
| Editor support for the Python code | none (it is a Rust string) | full (it is a `.py` file) | full |
| Testable with `pytest` on its own | no | yes | yes |
| Good for | short snippets, glue code | larger scripts, functions, classes | plugins, user scripts |
| Python itself | needs `libpython` + stdlib on the target machine | same | same |

A common compromise: embed a default script, and load an override from disk if one
exists.

---

## 12. Pitfalls and best practices

1. **Cache modules.** Calling `from_code` on every call runs the script again each time
   (slow, and module-level state is reset). Use a `PyOnceLock<Py<PyModule>>`.
2. **Don't keep `Bound<'py, T>` beyond `attach`.** It is tied to the `'py` lifetime.
   Store `Py<T>` and `bind(py)` again when needed. The compiler enforces this.
3. **Keep attached sections short.** Only one thread runs Python bytecode at a time.
   Do heavy Rust work outside `Python::attach`, or use `py.detach(|| ...)`
   (`allow_threads` in older versions) to let other threads run Python meanwhile.
4. **Watch out for `Python::attach` vs. Rust locks.** Holding a `Mutex` while attaching,
   while another thread holds the interpreter and waits for that `Mutex`, deadlocks.
   That is why `PyOnceLock` is used instead of `std::sync::OnceLock` around Python
   initialisation.
5. **Untrusted code.** `eval`/`run`/`from_code` execute arbitrary code with the
   permissions of your process. A custom `globals` dict is **not** a sandbox.
6. **Don't use `extension-module` in embedding programs.** It removes the link to
   `libpython` and the binary fails to link.
7. **One interpreter per process.** It cannot be finalised and restarted. Design tests
   and libraries for that.
8. **Typed wrappers.** Keep `getattr("...")` strings in one place (the wrapper layer) so
   the rest of the Rust code is free of Python details and typos fail in tests.
9. **Keep the script testable on its own.** Plain Python without Rust-specific hacks
   can also be tested with `pytest`.

---

## 13. Extending the example

* **Keyword arguments:**
  ```rust
  let kwargs = [("top_n", 3)].into_py_dict(py)?;
  module.getattr("word_frequencies")?.call((text,), Some(&kwargs))?;
  ```
* **Pass a Rust callback to Python:** wrap a closure with
  `PyCFunction::new_closure(py, None, None, |args, _kwargs| { ... })` and pass it as an
  argument. Python can then call back into Rust.
* **Expose Rust functions as a module the script can `import`:** create a module with
  `#[pymodule]` and register it before loading the script with
  `pyo3::append_to_inittab!(my_rust_module)` (must happen before the interpreter starts).
  `analytics.py` can then use `import my_rust_module`.
* **Several embedded scripts:** one `const` + one `PyOnceLock` per script, or a
  `HashMap<&str, &CStr>` registry with a generic loader.
* **Hot reload during development:** under `#[cfg(debug_assertions)]` read the script with
  `std::fs::read_to_string`, turn it into a `CString`, and call `from_code` without
  caching.
* **Use the virtual environment:** before the first `attach`, set `PYTHONHOME` /
  `PYTHONPATH` or add the venv's `site-packages` to `sys.path` from Rust.

---

## 14. Troubleshooting

| Symptom | Cause / fix |
|---|---|
| `error while loading shared libraries: libpython3.12.so.1.0` | The runtime linker can't find libpython. Install `python3-dev`, or set `LD_LIBRARY_PATH` to the directory that contains it (pyenv/conda). |
| Link error `undefined reference to Py...` | `extension-module` feature enabled, or Python built without `--enable-shared`. |
| Wrong Python version is used | Set `PYO3_PYTHON=/path/to/python3` and run `cargo clean -p pyo3-build-config`. |
| `ModuleNotFoundError: No module named 'numpy'` | Third-party imports come from the runtime environment. Install them there or activate the venv / set `PYTHONPATH`. |
| `Fatal Python error: init_fs_encoding ... No module named 'encodings'` | The interpreter can't find its standard library. Set `PYTHONHOME` to the Python prefix. |
| ``TypeError: Can't extract `str` to `Vec` `` | Trying to extract a Python `str` into `Vec<T>`. Extract `String` instead. |
| `OverflowError` on extract | A negative Python `int` extracted into `usize`, or a value too large for the Rust type. |
| Script changes not visible | `include_str!` embeds at compile time. Rebuild (`cargo build`). |

---

## 15. PyO3 API cheat sheet

```rust
use pyo3::prelude::*;
use pyo3::ffi::c_str;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyModule, IntoPyDict};

// interpreter access
Python::attach(|py| { /* ... */ });                 // attach thread, get token
py.detach(|| { /* pure Rust work */ });             // let other threads run Python

// running code
PyModule::from_code(py, c_str!(include_str!("x.py")), c"x.py", c"x")?;
py.import("math")?;
py.eval(c"1 + 1", None, None)?;
py.run(c"x = 1", Some(&globals), None)?;

// objects
obj.getattr("name")?;            obj.setattr("name", value)?;
obj.call0()?;  obj.call1((a, b))?;  obj.call((a,), Some(&kwargs))?;
obj.call_method1("m", (a,))?;    obj.len()?;  obj.repr()?;  obj.str()?;
obj.extract::<T>()?;             obj.cast::<PyDict>()?;
let owned: Py<PyAny> = obj.unbind();   let b = owned.bind(py);

// dicts
let d = PyDict::new(py);  d.set_item("k", 1)?;  d.get_item("k")?;   // -> Option
let d = [("k", 1)].into_py_dict(py)?;

// errors
PyValueError::new_err("msg");    err.is_instance_of::<PyValueError>(py);
err.get_type(py).name()?;        err.value(py);   err.display(py);

// one-time init
static CELL: PyOnceLock<Py<PyModule>> = PyOnceLock::new();
CELL.get_or_try_init(py, || -> PyResult<_> { Ok(module.unbind()) })?;
```

Names that changed in recent PyO3 versions (useful when reading older examples):

| Old | PyO3 0.30 |
|---|---|
| `Python::with_gil` | `Python::attach` |
| `py.allow_threads` | `py.detach` |
| `GILOnceCell` | `PyOnceLock` |
| `downcast` / `downcast_into` | `cast` / `cast_into` |
| `PyObject` | `Py<PyAny>` |
| `from_code(py, &str, &str, &str)` | `from_code(py, &CStr, &CStr, &CStr)` |

---

## 16. Python code embedded as a string (`src/inline.rs`)

The `analytics.py` example keeps Python in its own file. For short snippets it is often
simpler to write the Python code **directly in the Rust source** as a string literal and
run it with `Python::run`. The module `embedpy::inline` shows three patterns:

| Rust function | Pattern |
|---|---|
| `run_invoice_script(items, tax_rate) -> PyResult<Invoice>` | inputs into `globals` → run statements → read result variables |
| `fibonacci(n) -> PyResult<Vec<u64>>` | the string **defines a function**; Rust fetches it from `globals` and calls it |
| `run_capturing_output(code) -> PyResult<String>` | runs any code string and returns what it `print`ed |

### 16.1 Writing Python inside Rust string literals

```rust
pub const INVOICE_SCRIPT: &CStr = cr#"
if tax_rate < 0:
    raise ValueError(f"tax_rate must not be negative, got {tax_rate}")

lines = []
subtotal = 0.0
for name, quantity, unit_price in items:
    amount = quantity * unit_price
    subtotal += amount
    lines.append(f"{quantity:>3} x {name:<12} {amount:>9.2f}")

subtotal = round(subtotal, 2)
tax = round(subtotal * tax_rate, 2)
total = round(subtotal + tax, 2)
"#;
```

Which literal to use:

| Literal | Type | When |
|---|---|---|
| `c"..."` | `&'static CStr` | short code without `"` or `\` (escapes like `\n` are processed by Rust) |
| `cr#"..."#` | `&'static CStr` | **multi-line code with quotes, f-strings or backslashes**: raw, nothing escaped |
| `"..."` / `r#"..."#` | `&'static str` | when the code has to be built or changed at run time; convert with `CString::new` |

Rules:

* **Indentation matters.** Python sees the string exactly as written, so the code has to
  start at column 0 even though it sits inside indented Rust code. A leading empty line
  (right after `cr#"`) does no harm.
* The compiler adds the terminating NUL byte to `c"..."`/`cr"..."` and rejects interior
  NUL bytes at compile time. `CString::new` checks this at run time instead (see
  `run_capturing_output`).
* `{` and `}` have no special meaning in these literals. They are only special inside
  `format!`, so **don't build Python code with `format!`**: f-string braces would have to
  be doubled, and putting values into code invites injection bugs. Pass values through
  `globals` instead.

### 16.2 Data exchange through `globals`

```rust
pub fn run_invoice_script(items: &[(&str, u32, f64)], tax_rate: f64) -> PyResult<Invoice> {
    Python::attach(|py| {
        // 1. inputs
        let globals = PyDict::new(py);
        globals.set_item("items", items.to_vec())?;   // list[tuple[str, int, float]]
        globals.set_item("tax_rate", tax_rate)?;

        // 2. execute; locals = None -> top-level assignments go into globals
        py.run(INVOICE_SCRIPT, Some(&globals), None)?;

        // 3. outputs
        Ok(Invoice {
            lines: get_var(&globals, "lines")?,
            subtotal: get_var(&globals, "subtotal")?,
            tax: get_var(&globals, "tax")?,
            total: get_var(&globals, "total")?,
        })
    })
}
```

```text
 Rust                       globals dict                 Python script
 ────                       ────────────                 ─────────────
 set_item("items", ..)  ──► items, tax_rate        ──►  reads items, tax_rate
                                                        assigns lines, subtotal, ...
 get_var("total")       ◄── lines, subtotal, tax,  ◄──
                            total, amount, name, ...
```

* **A fresh `PyDict` for every run** keeps runs independent. Variables of one run (e.g.
  `subtotal`) cannot leak into the next. The test `invoice_runs_do_not_share_state`
  checks this. With `globals = None`, the code would run in `__main__`, and all runs
  (and threads) would share and overwrite the same variables.
* **`locals = None`** makes Python use `globals` for local variables too, the same as a
  normal module. If you pass a separate `locals` dict, top-level assignments go there,
  and functions defined in the snippet cannot see each other's names, because function
  bodies look up names in `globals`. Use one dict unless you have a reason not to.
* `__builtins__` is added automatically by PyO3 if the dict doesn't contain it, so
  `round`, `range`, `print` etc. work.
* `get_var` turns a missing variable into a clear `KeyError: script did not define '...'`
  instead of a panic.
* Helper variables (`amount`, `name`, `quantity`) also end up in `globals`. Rust just
  ignores them.

### 16.3 Defining a Python function in a string and calling it from Rust

```rust
pub const FIBONACCI_SCRIPT: &CStr = c"
def fibonacci(n):
    if n < 0:
        raise ValueError('n must not be negative')
    result, a, b = [], 0, 1
    for _ in range(n):
        result.append(a)
        a, b = b, a + b
    return result
";

pub fn fibonacci(n: i64) -> PyResult<Vec<u64>> {
    Python::attach(|py| {
        let globals = PyDict::new(py);
        py.run(FIBONACCI_SCRIPT, Some(&globals), None)?;      // only defines the function
        let function = globals
            .get_item("fibonacci")?
            .ok_or_else(|| PyKeyError::new_err("fibonacci not defined"))?;
        function.call1((n,))?.extract()                       // call it like any callable
    })
}
```

* Running a `def` statement only creates the function object and binds it to the name
  `fibonacci` in `globals`. Nothing is computed yet.
* The function object can be kept (`function.unbind()` → `Py<PyAny>`) and called many
  times. Here the script is simply run again on each call for clarity. To define it
  once, cache it in a `PyOnceLock` as `analytics_module` does.
* **Big integers:** Python `int` has no size limit, Rust `u64` does. `fibonacci(94)`
  still fits (the largest value is F(93) = 12 200 160 415 121 876 738), but `fibonacci(95)`
  makes `extract::<Vec<u64>>()` fail with `OverflowError`. The test
  `fibonacci_large_values_use_python_big_ints` checks this boundary.

### 16.4 Capturing `print` output

```rust
pub fn run_capturing_output(code: &str) -> PyResult<String> {
    let code = CString::new(code)
        .map_err(|e| PyValueError::new_err(format!("code contains NUL byte: {e}")))?;
    Python::attach(|py| {
        let buffer = py.import("io")?.getattr("StringIO")?.call0()?;

        // print = functools.partial(builtins.print, file=buffer)
        let kwargs = PyDict::new(py);
        kwargs.set_item("file", &buffer)?;
        let builtin_print = py.import("builtins")?.getattr("print")?;
        let captured_print = py
            .import("functools")?
            .getattr("partial")?
            .call((builtin_print,), Some(&kwargs))?;

        let globals = PyDict::new(py);
        globals.set_item("print", captured_print)?;
        py.run(&code, Some(&globals), None)?;

        buffer.call_method0("getvalue")?.extract()
    })
}
```

* Python looks up names in `globals` before `__builtins__`, so a `print` entry in
  `globals` **shadows** the built-in `print` for this snippet only.
* `functools.partial(print, file=buffer)` behaves exactly like `print` (`sep=`, `end=`,
  several arguments), but always writes into the `StringIO` buffer.
* **Why not replace `sys.stdout`?** `sys.stdout` (also through
  `contextlib.redirect_stdout`) is **process-wide**. With several threads (for example
  `cargo test`, which runs tests in parallel) output from other threads would end up in
  the buffer, or the buffer would be swapped out under a running snippet. Shadowing
  `print` only affects this snippet. Its limit: output written another way
  (`sys.stdout.write`, C extensions, functions defined in other modules that call the
  real `print`) is not captured. In that case redirect `sys.stdout` and make sure only
  one thread runs such code at a time.
* The code comes from a run-time `&str`, so `CString::new` is required, and a NUL byte
  becomes a `ValueError` instead of a panic.
* **Security:** this function runs any code it is given. Never pass user input.

### 16.5 Errors in inline code

Errors behave the same as in the module-based example:

| Situation | Result |
|---|---|
| `raise ValueError(...)` in the script | `Err(PyErr)`, `is_instance_of::<PyValueError>` |
| Undefined name | `NameError` |
| Broken syntax (`def broken(:`) | `SyntaxError`, raised by `py.run` before any statement runs |
| Variable missing after the run | `KeyError` from `get_var` |
| Result too large / wrong type for the Rust target | `OverflowError` / `TypeError` from `extract` |

Tracebacks show the file name `<string>` (that is what `py.run` uses), and line numbers
count from the start of the literal, including the leading empty line of `cr#"\n...`.
If you need a real file name in tracebacks, use `PyModule::from_code` with the inline
string instead (see 16.6).

### 16.6 `run` with a string vs. `from_code` with a string

Inline strings also work with `PyModule::from_code`, which `analytics.py` uses:

```rust
const GREETER: &CStr = c"def greet(name):\n    return f'Hello, {name}!'\n";
let module = PyModule::from_code(py, GREETER, c"greeter.py", c"greeter")?;
let text: String = module.getattr("greet")?.call1(("Rust",))?.extract()?;
```

| | `py.run(code, Some(&globals), None)` | `PyModule::from_code(py, code, file, name)` |
|---|---|---|
| Namespace | your own `PyDict` | a new module object |
| Inputs before execution | yes: put them into `globals` | no (module code runs immediately) |
| Results | read variables from the dict | `getattr` on the module |
| Name in tracebacks | `<string>` | the given file name |
| Registered in `sys.modules` | no | yes, so other Python code can `import` it |
| Typical use | one-off scripts with inputs and outputs | reusable library code, cached once |

### 16.7 Tests of the inline module

All in `src/inline.rs` → `mod tests` (run only these with `cargo test inline`):

| Test | Checks |
|---|---|
| `invoice_script_computes_totals` | inputs via globals, numeric results, formatted lines |
| `invoice_script_handles_no_items` | empty list edge case |
| `invoice_script_raises_for_negative_tax` | `raise` in inline code → `ValueError` + message |
| `invoice_runs_do_not_share_state` | fresh `globals` per run, no leaking variables |
| `fibonacci_function_defined_in_string_is_callable` | function defined by `run`, called from Rust |
| `fibonacci_large_values_use_python_big_ints` | F(93) fits in `u64`; one more → `OverflowError` |
| `fibonacci_negative_input_raises` | exception raised inside the Python function |
| `captured_output_contains_prints` | multi-line code, loop output captured |
| `captured_output_supports_print_keywords` | `sep=` / `end=` still work with the shadowed `print` |
| `captured_output_is_empty_without_prints` | no output → empty string |
| `captured_output_propagates_errors` | `NameError`, `SyntaxError`, NUL byte → `ValueError` |
