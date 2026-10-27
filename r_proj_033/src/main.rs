//! A comprehensive tour of `std::collections::BTreeMap` in Rust.
//!
//! A `BTreeMap<K, V>` stores key/value pairs in a B-tree, which keeps the
//! keys sorted at all times. Lookups, inserts and removals take O(log n).
//! Keys must implement `Ord`; values can be any type. Compared to a
//! `HashMap`, a `BTreeMap` is a little slower for single lookups, but it
//! offers things a hash map cannot:
//!
//! * iteration is always in ascending key order, so output is deterministic,
//! * range queries (`range`, `range_mut`) over a span of keys,
//! * cheap access to the smallest and largest entries (`first_key_value`,
//!   `last_key_value`, `pop_first`, `pop_last`),
//! * splitting and merging maps by key (`split_off`, `append`),
//! * the map itself implements `Ord` and `Hash`, so it can be a key too.
//!
//! Part 1 covers the core API: creating maps, inserting, reading, updating,
//! the Entry API, removing and iterating.
//!
//! Part 2 covers what makes a BTreeMap special: ordered access, range
//! queries, splitting/merging and comparing whole maps.
//!
//! Part 3 covers custom key types: deriving `Ord`, writing `Ord` by hand,
//! reverse ordering with `Reverse`, and borrowed lookups.
//!
//! Part 4 shows typical real-world uses: a sorted word index, a time series
//! with range queries, a leaderboard, an interval lookup table, an event
//! scheduler and a simple order book.
//!
//! Each section is a small function that demonstrates one group of features
//! and prints its results, so you can run the program and compare the output
//! with the code.

use std::cmp::{Ordering, Reverse};
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::ops::Bound::{Excluded, Included, Unbounded};

fn main() {
    // Part 1: core API
    creating_maps();
    inserting_and_reading();
    updating_values();
    entry_api();
    removing_entries();
    iterating();

    // Part 2: ordered operations
    first_and_last();
    range_queries();
    floor_and_ceiling();
    splitting_and_merging();
    comparing_maps();

    // Part 3: keys
    derived_key_types();
    custom_ord_impl();
    reverse_ordering();
    borrowed_lookups();

    // Part 4: practical use cases
    sorted_word_index();
    time_series();
    leaderboard();
    grade_boundaries();
    event_scheduler();
    order_book();
}

fn section(title: &str) {
    println!("\n=== {title} ===");
}

// ---------------------------------------------------------------------------
// Part 1: core API
// ---------------------------------------------------------------------------

/// Different ways to construct a BTreeMap.
fn creating_maps() {
    section("Creating maps");

    // Empty map. Unlike HashMap, BTreeMap::new() is a const fn and does not
    // allocate, so it can even be used in a `static`.
    let empty: BTreeMap<String, i32> = BTreeMap::new();
    println!("empty: {empty:?}, len = {}, is_empty = {}", empty.len(), empty.is_empty());

    // From<[(K, V); N]>. The input order does not matter - the map is sorted.
    let capitals = BTreeMap::from([
        ("Japan", "Tokyo"),
        ("France", "Paris"),
        ("Germany", "Berlin"),
    ]);
    println!("from array (sorted by key): {capitals:?}");

    // Collect from an iterator of (key, value) tuples.
    let squares: BTreeMap<u32, u32> = (1..=5).rev().map(|n| (n, n * n)).collect();
    println!("collected squares: {squares:?}");

    // Duplicate keys while collecting: the last value wins.
    let dupes: BTreeMap<char, i32> = [('a', 1), ('b', 2), ('a', 3)].into_iter().collect();
    println!("collected with duplicates: {dupes:?}");

    // Converting from a HashMap gives a sorted, reproducible view of it.
    let hashed: HashMap<&str, i32> = HashMap::from([("z", 26), ("a", 1), ("m", 13)]);
    let ordered: BTreeMap<_, _> = hashed.into_iter().collect();
    println!("from HashMap: {ordered:?}");

    // Default gives an empty map as well.
    let default: BTreeMap<char, bool> = BTreeMap::default();
    println!("default: {default:?}");

    // There is no with_capacity: a B-tree allocates node by node as it grows.
}

