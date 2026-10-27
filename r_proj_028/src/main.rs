//! A comprehensive tour of Rust's `Vec<T>` collection.
//!
//! Each section is a small function that demonstrates one group of
//! features and prints its results, so you can run the program and
//! compare the output with the code.

use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq)]
struct Person {
    name: String,
    age: u32,
}

fn main() {
    creating_vectors();
    adding_and_removing();
    accessing_elements();
    iterating();
    slices_and_borrowing();
    searching();
    sorting();
    deduplicating_and_filtering();
    transforming_with_iterators();
    splitting_and_combining();
    capacity_and_memory();
    vectors_of_structs();
    nested_vectors();
    ownership_and_moves();
}

fn section(title: &str) {
    println!("\n=== {title} ===");
}

/// Different ways to construct a Vec.
fn creating_vectors() {
    section("Creating vectors");

    // Empty vector; the element type is given explicitly.
    let empty: Vec<i32> = Vec::new();
    println!("empty: {empty:?}, len = {}", empty.len());

    // The vec! macro with a list of elements.
    let numbers = vec![1, 2, 3, 4, 5];
    println!("vec! literal: {numbers:?}");

    // The vec! macro with a repeated value: [value; count].
    let zeros = vec![0; 5];
    println!("repeated value: {zeros:?}");

    // Pre-allocate space without adding any elements.
    let with_cap: Vec<String> = Vec::with_capacity(10);
    println!("with_capacity: len = {}, capacity >= {}", with_cap.len(), with_cap.capacity());

    // Collect from an iterator.
    let squares: Vec<u32> = (1..=5).map(|x| x * x).collect();
    println!("collected squares: {squares:?}");

    // Convert from arrays, slices and strings.
    let from_array = Vec::from([10, 20, 30]);
    let from_slice = [7, 8, 9].to_vec();
    let chars: Vec<char> = "hello".chars().collect();
    let bytes: Vec<u8> = "hi".into();
    println!("from array: {from_array:?}, from slice: {from_slice:?}");
    println!("chars: {chars:?}, bytes: {bytes:?}");
}

/// push, pop, insert, remove, swap_remove, truncate, clear, drain, retain.
fn adding_and_removing() {
    section("Adding and removing elements");

    let mut v = Vec::new();
    v.push(1);
    v.push(2);
    v.push(3);
    println!("after push: {v:?}");

    // pop returns Option<T> because the vector may be empty.
    let last = v.pop();
    println!("popped {last:?}, now {v:?}");

    // insert shifts later elements to the right: O(n).
    v.insert(0, 100);
    println!("after insert(0, 100): {v:?}");

    // remove shifts later elements to the left: O(n).
    let removed = v.remove(1);
    println!("removed index 1 -> {removed}, now {v:?}");

    // swap_remove is O(1) but does not preserve order.
    let mut letters = vec!['a', 'b', 'c', 'd', 'e'];
    let taken = letters.swap_remove(1);
    println!("swap_remove(1) -> {taken}, now {letters:?}");

    // extend appends everything from an iterator.
    let mut nums = vec![1, 2, 3];
    nums.extend([4, 5, 6]);
    nums.extend_from_slice(&[7, 8]);
    println!("after extend: {nums:?}");

    // append moves all elements out of another vector.
    let mut other = vec![9, 10];
    nums.append(&mut other);
    println!("after append: {nums:?}, other is now {other:?}");

    // drain removes a range and yields the removed elements.
    let drained: Vec<i32> = nums.drain(2..5).collect();
    println!("drained {drained:?}, remaining {nums:?}");

    // retain keeps only elements matching a predicate.
    nums.retain(|&x| x % 2 == 0);
    println!("after retain(even): {nums:?}");

    // truncate shortens; clear empties (capacity is kept).
    nums.truncate(2);
    println!("after truncate(2): {nums:?}");
    nums.clear();
    println!("after clear: {nums:?}, is_empty = {}", nums.is_empty());

    // resize grows (filling with a value) or shrinks.
    let mut r = vec![1, 2];
    r.resize(5, 0);
    println!("resize(5, 0): {r:?}");
}

