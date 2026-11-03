use std::thread;
use std::sync::mpsc;
use std::sync::Mutex;
use std::sync::Arc;
use rayon::prelude::*;

fn main() {
    println!("Hello, r_proj_056!");

    let handle = thread::spawn(|| {
        println!("Thread is running");
    });

    handle.join().unwrap();

    let (tx,rx) = mpsc::channel();

    thread::spawn(move || {
        let msg = String::from("Welt");
        tx.send(msg).unwrap();
    });

    let received = rx.recv().unwrap();
    println!("Hallo {}", received);

    let data = Mutex::new(99);

    {
        let mut data_access = data.lock().unwrap();
        *data_access += 1;
    }

    println!("data: {:?}", *data.lock().unwrap());

    #[derive(Debug)]
    struct Db {
        a: i32,
        b: i32,
    }

    //let data1 = Arc::new("Arc example");
    let data1 = Arc::new( Db {a: 0, b: 4});

    for _ in 0..3 {
        let data_shared = data1.clone();
        thread::spawn(move || {
            println!("{:?}", data_shared);
            println!("{}, {}", data_shared.a, data_shared.b);
        });
    }

    let data2 = vec![1, 2, 4];

    let parallel_sum: i32 = data2.par_iter()
        .map(|x| x * x)
        .sum();

    println!("Sum: {}", parallel_sum);
}
