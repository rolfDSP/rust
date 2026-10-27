//! A comprehensive tour of `std::collections::HashMap` in Rust.
//!
//! A `HashMap<K, V>` stores key/value pairs and finds a value by its key in
//! O(1) on average. Keys must implement `Eq` and `Hash`; values can be any
//! type. Iteration order is unspecified and changes from run to run, because
//! the default hasher (SipHash 1-3) is seeded randomly to resist HashDoS
//! attacks. Whenever this program prints a whole map, it therefore sorts the
//! entries first (or uses a `BTreeMap`) so the output is reproducible.
//!
//! Part 1 covers the core API: creating maps, inserting, reading, updating,
//! the Entry API, removing, iterating and capacity management.
//!
//! Part 2 covers custom key types: deriving `Hash`/`Eq`, writing `Hash` by
//! hand, and looking up `String` keys with a `&str` via `Borrow`.
//!
//! Part 3 shows typical real-world uses: word counting, grouping, memoization,
//! inverting a map, and a small inventory system.
//!
//! Each section is a small function that demonstrates one group of features
//! and prints its results, so you can run the program and compare the output
//! with the code.

use std::collections::hash_map::{DefaultHasher, Entry};
use std::collections::{BTreeMap, HashMap};
use std::fmt::Debug;
use std::hash::{BuildHasher, Hash, Hasher, RandomState};

fn main() {
    // Part 1: core API
    creating_maps();
    inserting_and_reading();
    updating_values();
    entry_api();
    removing_entries();
    iterating();
    capacity_management();
    combining_maps();
    comparing_maps();

    // Part 2: keys
    derived_key_types();
    custom_hash_impl();
    borrowed_lookups();
    hashing_by_hand();

    // Part 3: practical use cases
    word_frequencies();
    grouping_by_key();
    memoized_fibonacci();
    inverting_a_map();
    inventory_system();
}

fn section(title: &str) {
    println!("\n=== {title} ===");
}

/// Returns the map's entries sorted by key, so printing is deterministic.
fn sorted<K: Ord + Clone, V: Clone>(map: &HashMap<K, V>) -> Vec<(K, V)> {
    let mut entries: Vec<(K, V)> = map.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    entries
}

/// Prints a map with its entries in key order.
fn show<K: Ord + Clone + Debug, V: Clone + Debug>(label: &str, map: &HashMap<K, V>) {
    println!("{label}: {:?}", sorted(map));
}

// ---------------------------------------------------------------------------
// Part 1: core API
// ---------------------------------------------------------------------------

/// Different ways to construct a HashMap.
fn creating_maps() {
    section("Creating maps");

    // Empty map; the types are usually inferred from the first insert, but
    // here nothing is inserted, so they are spelled out.
    let empty: HashMap<String, i32> = HashMap::new();
    println!("empty: {empty:?}, len = {}, is_empty = {}", empty.len(), empty.is_empty());

    // From<[(K, V); N]> is the most convenient way to build a small map.
    let capitals = HashMap::from([
        ("France", "Paris"),
        ("Germany", "Berlin"),
        ("Japan", "Tokyo"),
    ]);
    show("from array", &capitals);

    // Collect from an iterator of (key, value) tuples.
    let squares: HashMap<u32, u32> = (1..=5).map(|n| (n, n * n)).collect();
    show("collected squares", &squares);

    // Zip two sequences together into keys and values.
    let names = ["alice", "bob", "carol"];
    let ages = [31, 27, 45];
    let people: HashMap<_, _> = names.into_iter().zip(ages).collect();
    show("zipped", &people);

    // Pre-allocate room for at least 100 entries to avoid rehashing.
    let big: HashMap<u64, String> = HashMap::with_capacity(100);
    println!("with_capacity(100): capacity = {} (>= 100)", big.capacity());

    // Default gives an empty map as well.
    let default: HashMap<char, bool> = HashMap::default();
    println!("default: {default:?}");
}

