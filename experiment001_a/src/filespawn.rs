//use std::fs::File;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use mio::{Events, Interest, Poll, Token};
use udev::{MonitorBuilder, EventType};

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
        let mut socket = MonitorBuilder::new().unwrap()
            .match_subsystem("block").unwrap()
            .listen().unwrap();

        let mut poll = Poll::new().unwrap();
        let mut mio_events = Events::with_capacity(16);

        poll.registry()
            .register(&mut socket, Token(0), Interest::READABLE)
            .unwrap();

        println!("Watching for storage devices...");

        while running.load(Ordering::SeqCst) {
            // 1-second timeout so the running flag is checked regularly
            poll.poll(&mut mio_events, Some(Duration::from_secs(1))).unwrap();

            // drain all pending udev events
            for event in socket.iter() {
                match event.event_type() {
                    EventType::Add => {
                        let node = event.devnode()
                            .map(|p| p.display().to_string())
                            .unwrap_or_default();
                        println!("Device added:   {}", node);
                    }
                    EventType::Remove => {
                        let node = event.devnode()
                            .map(|p| p.display().to_string())
                            .unwrap_or_default();
                        println!("Device removed: {}", node);
                    }
                    _ => {}
                }
            }
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
