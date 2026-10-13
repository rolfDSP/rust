mod helper;
mod experiment;

mod math{
    pub fn add(a: f32, b: f32) -> f32{
        a + b
    }
}

fn main() {
    println!("Hello, world!");

    let a = 1.0;
    let b = &a;

    println!("The value of a is: {} and of b is: {}",a, b);

    let c: f32 = 0.5;
    let d: f32 = 1.8;
    let e = math::add(c, d);
    println!("The value of e is: {}", e);

    let s = helper::fmt1(String::from("value: "), 42);
    println!("s contains the text: {}", s);

    let f: i32 = 10;
    println!("Use the function experiment::my_fn1: {}", experiment::mod1::my_fn1(f, 10));

    use lib_test::add;
    let x = add(2, 4);
    println!("Greetings from libTest: {}", x);
}