/// insert, get, contains_key and indexing.
fn inserting_and_reading() {
    section("Inserting and reading");

    let mut scores: HashMap<String, u32> = HashMap::new();

    // insert returns the previous value for that key, if any.
    println!("insert Blue=10 -> {:?}", scores.insert("Blue".to_string(), 10));
    println!("insert Red=50  -> {:?}", scores.insert("Red".to_string(), 50));
    println!("insert Blue=25 -> {:?} (old value replaced)", scores.insert("Blue".to_string(), 25));
    show("scores", &scores);

    // get returns Option<&V>, so a missing key is not an error.
    println!("get(\"Blue\")  = {:?}", scores.get("Blue"));
    println!("get(\"Green\") = {:?}", scores.get("Green"));

    // A common pattern: fall back to a default for missing keys.
    let green = scores.get("Green").copied().unwrap_or(0);
    println!("Green or 0  = {green}");

    // get_key_value returns the stored key too.
    println!("get_key_value(\"Red\") = {:?}", scores.get_key_value("Red"));

    println!("contains_key(\"Red\")   = {}", scores.contains_key("Red"));
    println!("contains_key(\"Black\") = {}", scores.contains_key("Black"));

    // Indexing with [] panics if the key is missing - only use it when the
    // key is known to be present.
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

    let mut stock = HashMap::from([("apples", 3), ("pears", 7)]);

    // get_mut gives a mutable reference to an existing value.
    if let Some(count) = stock.get_mut("apples") {
        *count += 10;
    }
    show("after get_mut", &stock);

    // Mutate every value at once.
    for count in stock.values_mut() {
        *count *= 2;
    }
    show("after doubling via values_mut", &stock);

    // iter_mut yields (&K, &mut V).
    for (fruit, count) in stock.iter_mut() {
        if fruit.starts_with('p') {
            *count -= 1;
        }
    }
    show("after iter_mut", &stock);

    // get_disjoint_mut borrows several values mutably at the same time.
    // It panics if the same key is requested twice.
    let [apples, pears] = stock.get_disjoint_mut(["apples", "pears"]);
    if let (Some(a), Some(p)) = (apples, pears) {
        std::mem::swap(a, p);
    }
    show("after swapping with get_disjoint_mut", &stock);
}

/// The Entry API: look up a key once and then insert or modify.
fn entry_api() {
    section("Entry API");

    let mut counts: HashMap<&str, i32> = HashMap::new();

    // or_insert: insert a default if the key is absent, then return &mut V.
    *counts.entry("a").or_insert(0) += 1;
    *counts.entry("a").or_insert(0) += 1;
    *counts.entry("b").or_insert(0) += 1;
    show("or_insert counts", &counts);

    // or_default uses V::default() (0 for integers).
    *counts.entry("c").or_default() += 100;
    show("or_default", &counts);

    // or_insert_with only runs the closure when the key is absent - useful
    // when the default is expensive to build.
    let mut cache: HashMap<u32, String> = HashMap::new();
    cache.entry(1).or_insert_with(|| {
        println!("  (computing value for 1)");
        "one".to_string()
    });
    cache.entry(1).or_insert_with(|| unreachable!("key 1 already exists"));
    show("or_insert_with", &cache);

    // or_insert_with_key passes the key to the closure.
    let mut lengths: HashMap<&str, usize> = HashMap::new();
    for word in ["hello", "rust", "hello"] {
        lengths.entry(word).or_insert_with_key(|w| w.len());
    }
    show("or_insert_with_key", &lengths);

    // and_modify + or_insert: update if present, insert otherwise.
    let mut visits: HashMap<&str, u32> = HashMap::new();
    for page in ["home", "about", "home", "home"] {
        visits.entry(page).and_modify(|n| *n += 1).or_insert(1);
    }
    show("and_modify/or_insert", &visits);

    // Matching on Entry gives full control over both cases.
    let mut users: HashMap<u32, String> = HashMap::from([(1, "root".to_string())]);
    for (id, name) in [(1, "admin"), (2, "guest")] {
        match users.entry(id) {
            Entry::Occupied(mut e) => {
                println!("  id {id} taken by {:?}; renaming to {name}", e.get());
                e.insert(name.to_string());
            }
            Entry::Vacant(e) => {
                println!("  id {id} free; inserting {name}");
                e.insert(name.to_string());
            }
        }
    }
    show("after matching on Entry", &users);

    // An OccupiedEntry can also remove itself.
    if let Entry::Occupied(e) = users.entry(2) {
        println!("  removed via entry: {:?}", e.remove_entry());
    }
    show("final users", &users);
}

