fn main() {
    println!("Hello, r_proj_012!");
    // loops in Rust
    // count up to end - 1 (10 - 1)
    for i in 1..10 {
        println!("i = {}", i);
    }

    //including the last counter element
    println!("===========");
    for i in 1..=10 {
        println!("i = {}", i);
    }

    // range as a Rust object
    println!("==========");
    for i in {1..10}.rev()  {
        println!("i = {}", i);
    }

    // other increment than 1
    println!("=================");
    for i in (1..10).step_by(2) {
        println!("i = {}", i);
    }

    println!("=================");
    let v = vec![1, 7, 9, 3, 8];
    for k in v {
        println!("v = {}", k);
    }

    println!("==================");
    // loop control
    for i in 1..10 {
        // skip 3
        if i==3 {
            continue;
        }

        // break out at 7
        if i == 7 {
            break;
        }
        println!("i = {}", i)
    }

    let s = "Hello";
    match s {
        "Hello" => println!(", welcome to my place"),
        "Goodbye" => println!(", would be nice to see you again"),
        _ => println!("Do not understand"),
    }
}
