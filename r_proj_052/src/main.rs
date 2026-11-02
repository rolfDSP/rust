use std::sync::{Arc, Mutex};
use std::thread;

fn main() {
    // 1) Move ownership into the thread and get the modified data back via join()
    let numbers = vec![1, 2, 3];
    let handle = thread::spawn(move || {
        let mut numbers = numbers; // the thread now owns the vector
        numbers.push(4);
        numbers.iter_mut().for_each(|n| *n *= 10);
        numbers // return it to the caller
    });
    let numbers = handle.join().unwrap();
    println!("1) moved and returned: {:?}", numbers);

    // 2) Scoped threads: borrow outside data mutably, no move needed.
    //    The scope guarantees all threads finish before `scope` returns.
    let mut counter = 0;
    let mut text = String::from("Hello");
    thread::scope(|s| {
        s.spawn(|| {
            counter += 5;
        });
        s.spawn(|| {
            text.push_str(", world!");
        });
    });
    println!("2) scoped: counter = {}, text = {}", counter, text);

    // 3) Shared ownership: several threads modify the same data via Arc<Mutex<T>>
    let shared = Arc::new(Mutex::new(Vec::new()));
    let mut handles = Vec::new();
    for i in 0..4 {
        let shared = Arc::clone(&shared);
        handles.push(thread::spawn(move || {
            let mut data = shared.lock().unwrap();
            data.push(i * i);
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    let mut result = shared.lock().unwrap().clone();
    result.sort(); // thread order is not deterministic
    println!("3) Arc<Mutex>: {:?}", result);
}