/// A BTreeMap can live in a static because `new` is const.
static EMPTY_REGISTRY: BTreeMap<&str, u32> = BTreeMap::new();

/// insert, get, contains_key and indexing.
fn inserting_and_reading() {
    section("Inserting and reading");

    println!("static map is empty: {}", EMPTY_REGISTRY.is_empty());

    let mut scores: BTreeMap<String, u32> = BTreeMap::new();

    // insert returns the previous value for that key, if any.
    println!("insert Red=50  -> {:?}", scores.insert("Red".to_string(), 50));
    println!("insert Blue=10 -> {:?}", scores.insert("Blue".to_string(), 10));
    println!("insert Blue=25 -> {:?} (old value replaced)", scores.insert("Blue".to_string(), 25));
    println!("scores: {scores:?}");

    // get returns Option<&V>, so a missing key is not an error.
    println!("get(\"Blue\")  = {:?}", scores.get("Blue"));
    println!("get(\"Green\") = {:?}", scores.get("Green"));

    // Fall back to a default for missing keys.
    let green = scores.get("Green").copied().unwrap_or(0);
    println!("Green or 0  = {green}");

    // get_key_value returns the stored key too.
    println!("get_key_value(\"Red\") = {:?}", scores.get_key_value("Red"));

    println!("contains_key(\"Red\")   = {}", scores.contains_key("Red"));
    println!("contains_key(\"Black\") = {}", scores.contains_key("Black"));

    // Indexing with [] panics if the key is missing.
    println!("scores[\"Red\"] = {}", scores["Red"]);

    // Ownership: inserting a String moves it into the map.
    let key = String::from("Yellow");
    scores.insert(key, 5);
    // println!("{key}"); // error: borrow of moved value `key`
    println!("len after inserting Yellow = {}", scores.len());
}

/// Modifying existing values in place.
fn updating_values() {
    section("Updating values");

    let mut stock = BTreeMap::from([("apples", 3), ("pears", 7), ("plums", 2)]);

    // get_mut gives a mutable reference to an existing value.
    if let Some(count) = stock.get_mut("apples") {
        *count += 10;
    }
    println!("after get_mut: {stock:?}");

    // Mutate every value at once (visited in key order).
    for count in stock.values_mut() {
        *count *= 2;
    }
    println!("after doubling via values_mut: {stock:?}");

    // iter_mut yields (&K, &mut V). Keys are never mutable, since changing
    // a key could break the sort order.
    for (fruit, count) in stock.iter_mut() {
        if fruit.starts_with('p') {
            *count -= 1;
        }
    }
    println!("after iter_mut: {stock:?}");

    // range_mut mutates only the values whose keys fall into a range.
    for (_, count) in stock.range_mut("p".."q") {
        *count = 0;
    }
    println!("after zeroing all \"p...\" keys with range_mut: {stock:?}");
}