/// remove, remove_entry, retain, extract_if, drain and clear.
fn removing_entries() {
    section("Removing entries");

    let mut map: HashMap<i32, &str> =
        HashMap::from([(1, "one"), (2, "two"), (3, "three"), (4, "four"), (5, "five"), (6, "six")]);

    // remove returns the value if the key was present.
    println!("remove(1)  -> {:?}", map.remove(&1));
    println!("remove(99) -> {:?}", map.remove(&99));

    // remove_entry returns both key and value.
    println!("remove_entry(2) -> {:?}", map.remove_entry(&2));
    show("remaining", &map);

    // retain keeps only the entries for which the predicate returns true.
    map.retain(|k, _| k % 2 == 0 || *k == 3);
    show("after retain (even or 3)", &map);

    // extract_if removes matching entries and hands them to you.
    let mut extracted: Vec<_> = map.extract_if(|_, v| v.len() > 4).collect();
    extracted.sort();
    println!("extract_if (len > 4) -> {extracted:?}");
    show("after extract_if", &map);

    // drain removes everything and yields owned pairs; capacity is kept.
    let mut drained: Vec<_> = map.drain().collect();
    drained.sort();
    println!("drained: {drained:?}, map is_empty = {}", map.is_empty());

    // clear drops everything without yielding it.
    let mut other = HashMap::from([("x", 1)]);
    other.clear();
    println!("after clear: {other:?}");
}

/// Iterating over keys, values and pairs.
fn iterating() {
    section("Iterating");

    let prices = HashMap::from([("coffee", 3.5), ("tea", 2.0), ("cake", 4.25)]);

    // Iteration order is arbitrary! Run the program twice and this line may
    // differ. Everything below sorts to get stable output.
    print!("raw order (unspecified):");
    for (item, price) in &prices {
        print!(" {item}={price}");
    }
    println!();

    let mut keys: Vec<_> = prices.keys().collect();
    keys.sort();
    println!("keys:   {keys:?}");

    let mut values: Vec<f64> = prices.values().copied().collect();
    values.sort_by(f64::total_cmp);
    println!("values: {values:?}");

    // Iterator adapters work as usual.
    let total: f64 = prices.values().sum();
    println!("total = {total}");

    let cheapest = prices.iter().min_by(|a, b| a.1.total_cmp(b.1));
    println!("cheapest = {cheapest:?}");

    let mut expensive: Vec<_> = prices.iter().filter(|&(_, &p)| p > 3.0).map(|(k, _)| *k).collect();
    expensive.sort();
    println!("over 3.0 = {expensive:?}");

    // into_iter consumes the map; into_keys / into_values do the same for
    // one half.
    let mut owned_keys: Vec<&str> = prices.clone().into_keys().collect();
    owned_keys.sort();
    println!("into_keys = {owned_keys:?}");

    // Sorting by value: collect into a Vec and sort it.
    let mut by_price: Vec<(&str, f64)> = prices.into_iter().collect();
    by_price.sort_by(|a, b| b.1.total_cmp(&a.1));
    println!("by price, descending = {by_price:?}");

    // If you need sorted iteration all the time, use a BTreeMap instead.
    let sorted_map: BTreeMap<_, _> = by_price.into_iter().collect();
    println!("as BTreeMap (always key-sorted) = {sorted_map:?}");
}

/// capacity, reserve, try_reserve and shrink_to_fit.
fn capacity_management() {
    section("Capacity management");

    let mut map: HashMap<u32, u32> = HashMap::new();
    println!("new: len = {}, capacity = {}", map.len(), map.capacity());

    map.reserve(50);
    println!("after reserve(50): capacity = {}", map.capacity());

    for i in 0..10 {
        map.insert(i, i);
    }
    println!("after 10 inserts: len = {}, capacity = {}", map.len(), map.capacity());

    map.shrink_to_fit();
    println!("after shrink_to_fit: capacity = {}", map.capacity());

    // try_reserve reports allocation failure instead of aborting.
    match map.try_reserve(usize::MAX) {
        Ok(()) => println!("try_reserve(usize::MAX) succeeded?!"),
        Err(e) => println!("try_reserve(usize::MAX) failed: {e}"),
    }
}