/// Indexing, get, first/last, and mutable access.
fn accessing_elements() {
    section("Accessing elements");

    let mut v = vec![10, 20, 30, 40, 50];

    // Indexing panics when out of bounds.
    println!("v[2] = {}", v[2]);

    // get returns an Option and never panics.
    match v.get(10) {
        Some(x) => println!("v.get(10) = {x}"),
        None => println!("v.get(10) = None (out of bounds)"),
    }

    println!("first = {:?}, last = {:?}", v.first(), v.last());

    // Mutable access by index or via get_mut.
    v[0] = 11;
    if let Some(x) = v.get_mut(1) {
        *x += 1;
    }
    if let Some(last) = v.last_mut() {
        *last = 99;
    }
    println!("after mutation: {v:?}");

    // swap two positions.
    v.swap(0, 4);
    println!("after swap(0, 4): {v:?}");

    // Pattern matching on a slice of the vector.
    match v.as_slice() {
        [first, .., last] => println!("pattern: first = {first}, last = {last}"),
        _ => println!("fewer than two elements"),
    }
}

/// The three ways to iterate: by reference, by mutable reference, by value.
fn iterating() {
    section("Iterating");

    let mut v = vec![1, 2, 3];

    // Shared borrow: v is still usable afterwards.
    for x in &v {
        print!("{x} ");
    }
    println!("(by reference)");

    // Mutable borrow: modify each element in place.
    for x in &mut v {
        *x *= 10;
    }
    println!("after &mut loop: {v:?}");

    // With an index.
    for (i, x) in v.iter().enumerate() {
        print!("[{i}]={x} ");
    }
    println!();

    // Reverse iteration.
    let reversed: Vec<_> = v.iter().rev().collect();
    println!("reversed: {reversed:?}");

    // By value: consumes the vector, v cannot be used afterwards.
    let total: i32 = v.into_iter().sum();
    println!("sum (into_iter consumed the vec): {total}");
}

/// A &Vec<T> coerces to &[T]; functions should usually take slices.
fn slices_and_borrowing() {
    section("Slices and borrowing");

    fn average(values: &[f64]) -> Option<f64> {
        if values.is_empty() {
            None
        } else {
            Some(values.iter().sum::<f64>() / values.len() as f64)
        }
    }

    fn double_all(values: &mut [i32]) {
        for v in values {
            *v *= 2;
        }
    }

    let data = vec![1.0, 2.5, 4.0, 8.5];
    println!("average of whole vec: {:?}", average(&data));
    println!("average of data[1..3]: {:?}", average(&data[1..3]));

    let mut ints = vec![1, 2, 3, 4, 5];
    double_all(&mut ints[2..]);
    println!("double_all on ints[2..]: {ints:?}");

    // Ranges of all kinds.
    println!("[..2] = {:?}, [3..] = {:?}, [1..=3] = {:?}", &ints[..2], &ints[3..], &ints[1..=3]);
}

/// contains, position, find, binary_search, any, all.
fn searching() {
    section("Searching");

    let v = Vec::from([3, 8, 15, 23, 42, 57]);

    println!("contains(&15) = {}", v.contains(&15));
    println!("position of 23 = {:?}", v.iter().position(|&x| x == 23));
    println!("first > 20 = {:?}", v.iter().find(|&&x| x > 20));
    println!("any even = {}", v.iter().any(|x| x % 2 == 0));
    println!("all positive = {}", v.iter().all(|&x| x > 0));

    // binary_search requires a sorted vec.
    // Ok(index) if found, Err(insert_position) otherwise.
    println!("binary_search(&42) = {:?}", v.binary_search(&42));
    println!("binary_search(&20) = {:?}", v.binary_search(&20));

    println!("max = {:?}, min = {:?}", v.iter().max(), v.iter().min());
}

/// sort, sort_unstable, sort_by, sort_by_key, reverse, floats.
fn sorting() {
    section("Sorting");

    let mut v = vec![5, 3, 9, 1, 7];
    v.sort();
    println!("sort: {v:?}");

    v.sort_by(|a, b| b.cmp(a));
    println!("sort_by descending: {v:?}");

    v.reverse();
    println!("reverse: {v:?}");

    // sort_unstable is usually faster and doesn't allocate.
    let mut u = vec![4, 4, 2, 8, 1];
    u.sort_unstable();
    println!("sort_unstable: {u:?}");

    // Sort strings by length, then alphabetically.
    let mut words = vec!["pear", "fig", "banana", "kiwi", "apple"];
    words.sort_by_key(|w| (w.len(), *w));
    println!("sort_by_key(len, alpha): {words:?}");

    // f64 doesn't implement Ord; use total_cmp.
    let mut floats: Vec<f64> = vec![3.2, -1.0, 2.7, 0.5];
    floats.sort_by(|a, b| a.total_cmp(b));
    println!("sorted floats: {floats:?}");

    println!("is_sorted: {}", floats.is_sorted_by(|a, b| a <= b));
}