/// The Entry API: look up a key once and then insert or modify.
fn entry_api() {
    section("Entry API");

    let mut counts: BTreeMap<&str, i32> = BTreeMap::new();

    // or_insert: insert a default if the key is absent, then return &mut V.
    *counts.entry("b").or_insert(0) += 1;
    *counts.entry("a").or_insert(0) += 1;
    *counts.entry("a").or_insert(0) += 1;
    println!("or_insert counts: {counts:?}");

    // or_default uses V::default() (0 for integers).
    *counts.entry("c").or_default() += 100;
    println!("or_default: {counts:?}");

    // or_insert_with only runs the closure when the key is absent.
    let mut cache: BTreeMap<u32, String> = BTreeMap::new();
    let value = cache.entry(7).or_insert_with(|| {
        println!("  (computing value for 7)");
        "seven".to_string()
    });
    println!("first lookup  -> {value}");
    let value = cache.entry(7).or_insert_with(|| unreachable!("already cached"));
    println!("second lookup -> {value}");

    // or_insert_with_key passes the key to the closure.
    let label = cache.entry(42).or_insert_with_key(|k| format!("number {k}"));
    println!("or_insert_with_key -> {label}");

    // and_modify changes an existing value; chain it with or_insert for an
    // "update or create" in one expression.
    counts.entry("a").and_modify(|v| *v *= 10).or_insert(1);
    counts.entry("z").and_modify(|v| *v *= 10).or_insert(1);
    println!("and_modify/or_insert: {counts:?}");

    // Match on the Entry directly for full control.
    for key in ["b", "q"] {
        match counts.entry(key) {
            Entry::Occupied(mut occupied) => {
                println!("  {key} occupied with {}, removing it", occupied.get());
                *occupied.get_mut() += 1;
                let removed = occupied.remove();
                println!("  removed value {removed}");
            }
            Entry::Vacant(vacant) => {
                println!("  {key} vacant, inserting 99 (key = {:?})", vacant.key());
                vacant.insert(99);
            }
        }
    }
    println!("after matching entries: {counts:?}");

    // The entry key is available on both variants.
    println!("entry(\"c\").key() = {:?}", counts.entry("c").key());
}

/// remove, remove_entry, retain, extract_if and clear.
fn removing_entries() {
    section("Removing entries");

    let mut map: BTreeMap<u32, &str> = (1..=10)
        .map(|n| (n, if n % 2 == 0 { "even" } else { "odd" }))
        .collect();
    println!("start: {map:?}");

    // remove returns the value if the key was present.
    println!("remove(3)  -> {:?}", map.remove(&3));
    println!("remove(3)  -> {:?} (already gone)", map.remove(&3));

    // remove_entry returns both key and value.
    println!("remove_entry(4) -> {:?}", map.remove_entry(&4));

    // retain keeps only the entries for which the closure returns true.
    map.retain(|&k, _| k != 5);
    println!("after retain(k != 5): {map:?}");

    // extract_if removes matching entries lazily and hands them to you.
    // Here: pull out every key in 7..=9 that is odd.
    let extracted: BTreeMap<u32, &str> = map.extract_if(7..=9, |k, _| k % 2 == 1).collect();
    println!("extracted odd keys in 7..=9: {extracted:?}");
    println!("remaining: {map:?}");

    // clear removes everything.
    map.clear();
    println!("after clear: {map:?}, is_empty = {}", map.is_empty());
}

/// All the ways to walk a BTreeMap. Order is always ascending by key.
fn iterating() {
    section("Iterating");

    let planets = BTreeMap::from([
        ("Mercury", 0.39),
        ("Venus", 0.72),
        ("Earth", 1.00),
        ("Mars", 1.52),
    ]);

    // Borrowing iteration over (&K, &V), sorted alphabetically.
    for (name, au) in &planets {
        println!("  {name:<8} {au:.2} AU");
    }

    // keys() and values() are sorted by key as well.
    let names: Vec<_> = planets.keys().collect();
    let distances: Vec<_> = planets.values().collect();
    println!("keys:   {names:?}");
    println!("values: {distances:?}");

    // All BTreeMap iterators are double-ended, so .rev() is free.
    let reversed: Vec<_> = planets.keys().rev().collect();
    println!("keys reversed: {reversed:?}");

    // ... and they know their exact length.
    println!("iter().len() = {}", planets.iter().len());

    // Iterator adapters work as usual.
    let far: Vec<_> = planets
        .iter()
        .filter(|&(_, &au)| au > 0.9)
        .map(|(name, _)| *name)
        .collect();
    println!("farther than 0.9 AU: {far:?}");

    // into_keys / into_values consume the map.
    let owned_names: Vec<&str> = planets.clone().into_keys().collect();
    let total: f64 = planets.clone().into_values().sum();
    println!("into_keys: {owned_names:?}, sum of into_values = {total:.2}");

    // into_iter consumes the map and yields owned (K, V) pairs.
    let as_vec: Vec<(&str, f64)> = planets.into_iter().collect();
    println!("into_iter -> Vec: {as_vec:?}");
}

// ---------------------------------------------------------------------------
// Part 2: ordered operations
// ---------------------------------------------------------------------------

