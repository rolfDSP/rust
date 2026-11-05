//! Racing futures: `select!`, timeouts, intervals, and graceful shutdown.
//! Run with: cargo run --example tokio_select

use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tokio::time::{MissedTickBehavior, interval, sleep, timeout};

async fn slow_operation() -> &'static str {
    sleep(Duration::from_millis(500)).await;
    "slow result"
}

/// A background worker that does periodic work until it is told to stop.
async fn worker(mut shutdown: watch::Receiver<bool>) -> u32 {
    let mut ticker = interval(Duration::from_millis(100));
    // If a tick is late, carry on from now instead of firing a burst to catch up.
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut ticks = 0;
    loop {
        tokio::select! {
            _ = ticker.tick() => {
                ticks += 1;
                println!("worker:   tick {ticks}");
            }
            _ = shutdown.changed() => {
                println!("worker:   shutdown received, cleaning up");
                return ticks;
            }
        }
    }
}

#[tokio::main]
async fn main() {
    // 1. `select!` waits for the first branch to finish and DROPS the others. Dropping a
    //    future cancels it: it stops at its current .await and never resumes.
    tokio::select! {
        result = slow_operation() => println!("select:   {result}"),
        _ = sleep(Duration::from_millis(100)) => println!("select:   the sleep won, slow_operation was cancelled"),
    }

    // 2. `timeout` is the shorthand for the race above. It returns Err(Elapsed) if time runs out.
    match timeout(Duration::from_millis(100), slow_operation()).await {
        Ok(result) => println!("timeout:  {result}"),
        Err(elapsed) => println!("timeout:  gave up: {elapsed}"),
    }
    let in_time = timeout(Duration::from_secs(1), slow_operation()).await;
    println!("timeout:  with more time: {in_time:?}");

    // 3. `select!` in a loop handles several event sources. `biased` checks branches from top
    //    to bottom rather than in random order. The `Some(..)` pattern disables the branch
    //    when the channel closes, and `else` runs once every branch is disabled.
    let (fast_tx, mut fast) = mpsc::channel(4);
    let (slow_tx, mut slow) = mpsc::channel(4);
    tokio::spawn(async move {
        for n in 0..3 {
            fast_tx.send(n).await.unwrap();
            sleep(Duration::from_millis(30)).await;
        }
    });
    tokio::spawn(async move {
        sleep(Duration::from_millis(50)).await;
        slow_tx.send("only message").await.unwrap();
    });
    loop {
        tokio::select! {
            biased;
            Some(n) = fast.recv() => println!("loop:     fast channel: {n}"),
            Some(text) = slow.recv() => println!("loop:     slow channel: {text}"),
            else => {
                println!("loop:     both channels closed");
                break;
            }
        }
    }

    // 4. Graceful shutdown. A real server would wait for `tokio::signal::ctrl_c()` instead
    //    of this sleep. The worker finishes its cleanup, and we can still read its result.
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let handle = tokio::spawn(worker(shutdown_rx));
    sleep(Duration::from_millis(350)).await;
    shutdown_tx.send(true).unwrap();
    println!("main:     worker stopped after {} ticks", handle.await.unwrap());
}
