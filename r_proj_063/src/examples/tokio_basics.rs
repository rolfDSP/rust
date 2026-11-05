//! Tasks and concurrency: sequential vs. `join!`, `spawn`, and `JoinSet`.
//! Run with: cargo run --example tokio_basics

use std::time::{Duration, Instant};
use tokio::task::JoinSet;
use tokio::time::sleep;

/// Pretends to be slow I/O. `sleep` yields to the runtime, so other tasks can run meanwhile.
async fn fetch(name: &str, ms: u64) -> String {
    sleep(Duration::from_millis(ms)).await;
    format!("{name} ({ms} ms)")
}

async fn parse_number(text: &str) -> Result<i32, std::num::ParseIntError> {
    sleep(Duration::from_millis(50)).await;
    text.parse()
}

// `#[tokio::main]` starts a multi-threaded runtime and blocks on this async fn.
#[tokio::main]
async fn main() {
    // 1. Sequential: each .await finishes before the next one starts. About 500 ms in total.
    //    Calling an async fn does nothing on its own: futures are lazy until awaited.
    let start = Instant::now();
    let a = fetch("a", 200).await;
    let b = fetch("b", 300).await;
    println!("sequential: {a}, {b} in {:?}", start.elapsed());

    // 2. `join!` runs futures concurrently on the current task. About 300 ms in total.
    let start = Instant::now();
    let (a, b) = tokio::join!(fetch("a", 200), fetch("b", 300));
    println!("join!:      {a}, {b} in {:?}", start.elapsed());

    // 3. `try_join!` returns at the first error and drops the remaining futures.
    let ok = tokio::try_join!(parse_number("1"), parse_number("2"));
    let err = tokio::try_join!(parse_number("1"), parse_number("oops"));
    println!("try_join!:  {ok:?}, {err:?}");

    // 4. `spawn` starts an independent task that the runtime may run on another thread.
    //    It needs a `'static` future, hence the `move`. The JoinHandle yields the result,
    //    or a JoinError if the task panicked.
    let name = String::from("spawned");
    let handle = tokio::spawn(async move { fetch(&name, 100).await });
    println!("spawn:      {}", handle.await.unwrap());

    // 5. `JoinSet` manages a dynamic number of tasks. Results arrive in completion order,
    //    so the shortest sleep comes out first here.
    let mut set = JoinSet::new();
    for (id, ms) in [(1, 300), (2, 100), (3, 200)] {
        set.spawn(async move { fetch(&format!("job {id}"), ms).await });
    }
    while let Some(result) = set.join_next().await {
        println!("JoinSet:    {}", result.unwrap());
    }
}
