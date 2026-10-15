//! Multithreading example: three worker threads run continuously with
//! different scheduling priorities.
//!
//! Rust's standard library has no API for thread priorities, so we call the
//! Linux API through the `libc` crate:
//!   - `setpriority()` sets the "nice" value of a single thread
//!     (0 = normal, 19 = lowest). Lowering the priority needs no root rights;
//!     raising it above normal or using real-time policies would.
//!   - `sched_setaffinity()` pins all workers onto the same CPU core. On a
//!     multi-core machine every thread would otherwise get its own core and
//!     the priorities would have no visible effect.
//!
//! The main thread prints once per second how much work each thread managed
//! to do. Press <Enter> to stop all threads cleanly.

use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// Description of one worker thread.
#[derive(Clone)]
struct WorkerConfig {
    name: &'static str,
    /// Linux nice value: lower number = higher priority.
    nice: i32,
}

/// Data shared between a worker and the main thread.
struct Worker {
    config: WorkerConfig,
    counter: Arc<AtomicU64>,
    handle: JoinHandle<()>,
}

/// CPU core that all workers share, so they have to compete for it.
const SHARED_CPU: usize = 0;

/// Sets the nice value of the calling thread.
fn set_current_thread_nice(nice: i32) -> io::Result<()> {
    // SAFETY: plain syscalls without pointers; gettid() always succeeds.
    let result = unsafe {
        let tid = libc::gettid();
        libc::setpriority(libc::PRIO_PROCESS, tid as libc::id_t, nice)
    };
    if result == 0 { Ok(()) } else { Err(io::Error::last_os_error()) }
}

/// Pins the calling thread to a single CPU core.
fn pin_current_thread_to_cpu(cpu: usize) -> io::Result<()> {
    // SAFETY: `set` is a properly initialized cpu_set_t living on the stack.
    let result = unsafe {
        let mut set: libc::cpu_set_t = std::mem::zeroed();
        libc::CPU_SET(cpu, &mut set);
        libc::sched_setaffinity(0, size_of::<libc::cpu_set_t>(), &set)
    };
    if result == 0 { Ok(()) } else { Err(io::Error::last_os_error()) }
}

/// Some CPU-bound dummy work.
fn do_work_chunk() {
    let mut x: u64 = 1;
    for i in 0..10_000u64 {
        x = std::hint::black_box(x.wrapping_mul(6364136223846793005).wrapping_add(i));
    }
}

/// Body of every worker thread: runs until `running` becomes false.
fn worker_loop(config: &WorkerConfig, running: &AtomicBool, counter: &AtomicU64) {
    if let Err(e) = pin_current_thread_to_cpu(SHARED_CPU) {
        eprintln!("[{}] could not set CPU affinity: {e}", config.name);
    }
    if let Err(e) = set_current_thread_nice(config.nice) {
        eprintln!("[{}] could not set nice value {}: {e}", config.name, config.nice);
    }

    while running.load(Ordering::Relaxed) {
        do_work_chunk();
        counter.fetch_add(1, Ordering::Relaxed);
    }
    println!("[{}] stopped", config.name);
}

fn spawn_worker(config: WorkerConfig, running: Arc<AtomicBool>) -> Worker {
    let counter = Arc::new(AtomicU64::new(0));
    let thread_counter = Arc::clone(&counter);
    let thread_config = config.clone();

    let handle = thread::Builder::new()
        .name(config.name.to_string())
        .spawn(move || worker_loop(&thread_config, &running, &thread_counter))
        .expect("failed to spawn thread");

    Worker { config, counter, handle }
}

fn main() {
    let running = Arc::new(AtomicBool::new(true));

    let configs = [
        WorkerConfig { name: "high", nice: 0 },
        WorkerConfig { name: "medium", nice: 5 },
        WorkerConfig { name: "low", nice: 10 },
    ];
    let workers: Vec<Worker> = configs
        .into_iter()
        .map(|config| spawn_worker(config, Arc::clone(&running)))
        .collect();

    // Separate thread waits for <Enter> and then requests a shutdown.
    {
        let running = Arc::clone(&running);
        thread::spawn(move || {
            let mut line = String::new();
            let _ = io::stdin().read_line(&mut line);
            running.store(false, Ordering::Relaxed);
        });
    }

    println!("All workers share CPU {SHARED_CPU}. Press <Enter> to stop.\n");
    let mut last = vec![0u64; workers.len()];
    while running.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_secs(1));

        let current: Vec<u64> = workers.iter().map(|w| w.counter.load(Ordering::Relaxed)).collect();
        let deltas: Vec<u64> = current.iter().zip(&last).map(|(c, l)| c - l).collect();
        let total = deltas.iter().sum::<u64>().max(1);
        last = current;

        let line: Vec<String> = workers
            .iter()
            .zip(&deltas)
            .map(|(w, d)| {
                let share = *d as f64 * 100.0 / total as f64;
                format!("{:>6} (nice {:>2}): {share:5.1}%", w.config.name, w.config.nice)
            })
            .collect();
        println!("{}", line.join(" | "));
    }

    for worker in workers {
        worker.handle.join().expect("worker thread panicked");
    }
    println!("All threads finished.");
}
