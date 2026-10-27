//! A comprehensive tour of Rust's `VecDeque<T>` collection.
//!
//! `VecDeque` is a double-ended queue implemented as a growable ring
//! buffer. Pushing and popping at *both* ends is O(1), which makes it
//! the right choice for queues, sliding windows and bounded histories.
//!
//! Each section is a small function that demonstrates one group of
//! features and prints its results, so you can run the program and
//! compare the output with the code.

use std::collections::{HashSet, VecDeque};

#[derive(Debug, Clone, PartialEq)]
struct Task {
    name: String,
    remaining: u32,
}

fn main() {
    creating_deques();
    adding_and_removing();
    accessing_elements();
    iterating();
    ring_buffer_internals();
    rotating();
    searching_and_sorting();
    ranges_draining_and_retaining();
    splitting_and_combining();
    converting_to_and_from_vec();
    capacity_and_memory();

    // Practical use cases.
    fifo_queue_bfs();
    round_robin_scheduler();
    bounded_history();
    sliding_window_maximum();
    palindrome_check();
}

fn section(title: &str) {
    println!("\n=== {title} ===");
}

/// Different ways to construct a VecDeque.
fn creating_deques() {
    section("Creating deques");

    // Empty deque; the element type is given explicitly.
    let empty: VecDeque<i32> = VecDeque::new();
    println!("empty: {empty:?}, len = {}", empty.len());

    // There is no vecdeque! macro, but From<[T; N]> is just as convenient.
    let numbers = VecDeque::from([1, 2, 3, 4, 5]);
    println!("from array: {numbers:?}");

    // Pre-allocate space without adding any elements.
    let with_cap: VecDeque<String> = VecDeque::with_capacity(10);
    println!("with_capacity: len = {}, capacity >= {}", with_cap.len(), with_cap.capacity());

    // Collect from an iterator.
    let squares: VecDeque<u32> = (1..=5).map(|x| x * x).collect();
    println!("collected squares: {squares:?}");

    // Convert from a Vec (cheap: reuses the Vec's allocation).
    let from_vec: VecDeque<char> = vec!['a', 'b', 'c'].into();
    println!("from Vec: {from_vec:?}");

    // Repeated values via an iterator.
    let zeros: VecDeque<u8> = std::iter::repeat_n(0, 4).collect();
    println!("repeated value: {zeros:?}");
}

/// push/pop at both ends, insert, remove, swap_remove_*, truncate, clear.
fn adding_and_removing() {
    section("Adding and removing elements");

    let mut d = VecDeque::new();
    d.push_back(2);
    d.push_back(3);
    d.push_front(1);
    d.push_front(0);
    println!("after push_back(2,3) and push_front(1,0): {d:?}");

    // Both pops return Option<T> because the deque may be empty.
    let front = d.pop_front();
    let back = d.pop_back();
    println!("pop_front -> {front:?}, pop_back -> {back:?}, now {d:?}");

    // insert and remove work at arbitrary indices (O(min(i, len - i))).
    d.insert(1, 99);
    println!("insert(1, 99): {d:?}");
    let removed = d.remove(1); // Option<T>, unlike Vec::remove
    println!("remove(1) -> {removed:?}, now {d:?}");

    // swap_remove_* is O(1): the gap is filled from the chosen end.
    let mut letters = VecDeque::from(['a', 'b', 'c', 'd', 'e']);
    let x = letters.swap_remove_back(1);
    println!("swap_remove_back(1) -> {x:?}, now {letters:?}");
    let y = letters.swap_remove_front(2);
    println!("swap_remove_front(2) -> {y:?}, now {letters:?}");

    // Resize, truncate and clear.
    let mut r = VecDeque::from([1, 2, 3]);
    r.resize(6, 0);
    println!("resize(6, 0): {r:?}");
    r.truncate(2);
    println!("truncate(2): {r:?}");
    r.clear();
    println!("clear: {r:?}, is_empty = {}", r.is_empty());

    // Popping from an empty deque is safe.
    println!("pop_front on empty: {:?}", r.pop_front());
}