/// Accessing and removing the smallest and largest entries.
fn first_and_last() {
    section("First and last entries");

    let mut temps = BTreeMap::from([(3, "Wed"), (1, "Mon"), (5, "Fri"), (2, "Tue"), (4, "Thu")]);

    println!("first_key_value = {:?}", temps.first_key_value());
    println!("last_key_value  = {:?}", temps.last_key_value());

    // first_entry / last_entry give an OccupiedEntry you can modify or remove.
    if let Some(mut entry) = temps.first_entry() {
        *entry.get_mut() = "Monday";
    }
    if let Some(entry) = temps.last_entry() {
        println!("removing last entry {:?}", entry.remove_entry());
    }
    println!("after entry edits: {temps:?}");

    // pop_first / pop_last remove and return the extreme entries. This makes
    // a BTreeMap usable as a double-ended priority queue.
    println!("pop_first -> {:?}", temps.pop_first());
    println!("pop_last  -> {:?}", temps.pop_last());
    println!("remaining: {temps:?}");

    // Draining in order with a while-let loop.
    while let Some((day, name)) = temps.pop_first() {
        println!("  processing day {day}: {name}");
    }
    println!("empty now: {}, pop_first -> {:?}", temps.is_empty(), temps.pop_first());
}

/// range() accepts any RangeBounds: a..b, a..=b, a.., ..b, or explicit Bounds.
fn range_queries() {
    section("Range queries");

    let map: BTreeMap<u32, char> = (1..=10).zip('a'..).collect();
    println!("map: {map:?}");

    let show = |label: &str, it: std::collections::btree_map::Range<u32, char>| {
        let entries: Vec<_> = it.map(|(k, v)| format!("{k}{v}")).collect();
        println!("  {label:<22} {entries:?}");
    };

    show("range(3..6)", map.range(3..6));
    show("range(3..=6)", map.range(3..=6));
    show("range(8..)", map.range(8..));
    show("range(..3)", map.range(..3));
    show("range(..)", map.range(..));

    // Explicit Bound values let you express an exclusive start.
    show("(Excluded(3), Included(6))", map.range((Excluded(3), Included(6))));
    show("(Excluded(8), Unbounded)", map.range((Excluded(8), Unbounded)));

    // Ranges are double-ended too: the last 3 entries below 9.
    let last_three: Vec<_> = map.range(..9).rev().take(3).collect();
    println!("  last 3 keys below 9:   {last_three:?}");

    // Counting entries in a range.
    println!("  keys in 2..=7: {}", map.range(2..=7).count());

    // Prefix search on string keys: every key starting with "car".
    let words = BTreeSet::from(["apple", "car", "card", "care", "cart", "cat", "dog"]);
    let dictionary: BTreeMap<&str, usize> = words.iter().map(|w| (*w, w.len())).collect();
    let prefix = "car";
    let matches: Vec<_> = dictionary
        .range(prefix..)
        .take_while(|(k, _)| k.starts_with(prefix))
        .map(|(k, _)| *k)
        .collect();
    println!("  words starting with {prefix:?}: {matches:?}");

    // range panics if start > end or if both bounds exclude the same key,
    // e.g. map.range(5..3) or map.range((Excluded(5), Excluded(5))).
}

/// Finding the nearest key at or below / at or above a given value.
fn floor_and_ceiling() {
    section("Floor and ceiling lookups");

    let checkpoints = BTreeMap::from([(0, "start"), (10, "lake"), (25, "bridge"), (40, "summit")]);

    // Largest key <= x.
    let floor = |x: i32| checkpoints.range(..=x).next_back();
    // Smallest key >= x.
    let ceiling = |x: i32| checkpoints.range(x..).next();
    // Strictly before / after.
    let before = |x: i32| checkpoints.range(..x).next_back();
    let after = |x: i32| checkpoints.range((Excluded(x), Unbounded)).next();

    for km in [-5, 0, 17, 25, 50] {
        println!(
            "  km {km:>3}: floor={:?} ceiling={:?} before={:?} after={:?}",
            floor(km),
            ceiling(km),
            before(km),
            after(km)
        );
    }
}

