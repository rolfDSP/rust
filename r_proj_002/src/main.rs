mod shapes;

use shapes::{Circle, Rectangle, Shape, Square};

// Static dispatch (like a C++ template): resolved at compile time.
fn print_static<T: Shape>(shape: &T) {
    println!("[static]  {}", shape.describe());
}

// Dynamic dispatch (like a virtual call through a base-class pointer).
fn print_dynamic(shape: &dyn Shape) {
    println!("[dynamic] {}", shape.describe());
}

fn main() {
    // Encapsulation: fields are private, access goes through methods.
    let mut c = Circle::new(1.0);
    println!("{c} has radius {}", c.radius());
    c.set_radius(2.0);
    c.set_radius(-5.0); // rejected by the setter
    println!("{c} after set_radius");

    print_static(&c);
    print_dynamic(&c);

    // Polymorphism: a heterogeneous collection, like std::vector<std::unique_ptr<Shape>>.
    let shapes: Vec<Box<dyn Shape>> = vec![
        Box::new(Circle::new(0.5)),
        Box::new(Rectangle::new(3.0, 4.0)),
        Box::new(Square::new(2.0)),
    ];

    for s in &shapes {
        println!("{}", s.describe());
    }

    let total: f64 = shapes.iter().map(|s| s.area()).sum();
    println!("Total area: {total:.2}");

    println!("End of main");
    // `shapes` and `c` are dropped here -> Circle's Drop impl runs.
}
