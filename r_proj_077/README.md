# r_proj_077 – Python ↔ Rust bindings with PyO3

A broad example of a native Python module (`rustypy`) written in Rust with
[PyO3](https://pyo3.rs) 0.30 and built with [maturin](https://www.maturin.rs).

## Build & run

```bash
python3 -m venv .venv
source .venv/bin/activate
pip install maturin
unset CONDA_PREFIX            # only needed if Anaconda is active as well
maturin develop --release     # compiles and installs `rustypy` into the venv
python python/demo.py         # runs every example and checks the results
```

`maturin build --release` creates a wheel (`target/wheels/*.whl`) to install anywhere.
Rust unit tests: `PYO3_PYTHON=.venv/bin/python cargo test --lib`.

## Layout

| File                 | Topic |
|----------------------|-------|
| `src/lib.rs`         | `#[pymodule]` entry point, module constants |
| `src/scalars.rs`     | `int` (i8…u128, overflow checks), `float`, `bool`, `str`/`char`, `bytes`, `complex`, `Option`/`None` |
| `src/collections.rs` | `list` ↔ `Vec`, fixed arrays, nested lists, in-place `PyList` changes, tuples, `dict` ↔ `HashMap`/`BTreeMap`, `PyDict`, `set` ↔ `HashSet` |
| `src/classes.rs`     | `#[pyclass]`: properties, getters/setters, operators (`+ - * / neg abs bool`), `__eq__`/`__hash__`, pickle, `@staticmethod`/`@classmethod`, container protocol (`len`, `[]`, `in`, `del`, iteration), inheritance (Rust and Python subclasses) |
| `src/enums.rs`       | C-like enum (`Color.Red`, `int()`), data-carrying enum (`Shape.Circle(radius=…)`) working with `match` |
| `src/errors.rs`      | `Result` → exceptions, custom exception hierarchy (`RustyError` → `ParseError`, `LimitError`), `From<MyError> for PyErr`, catching Python exceptions |
| `src/dynamic.rs`     | positional-only / keyword-only / default args, `*args`/`**kwargs`, `Bound<PyAny>` type checks, `#[derive(FromPyObject)]` union types and attribute/dict extraction, `#[derive(IntoPyObject)]`, Python callbacks, using the Python stdlib, releasing the GIL with `py.detach` |
| `python/demo.py`     | Python script that uses and checks all of the above |

## Type mapping cheat sheet

| Python                | Rust argument                          | Rust return value |
|-----------------------|----------------------------------------|-------------------|
| `int`                 | `i64`, `u8`, `u128`, …                 | same |
| `float`               | `f64`, `f32`                           | same |
| `bool`                | `bool`                                 | `bool` |
| `str`                 | `&str`, `String`, `char`               | `String`, `&'static str`, `char` |
| `bytes`               | `&[u8]`, `Vec<u8>`                     | `Cow<[u8]>`, `Bound<PyBytes>` (`Vec<u8>` returns a `list`!) |
| `complex`             | `Complex64` (feature `num-complex`)    | `Complex64` |
| `None`                | `Option<T>`                            | `Option<T>`, `()` |
| `list` / sequence     | `Vec<T>`, `[T; N]`, `Bound<PyList>`    | `Vec<T>`, `[T; N]` |
| `tuple`               | `(A, B, …)`, `Bound<PyTuple>`          | `(A, B, …)` |
| `dict`                | `HashMap<K,V>`, `BTreeMap<K,V>`, `Bound<PyDict>` | same |
| `set` / `frozenset`   | `HashSet<T>`, `BTreeSet<T>`            | `HashSet<T>` |
| any object            | `Bound<PyAny>`                         | `Py<PyAny>`, `Bound<PyAny>` |
| own class             | `&T`, `T: Clone`, `PyRef<T>`, `PyRefMut<T>` | `T` |