/// split_off and append move whole key ranges between maps.
fn splitting_and_merging() {
    section("Splitting and merging");

    let mut low: BTreeMap<u32, &str> = BTreeMap::from([
        (1, "one"),
        (2, "two"),
        (3, "three"),
        (4, "four"),
        (5, "five"),
        (6, "six"),
    ]);

    // split_off(&k) moves every entry with key >= k into a new map.
    let mut high = low.split_off(&4);
    println!("after split_off(&4): low = {low:?}");
    println!("                     high = {high:?}");

    // append moves everything from `other` into `self`, leaving `other`
    // empty. On duplicate keys the value from `other` wins.
    high.insert(1, "ONE");
    low.append(&mut high);
    println!("after low.append(&mut high): low = {low:?}");
    println!("                             high = {high:?} (now empty)");

    // extend works with any iterator of pairs; later values overwrite.
    low.extend([(7, "seven"), (2, "TWO")]);
    println!("after extend: {low:?}");

    // Merging with a custom rule: add counts instead of overwriting.
    let mut a = BTreeMap::from([("x", 1), ("y", 2)]);
    let b = BTreeMap::from([("y", 10), ("z", 20)]);
    for (k, v) in b {
        *a.entry(k).or_insert(0) += v;
    }
    println!("summed merge: {a:?}");
}

/// Equality, ordering and hashing of whole maps.
fn comparing_maps() {
    section("Comparing maps");

    let a = BTreeMap::from([(1, 'a'), (2, 'b')]);
    let b = BTreeMap::from([(2, 'b'), (1, 'a')]);
    let c = BTreeMap::from([(1, 'a'), (3, 'c')]);

    // Equal if they contain the same entries; insertion order is irrelevant.
    println!("a == b: {}", a == b);
    println!("a == c: {}", a == c);

    // BTreeMap implements Ord (lexicographic over the sorted entries), which
    // HashMap cannot do.
    println!("a.cmp(&c) = {:?}  (key 2 < key 3)", a.cmp(&c));

    // Because it is Ord + Hash, a BTreeMap can be a key in another map or a
    // member of a set, e.g. to deduplicate configurations.
    let mut seen: BTreeSet<BTreeMap<i32, char>> = BTreeSet::new();
    for m in [a.clone(), b, c] {
        let new = seen.insert(m.clone());
        println!("  insert {m:?} -> new = {new}");
    }
    println!("distinct maps: {}", seen.len());
}

// ---------------------------------------------------------------------------
// Part 3: keys
// ---------------------------------------------------------------------------

/// Deriving Ord gives lexicographic ordering by field order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Version {
    major: u32,
    minor: u32,
    patch: u32,
}

impl Version {
    const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self { major, minor, patch }
    }
}

/// Enums derive Ord by declaration order of their variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Priority {
    Critical,
    High,
    Normal,
    Low,
}

fn derived_key_types() {
    section("Derived key types");

    let releases = BTreeMap::from([
        (Version::new(1, 10, 0), "big feature release"),
        (Version::new(1, 2, 3), "bug fixes"),
        (Version::new(2, 0, 0), "breaking changes"),
        (Version::new(1, 2, 0), "initial 1.2"),
    ]);
    // Note that 1.10.0 correctly sorts after 1.2.x, unlike string ordering.
    for (v, notes) in &releases {
        println!("  {}.{}.{}: {notes}", v.major, v.minor, v.patch);
    }

    // Range queries on a struct key: every 1.x release.
    let one_x: Vec<_> = releases
        .range(Version::new(1, 0, 0)..Version::new(2, 0, 0))
        .map(|(v, _)| format!("{}.{}.{}", v.major, v.minor, v.patch))
        .collect();
    println!("1.x releases: {one_x:?}");

    // Tuples are Ord too, which is handy for composite keys.
    let mut tasks: BTreeMap<(Priority, u32), &str> = BTreeMap::new();
    tasks.insert((Priority::Low, 1), "water plants");
    tasks.insert((Priority::Critical, 2), "server is down");
    tasks.insert((Priority::Normal, 3), "write report");
    tasks.insert((Priority::Critical, 1), "data loss alert");
    println!("tasks by (priority, id):");
    for ((prio, id), task) in &tasks {
        println!("  {prio:?} #{id}: {task}");
    }

    // All critical tasks via a range over the tuple key.
    let critical: Vec<_> = tasks
        .range((Priority::Critical, 0)..(Priority::High, 0))
        .map(|(_, t)| *t)
        .collect();
    println!("critical only: {critical:?}");
}