/// Merging maps together.
fn combining_maps() {
    section("Combining maps");

    let defaults = HashMap::from([("theme", "light"), ("lang", "en"), ("font", "mono")]);
    let user = HashMap::from([("theme", "dark"), ("lang", "de")]);

    // extend: later values overwrite earlier ones for the same key.
    let mut config = defaults.clone();
    config.extend(user.iter().map(|(k, v)| (*k, *v)));
    show("defaults extended with user", &config);

    // Merge with a custom rule for conflicts: sum the values.
    let week1 = HashMap::from([("alice", 5), ("bob", 3)]);
    let week2 = HashMap::from([("bob", 4), ("carol", 7)]);
    let mut total = week1.clone();
    for (name, hours) in week2 {
        *total.entry(name).or_insert(0) += hours;
    }
    show("hours summed over two weeks", &total);

    // Keys present in both maps (an intersection).
    let mut common: Vec<_> = week1.keys().filter(|k| total.contains_key(*k)).collect();
    common.sort();
    println!("keys of week1 also in total: {common:?}");
}

/// Equality ignores insertion order.
fn comparing_maps() {
    section("Comparing maps");

    let a = HashMap::from([(1, 'a'), (2, 'b')]);
    let mut b = HashMap::new();
    b.insert(2, 'b');
    b.insert(1, 'a');
    println!("same pairs, different insert order: a == b -> {}", a == b);

    b.insert(3, 'c');
    println!("after adding a key: a == b -> {}", a == b);

    // HashMap does not implement Ord or Hash, so it cannot be sorted or used
    // as a key itself. Convert to BTreeMap if you need that.
}

// ---------------------------------------------------------------------------
// Part 2: keys
// ---------------------------------------------------------------------------

/// A key type that derives Hash, Eq and PartialEq.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct Point {
    x: i32,
    y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Suit {
    Hearts,
    Spades,
}

/// Structs, enums and tuples make perfectly good keys.
fn derived_key_types() {
    section("Derived key types");

    // Sparse 2D grid: only the occupied cells are stored.
    let mut grid: HashMap<Point, char> = HashMap::new();
    grid.insert(Point { x: 0, y: 0 }, '#');
    grid.insert(Point { x: 3, y: -1 }, '@');
    grid.insert(Point { x: 0, y: 0 }, '+'); // same key -> replaced
    show("grid", &grid);
    println!("cell (3,-1) = {:?}", grid.get(&Point { x: 3, y: -1 }));
    println!("cell (1,1)  = {:?}", grid.get(&Point { x: 1, y: 1 }));

    // Enum keys.
    let mut cards: HashMap<Suit, Vec<u8>> = HashMap::new();
    cards.entry(Suit::Hearts).or_default().push(10);
    cards.entry(Suit::Spades).or_default().push(1);
    cards.entry(Suit::Hearts).or_default().push(3);
    show("cards by suit", &cards);

    // Tuple keys: a simple composite key without defining a struct.
    let mut distances: HashMap<(&str, &str), u32> = HashMap::new();
    distances.insert(("Berlin", "Munich"), 585);
    distances.insert(("Berlin", "Hamburg"), 289);
    println!("Berlin->Hamburg = {:?}", distances.get(&("Berlin", "Hamburg")));

    // Note: f32/f64 do not implement Eq or Hash (because NaN != NaN), so they
    // cannot be keys directly. Use integers (e.g. cents) or f64::to_bits.
}

/// A key whose equality is case-insensitive. Hash must agree with Eq:
/// if a == b then hash(a) == hash(b), otherwise lookups silently fail.
#[derive(Debug, Clone)]
struct CaseInsensitive(String);

impl PartialEq for CaseInsensitive {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq_ignore_ascii_case(&other.0)
    }
}

impl Eq for CaseInsensitive {}

impl Hash for CaseInsensitive {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for byte in self.0.bytes() {
            state.write_u8(byte.to_ascii_lowercase());
        }
    }
}

/// A key that hashes and compares only some of its fields.
#[derive(Debug)]
struct Employee {
    id: u32,
    name: String,
}

