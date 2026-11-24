use std::sync::{Arc, Mutex};
use std::thread;

fn main() {
    println!("Welcome to r_proj_082!");

    // protected by simultaneous access
    let v1 = Arc::new(Mutex::new(vec![1,2,3]));

    let handles: Vec<_> = (0..3).map(|i| {
        let v1 = Arc::clone(&v1);

        thread::spawn(move || {
            v1.lock().unwrap()[i] += 1;
        })
    }).collect();

    for h in handles {
        h.join().unwrap();
    }

    println!("The number of overwrites is: {:?}", v1);

}
