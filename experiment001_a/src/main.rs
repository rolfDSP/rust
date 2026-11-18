pub mod filespawn;

use std::io::{self, BufRead};

fn main() {
    println!("Hello, world!");

    println!("2+3={}", filespawn::test(2,3));

    let stdin = io::stdin();
    let mut r = 1;

    // create and start thread
    let mut f_sp = filespawn::FileSpawn::new();
    f_sp.set_callback(|path| {
        println!(">>> Callback: device mounted at {}", path);
    });
    f_sp.start_thread();

    while r == 1 {
        let line = stdin.lock().lines().next().unwrap().unwrap();
        let ch = line.chars().next().unwrap_or('\0');
        println!("You have entered: {}", ch);

        if ch == 'e' {
            r = 0;
        }
    }

    f_sp.stop_thread();

}
