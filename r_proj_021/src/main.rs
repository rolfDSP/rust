use std::io;
use myLib::read_stdin;
use myLib::colors::red;

fn main() {
    println!("Hello, r_proj_021!");

    let inp = read_stdin();
    println!("Read from input: {}", inp);

    let red_string = red("This text should appear in red color");
    println!("{}", red_string);

//     let mut input = String::new();
//
//     while input.trim() != "stop" {
//         input.clear();
//         println!("Please enter a string, 'stop' to exit: ");
//         io::stdin().read_line(&mut input).expect("Failed to read input");
//         println!("You have entered: {}", input);
//     }
//     println!("Goodbye");
}
