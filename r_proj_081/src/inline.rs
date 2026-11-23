//! Python code embedded **as a string literal directly in the Rust source**
//! and executed with [`Python::run`].
//!
//! In contrast to `analytics.py` (a separate file pulled in with
//! `include_str!` and loaded as a module), the scripts here are written inline
//! with Rust's C-string literals (`c"..."` / `cr#"..."#`). Data is exchanged
//! through a `globals` dictionary:
//!
//! * Rust puts input variables into the dict before running the script,
//! * the script runs as top-level statements and assigns result variables,
//! * Rust reads the results back from the same dict.

use std::ffi::{CStr, CString};

use pyo3::exceptions::{PyKeyError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyDict;

/// Inline script #1: plain statements working on variables provided by Rust.
///
/// Inputs (set by Rust): `items` = list of `(name, quantity, unit_price)`,
/// `tax_rate` = float. Outputs (read by Rust): `subtotal`, `tax`, `total`,
/// `lines`.
///
/// Python is indentation sensitive, so the code starts at column 0 inside the
/// raw string. `cr#"..."#` is a *raw C-string literal*: no escaping needed and
/// the compiler appends the terminating NUL byte.
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

/// Inline script #2: the string only *defines* a function; Rust looks it up
/// in the globals dict afterwards and calls it like any Python callable.
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

/// Result of [`run_invoice_script`].
#[derive(Debug, Clone, PartialEq)]
pub struct Invoice {
    pub lines: Vec<String>,
    pub subtotal: f64,
    pub tax: f64,
    pub total: f64,
}

/// Reads a variable the script assigned, failing with `KeyError` if the
/// script did not define it.
fn get_var<'py, T: for<'a> FromPyObject<'a, 'py>>(
    globals: &Bound<'py, PyDict>,
    name: &str,
) -> PyResult<T> {
    globals
        .get_item(name)?
        .ok_or_else(|| PyKeyError::new_err(format!("script did not define '{name}'")))?
        .extract()
        .map_err(Into::into)
}

/// Runs [`INVOICE_SCRIPT`] with Rust-provided inputs and returns its results.
pub fn run_invoice_script(items: &[(&str, u32, f64)], tax_rate: f64) -> PyResult<Invoice> {
    Python::attach(|py| {
        // 1. Inputs: a fresh namespace, so every run starts clean.
        let globals = PyDict::new(py);
        globals.set_item("items", items.to_vec())?; // -> list[tuple[str, int, float]]
        globals.set_item("tax_rate", tax_rate)?;

        // 2. Execute the statements. `locals = None` means "use globals",
        //    so all top-level assignments end up in `globals`.
        py.run(INVOICE_SCRIPT, Some(&globals), None)?;

        // 3. Outputs.
        Ok(Invoice {
            lines: get_var(&globals, "lines")?,
            subtotal: get_var(&globals, "subtotal")?,
            tax: get_var(&globals, "tax")?,
            total: get_var(&globals, "total")?,
        })
    })
}

/// Runs [`FIBONACCI_SCRIPT`] to define `fibonacci`, then calls it from Rust.
pub fn fibonacci(n: i64) -> PyResult<Vec<u64>> {
    Python::attach(|py| {
        let globals = PyDict::new(py);
        py.run(FIBONACCI_SCRIPT, Some(&globals), None)?;

        let function = globals
            .get_item("fibonacci")?
            .ok_or_else(|| PyKeyError::new_err("fibonacci not defined"))?;
        function.call1((n,))?.extract()
    })
}

/// Runs arbitrary Python statements given as a Rust string and returns
/// everything the code printed with `print(...)`.
///
/// Instead of redirecting the process-wide `sys.stdout` (shared by all
/// threads), a private `print` is placed into the script's globals that
/// writes into an `io.StringIO` buffer. That shadows the built-in `print`
/// only for this script.
///
/// **Never** pass untrusted input: the code runs with full permissions.
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

#[cfg(test)]
mod tests {
    use super::*;
    use pyo3::exceptions::{PyNameError, PyOverflowError};

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn invoice_script_computes_totals() {
        let invoice = run_invoice_script(&[("Coffee", 2, 3.5), ("Cake", 1, 4.25)], 0.19).unwrap();
        assert!(approx(invoice.subtotal, 11.25));
        assert!(approx(invoice.tax, 2.14));
        assert!(approx(invoice.total, 13.39));
        assert_eq!(
            invoice.lines,
            vec![
                "  2 x Coffee            7.00".to_string(),
                "  1 x Cake              4.25".to_string(),
            ]
        );
    }

    #[test]
    fn invoice_script_handles_no_items() {
        let invoice = run_invoice_script(&[], 0.19).unwrap();
        assert!(invoice.lines.is_empty());
        assert_eq!(
            (invoice.subtotal, invoice.tax, invoice.total),
            (0.0, 0.0, 0.0)
        );
    }

    #[test]
    fn invoice_script_raises_for_negative_tax() {
        let err = run_invoice_script(&[("X", 1, 1.0)], -0.1).unwrap_err();
        Python::attach(|py| assert!(err.is_instance_of::<PyValueError>(py)));
        assert!(err.to_string().contains("tax_rate must not be negative"));
    }

    #[test]
    fn invoice_runs_do_not_share_state() {
        let first = run_invoice_script(&[("A", 1, 10.0)], 0.0).unwrap();
        let second = run_invoice_script(&[("B", 1, 1.0)], 0.0).unwrap();
        assert!(approx(first.total, 10.0));
        assert!(
            approx(second.total, 1.0),
            "subtotal must not leak between runs"
        );
        assert_eq!(second.lines.len(), 1);
    }

    #[test]
    fn fibonacci_function_defined_in_string_is_callable() {
        assert_eq!(fibonacci(0).unwrap(), Vec::<u64>::new());
        assert_eq!(
            fibonacci(10).unwrap(),
            vec![0, 1, 1, 2, 3, 5, 8, 13, 21, 34]
        );
    }

    #[test]
    fn fibonacci_large_values_use_python_big_ints() {
        // fib(93) still fits into u64, fib(94) does not -> OverflowError on extract.
        assert_eq!(
            *fibonacci(94).unwrap().last().unwrap(),
            12_200_160_415_121_876_738
        );
        let err = fibonacci(95).unwrap_err();
        Python::attach(|py| assert!(err.is_instance_of::<PyOverflowError>(py)));
    }

    #[test]
    fn fibonacci_negative_input_raises() {
        let err = fibonacci(-1).unwrap_err();
        Python::attach(|py| assert!(err.is_instance_of::<PyValueError>(py)));
    }

    #[test]
    fn captured_output_contains_prints() {
        let out = run_capturing_output("for i in range(3):\n    print('line', i)").unwrap();
        assert_eq!(out, "line 0\nline 1\nline 2\n");
    }

    #[test]
    fn captured_output_supports_print_keywords() {
        let out = run_capturing_output("print('a', 'b', sep='-', end='!')").unwrap();
        assert_eq!(out, "a-b!");
    }

    #[test]
    fn captured_output_is_empty_without_prints() {
        assert_eq!(run_capturing_output("x = 1 + 1").unwrap(), "");
    }

    #[test]
    fn captured_output_propagates_errors() {
        let name_err = run_capturing_output("print(undefined)").unwrap_err();
        let syntax_err = run_capturing_output("def broken(:").unwrap_err();
        let nul_err = run_capturing_output("print(1)\0").unwrap_err();
        Python::attach(|py| {
            assert!(name_err.is_instance_of::<PyNameError>(py));
            assert_eq!(
                syntax_err.get_type(py).name().unwrap().to_string(),
                "SyntaxError"
            );
            assert!(nul_err.is_instance_of::<PyValueError>(py));
        });
    }
}