impl PartialEq for Employee {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Employee {}

impl Hash for Employee {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

/// Implementing Hash and Eq manually.
fn custom_hash_impl() {
    section("Custom Hash implementations");

    let mut headers: HashMap<CaseInsensitive, &str> = HashMap::new();
    headers.insert(CaseInsensitive("Content-Type".into()), "text/html");
    headers.insert(CaseInsensitive("content-type".into()), "application/json");
    println!("headers len = {} (both inserts used the same key)", headers.len());
    println!(
        "lookup CONTENT-TYPE -> {:?}",
        headers.get(&CaseInsensitive("CONTENT-TYPE".into()))
    );

    let mut salaries: HashMap<Employee, u32> = HashMap::new();
    salaries.insert(Employee { id: 7, name: "Ann".into() }, 50_000);
    // Same id, different name: treated as the same key. The value is
    // replaced but the original key (name "Ann") is kept.
    salaries.insert(Employee { id: 7, name: "Annie".into() }, 55_000);
    for (emp, salary) in &salaries {
        println!("employee #{} {} -> {salary}", emp.id, emp.name);
    }
}

/// Looking up String keys with &str thanks to the Borrow trait.
fn borrowed_lookups() {
    section("Borrowed lookups");

    let mut owners: HashMap<String, String> = HashMap::new();
    owners.insert("rex".to_string(), "Ann".to_string());
    owners.insert("tom".to_string(), "Bob".to_string());

    // String: Borrow<str>, so a &str works for get/contains_key/remove -
    // no temporary String has to be allocated.
    let pet: &str = "rex";
    println!("owners.get(\"rex\") = {:?}", owners.get(pet));
    println!("owners.remove(\"tom\") = {:?}", owners.remove("tom"));

    // The same holds for Vec<T> keys looked up by &[T], Box<T> by &T, etc.
    let mut paths: HashMap<Vec<u8>, &str> = HashMap::new();
    paths.insert(vec![1, 2, 3], "found");
    let slice: &[u8] = [1, 2, 3].as_slice();
    println!("Vec key looked up by slice: {:?}", paths.get(slice));

    // Maps may also store references as keys, borrowing data from elsewhere.
    let text = String::from("the quick brown fox");
    let positions: HashMap<&str, usize> =
        text.split_whitespace().enumerate().map(|(i, w)| (w, i)).collect();
    println!("position of \"brown\" = {:?}", positions.get("brown"));
    // `text` must outlive `positions`, which the borrow checker enforces.
}

/// What the hasher actually does, and how to plug in another one.
fn hashing_by_hand() {
    section("Hashing by hand");

    fn hash_of<T: Hash>(value: &T) -> u64 {
        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }

    // Equal values give equal hashes; DefaultHasher::new() uses fixed keys,
    // so these numbers are stable within one Rust version.
    println!("hash(\"abc\")  = {:#018x}", hash_of(&"abc"));
    println!("hash(\"abc\")  = {:#018x} (same)", hash_of(&"abc"));
    println!("hash(\"abd\")  = {:#018x} (different)", hash_of(&"abd"));

    // A HashMap's RandomState is seeded per map, so two maps hash the same
    // key differently. That is what randomizes iteration order.
    let s1 = RandomState::new();
    let s2 = RandomState::new();
    println!(
        "same key, two RandomStates -> equal hashes? {}",
        s1.hash_one("abc") == s2.hash_one("abc")
    );

    // A map can use any BuildHasher. Here a deterministic one is plugged in;
    // crates like `ahash` or `rustc-hash` (FxHashMap) provide faster ones
    // for when HashDoS resistance is not needed.
    use std::hash::BuildHasherDefault;
    type DeterministicMap<K, V> = HashMap<K, V, BuildHasherDefault<DefaultHasher>>;
    let mut map: DeterministicMap<&str, i32> = DeterministicMap::default();
    map.insert("one", 1);
    map.insert("two", 2);
    println!("map with custom hasher: get(\"two\") = {:?}", map.get("two"));
}

// ---------------------------------------------------------------------------
// Part 3: practical use cases
// ---------------------------------------------------------------------------

/// Counting words - the classic HashMap example.
fn word_frequencies() {
    section("Word frequencies");

    let text = "the cat sat on the mat the cat was happy";

    let mut freq: HashMap<&str, usize> = HashMap::new();
    for word in text.split_whitespace() {
        *freq.entry(word).or_insert(0) += 1;
    }

    // Top words: sort by count descending, then alphabetically for ties.
    let mut ranked: Vec<(&str, usize)> = freq.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    for (word, count) in ranked.iter().take(3) {
        println!("  {word:<5} {count}");
    }

    // Character frequencies, collected in one expression with fold.
    let letters = "mississippi".chars().fold(HashMap::new(), |mut m, c| {
        *m.entry(c).or_insert(0) += 1;
        m
    });
    show("letters in mississippi", &letters);
}

/// Grouping items into buckets: HashMap<K, Vec<V>>.
fn grouping_by_key() {
    section("Grouping by key");

    let words = ["apple", "avocado", "banana", "blueberry", "cherry", "apricot"];

    let mut by_letter: HashMap<char, Vec<&str>> = HashMap::new();
    for w in words {
        let first = w.chars().next().unwrap();
        by_letter.entry(first).or_default().push(w);
    }
    show("grouped by first letter", &by_letter);

    // Group anagrams: the sorted letters form the key.
    let candidates = ["listen", "silent", "enlist", "google", "tinsel", "banana"];
    let mut anagrams: HashMap<String, Vec<&str>> = HashMap::new();
    for w in candidates {
        let mut key: Vec<char> = w.chars().collect();
        key.sort_unstable();
        anagrams.entry(key.into_iter().collect()).or_default().push(w);
    }
    let mut groups: Vec<_> = anagrams.into_values().filter(|g| g.len() > 1).collect();
    groups.sort();
    println!("anagram groups: {groups:?}");
}

/// Caching results of an expensive recursive function.
fn memoized_fibonacci() {
    section("Memoization");

    fn fib(n: u64, memo: &mut HashMap<u64, u128>) -> u128 {
        if n < 2 {
            return n as u128;
        }
        if let Some(&cached) = memo.get(&n) {
            return cached;
        }
        let result = fib(n - 1, memo) + fib(n - 2, memo);
        memo.insert(n, result);
        result
    }

    let mut memo = HashMap::new();
    println!("fib(90)  = {}", fib(90, &mut memo));
    println!("fib(150) = {}", fib(150, &mut memo));
    println!("memo holds {} cached results", memo.len());
}

/// Swapping keys and values.
fn inverting_a_map() {
    section("Inverting a map");

    let codes = HashMap::from([("DE", 49), ("FR", 33), ("US", 1), ("CA", 1)]);

    // One-to-one inversion loses data when values repeat (US and CA share 1).
    let lossy: HashMap<i32, &str> = codes.iter().map(|(&k, &v)| (v, k)).collect();
    println!("lossy inversion has {} entries (from {})", lossy.len(), codes.len());

    // Inverting into HashMap<V, Vec<K>> keeps everything.
    let mut inverse: HashMap<i32, Vec<&str>> = HashMap::new();
    for (&country, &code) in &codes {
        inverse.entry(code).or_default().push(country);
    }
    for list in inverse.values_mut() {
        list.sort();
    }
    show("full inversion", &inverse);
}

/// Struct values with a small API built around a HashMap.
#[derive(Debug, Clone)]
struct Item {
    name: String,
    quantity: u32,
    price_cents: u64,
}

#[derive(Default)]
struct Inventory {
    items: HashMap<String, Item>,
}

impl Inventory {
    fn add(&mut self, sku: &str, name: &str, quantity: u32, price_cents: u64) {
        self.items
            .entry(sku.to_string())
            .and_modify(|item| item.quantity += quantity)
            .or_insert_with(|| Item { name: name.to_string(), quantity, price_cents });
    }

