mod times;

use std::io::{self, BufRead, Write};

fn main() {
    let handle = times::start();

    println!("Counter started. Commands: 'f' = show counter, 'r' = reset, 's' = stop");

    let stdin = io::stdin();
    loop {
        print!("> ");
        io::stdout().flush().unwrap();

        let mut line = String::new();
        // End the program on EOF (Ctrl+D) or read error.
        if stdin.lock().read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }

        match line.trim() {
            "f" => println!("Counter: {}", times::get()),
            "r" => {
                times::reset();
                println!("Counter reset to 0");
            }
            "s" => break,
            "" => {}
            other => println!("Unknown command '{other}'. Use 'f', 'r' or 's'."),
        }
    }

    times::stop();
    handle.join().expect("counter thread panicked");
    println!("Thread stopped. Bye!");
}
