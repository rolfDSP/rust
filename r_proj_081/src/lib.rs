//! # embedpy
//!
//! Embeds the Python script `python/analytics.py` into the compiled Rust
//! binary and executes it with the CPython interpreter via PyO3.
//!
//! * The script source is included at **compile time** (`include_str!`), so
//!   the executable does not need the `.py` file at run time.
//! * On first use the source is compiled into a Python module with
//!   [`PyModule::from_code`] and cached in a [`PyOnceLock`].
//! * Each public function is a typed, safe Rust wrapper around one Python
//!   function / class of the script. Python exceptions come back as
//!   [`PyErr`] inside a [`PyResult`].
//!
//! The [`inline`] module shows the alternative: Python code written as string
//! literals directly in the Rust source and executed with `Python::run`.

pub mod inline;

use std::ffi::{CStr, CString};

use pyo3::exceptions::PyValueError;
use pyo3::ffi::c_str;
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyModule};

/// Source code of the embedded script as a NUL-terminated C string.
///
/// `include_str!` reads the file at compile time, `c_str!` appends the
/// terminating `\0` (and checks at compile time that there is no interior NUL).
pub const SCRIPT_SOURCE: &CStr = c_str!(include_str!("../python/analytics.py"));

/// File name shown in Python tracebacks.
const SCRIPT_FILE: &CStr = c"analytics.py";
/// Name of the module inside Python (`sys.modules["analytics"]`).
const MODULE_NAME: &CStr = c"analytics";

/// Module cache: the script is executed only once per process.
static ANALYTICS: PyOnceLock<Py<PyModule>> = PyOnceLock::new();

/// Returns the (cached) `analytics` module, executing the embedded script on
/// the first call.
pub fn analytics_module(py: Python<'_>) -> PyResult<&Bound<'_, PyModule>> {
    ANALYTICS
        .get_or_try_init(py, || {
            PyModule::from_code(py, SCRIPT_SOURCE, SCRIPT_FILE, MODULE_NAME).map(Bound::unbind)
        })
        .map(|module| module.bind(py))
}

/// Reads the module-level constant `SCRIPT_VERSION`.
pub fn script_version() -> PyResult<String> {
    Python::attach(|py| analytics_module(py)?.getattr("SCRIPT_VERSION")?.extract())
}

/// Calls `analytics.word_frequencies(text, top_n)`.
///
/// Returns the `top_n` most frequent words (case-insensitive) together with
/// their counts, most frequent first, ties sorted alphabetically.
pub fn word_frequencies(text: &str, top_n: usize) -> PyResult<Vec<(String, usize)>> {
    Python::attach(|py| {
        analytics_module(py)?
            .getattr("word_frequencies")?
            .call1((text, top_n))?
            .extract()
    })
}

/// Descriptive statistics returned by [`describe`].
#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    pub count: usize,
    pub mean: f64,
    pub median: f64,
    /// Sample standard deviation; `None` for fewer than two values.
    pub stdev: Option<f64>,
    pub min: f64,
    pub max: f64,
}

/// Calls `analytics.describe(numbers)` and converts the returned `dict`
/// into a [`Summary`].
///
/// # Errors
/// An empty slice makes Python raise `statistics.StatisticsError`
/// (a subclass of `ValueError`), which is returned as `Err(PyErr)`.
pub fn describe(numbers: &[f64]) -> PyResult<Summary> {
    Python::attach(|py| {
        let result = analytics_module(py)?
            .getattr("describe")?
            .call1((numbers.to_vec(),))?;
        let dict = result.cast::<PyDict>()?;

        // Helper: fetch a required key and extract it as `T`.
        fn field<'py, T: for<'a> FromPyObject<'a, 'py>>(
            dict: &Bound<'py, PyDict>,
            key: &str,
        ) -> PyResult<T> {
            dict.get_item(key)?
                .ok_or_else(|| PyValueError::new_err(format!("missing key '{key}'")))?
                .extract()
                .map_err(Into::into)
        }

        Ok(Summary {
            count: field(dict, "count")?,
            mean: field(dict, "mean")?,
            median: field(dict, "median")?,
            stdev: field(dict, "stdev")?, // Python `None` -> Rust `None`
            min: field(dict, "min")?,
            max: field(dict, "max")?,
        })
    })
}

/// Calls `analytics.render_report(title, rows)` and returns the text table.
pub fn render_report(title: &str, rows: &[(&str, f64)]) -> PyResult<String> {
    Python::attach(|py| {
        analytics_module(py)?
            .getattr("render_report")?
            .call1((title, rows.to_vec()))?
            .extract()
    })
}