    fn take(&mut self, sku: &str, quantity: u32) -> Result<u32, String> {
        let item = self.items.get_mut(sku).ok_or_else(|| format!("unknown SKU {sku}"))?;
        if item.quantity < quantity {
            return Err(format!("only {} of {} left", item.quantity, item.name));
        }
        item.quantity -= quantity;
        let left = item.quantity;
        if left == 0 {
            self.items.remove(sku);
        }
        Ok(left)
    }

    fn total_value_cents(&self) -> u64 {
        self.items.values().map(|i| i.quantity as u64 * i.price_cents).sum()
    }

    fn report(&self) {
        let mut skus: Vec<_> = self.items.keys().collect();
        skus.sort();
        for sku in skus {
            let i = &self.items[sku];
            println!("  {sku}: {:<8} x{:<3} @ {:>5.2}", i.name, i.quantity, i.price_cents as f64 / 100.0);
        }
        println!("  total value: {:.2}", self.total_value_cents() as f64 / 100.0);
    }
}

fn inventory_system() {
    section("Inventory system");

    let mut inv = Inventory::default();
    inv.add("A-100", "widget", 10, 250);
    inv.add("B-200", "gadget", 3, 1999);
    inv.add("A-100", "widget", 5, 250); // restock merges quantities
    inv.report();

    println!("take 4 widgets  -> {:?}", inv.take("A-100", 4));
    println!("take 9 gadgets  -> {:?}", inv.take("B-200", 9));
    println!("take 3 gadgets  -> {:?}", inv.take("B-200", 3));
    println!("take from Z-999 -> {:?}", inv.take("Z-999", 1));
    inv.report();
}