/// Indexing, get, front/back and their mutable variants.
fn accessing_elements() {
    section("Accessing elements");

    let mut d = VecDeque::from([10, 20, 30, 40]);

    // Indexing panics on out-of-bounds; get returns Option.
    println!("d[0] = {}, d[3] = {}", d[0], d[3]);
    println!("get(1) = {:?}, get(10) = {:?}", d.get(1), d.get(10));

    // Peek at both ends without removing.
    println!("front = {:?}, back = {:?}", d.front(), d.back());

    // Mutable access.
    d[1] += 1;
    if let Some(f) = d.front_mut() {
        *f = 11;
    }
    if let Some(b) = d.back_mut() {
        *b *= 10;
    }
    if let Some(x) = d.get_mut(2) {
        *x = 33;
    }
    println!("after mutations: {d:?}");

    // Swap two elements by index.
    d.swap(0, 3);
    println!("swap(0, 3): {d:?}");

    // Empty deque: front/back are None.
    let empty: VecDeque<i32> = VecDeque::new();
    println!("empty front = {:?}, back = {:?}", empty.front(), empty.back());
}

/// Borrowing, mutable and consuming iteration, in both directions.
fn iterating() {
    section("Iterating");

    let mut d = VecDeque::from([1, 2, 3, 4]);

    print!("iter():       ");
    for x in &d {
        print!("{x} ");
    }
    println!();

    // VecDeque iterators are double-ended, so rev() is free.
    let reversed: Vec<_> = d.iter().rev().collect();
    println!("iter().rev(): {reversed:?}");

    for x in d.iter_mut() {
        *x *= 10;
    }
    println!("after iter_mut: {d:?}");

    // Iterator adapters work as usual.
    let sum: i32 = d.iter().sum();
    let evens: Vec<_> = d.iter().enumerate().filter(|(i, _)| i % 2 == 0).collect();
    println!("sum = {sum}, even-index elements = {evens:?}");

    // into_iter consumes the deque.
    let strings: Vec<String> = d.into_iter().map(|x| format!("#{x}")).collect();
    println!("into_iter + map: {strings:?}");
}

/// The ring buffer layout: as_slices and make_contiguous.
fn ring_buffer_internals() {
    section("Ring buffer internals");

    // Pushing to the front of a deque that has data at the back makes
    // the contents wrap around the end of the buffer.
    let mut d: VecDeque<i32> = VecDeque::with_capacity(8);
    d.extend([3, 4, 5]);
    d.push_front(2);
    d.push_front(1);
    println!("logical order: {d:?}");

    // as_slices exposes the two physical halves of the buffer.
    let (a, b) = d.as_slices();
    println!("as_slices: front part = {a:?}, back part = {b:?}");

    // make_contiguous rearranges in place so all data is in one slice.
    // This lets you use any slice method (sort, binary_search, windows...).
    let slice: &mut [i32] = d.make_contiguous();
    slice.reverse();
    println!("make_contiguous + reverse: {d:?}");
    let (a, b) = d.as_slices();
    println!("as_slices now: {a:?} and {b:?}");

    // as_mut_slices allows modifying both halves without moving data.
    let (a, b) = d.as_mut_slices();
    a.iter_mut().chain(b.iter_mut()).for_each(|x| *x += 100);
    println!("after as_mut_slices: {d:?}");
}

/// rotate_left and rotate_right.
fn rotating() {
    section("Rotating");

    let mut d: VecDeque<_> = (1..=6).collect();
    println!("start:          {d:?}");

    // rotate_left(n): the first n elements move to the back.
    d.rotate_left(2);
    println!("rotate_left(2): {d:?}");

    // rotate_right(n): the last n elements move to the front.
    d.rotate_right(3);
    println!("rotate_right(3):{d:?}");

    // A manual rotation by one, using the O(1) end operations.
    if let Some(x) = d.pop_front() {
        d.push_back(x);
    }
    println!("manual rotate:  {d:?}");
}

