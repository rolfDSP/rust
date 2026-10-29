use std::env;
use std::fs;
use std::process;

/// Reads a CSV file of f32 values into an N x M matrix (rows x columns).
/// Returns None if the file cannot be read, a value cannot be parsed,
/// the file is empty, or the rows have different lengths.
fn read_csv(file_name: &str) -> Option<Vec<Vec<f32>>> {
    let content = fs::read_to_string(file_name).ok()?;

    let mut matrix: Vec<Vec<f32>> = Vec::new();
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let row = line
            .split(',')
            .map(|field| field.trim().parse::<f32>().ok())
            .collect::<Option<Vec<f32>>>()?;
        matrix.push(row);
    }

    // All rows must have the same number of columns.
    let cols = matrix.first()?.len();
    if matrix.iter().any(|row| row.len() != cols) {
        return None;
    }

    Some(matrix)
}

/// Computes (min, max, average) for each column of the matrix.
fn column_stats(matrix: &[Vec<f32>]) -> Vec<(f32, f32, f32)> {
    let cols = matrix[0].len();
    (0..cols)
        .map(|c| {
            let column = matrix.iter().map(|row| row[c]);
            let min = column.clone().fold(f32::INFINITY, f32::min);
            let max = column.clone().fold(f32::NEG_INFINITY, f32::max);
            let avg = column.sum::<f32>() / matrix.len() as f32;
            (min, max, avg)
        })
        .collect()
}

fn main() {
    // The first element of args() is the program name, so skip it.
    let arg: String = match env::args().nth(1) {
        Some(value) => value,
        None => {
            eprintln!("Usage: r_proj_048 <csv-file>");
            process::exit(1);
        }
    };

    println!("Argument: {arg}");

    match read_csv(&arg) {
        Some(matrix) => {
            println!("Read {} x {} matrix", matrix.len(), matrix[0].len());
            println!("{:>6} {:>12} {:>12} {:>12}", "column", "min", "max", "average");
            for (c, (min, max, avg)) in column_stats(&matrix).iter().enumerate() {
                println!("{c:>6} {min:>12.6} {max:>12.6} {avg:>12.6}");
            }
        }
        None => {
            eprintln!("Failed to read a valid f32 matrix from '{arg}'");
            process::exit(1);
        }
    }
}
