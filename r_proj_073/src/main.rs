use std::fs;
use std::path::Path;
use std::io::{Read, BufReader};

fn walk_path(path : &Path) {
    for entry in fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            walk_path(&path);
        } else {
            let file = fs::File::open(&path).unwrap();
            let mut buffer = [0;1024];
            let mut reader = BufReader::new(file);
            let bytes_read = reader.read(&mut buffer).unwrap();
            //println!("{}", path.display());
            if bytes_read > 0 && std::str::from_utf8(&buffer[..bytes_read]).is_ok()
            {
                println!("plain text file: {}", path.display());
            }
            else {
                println!("binary file: {}", path.display());
            }
        }
    }
}

fn main() {
    println!("Hello, r_proj_073!");

    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("Usage: r_proj_073 [path]");
        return;
    }

    let path = Path::new(&args[1]);

    if !path.exists() {
        println!("No such file or directory");
        return;
    }

    walk_path(path);

}