/// dedup and friends, plus removing duplicates from unsorted data.
fn deduplicating_and_filtering() {
    section("Deduplicating and filtering");

    // dedup removes only *consecutive* duplicates.
    let mut v = vec![1, 1, 2, 3, 3, 3, 1, 4];
    v.dedup();
    println!("dedup: {v:?}");

    // Sort + dedup gives unique values.
    let mut s = vec![5, 1, 5, 2, 1, 3];
    s.sort();
    s.dedup();
    println!("sort + dedup: {s:?}");

    // Keep first occurrence order using a HashSet.
    let words = vec!["b", "a", "b", "c", "a"];
    let mut seen = HashSet::new();
    let unique: Vec<_> = words.into_iter().filter(|w| seen.insert(*w)).collect();
    println!("order-preserving unique: {unique:?}");

    // dedup_by_key: treat values as equal when key matches.
    let mut nums = vec![10, 11, 20, 21, 22, 30];
    nums.dedup_by_key(|x| *x / 10);
    println!("dedup_by_key(x / 10): {nums:?}");
}

/// map, filter, fold, zip, chunks, windows, partition.
fn transforming_with_iterators() {
    section("Transforming with iterators");

    let v: Vec<i32> = (1..=10).collect();

    let evens_squared: Vec<i32> = v.iter().filter(|&&x| x % 2 == 0).map(|x| x * x).collect();
    println!("evens squared: {evens_squared:?}");

    let product: i32 = v.iter().take(5).product();
    println!("product of first 5: {product}");

    let (small, large): (Vec<i32>, Vec<i32>) = v.iter().partition(|&&x| x <= 5);
    println!("partition: {small:?} / {large:?}");

    let names = ["Ann", "Bob", "Cid"];
    let scores = vec![90, 75, 82];
    let paired: Vec<(&str, i32)> = names.iter().copied().zip(scores).collect();
    println!("zip: {paired:?}");

    for chunk in v.chunks(4) {
        print!("{chunk:?} ");
    }
    println!("<- chunks(4)");

    let diffs: Vec<i32> = [1, 4, 9, 16, 25].windows(2).map(|w| w[1] - w[0]).collect();
    println!("windows(2) differences: {diffs:?}");

    // flat_map / flatten
    let nested = vec![vec![1, 2], vec![3], vec![4, 5, 6]];
    let flat: Vec<i32> = nested.into_iter().flatten().collect();
    println!("flatten: {flat:?}");

    // Collecting Results: stops at the first error.
    let inputs = ["1", "2", "x", "4"];
    let parsed: Result<Vec<i32>, _> = inputs.iter().map(|s| s.parse::<i32>()).collect();
    println!("collect into Result: {parsed:?}");

    let joined = ["a", "b", "c"].join("-");
    println!("join: {joined}");
}

/// split_off, split_at, concat, repeat, rotate.
fn splitting_and_combining() {
    section("Splitting and combining");

    let mut v = vec![1, 2, 3, 4, 5, 6];
    let tail = v.split_off(4);
    println!("split_off(4): head = {v:?}, tail = {tail:?}");

    let (left, right) = v.split_at(2);
    println!("split_at(2): {left:?} | {right:?}");

    let combined = [v.as_slice(), tail.as_slice()].concat();
    println!("concat: {combined:?}");

    let repeated = [1, 2].repeat(3);
    println!("repeat(3): {repeated:?}");

    let mut r = vec![1, 2, 3, 4, 5];
    r.rotate_left(2);
    println!("rotate_left(2): {r:?}");

    let csv = "red,green,blue";
    let parts: Vec<&str> = csv.split(',').collect();
    println!("split string into vec: {parts:?}");
}

