use std::io;

fn main() {
    println!("Hello, r_proj_011!");

    let mut input = String::new();
    while input.trim() != "stop" {
        input.clear();
        println!("Enter a word (use stop to break the loop: ");
        io::stdin().read_line(&mut input).expect("Failed to read input");
        println!("You have entered: {}", input);
    }
    println!("Good bye!");
}