/// contains, position, binary_search, partition_point and sorting.
fn searching_and_sorting() {
    section("Searching and sorting");

    let mut d = VecDeque::from([42, 7, 19, 3, 25, 7]);

    println!("contains(&19) = {}", d.contains(&19));
    println!("position of 7 = {:?}", d.iter().position(|&x| x == 7));
    println!("last position of 7 = {:?}", d.iter().rposition(|&x| x == 7));
    println!("max = {:?}, min = {:?}", d.iter().max(), d.iter().min());

    // VecDeque has no sort method; sort the contiguous slice instead.
    d.make_contiguous().sort();
    println!("sorted: {d:?}");

    // binary_search and partition_point are available directly.
    println!("binary_search(&25) = {:?}", d.binary_search(&25));
    println!("binary_search(&10) = {:?} (insertion point)", d.binary_search(&10));

    // Insert while keeping the deque sorted.
    let value = 10;
    let pos = d.partition_point(|&x| x < value);
    d.insert(pos, value);
    println!("sorted insert of {value}: {d:?}");

    // Sort by a key, descending.
    d.make_contiguous().sort_by(|a, b| b.cmp(a));
    println!("descending: {d:?}");
}

/// range, range_mut, drain, retain and retain_mut.
fn ranges_draining_and_retaining() {
    section("Ranges, draining and retaining");

    let mut d: VecDeque<_> = (1..=10).collect();

    // range gives an iterator over a sub-range (there are no &d[a..b] slices).
    let middle: Vec<_> = d.range(3..6).collect();
    println!("range(3..6): {middle:?}");

    for x in d.range_mut(..3) {
        *x *= 100;
    }
    println!("range_mut(..3) *= 100: {d:?}");

    // drain removes a range and yields the removed elements.
    let drained: Vec<_> = d.drain(2..5).collect();
    println!("drain(2..5) -> {drained:?}, remaining {d:?}");

    // retain keeps only matching elements, preserving order.
    d.retain(|&x| x % 2 == 0);
    println!("retain even: {d:?}");

    // retain_mut can modify the elements it keeps.
    d.retain_mut(|x| {
        *x += 1;
        *x < 100
    });
    println!("retain_mut (+1, keep < 100): {d:?}");

    // Drain everything (empties the deque, keeps the allocation).
    let all: Vec<_> = d.drain(..).collect();
    println!("drain(..) -> {all:?}, deque is_empty = {}", d.is_empty());
}

/// extend, append, split_off and deduplication.
fn splitting_and_combining() {
    section("Splitting and combining");

    let mut a = VecDeque::from([1, 2, 3]);
    let mut b = VecDeque::from([4, 5, 6]);

    // append moves all elements of b to the back of a, leaving b empty.
    a.append(&mut b);
    println!("append: a = {a:?}, b = {b:?}");

    // extend adds from any iterator to the back.
    a.extend([7, 8]);
    println!("extend: {a:?}");

    // To prepend several elements, push_front them in reverse order.
    for x in [-1, 0].into_iter().rev() {
        a.push_front(x);
    }
    println!("prepend [-1, 0]: {a:?}");

    // split_off(at) returns the tail starting at `at`.
    let tail = a.split_off(5);
    println!("split_off(5): head = {a:?}, tail = {tail:?}");

    // Removing duplicates while preserving first occurrence.
    let words = VecDeque::from(["a", "b", "a", "c", "b", "d"]);
    let mut seen = HashSet::new();
    let unique: VecDeque<_> = words.into_iter().filter(|w| seen.insert(*w)).collect();
    println!("unique (order kept): {unique:?}");

    // Equality compares the logical contents, regardless of layout.
    let mut x = VecDeque::from([2, 3]);
    x.push_front(1);
    let y: VecDeque<_> = (1..=3).collect();
    println!("[1,2,3] == built from push_front: {}", x == y);
    println!("compares with Vec and arrays too: {}", x == vec![1, 2, 3] && x == [1, 2, 3]);
}