/// len vs capacity, reserve, shrink_to_fit.
fn capacity_and_memory() {
    section("Capacity and memory");

    let mut v: Vec<i32> = Vec::new();
    println!("new: len = {}, capacity = {}", v.len(), v.capacity());

    let mut last_cap = v.capacity();
    for i in 0..20 {
        v.push(i);
        if v.capacity() != last_cap {
            println!("  reallocated at len {} -> capacity {}", v.len(), v.capacity());
            last_cap = v.capacity();
        }
    }

    v.reserve(100);
    println!("after reserve(100): capacity >= {}", v.capacity());

    v.truncate(5);
    v.shrink_to_fit();
    println!("after truncate(5) + shrink_to_fit: len = {}, capacity = {}", v.len(), v.capacity());

    // A Vec on the stack is just (pointer, capacity, length).
    println!("size_of::<Vec<i32>>() = {} bytes", std::mem::size_of::<Vec<i32>>());
}

/// Working with a Vec of user-defined structs.
fn vectors_of_structs() {
    section("Vectors of structs");

    let mut people = vec![
        Person { name: "Alice".into(), age: 34 },
        Person { name: "Bob".into(), age: 27 },
        Person { name: "Carol".into(), age: 45 },
        Person { name: "Dave".into(), age: 27 },
    ];

    people.sort_by(|a, b| a.age.cmp(&b.age).then_with(|| a.name.cmp(&b.name)));
    for p in &people {
        println!("  {:<6} {}", p.name, p.age);
    }

    let names: Vec<&str> = people.iter().map(|p| p.name.as_str()).collect();
    println!("names: {names:?}");

    let avg_age = people.iter().map(|p| p.age).sum::<u32>() as f64 / people.len() as f64;
    println!("average age: {avg_age:.1}");

    let oldest = people.iter().max_by_key(|p| p.age);
    println!("oldest: {oldest:?}");

    // Mutate in place.
    for p in people.iter_mut() {
        p.age += 1;
    }
    println!("after birthday: {:?}", people.iter().map(|p| p.age).collect::<Vec<_>>());

    // Remove matching items.
    people.retain(|p| p.age < 40);
    println!("under 40: {:?}", people.iter().map(|p| &p.name).collect::<Vec<_>>());
}

/// A Vec<Vec<T>> used as a 2D grid.
fn nested_vectors() {
    section("Nested vectors (2D grid)");

    let rows = 3;
    let cols = 4;
    let mut grid = vec![vec![0; cols]; rows];

    for (r, row) in grid.iter_mut().enumerate() {
        for (c, cell) in row.iter_mut().enumerate() {
            *cell = r * cols + c;
        }
    }

    for row in &grid {
        println!("  {}", row.iter().map(|n| format!("{n:2}")).collect::<Vec<_>>().join(" "));
    }

    // Transpose.
    let transposed: Vec<Vec<usize>> =
        (0..cols).map(|c| grid.iter().map(|row| row[c]).collect()).collect();
    println!("transposed: {transposed:?}");
}

/// Moves, clones, and the borrow checker with vectors.
fn ownership_and_moves() {
    section("Ownership and moves");

    let original = vec![String::from("one"), String::from("two")];

    // clone creates an independent deep copy.
    let mut copy = original.clone();
    copy.push(String::from("three"));
    println!("original: {original:?}, copy: {copy:?}");

    // Moving the Vec transfers ownership; `original` is no longer usable.
    let moved = original;
    println!("moved: {moved:?}");

    // Take an element out by value without cloning.
    let mut items = vec![String::from("a"), String::from("b")];
    let taken = std::mem::take(&mut items[0]);
    println!("mem::take -> {taken:?}, items now {items:?}");

    // Borrowing rule: you cannot push while holding a reference into the
    // vector, because push may reallocate and invalidate the reference.
    //
    //     let first = &items[0];
    //     items.push(String::from("c")); // compile error
    //     println!("{first}");
    //
    // Fix: copy/clone the value first, or end the borrow before mutating.
    let second = items[1].clone();
    items.push(String::from("c"));
    println!("second (cloned) = {second:?}, items = {items:?}");

    // Comparing vectors.
    println!("vec![1, 2] == vec![1, 2]: {}", vec![1, 2] == vec![1, 2]);
    println!("vec![1, 2] < vec![1, 3]: {}", vec![1, 2] < vec![1, 3]);
}
