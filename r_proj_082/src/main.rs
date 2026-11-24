use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Debug)]
struct mydata {
    text: String,
    c: usize,
    v: Vec<usize>,
}

fn main() {
    println!("Welcome to r_proj_082!");

    let p1 = Arc::new(Mutex::new(mydata {text: String::from("hello world"), c: 0, v: Vec::new()}));
    {
        let mut p2 = p1.lock().unwrap();
        p2.c += 1;
    }
    // protected by simultaneous access
    let v1 = Arc::new(Mutex::new(vec![1,2,3]));

    let handles: Vec<_> = (0..3).map(|i| {
        let v1 = Arc::clone(&v1);
        let p2 = Arc::clone(&p1);

        thread::spawn(move || {
            v1.lock().unwrap()[i] += 1;
            let mut v2 = p2.lock().unwrap();
            v2.c = i;
            v2.v.push(i);
        })
    }).collect();

    for h in handles {
        h.join().unwrap();
    }

    println!("The number of overwrites is: {:?}", v1);
    println!("My data is: {:?}", p1);

}