/// Moving between Vec and VecDeque.
fn converting_to_and_from_vec() {
    section("Converting to and from Vec");

    let v = vec![1, 2, 3];
    let mut d = VecDeque::from(v); // O(1), reuses the buffer
    d.push_front(0);

    // Into<Vec<T>> rearranges the data to be contiguous, then reuses the
    // buffer, so it is at most O(n) and never reallocates.
    let back_to_vec: Vec<i32> = d.into();
    println!("VecDeque -> Vec: {back_to_vec:?}");

    // Alternatively, collect into a new Vec (always copies/moves elements).
    let d2 = VecDeque::from([5, 6, 7]);
    let collected: Vec<_> = d2.iter().copied().collect();
    println!("collected copy: {collected:?}, original still usable: {d2:?}");

    // Getting a single slice view without converting.
    let mut d3 = VecDeque::from([2, 3]);
    d3.push_front(1);
    let slice: &[i32] = d3.make_contiguous();
    println!("as one slice: {slice:?}, windows(2): {:?}", slice.windows(2).collect::<Vec<_>>());
}

/// Capacity management.
fn capacity_and_memory() {
    section("Capacity and memory");

    let mut d: VecDeque<u64> = VecDeque::new();
    println!("new: len = {}, capacity = {}", d.len(), d.capacity());

    let mut last_cap = d.capacity();
    for i in 0..20 {
        d.push_back(i);
        if d.capacity() != last_cap {
            println!("  grew to capacity {} at len {}", d.capacity(), d.len());
            last_cap = d.capacity();
        }
    }

    d.reserve(100);
    println!("after reserve(100): capacity >= {}", d.capacity());

    d.truncate(5);
    d.shrink_to_fit();
    println!("after truncate(5) + shrink_to_fit: len = {}, capacity = {}", d.len(), d.capacity());

    println!(
        "size of VecDeque<u64> handle on the stack: {} bytes",
        std::mem::size_of::<VecDeque<u64>>()
    );
}

// ---------------------------------------------------------------------
// Practical use cases
// ---------------------------------------------------------------------

/// Breadth-first search on a small grid, using VecDeque as a FIFO queue.
fn fifo_queue_bfs() {
    section("Use case: BFS shortest path on a grid");

    let grid = [
        "S.#.....",
        ".##.###.",
        "....#...",
        ".##...#E",
    ];
    let grid: Vec<Vec<char>> = grid.iter().map(|row| row.chars().collect()).collect();
    let (rows, cols) = (grid.len(), grid[0].len());

    let find = |c: char| -> (usize, usize) {
        for (r, row) in grid.iter().enumerate() {
            if let Some(col) = row.iter().position(|&x| x == c) {
                return (r, col);
            }
        }
        panic!("{c} not found");
    };
    let (start, end) = (find('S'), find('E'));

    let mut dist = vec![vec![None; cols]; rows];
    let mut queue = VecDeque::new();
    dist[start.0][start.1] = Some(0);
    queue.push_back(start);

    // Pop from the front, push neighbours to the back.
    while let Some((r, c)) = queue.pop_front() {
        if (r, c) == end {
            break;
        }
        let d = dist[r][c].unwrap();
        let neighbours = [
            (r.wrapping_sub(1), c),
            (r + 1, c),
            (r, c.wrapping_sub(1)),
            (r, c + 1),
        ];
        for (nr, nc) in neighbours {
            if nr < rows && nc < cols && grid[nr][nc] != '#' && dist[nr][nc].is_none() {
                dist[nr][nc] = Some(d + 1);
                queue.push_back((nr, nc));
            }
        }
    }

    for row in &grid {
        println!("  {}", row.iter().collect::<String>());
    }
    match dist[end.0][end.1] {
        Some(steps) => println!("shortest path from S to E: {steps} steps"),
        None => println!("E is unreachable"),
    }
}

