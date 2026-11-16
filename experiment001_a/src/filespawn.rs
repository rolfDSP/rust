//use std::fs::File;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use mio::{Events, Interest, Poll, Token};
use udev::{MonitorBuilder, EventType};

pub fn test(a: i32, b: i32) -> i32{
    a + b
}

fn find_mount_point(device_node: &str) -> Option<String> {
    let content = std::fs::read_to_string("/proc/mounts").ok()?;
    for line in content.lines() {
        let mut parts = line.splitn(3, ' ');
        let device = parts.next()?;
        let mount  = parts.next()?;
        if device == device_node {
            return Some(unescape_mount_path(mount));
        }
    }
    None
}

fn unescape_mount_path(path: &str) -> String {
    // /proc/mounts encodes spaces as \040, tabs as \011, etc.
    let mut result = String::new();
    let mut chars = path.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            let octal: String = chars.by_ref().take(3).collect();
            if let Ok(n) = u8::from_str_radix(&octal, 8) {
                result.push(n as char);
            } else {
                result.push('\\');
                result.push_str(&octal);
            }
        } else {
            result.push(ch);
        }
    }
    result
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
                        let mut mounted = false;
                        for _ in 0..10 {
                            thread::sleep(Duration::from_millis(500));
                            if let Some(mount_path) = find_mount_point(&node) {
                                println!("Mounted at:     {}", mount_path);
                                mounted = true;
                                break;
                            }
                        }
                        if !mounted {
                            println!("Not automounted (may need manual mount)");
                        }
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