/// Evaluates a single Python expression with the given variables in scope.
///
/// This shows `Python::eval` with an explicit globals dictionary.
/// **Never** pass untrusted input: the expression can run arbitrary code.
pub fn evaluate_expression(expression: &str, variables: &[(&str, f64)]) -> PyResult<f64> {
    let code = CString::new(expression)
        .map_err(|e| PyValueError::new_err(format!("expression contains NUL byte: {e}")))?;
    Python::attach(|py| {
        let globals = PyDict::new(py);
        // Make the embedded module and `math` available inside the expression.
        globals.set_item("analytics", analytics_module(py)?)?;
        globals.set_item("math", py.import("math")?)?;
        for (name, value) in variables {
            globals.set_item(*name, *value)?;
        }
        py.eval(&code, Some(&globals), None)?.extract()
    })
}

/// Rust handle to an instance of the Python class `analytics.TemperatureLog`.
///
/// The Python object is stored as [`Py<PyAny>`], which is independent of the
/// interpreter lock, so the Rust struct can be kept and moved around freely.
/// Every method attaches to the interpreter only for the duration of the call.
#[derive(Debug)]
pub struct TemperatureLog {
    inner: Py<PyAny>,
}

impl TemperatureLog {
    /// Creates `analytics.TemperatureLog(location)`.
    pub fn new(location: &str) -> PyResult<Self> {
        Python::attach(|py| {
            let class = analytics_module(py)?.getattr("TemperatureLog")?;
            let instance = class.call1((location,))?;
            Ok(Self {
                inner: instance.unbind(),
            })
        })
    }

    /// Calls `log.add(celsius)`. Fails with `ValueError` below absolute zero.
    pub fn add(&self, celsius: f64) -> PyResult<()> {
        Python::attach(|py| {
            self.inner
                .bind(py)
                .call_method1("add", (celsius,))
                .map(drop)
        })
    }

    /// Calls `log.average()`; `None` while the log is empty.
    pub fn average(&self) -> PyResult<Option<f64>> {
        Python::attach(|py| self.inner.bind(py).call_method0("average")?.extract())
    }

    /// Calls `log.to_fahrenheit()`.
    pub fn to_fahrenheit(&self) -> PyResult<Vec<f64>> {
        Python::attach(|py| self.inner.bind(py).call_method0("to_fahrenheit")?.extract())
    }

    /// Calls Python's `len(log)` (dispatches to `__len__`).
    pub fn len(&self) -> PyResult<usize> {
        Python::attach(|py| self.inner.bind(py).len())
    }

    /// `true` if no readings were added yet.
    pub fn is_empty(&self) -> PyResult<bool> {
        self.len().map(|n| n == 0)
    }

    /// Reads the instance attribute `location`.
    pub fn location(&self) -> PyResult<String> {
        Python::attach(|py| self.inner.bind(py).getattr("location")?.extract())
    }

