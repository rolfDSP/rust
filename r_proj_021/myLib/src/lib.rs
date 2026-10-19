//! This library provides some good tools
//! For example read a line from the text console
//! ```
//! use myLib::read_stdin;
//! let input = read_stdin();
//! ```
//! # Panics:
//! Panic will be thrown on error
use std::io::{BufReader, BufRead};

pub mod colors;
pub mod config;
pub fn add_u64(left: u64, right: u64) -> u64 {
    left + right
}
/// This function adds to uInt32 numbers and returns the result in uInt32
pub fn add_u32(left: u32, right: u32) -> u32 {
    left + right
}

/// Read a text line from the console input and return a String
/// # Examples:
/// ```
/// use myLib::read_stdin;
/// let line_in = read_stdin();
/// ```
pub fn read_stdin() -> String {
    let stdin = std::io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let mut line = String::new();
    reader.read_line(&mut line).expect("Failed to read input line");
    line.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add_u64(2, 2);
        assert_eq!(result, 4);
    }
    #[test]
    fn test_add_u32() {
        let result = add_u32(10,20);
        assert_eq!(result, 30);
    }
}
