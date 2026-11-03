//! Transferring data objects between threads using the `Send` and `Sync` traits.
//!
//! - `Send`: a value of type T may be *moved* to another thread (ownership transfer).
//! - `Sync`: a `&T` may be *shared* between threads (T is Sync  <=>  &T is Send).
//!
//! Both are auto traits: the compiler implements them automatically for a struct
//! if all of its fields are Send / Sync. You only write `unsafe impl` when the
//! compiler cannot prove it (e.g. raw pointers) and you guarantee it yourself.

use std::sync::{Arc, Mutex, mpsc};
use std::thread;

/// A plain data object. All fields (String, Vec<u32>, u64) are Send + Sync,
/// so `Message` is automatically Send + Sync as well.
#[derive(Debug)]
struct Message {
    id: u64,
    sender: String,
    payload: Vec<u32>,
}

/// Compile-time check helpers: these only compile if T implements the trait.
fn assert_send<T: Send>() {}
fn assert_sync<T: Sync>() {}

/// A type the compiler does NOT consider Send/Sync, because it holds a raw pointer.
struct RawBuffer {
    ptr: *mut u8,
    len: usize,
}

impl RawBuffer {
    fn new(len: usize) -> Self {
        let mut v = vec![0u8; len].into_boxed_slice();
        let ptr = v.as_mut_ptr();
        std::mem::forget(v); // ownership now lives in `ptr`
        RawBuffer { ptr, len }
    }

    fn as_slice_mut(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.len) }
    }
}

impl Drop for RawBuffer {
    fn drop(&mut self) {
        unsafe {
            drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(self.ptr, self.len)));
        }
    }
}

// SAFETY: RawBuffer exclusively owns its allocation (like Box<[u8]>), so moving
// it to another thread is fine. Mutation requires &mut self, so a shared &RawBuffer
// gives no way to cause a data race -> Sync is also sound.
unsafe impl Send for RawBuffer {}
unsafe impl Sync for RawBuffer {}

fn main() {
    // Verified at compile time:
    assert_send::<Message>();
    assert_sync::<Message>();
    assert_send::<RawBuffer>(); // only compiles because of the unsafe impl above
    // assert_send::<std::rc::Rc<Message>>(); // ERROR: Rc is neither Send nor Sync

    example_1_move_into_thread();
    example_2_channel();
    example_3_shared_arc_mutex();
    example_4_custom_send();
}

/// 1) Send: move ownership of a value into a spawned thread and get it back via join.
fn example_1_move_into_thread() {
    println!("--- 1) move into thread (Send) ---");
    let msg = Message { id: 1, sender: "main".into(), payload: vec![1, 2, 3] };

    // `move` transfers ownership; thread::spawn requires the closure to be Send + 'static.
    let handle = thread::spawn(move || {
        let mut msg = msg;
        msg.payload.iter_mut().for_each(|x| *x *= 10);
        msg.sender = "worker".into();
        msg // return ownership to the joining thread
    });

    // `msg` is no longer usable here — it was moved.
    let msg_back = handle.join().unwrap();
    println!("got back: {msg_back:?}");
}

/// 2) Send: transfer objects from a producer thread to a consumer thread via a channel.
fn example_2_channel() {
    println!("--- 2) channel (Send) ---");
    // mpsc::Sender<T> is only Send if T: Send.
    let (tx, rx) = mpsc::channel::<Message>();

    let producer = thread::spawn(move || {
        for i in 0..3 {
            let msg = Message { id: i, sender: "producer".into(), payload: vec![i as u32; 3] };
            tx.send(msg).unwrap(); // ownership of `msg` moves into the channel
        }
        // tx dropped here -> channel closes -> consumer loop ends
    });

    let consumer = thread::spawn(move || {
        for msg in rx {
            println!("consumer received #{}: {msg:?}", msg.id);
        }
    });

    producer.join().unwrap();
    consumer.join().unwrap();
}

/// 3) Sync: share ONE object between several threads.
///    Arc<T> is Send + Sync only if T: Send + Sync.
///    Mutex<T> turns any T: Send into something Sync (it serializes access).
fn example_3_shared_arc_mutex() {
    println!("--- 3) shared object with Arc<Mutex<T>> (Sync) ---");
    let shared = Arc::new(Mutex::new(Message { id: 42, sender: "shared".into(), payload: vec![] }));

    let handles: Vec<_> = (0..4)
        .map(|t| {
            let shared = Arc::clone(&shared); // each thread gets its own Arc handle
            thread::spawn(move || {
                let mut guard = shared.lock().unwrap();
                guard.payload.push(t);
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }
    println!("final shared object: {:?}", shared.lock().unwrap());

    // Read-only sharing without a Mutex works too, since Message: Sync.
    // thread::scope lets threads borrow from the stack (no Arc / 'static needed).
    let readonly = Message { id: 7, sender: "readonly".into(), payload: vec![5, 6, 7] };
    thread::scope(|s| {
        for _ in 0..2 {
            s.spawn(|| {
                // `&readonly` is sent to this thread — allowed because Message: Sync.
                let sum: u32 = readonly.payload.iter().sum();
                println!("scoped reader {:?}: sum = {sum}", thread::current().id());
            });
        }
    });
}

/// 4) A type with a manual `unsafe impl Send` moved into another thread.
fn example_4_custom_send() {
    println!("--- 4) custom type with unsafe impl Send ---");
    let mut buf = RawBuffer::new(4);
    buf.as_slice_mut().copy_from_slice(&[1, 2, 3, 4]);

    let handle = thread::spawn(move || {
        // Without `unsafe impl Send for RawBuffer` this would not compile:
        // "`*mut u8` cannot be sent between threads safely"
        let s = buf.as_slice_mut();
        s.reverse();
        println!("worker reversed buffer: {s:?}");
    });
    handle.join().unwrap();
}
