//! Sharing data and limiting work: Mutex, RwLock, Semaphore, and `spawn_blocking`.
//! Run with: cargo run --example tokio_shared_state

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore};
use tokio::task::JoinSet;
use tokio::time::sleep;

/// CPU-heavy work with no .await in it, which would starve other tasks on a runtime thread.
fn count_primes(limit: u64) -> usize {
    (2..limit)
        .filter(|n| (2..).take_while(|d| d * d <= *n).all(|d| n % d != 0))
        .count()
}

#[tokio::main]
async fn main() {
    // 1. std::sync::Mutex is the right choice when the lock is NOT held across an .await.
    //    It is cheaper than the tokio Mutex, and a guard held across .await won't compile
    //    inside `tokio::spawn`, because the guard isn't Send.
    let counter = Arc::new(Mutex::new(0));
    let mut tasks = JoinSet::new();
    for _ in 0..100 {
        let counter = Arc::clone(&counter);
        tasks.spawn(async move {
            *counter.lock().unwrap() += 1;
        });
    }
    tasks.join_all().await;
    println!("std Mutex: counter = {}", counter.lock().unwrap());

    // 2. tokio's RwLock (and Mutex) can be held across an .await, and acquiring them is
    //    itself awaited. Use these only when the guard really must live across an .await.
    let cache = Arc::new(RwLock::new(HashMap::new()));
    {
        let mut guard = cache.write().await;
        sleep(Duration::from_millis(10)).await; // Fine with the tokio lock.
        guard.insert("answer", 42);
    } // The write guard is dropped here. Readers would wait forever otherwise.
    let readers = (0..3).map(|id| {
        let cache = Arc::clone(&cache);
        async move {
            let guard = cache.read().await; // Many readers may hold the lock at once.
            println!("RwLock:    reader {id} sees {:?}", guard.get("answer"));
        }
    });
    futures_join_all(readers).await;

    // 3. A Semaphore limits concurrency, for example the number of open connections.
    //    8 jobs of 100 ms each, at most 2 at a time: about 400 ms in total.
    let start = Instant::now();
    let permits = Arc::new(Semaphore::new(2));
    let mut tasks = JoinSet::new();
    for _ in 0..8 {
        let permits = Arc::clone(&permits);
        tasks.spawn(async move {
            // The permit is released when dropped, at the end of this task.
            let _permit = permits.acquire().await.unwrap();
            sleep(Duration::from_millis(100)).await;
        });
    }
    tasks.join_all().await;
    println!("Semaphore: 8 jobs took {:?}", start.elapsed());

    // 4. `spawn_blocking` moves blocking or CPU-heavy work to a dedicated thread pool, so the
    //    runtime threads stay free. Here the ticker keeps running during the calculation.
    let calculation = tokio::task::spawn_blocking(|| count_primes(2_000_000));
    let ticker = tokio::spawn(async {
        let mut ticks = 0;
        loop {
            sleep(Duration::from_millis(10)).await;
            ticks += 1;
            if ticks == 5 {
                println!("blocking:  runtime still responsive after {ticks} ticks");
                break;
            }
        }
    });
    let primes = calculation.await.unwrap();
    ticker.await.unwrap();
    println!("blocking:  found {primes} primes below 2,000,000");
}

/// Awaits a collection of futures concurrently, like `futures::future::join_all`, but
/// without the extra crate. The futures here borrow nothing, so `spawn` accepts them.
async fn futures_join_all<F>(futures: impl Iterator<Item = F>)
where
    F: Future<Output = ()> + Send + 'static,
{
    let mut set = JoinSet::new();
    for future in futures {
        set.spawn(future);
    }
    set.join_all().await;
}
