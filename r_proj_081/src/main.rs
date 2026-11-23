//! Demo program: runs every function of the embedded Python script.

use embedpy::{
    TemperatureLog, describe, evaluate_expression, inline, render_report, script_version,
    word_frequencies,
};
use pyo3::prelude::*;

fn main() -> PyResult<()> {
    println!("Embedded analytics.py version {}\n", script_version()?);

    // 1. Function with simple arguments and a list-of-tuples result.
    let text = "The quick brown fox jumps over the lazy dog. The dog sleeps, the fox runs.";
    println!("Top words: {:?}\n", word_frequencies(text, 3)?);

    // 2. Function returning a dict -> Rust struct.
    let summary = describe(&[12.5, 17.0, 9.25, 21.0, 15.75])?;
    println!("{summary:#?}\n");

    // 3. Python formats a text report for Rust.
    let report = render_report(
        "Statistics",
        &[
            ("mean", summary.mean),
            ("median", summary.median),
            ("max", summary.max),
        ],
    )?;
    println!("{report}\n");

    // 4. Python class instance owned by Rust.
    let log = TemperatureLog::new("Stuttgart")?;
    for c in [3.5, 7.0, 11.25] {
        log.add(c)?;
    }
    println!(
        "{} -> average {:?} °C, in °F {:?}",
        log.repr()?,
        log.average()?,
        log.to_fahrenheit()?
    );

    // 5. Ad-hoc expression evaluation with Rust-provided variables.
    let area = evaluate_expression("math.pi * r ** 2", &[("r", 2.0)])?;
    println!("Area of a circle with r = 2: {area:.4}\n");

    // 6. Python exceptions arrive as PyErr values.
    match describe(&[]) {
        Ok(_) => unreachable!(),
        Err(err) => Python::attach(|py| {
            println!(
                "Expected error: {}: {}",
                err.get_type(py).name().unwrap(),
                err.value(py)
            );
        }),
    }
    if let Err(err) = log.add(-500.0) {
        println!("Expected error: {err}");
    }

    // 7. Python code embedded as string literals in Rust (src/inline.rs).
    println!("\n--- Inline Python scripts ---");
    let invoice = inline::run_invoice_script(&[("Coffee", 2, 3.5), ("Cake", 1, 4.25)], 0.19)?;
    for line in &invoice.lines {
        println!("{line}");
    }
    println!(
        "subtotal {:.2}, tax {:.2}, total {:.2}",
        invoice.subtotal, invoice.tax, invoice.total
    );
    println!("Fibonacci: {:?}", inline::fibonacci(12)?);
    let output = inline::run_capturing_output(
        "import platform\nprint('Hello from Python', platform.python_version())",
    )?;
    print!("Captured output: {output}");
    Ok(())
}
