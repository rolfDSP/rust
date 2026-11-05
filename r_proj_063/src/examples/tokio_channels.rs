//! Message passing between tasks: mpsc, oneshot, broadcast, and watch.
//! Run with: cargo run --example tokio_channels

use std::time::Duration;
use tokio::sync::{broadcast, mpsc, oneshot, watch};
use tokio::time::sleep;

/// A request for the actor below. It carries a oneshot sender for the reply.
enum Command {
    Add { amount: i64, reply: oneshot::Sender<i64> },
    Get { reply: oneshot::Sender<i64> },
}

/// The actor pattern: one task owns the state, and others talk to it through messages,
/// so no lock is needed. The loop ends once every sender has been dropped.
async fn counter_actor(mut commands: mpsc::Receiver<Command>) {
    let mut total = 0;
    while let Some(command) = commands.recv().await {
        match command {
            Command::Add { amount, reply } => {
                total += amount;
                let _ = reply.send(total); // Err only means the caller stopped waiting.
            }
            Command::Get { reply } => {
                let _ = reply.send(total);
            }
        }
    }
    println!("actor:     all senders gone, shutting down");
}

#[tokio::main]
async fn main() {
    // 1. mpsc: many producers, one consumer. The bound (4) gives backpressure, because
    //    `send().await` waits while the buffer is full.
    let (tx, mut rx) = mpsc::channel(4);
    for producer in 0..3 {
        let tx = tx.clone();
        tokio::spawn(async move {
            for n in 0..2 {
                tx.send(format!("producer {producer}, message {n}")).await.unwrap();
            }
        });
    }
    drop(tx); // Drop the original too, otherwise `recv` would never return None.
    while let Some(message) = rx.recv().await {
        println!("mpsc:      {message}");
    }

    // 2. oneshot: a single value, one time. Used here for request/response with an actor.
    let (tx, rx) = mpsc::channel(8);
    let actor = tokio::spawn(counter_actor(rx));
    for amount in [10, 32] {
        let (reply, response) = oneshot::channel();
        tx.send(Command::Add { amount, reply }).await.unwrap();
        println!("oneshot:   total after adding {amount} is {}", response.await.unwrap());
    }
    let (reply, response) = oneshot::channel();
    tx.send(Command::Get { reply }).await.unwrap();
    println!("oneshot:   final total is {}", response.await.unwrap());
    drop(tx);
    actor.await.unwrap();

    // 3. broadcast: every receiver gets every message. Slow receivers can lag behind and
    //    lose old messages, in which case `recv` returns `RecvError::Lagged`.
    let (tx, _) = broadcast::channel(16);
    let mut listeners = Vec::new();
    for id in 0..2 {
        let mut rx = tx.subscribe(); // Subscribe before sending, or messages are missed.
        listeners.push(tokio::spawn(async move {
            while let Ok(message) = rx.recv().await {
                println!("broadcast: listener {id} got {message}");
            }
        }));
    }
    tx.send("hello").unwrap();
    tx.send("world").unwrap();
    drop(tx);
    for listener in listeners {
        listener.await.unwrap();
    }

    // 4. watch: receivers only see the latest value, which suits config or status updates.
    //    Fast updates may be skipped, so don't use it where every value matters.
    let (tx, mut rx) = watch::channel("starting");
    let observer = tokio::spawn(async move {
        while rx.changed().await.is_ok() {
            println!("watch:     status is now {}", *rx.borrow_and_update());
        }
    });
    for status in ["loading", "ready"] {
        sleep(Duration::from_millis(50)).await;
        tx.send(status).unwrap();
    }
    sleep(Duration::from_millis(50)).await;
    drop(tx);
    observer.await.unwrap();
}