/// A key with a hand-written Ord: case-insensitive string ordering.
#[derive(Debug, Clone)]
struct CaseInsensitive(String);

impl PartialEq for CaseInsensitive {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for CaseInsensitive {}

impl PartialOrd for CaseInsensitive {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CaseInsensitive {
    fn cmp(&self, other: &Self) -> Ordering {
        // Compare char by char in lowercase without allocating.
        let a = self.0.chars().flat_map(char::to_lowercase);
        let b = other.0.chars().flat_map(char::to_lowercase);
        a.cmp(b)
    }
}

fn custom_ord_impl() {
    section("Custom Ord implementation");

    let mut headers: BTreeMap<CaseInsensitive, &str> = BTreeMap::new();
    headers.insert(CaseInsensitive("Content-Type".into()), "text/html");
    headers.insert(CaseInsensitive("accept".into()), "*/*");
    headers.insert(CaseInsensitive("X-Request-Id".into()), "abc123");

    // "CONTENT-TYPE" compares equal to "Content-Type", so it replaces the
    // value but keeps the original key.
    let old = headers.insert(CaseInsensitive("CONTENT-TYPE".into()), "application/json");
    println!("replaced Content-Type value: {old:?}");

    for (k, v) in &headers {
        println!("  {}: {v}", k.0);
    }

    let probe = CaseInsensitive("x-request-id".into());
    println!("lookup \"x-request-id\" -> {:?}", headers.get(&probe));

    // Ord must be a total order and consistent with Eq. If a key's ordering
    // changes while it is in the map (e.g. through interior mutability), the
    // map's behavior becomes unspecified (but never memory-unsafe).
}

/// std::cmp::Reverse flips the ordering of any Ord type.
fn reverse_ordering() {
    section("Reverse ordering");

    let mut desc: BTreeMap<Reverse<u32>, &str> = BTreeMap::new();
    desc.insert(Reverse(10), "ten");
    desc.insert(Reverse(30), "thirty");
    desc.insert(Reverse(20), "twenty");

    let keys: Vec<u32> = desc.keys().map(|Reverse(k)| *k).collect();
    println!("keys in descending order: {keys:?}");
    println!("first (= largest) entry: {:?}", desc.first_key_value());

    // Ranges are expressed in the reversed space: from 25 down to 10.
    let span: Vec<_> = desc.range(Reverse(25)..=Reverse(10)).map(|(_, v)| *v).collect();
    println!("range Reverse(25)..=Reverse(10): {span:?}");

    // Often simpler: keep the natural order and iterate with .rev().
    let asc = BTreeMap::from([(10, "ten"), (20, "twenty"), (30, "thirty")]);
    let via_rev: Vec<_> = asc.iter().rev().map(|(k, _)| *k).collect();
    println!("natural order + rev(): {via_rev:?}");
}

/// Looking up String keys with &str through the Borrow trait.
fn borrowed_lookups() {
    section("Borrowed lookups");

    let mut env: BTreeMap<String, String> = BTreeMap::new();
    env.insert("HOME".to_string(), "/home/rolf".to_string());
    env.insert("SHELL".to_string(), "/bin/bash".to_string());
    env.insert("PATH".to_string(), "/usr/bin".to_string());

    // String: Borrow<str>, so get/contains_key/remove/range accept &str and
    // no temporary String has to be allocated.
    println!("get(\"HOME\")         = {:?}", env.get("HOME"));
    println!("contains_key(\"USER\") = {}", env.contains_key("USER"));

    // range with &str bounds needs the explicit type because both String and
    // str could be the borrowed form.
    let from_p: Vec<&String> = env
        .range::<str, _>((Included("P"), Unbounded))
        .map(|(k, _)| k)
        .collect();
    println!("keys >= \"P\": {from_p:?}");

    // Vec<T> keys can be looked up with slices the same way.
    let paths = BTreeMap::from([(vec![1, 2], "a"), (vec![1, 3], "b")]);
    let probe = [1, 3].as_slice();
    println!("lookup by slice {probe:?} -> {:?}", paths.get(probe));
}

// ---------------------------------------------------------------------------
// Part 4: practical use cases
// ---------------------------------------------------------------------------

/// An alphabetical index: word -> sorted set of line numbers.
fn sorted_word_index() {
    section("Sorted word index");

    let text = "the quick brown fox\n\
                jumps over the lazy dog\n\
                the dog sleeps\n\
                a quick nap for the fox";

    let mut index: BTreeMap<&str, BTreeSet<usize>> = BTreeMap::new();
    for (line_no, line) in text.lines().enumerate() {
        for word in line.split_whitespace() {
            index.entry(word).or_default().insert(line_no + 1);
        }
    }

    // Output is alphabetical with no extra sorting step.
    for (word, lines) in &index {
        let lines: Vec<_> = lines.iter().map(ToString::to_string).collect();
        println!("  {word:<7} {}", lines.join(", "));
    }

    // Words in the alphabetical range "d".."g".
    let d_to_f: Vec<_> = index.range("d".."g").map(|(w, _)| *w).collect();
    println!("words from d to f: {d_to_f:?}");
}

/// Timestamped measurements with window queries.
fn time_series() {
    section("Time series");

    // Seconds since start -> temperature in °C.
    let readings: BTreeMap<u64, f64> = BTreeMap::from([
        (0, 20.1),
        (60, 20.4),
        (120, 21.0),
        (180, 21.8),
        (240, 22.5),
        (300, 22.1),
        (360, 21.7),
    ]);

    // Average temperature in a time window [from, to).
    let average = |from: u64, to: u64| {
        let window: Vec<f64> = readings.range(from..to).map(|(_, &t)| t).collect();
        if window.is_empty() {
            None
        } else {
            Some(window.iter().sum::<f64>() / window.len() as f64)
        }
    };
    println!("avg in [60, 240):  {:.2?}", average(60, 240));
    println!("avg in [400, 500): {:.2?}", average(400, 500));

    // The value in effect at a time between samples is the latest reading
    // at or before that time (a floor lookup).
    for t in [90, 300, 999] {
        let (at, temp) = readings.range(..=t).next_back().unwrap();
        println!("temperature at t={t}: {temp} (sampled at t={at})");
    }

    // Peak value and when it happened.
    let (peak_t, peak) = readings
        .iter()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .unwrap();
    println!("peak {peak} °C at t={peak_t}");

    // Keep only the last 3 minutes of data: split_off at the cutoff.
    let mut buffer = readings.clone();
    let cutoff = buffer.last_key_value().map(|(t, _)| t.saturating_sub(180)).unwrap();
    let recent = buffer.split_off(&cutoff);
    println!("dropped {} old readings, kept {:?}", buffer.len(), recent);
}

/// A leaderboard keyed by (Reverse(score), name) so the best come first and
/// ties are broken alphabetically.
fn leaderboard() {
    section("Leaderboard");

    let mut board: BTreeMap<(Reverse<u32>, String), ()> = BTreeMap::new();
    // Remember each player's current score to be able to update them.
    let mut current: BTreeMap<String, u32> = BTreeMap::new();

    let mut submit = |name: &str, score: u32| {
        if let Some(old) = current.insert(name.to_string(), score) {
            board.remove(&(Reverse(old), name.to_string()));
        }
        board.insert((Reverse(score), name.to_string()), ());
    };

    submit("alice", 1200);
    submit("bob", 950);
    submit("carol", 1200);
    submit("dave", 700);
    submit("bob", 1500); // bob improves

    println!("top 3:");
    for (rank, ((Reverse(score), name), _)) in board.iter().take(3).enumerate() {
        println!("  #{} {name:<6} {score}", rank + 1);
    }

    // Everyone with a score between 900 and 1300 (remember: reversed).
    let mid: Vec<_> = board
        .range((Reverse(1300), String::new())..(Reverse(899), String::new()))
        .map(|((_, n), _)| n.as_str())
        .collect();
    println!("scores in 900..=1300: {mid:?}");
}

/// Mapping a continuous value onto buckets with a floor lookup.
fn grade_boundaries() {
    section("Grade boundaries");

    // Lower bound of each grade (inclusive).
    let grades = BTreeMap::from([(0, 'F'), (50, 'E'), (60, 'D'), (70, 'C'), (80, 'B'), (90, 'A')]);

    let grade_for = |points: u32| grades.range(..=points).next_back().map(|(_, &g)| g);

    for points in [0, 49, 50, 73, 89, 90, 100] {
        println!("  {points:>3} points -> {}", grade_for(points).unwrap());
    }

    // Histogram of a class's results, in grade order.
    let results = [55, 91, 73, 68, 88, 42, 95, 77, 81, 60];
    let mut histogram: BTreeMap<char, u32> = BTreeMap::new();
    for p in results {
        *histogram.entry(grade_for(p).unwrap()).or_default() += 1;
    }
    println!("histogram: {histogram:?}");
}

/// A simple scheduler: events keyed by time, several events per time slot.
fn event_scheduler() {
    section("Event scheduler");

    let mut queue: BTreeMap<u32, Vec<&str>> = BTreeMap::new();
    let mut schedule = |at: u32, what| queue.entry(at).or_default().push(what);

    schedule(900, "standup");
    schedule(1300, "lunch");
    schedule(900, "coffee");
    schedule(1100, "code review");
    schedule(1600, "deploy");

    println!("next event at: {:?}", queue.first_key_value());

    // Run everything due up to and including 13:00.
    let now = 1300;
    while let Some(entry) = queue.first_entry() {
        if *entry.key() > now {
            break;
        }
        let (at, events) = entry.remove_entry();
        println!("  {at:04}: running {events:?}");
    }
    println!("still pending: {queue:?}");
}

/// A tiny order book: bids and asks sorted by price.
fn order_book() {
    section("Order book");

    // price (in cents) -> total quantity at that price
    let mut bids: BTreeMap<u32, u32> = BTreeMap::new();
    let mut asks: BTreeMap<u32, u32> = BTreeMap::new();

    for (price, qty) in [(9950, 10), (9900, 25), (9975, 5), (9950, 15)] {
        *bids.entry(price).or_default() += qty;
    }
    for (price, qty) in [(10025, 8), (10000, 12), (10050, 30)] {
        *asks.entry(price).or_default() += qty;
    }

    let best_bid = bids.last_key_value();
    let best_ask = asks.first_key_value();
    println!("best bid: {best_bid:?}, best ask: {best_ask:?}");
    if let (Some((b, _)), Some((a, _))) = (best_bid, best_ask) {
        println!("spread: {} cents", a - b);
    }

    // A market buy order for 15 units consumes the cheapest asks first.
    let mut to_buy = 15;
    let mut cost = 0;
    while to_buy > 0 {
        let Some(mut level) = asks.first_entry() else {
            println!("  not enough liquidity!");
            break;
        };
        let price = *level.key();
        let available = level.get_mut();
        let filled = to_buy.min(*available);
        *available -= filled;
        to_buy -= filled;
        cost += filled * price;
        println!("  filled {filled} @ {price}");
        if *level.get() == 0 {
            level.remove();
        }
    }
    println!("total cost: {cost} cents");

    println!("bids (high to low):");
    for (price, qty) in bids.iter().rev() {
        println!("  {price} x {qty}");
    }
    println!("asks (low to high):");
    for (price, qty) in &asks {
        println!("  {price} x {qty}");
    }
}
