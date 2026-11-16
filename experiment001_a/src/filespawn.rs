//use std::fs::File;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::thread::{self, JoinHandle};
use std::time::Duration;

pub fn test(a: i32, b: i32) -> i32{
    a + b
}

pub struct FileSpawn {
    running : Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl FileSpawn {
    pub fn new() -> Self{
        Self {
            running: Arc::new(AtomicBool::new(false)),
            handle: None,
        }
    }
    
    fn thread_fn(running: Arc<AtomicBool>) {
        let mut c: i32 = 0;
        while running.load(Ordering::SeqCst) {
            // sleep for 1 second
            c += 1;
            println!("Thread c: {}", c);
            thread::sleep(Duration::from_secs(1));
        }
    }

    /// Start and run the thread here, which calls the thread_fn()
    pub fn start_thread(&mut self) {
        self.running.store(true, Ordering::SeqCst);
        let running = Arc::clone(&self.running);
        self.handle = Some(thread::spawn(move  || {
            Self::thread_fn(running);
        }));
    }

    pub fn stop_thread(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            handle.join().unwrap();
        }
    }
}

impl Default for FileSpawn {
    fn default() -> Self {
        Self::new()
    }
}
