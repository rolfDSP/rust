//! Module `times`: a background counter that increments once per second.
//!
//! The counter is an `i32` protected by a `Mutex`. A worker thread increments it
//! every second. Interface functions allow reading, resetting and stopping.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// The shared counter, initialized to zero.
static COUNTER: Mutex<i32> = Mutex::new(0);

/// Flag that tells the worker thread to keep running.
static RUNNING: AtomicBool = AtomicBool::new(false);

/// Starts the counting thread and returns its handle.
pub fn start() -> JoinHandle<()> {
    RUNNING.store(true, Ordering::SeqCst);
    thread::spawn(|| {
        while RUNNING.load(Ordering::SeqCst) {
            {
                // Lock only for the increment, then release before sleeping.
                let mut value = COUNTER.lock().unwrap();
                *value += 1;
            }
            thread::sleep(Duration::from_secs(1));
        }
    })
}

/// Returns the current value of the counter (read under the mutex).
pub fn get() -> i32 {
    *COUNTER.lock().unwrap()
}

/// Resets the counter to zero.
pub fn reset() {
    *COUNTER.lock().unwrap() = 0;
}

/// Signals the counting thread to stop. Join the handle afterwards to wait for it.
pub fn stop() {
    RUNNING.store(false, Ordering::SeqCst);
}
