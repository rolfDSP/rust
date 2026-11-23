# r_proj_080 – Python exceptions with PyO3

A native Python module (`pyexc`) written in Rust with [PyO3](https://pyo3.rs) 0.30 and
built with [maturin](https://www.maturin.rs). It shows how to **raise**, **define**,
**chain** and **catch** Python exceptions from Rust. `python/demo.py` shows how a Python
script **handles** them.

## Build & run

```bash
python3 -m venv .venv
source .venv/bin/activate
pip install maturin
unset CONDA_PREFIX            # only needed if Anaconda is active as well
maturin develop               # compiles and installs `pyexc` into the venv
python python/demo.py         # runs every example and checks the results
```

Requires Python ≥ 3.11 (`ExceptionGroup`, `except*`, `add_note`).

Rust unit tests (the venv's base Python is Anaconda here, so libpython comes from its `lib`):

```bash
PYO3_PYTHON=.venv/bin/python cargo test --lib --no-run
LD_LIBRARY_PATH=$HOME/anaconda3/lib target/debug/deps/pyexc-<hash>
```

## Layout

| File                | Topic |
|---------------------|-------|
| `src/lib.rs`        | `#[pymodule]` entry point |
| `src/builtin.rs`    | Raising built-in exceptions (`ValueError`, `KeyError`, `IndexError`, `ZeroDivisionError`, `OverflowError`, …) and automatic `?` conversion of `std` errors |
| `src/bank.rs`       | Custom hierarchy with `create_exception!`, a Rust error enum + `impl From<BankFailure> for PyErr`, extra exception args, mapping `PyBorrowMutError`, a context manager whose `__exit__` rolls back and can suppress exceptions |
| `src/validation.rs` | `#[pyclass(extends = PyException)]` with attributes, chaining with `set_cause` (`__cause__`), `add_note` (`__notes__`), raising an `ExceptionGroup` |
| `src/catching.rs`   | Catching Python exceptions in Rust (`is_instance_of`), re-raising unchanged, retry logic, wrapping errors, `import_exception!` (`json.JSONDecodeError`), warnings (`PyErr::warn`), panics → `PanicException` |
| `python/demo.py`    | 14 sections of Python handling patterns: `try/except/else/finally`, hierarchy and order of `except` clauses, tuples, `e.args`, `raise … from`, `traceback`, `with`, `except*`, `warnings.catch_warnings`, `BaseException` |

## Cheat sheet

| Goal (Rust side)                         | How |
|------------------------------------------|-----|
| raise a built-in exception               | `Err(PyValueError::new_err("msg"))` |
| several args (`e.args`)                  | `PyKeyError::new_err((msg, a, b))` |
| define a new exception class             | `create_exception!(module, Name, BaseClass, "doc")` and `m.add("Name", py.get_type::<Name>())` |
| exception with typed attributes          | `#[pyclass(extends = PyException)]` + `PyErr::new::<T, _>(args)` |
| own Rust error → exception with `?`      | `impl From<MyError> for PyErr` |
| `raise X from Y`                         | `err.set_cause(py, Some(cause))` |
| add a note                               | `err.value(py).call_method1("add_note", (text,))` |
| raise an existing exception object       | `PyErr::from_value(obj)` |
| check a caught exception                 | `err.is_instance_of::<PyTypeError>(py)` |
| exception object / traceback / cause     | `err.value(py)`, `err.traceback(py)`, `err.cause(py)` |
| exception class from a Python module     | `import_exception!(json, JSONDecodeError)` |
| issue a warning                          | `PyErr::warn(py, &py.get_type::<PyUserWarning>(), c"msg", 1)?` |

Automatic `?` conversions in this example: `ParseIntError`, `ParseFloatError`, and
`TryFromIntError` → `ValueError`; `io::Error` → `FileNotFoundError`, `PermissionError`,
… (message only, `errno` is `None`); `FromUtf8Error` → `UnicodeDecodeError`.
`std::str::Utf8Error` has **no** conversion. A Rust `panic!` becomes
`PanicException`, which derives from `BaseException`, so `except Exception` does **not**
catch it.
