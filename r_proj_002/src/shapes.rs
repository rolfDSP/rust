//! OOP-style example: encapsulation, "methods", constructors,
//! polymorphism via traits, composition instead of inheritance,
//! and destructors via `Drop`.

use std::f64::consts::PI;
use std::fmt;

// ---------------------------------------------------------------------------
// Trait = roughly a C++ abstract base class with pure virtual functions.
// ---------------------------------------------------------------------------
pub trait Shape {
    fn area(&self) -> f64;
    fn perimeter(&self) -> f64;
    fn name(&self) -> String;

    // Default implementation = a virtual function with a base-class body.
    fn describe(&self) -> String {
        format!(
            "{}: area = {:.2}, perimeter = {:.2}",
            self.name(),
            self.area(),
            self.perimeter()
        )
    }
}

// ---------------------------------------------------------------------------
// Struct with private fields = class with private members.
// ---------------------------------------------------------------------------
pub struct Circle {
    radius: f64, // private: not accessible outside this module
}

impl Circle {
    // Associated function = static method / constructor.
    pub fn new(radius: f64) -> Self {
        Circle { radius }
    }

    // Getter and setter (with validation) = encapsulation.
    pub fn radius(&self) -> f64 {
        self.radius
    }

    pub fn set_radius(&mut self, radius: f64) {
        if radius >= 0.0 {
            self.radius = radius;
        }
    }
}

impl Shape for Circle {
    fn area(&self) -> f64 {
        PI * self.radius * self.radius
    }

    fn perimeter(&self) -> f64 {
        2.0 * PI * self.radius
    }

    fn name(&self) -> String {
        "Circle".to_string()
    }
}

pub struct Rectangle {
    width: f64,
    height: f64,
}

impl Rectangle {
    pub fn new(width: f64, height: f64) -> Self {
        Rectangle { width, height }
    }
}

impl Shape for Rectangle {
    fn area(&self) -> f64 {
        self.width * self.height
    }

    fn perimeter(&self) -> f64 {
        2.0 * (self.width + self.height)
    }

    fn name(&self) -> String {
        "Rectangle".to_string()
    }
}

// ---------------------------------------------------------------------------
// No inheritance in Rust: a Square *contains* a Rectangle (composition)
// and delegates to it.
// ---------------------------------------------------------------------------
pub struct Square {
    inner: Rectangle,
}

impl Square {
    pub fn new(side: f64) -> Self {
        Square {
            inner: Rectangle::new(side, side),
        }
    }
}

impl Shape for Square {
    fn area(&self) -> f64 {
        self.inner.area()
    }

    fn perimeter(&self) -> f64 {
        self.inner.perimeter()
    }

    fn name(&self) -> String {
        "Square".to_string()
    }

    // Overriding the default implementation.
    fn describe(&self) -> String {
        format!("Square with side {:.2} (area {:.2})", self.inner.width, self.area())
    }
}

// Implementing a standard trait = like overloading operator<< in C++.
impl fmt::Display for Circle {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Circle(r = {})", self.radius)
    }
}

// `Drop` = destructor; runs automatically when the value goes out of scope (RAII).
impl Drop for Circle {
    fn drop(&mut self) {
        println!("Dropping {self}");
    }
}
