use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};

fn main() -> Result<(), Box<dyn std::error::Error>>{
    println!("Hello, r_proj_014!");

    // try to open a file, if not existing create it new
    let file = OpenOptions::new().write(true).create(true).append(true).open("trytoread.txt");
    let mut file = match file {
        Ok(file) => file,
        Err(error) => {
            panic!("Unexpected error with file {}", error)
        }
    };

    file.write_all(b"I was here...\n")?;
    file.flush()?; // Automatic error handling

    let reader = BufReader::new(File::open("trytoread.txt")?); // Automatic error handling by Rust with the question mark

    for line in reader.lines() {
        let line = line?;
        println!("{}", line);
    }
    Ok(())
}
