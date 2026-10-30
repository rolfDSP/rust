fn main() {
    println!("Hello, r_proj_050!");

    use std::thread;

    // create a thread
    let handle = thread::spawn(|| {
        println!("This runs inside of a thread");
    });

    handle.join().unwrap();

    use std::sync::Mutex;

    let m = Mutex::new(5);

    {
        let mut num = m.lock().unwrap();

        *num = 1000;
    }

    println!("m = {:?}", m);
}
