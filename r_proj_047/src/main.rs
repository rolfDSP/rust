/// Returns an M x N matrix (M rows, N columns) of random f32 values in [0, 1).
#[allow(non_snake_case)]
fn genRand(M: usize, N: usize) -> Vec<Vec<f32>> {
    (0..M)
        .map(|_| (0..N).map(|_| rand::random::<f32>()).collect())
        .collect()
}

/// Saves the matrix to a CSV file, one matrix row per line.
#[allow(non_snake_case)]
fn saveCSV(matrix: &[Vec<f32>], filename: &str) -> Result<(), csv::Error> {
    let mut writer = csv::Writer::from_path(filename)?;
    for row in matrix {
        writer.write_record(row.iter().map(|x| x.to_string()))?;
    }
    writer.flush()?;
    Ok(())
}

/// Reads M and N and the file name of the CSV file from the command line arguments (excluding the program name).
fn parse_args(args: &[String]) -> Result<(usize, usize, String), String> {
    if args.len() != 3 {
        return Err(format!("expected 3 arguments (M N fileName), got {}", args.len()));
    }
    let m = args[0]
        .parse::<usize>()
        .map_err(|_| format!("invalid M: '{}'", args[0]))?;
    let n = args[1]
        .parse::<usize>()
        .map_err(|_| format!("invalid N: '{}'", args[1]))?;
    Ok((m, n, args[2].clone()))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (rows, cols, file_name) = match parse_args(&args) {
        Ok(dims) => dims,
        Err(e) => {
            eprintln!("Error: {e}");
            eprintln!("Usage: r_proj_047 <M> <N> <fileName>");
            std::process::exit(1);
        }
    };

    let m = genRand(rows, cols);
    for row in &m {
        println!("{:?}", row);
    }

    println!("File name is: {}", file_name);
    saveCSV(&m, &file_name).expect("failed to write CSV");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gen_rand_has_correct_shape() {
        let m = genRand(3, 5);
        assert_eq!(m.len(), 3);
        assert!(m.iter().all(|row| row.len() == 5));
    }

    #[test]
    fn gen_rand_values_in_unit_interval() {
        let m = genRand(10, 10);
        assert!(m.iter().flatten().all(|&x| (0.0..1.0).contains(&x)));
    }

    #[test]
    fn gen_rand_empty() {
        assert!(genRand(0, 4).is_empty());
        assert!(genRand(2, 0).iter().all(|row| row.is_empty()));
    }

    #[test]
    fn save_csv_round_trip() {
        let matrix = vec![vec![0.5, 0.25, 1.0], vec![0.125, 0.0, 0.75]];
        let path = std::env::temp_dir().join("r_proj_047_save_csv_test.csv");
        let filename = path.to_str().unwrap();

        saveCSV(&matrix, filename).unwrap();

        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .from_path(filename)
            .unwrap();
        let read_back: Vec<Vec<f32>> = reader
            .records()
            .map(|r| r.unwrap().iter().map(|s| s.parse().unwrap()).collect())
            .collect();
        std::fs::remove_file(&path).unwrap();

        assert_eq!(read_back, matrix);
    }

    fn strs(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parse_args_valid() {
        assert_eq!(
            parse_args(&strs(&["3", "5", "out.csv"])),
            Ok((3, 5, "out.csv".to_string()))
        );
    }

    #[test]
    fn parse_args_wrong_count() {
        assert!(parse_args(&strs(&[])).is_err());
        assert!(parse_args(&strs(&["3", "5"])).is_err());
        assert!(parse_args(&strs(&["3", "5", "out.csv", "extra"])).is_err());
    }

    #[test]
    fn parse_args_not_a_number() {
        assert!(parse_args(&strs(&["abc", "5", "out.csv"])).is_err());
        assert!(parse_args(&strs(&["3", "-1", "out.csv"])).is_err());
    }
}