/// Round-robin scheduling: take a task from the front, run one time
/// slice, and put it back at the end if it is not finished.
fn round_robin_scheduler() {
    section("Use case: round-robin scheduler");

    let mut queue: VecDeque<Task> = [("compile", 5), ("test", 3), ("deploy", 2)]
        .into_iter()
        .map(|(name, remaining)| Task { name: name.to_string(), remaining })
        .collect();

    const SLICE: u32 = 2;
    let mut clock = 0;

    while let Some(mut task) = queue.pop_front() {
        let run = task.remaining.min(SLICE);
        task.remaining -= run;
        clock += run;
        if task.remaining == 0 {
            println!("  t={clock:>2}: {} finished", task.name);
        } else {
            println!("  t={clock:>2}: {} ran {run}, {} left", task.name, task.remaining);
            queue.push_back(task);
        }
    }
}

/// A fixed-size history that drops the oldest entry when full.
struct History<T> {
    items: VecDeque<T>,
    limit: usize,
}

impl<T: std::fmt::Debug> History<T> {
    fn new(limit: usize) -> Self {
        Self { items: VecDeque::with_capacity(limit), limit }
    }

    fn record(&mut self, item: T) {
        if self.items.len() == self.limit {
            self.items.pop_front();
        }
        self.items.push_back(item);
    }

    fn undo(&mut self) -> Option<T> {
        self.items.pop_back()
    }

    fn latest(&self, n: usize) -> impl Iterator<Item = &T> {
        self.items.iter().rev().take(n)
    }
}

fn bounded_history() {
    section("Use case: bounded history with undo");

    let mut history = History::new(3);
    for cmd in ["open", "edit", "save", "close", "reopen"] {
        history.record(cmd);
        println!("  record {cmd:<7} -> {:?}", history.items);
    }
    println!("  undo -> {:?}, now {:?}", history.undo(), history.items);
    println!("  latest 2 (newest first): {:?}", history.latest(2).collect::<Vec<_>>());
}

/// Sliding window maximum in O(n) using a monotonic deque of indices.
fn sliding_window_maximum() {
    section("Use case: sliding window maximum");

    let data = [1, 3, -1, -3, 5, 3, 6, 7];
    let k = 3;

    // The deque holds indices whose values are in decreasing order.
    let mut window: VecDeque<usize> = VecDeque::new();
    let mut maxima = Vec::new();

    for (i, &value) in data.iter().enumerate() {
        // Drop the index that slid out of the window.
        if window.front().is_some_and(|&f| f + k <= i) {
            window.pop_front();
        }
        // Drop smaller values from the back; they can never be a maximum.
        while window.back().is_some_and(|&b| data[b] <= value) {
            window.pop_back();
        }
        window.push_back(i);

        if i + 1 >= k {
            maxima.push(data[*window.front().unwrap()]);
        }
    }

    println!("data = {data:?}, k = {k}");
    println!("window maxima = {maxima:?}");

    // Also a simple moving average with a running sum.
    let mut buf = VecDeque::with_capacity(k);
    let mut sum = 0;
    let averages: Vec<f64> = data
        .iter()
        .filter_map(|&x| {
            buf.push_back(x);
            sum += x;
            if buf.len() > k {
                sum -= buf.pop_front().unwrap();
            }
            (buf.len() == k).then(|| sum as f64 / k as f64)
        })
        .collect();
    println!("moving averages = {averages:.2?}");
}

/// Compare characters from both ends at once.
fn palindrome_check() {
    section("Use case: palindrome check from both ends");

    fn is_palindrome(s: &str) -> bool {
        let mut chars: VecDeque<char> = s
            .chars()
            .filter(|c| c.is_alphanumeric())
            .map(|c| c.to_ascii_lowercase())
            .collect();
        while chars.len() > 1 {
            if chars.pop_front() != chars.pop_back() {
                return false;
            }
        }
        true
    }

    for s in ["racecar", "A man, a plan, a canal: Panama", "VecDeque", ""] {
        println!("  {s:?} -> {}", is_palindrome(s));
    }
}