    /// Python's `repr(log)`.
    pub fn repr(&self) -> PyResult<String> {
        Python::attach(|py| Ok(self.inner.bind(py).repr()?.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pyo3::exceptions::{PyNameError, PyTypeError, PyZeroDivisionError};

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    // ---- module loading -------------------------------------------------

    #[test]
    fn embedded_source_is_not_empty_and_contains_functions() {
        let src = SCRIPT_SOURCE.to_str().unwrap();
        assert!(src.contains("def word_frequencies"));
        assert!(src.contains("class TemperatureLog"));
    }

    #[test]
    fn module_is_loaded_once_and_cached() {
        Python::attach(|py| {
            let first = analytics_module(py).unwrap().as_ptr();
            let second = analytics_module(py).unwrap().as_ptr();
            assert_eq!(first, second, "the module must be cached");
            let name: String = analytics_module(py)
                .unwrap()
                .getattr("__name__")
                .unwrap()
                .extract()
                .unwrap();
            assert_eq!(name, "analytics");
        });
    }

    #[test]
    fn script_version_is_read() {
        assert_eq!(script_version().unwrap(), "1.0.0");
    }

    // ---- word_frequencies -----------------------------------------------

    #[test]
    fn word_frequencies_counts_case_insensitively() {
        let result = word_frequencies("Rust rust RUST python Python go", 3).unwrap();
        assert_eq!(
            result,
            vec![("rust".into(), 3), ("python".into(), 2), ("go".into(), 1)]
        );
    }

    #[test]
    fn word_frequencies_breaks_ties_alphabetically() {
        let result = word_frequencies("b a c b a c", 10).unwrap();
        assert_eq!(
            result,
            vec![("a".into(), 2), ("b".into(), 2), ("c".into(), 2)]
        );
    }

    #[test]
    fn word_frequencies_respects_top_n_and_empty_input() {
        assert_eq!(word_frequencies("one two three", 0).unwrap(), vec![]);
        assert_eq!(word_frequencies("", 5).unwrap(), vec![]);
        assert_eq!(word_frequencies("x y z", 2).unwrap().len(), 2);
    }

    // ---- describe -------------------------------------------------------

    #[test]
    fn describe_computes_statistics() {
        let s = describe(&[2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]).unwrap();
        assert_eq!(s.count, 8);
        assert!(approx(s.mean, 5.0));
        assert!(approx(s.median, 4.5));
        assert!(approx(s.stdev.unwrap(), 2.138089935299395));
        assert!(approx(s.min, 2.0));
        assert!(approx(s.max, 9.0));
    }

    #[test]
    fn describe_single_value_has_no_stdev() {
        let s = describe(&[42.0]).unwrap();
        assert_eq!(s.count, 1);
        assert_eq!(s.stdev, None);
        assert!(approx(s.median, 42.0));
    }

    #[test]
    fn describe_empty_input_raises_statistics_error() {
        let err = describe(&[]).unwrap_err();
        Python::attach(|py| {
            // StatisticsError is a subclass of ValueError.
            assert!(err.is_instance_of::<PyValueError>(py));
            let type_name = err.get_type(py).name().unwrap().to_string();
            assert_eq!(type_name, "StatisticsError");
            assert!(err.to_string().contains("at least one value"));
        });
    }

    // ---- render_report --------------------------------------------------

    #[test]
    fn render_report_formats_table() {
        let report = render_report("Sales", &[("North", 1234.5), ("South", 99.0)]).unwrap();
        let lines: Vec<&str> = report.lines().collect();
        assert_eq!(lines[0], "Sales");
        assert!(lines[1].chars().all(|c| c == '='));
        assert!(lines[2].starts_with("North") && lines[2].ends_with("1234.50"));
        assert!(lines[3].starts_with("South") && lines[3].ends_with("99.00"));
    }

    #[test]
    fn render_report_without_rows() {
        assert_eq!(render_report("Empty", &[]).unwrap(), "Empty\n(no data)");
    }

    // ---- evaluate_expression --------------------------------------------

    #[test]
    fn evaluate_expression_uses_variables_and_math() {
        let v = evaluate_expression("math.sqrt(a**2 + b**2)", &[("a", 3.0), ("b", 4.0)]).unwrap();
        assert!(approx(v, 5.0));
    }

    #[test]
    fn evaluate_expression_can_call_embedded_module() {
        let v = evaluate_expression("analytics.describe([1, 2, 3])['mean'] * k", &[("k", 10.0)])
            .unwrap();
        assert!(approx(v, 20.0));
    }

    #[test]
    fn evaluate_expression_propagates_python_errors() {
        let zero = evaluate_expression("1 / x", &[("x", 0.0)]).unwrap_err();
        let unknown = evaluate_expression("undefined_name + 1", &[]).unwrap_err();
        let syntax = evaluate_expression("1 +", &[]).unwrap_err();
        let not_number = evaluate_expression("'text'", &[]).unwrap_err();
        Python::attach(|py| {
            assert!(zero.is_instance_of::<PyZeroDivisionError>(py));
            assert!(unknown.is_instance_of::<PyNameError>(py));
            assert_eq!(
                syntax.get_type(py).name().unwrap().to_string(),
                "SyntaxError"
            );
            assert!(not_number.is_instance_of::<PyTypeError>(py));
        });
    }

    #[test]
    fn evaluate_expression_rejects_nul_bytes() {
        let err = evaluate_expression("1\0+1", &[]).unwrap_err();
        Python::attach(|py| assert!(err.is_instance_of::<PyValueError>(py)));
    }

    // ---- TemperatureLog (Python class driven from Rust) -----------------

    #[test]
    fn temperature_log_keeps_state_between_calls() {
        let log = TemperatureLog::new("Berlin").unwrap();
        assert!(log.is_empty().unwrap());
        assert_eq!(log.average().unwrap(), None);

        for c in [10.0, 20.0, 30.0] {
            log.add(c).unwrap();
        }
        assert_eq!(log.len().unwrap(), 3);
        assert!(approx(log.average().unwrap().unwrap(), 20.0));
        assert_eq!(log.to_fahrenheit().unwrap(), vec![50.0, 68.0, 86.0]);
        assert_eq!(log.location().unwrap(), "Berlin");
        assert_eq!(
            log.repr().unwrap(),
            "TemperatureLog(location='Berlin', readings=3)"
        );
    }

    #[test]
    fn temperature_log_rejects_values_below_absolute_zero() {
        let log = TemperatureLog::new("Lab").unwrap();
        let err = log.add(-300.0).unwrap_err();
        Python::attach(|py| assert!(err.is_instance_of::<PyValueError>(py)));
        assert!(
            log.is_empty().unwrap(),
            "invalid reading must not be stored"
        );
    }

    #[test]
    fn temperature_logs_are_independent_instances() {
        let a = TemperatureLog::new("A").unwrap();
        let b = TemperatureLog::new("B").unwrap();
        a.add(1.0).unwrap();
        assert_eq!(a.len().unwrap(), 1);
        assert_eq!(b.len().unwrap(), 0);
    }

    #[test]
    fn temperature_log_can_be_used_from_another_thread() {
        let log = TemperatureLog::new("Thread").unwrap();
        let handle = std::thread::spawn(move || {
            log.add(5.0).unwrap();
            log.len().unwrap()
        });
        assert_eq!(handle.join().unwrap(), 1);
    }
}
