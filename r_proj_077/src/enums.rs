//! Rust enums in Python.
//!
//! * [`Color`] – a C-like enum: variants become class attributes (`Color.Red`),
//!   comparable with `==` and convertible to `int`.
//! * [`Shape`] – a data-carrying ("complex") enum: each variant becomes a
//!   subclass (`Shape.Circle(radius=1.0)`) usable with `isinstance` and `match`.

use pyo3::prelude::*;

#[pyclass(module = "rustypy", eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Color {
    Red = 1,
    Green = 2,
    Blue = 3,
}

#[pymethods]
impl Color {
    /// Hex string for the color.
    fn hex(&self) -> &'static str {
        match self {
            Color::Red => "#ff0000",
            Color::Green => "#00ff00",
            Color::Blue => "#0000ff",
        }
    }

    /// Parse from a name (case-insensitive).
    #[staticmethod]
    fn parse(name: &str) -> PyResult<Color> {
        match name.to_ascii_lowercase().as_str() {
            "red" => Ok(Color::Red),
            "green" => Ok(Color::Green),
            "blue" => Ok(Color::Blue),
            _ => Err(pyo3::exceptions::PyValueError::new_err(format!(
                "unknown color '{name}'"
            ))),
        }
    }
}

#[pyclass(module = "rustypy", from_py_object)]
#[derive(Clone, Debug)]
pub enum Shape {
    Circle { radius: f64 },
    Rectangle { width: f64, height: f64 },
    Triangle { a: f64, b: f64, c: f64 },
    /// Tuple variants are accessible via index: `shape[0]`
    Polygon(Vec<(f64, f64)>),
}

#[pymethods]
impl Shape {
    fn area(&self) -> f64 {
        match self {
            Shape::Circle { radius } => std::f64::consts::PI * radius * radius,
            Shape::Rectangle { width, height } => width * height,
            Shape::Triangle { a, b, c } => {
                // Heron's formula
                let s = (a + b + c) / 2.0;
                (s * (s - a) * (s - b) * (s - c)).max(0.0).sqrt()
            }
            Shape::Polygon(pts) => {
                // Shoelace formula
                let n = pts.len();
                (0..n)
                    .map(|i| {
                        let (x1, y1) = pts[i];
                        let (x2, y2) = pts[(i + 1) % n];
                        x1 * y2 - x2 * y1
                    })
                    .sum::<f64>()
                    .abs()
                    / 2.0
            }
        }
    }

    fn kind(&self) -> &'static str {
        match self {
            Shape::Circle { .. } => "circle",
            Shape::Rectangle { .. } => "rectangle",
            Shape::Triangle { .. } => "triangle",
            Shape::Polygon(_) => "polygon",
        }
    }
}

/// Enums as function arguments and return values.
#[pyfunction]
pub fn mix(a: Color, b: Color) -> String {
    if a == b {
        format!("pure {a:?}")
    } else {
        format!("{a:?}-{b:?} mix")
    }
}

/// Returns the shape with the biggest area.
#[pyfunction]
pub fn largest(shapes: Vec<Shape>) -> Option<Shape> {
    shapes
        .into_iter()
        .max_by(|a, b| a.area().total_cmp(&b.area()))
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Color>()?;
    m.add_class::<Shape>()?;
    m.add_function(wrap_pyfunction!(mix, m)?)?;
    m.add_function(wrap_pyfunction!(largest, m)?)?;
    Ok(())
}
