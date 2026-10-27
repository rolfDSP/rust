//! A comprehensive tour of linked lists in Rust.
//!
//! Part 1 covers `std::collections::LinkedList<T>`, the standard library's
//! doubly-linked list. Pushing and popping at either end is O(1), and two
//! lists can be joined (`append`) in O(1) as well. Splitting at an index
//! (`split_off`) has to walk to that position, so it is O(n).
//!
//! Part 2 builds a singly-linked list from scratch (`Stack<T>`) to show how
//! ownership, `Box` and `Option` fit together: push/pop, peeking, in-place
//! reversal, the three kinds of iterators, and a non-recursive `Drop`.
//!
//! A word of caution: `Vec` and `VecDeque` are almost always faster than a
//! linked list because their elements sit next to each other in memory.
//! Reach for a linked list only when you really need O(1) splicing or stable
//! element addresses.
//!
//! Each section is a small function that demonstrates one group of features
//! and prints its results, so you can run the program and compare the output
//! with the code.

use std::collections::LinkedList;
use std::fmt;

fn main() {
    // Part 1: std::collections::LinkedList
    creating_lists();
    adding_and_removing();
    accessing_elements();
    iterating();
    searching();
    splitting_and_appending();
    removing_from_the_middle();
    sorting_and_deduplicating();
    comparing_lists();

    // Part 2: a hand-written singly-linked list
    custom_stack_basics();
    custom_stack_iterators();
    custom_stack_reverse();

    // Practical use cases.
    playlist_splicing();
    merge_sorted_lists();
}

fn section(title: &str) {
    println!("\n=== {title} ===");
}

// ---------------------------------------------------------------------------
// Part 1: std::collections::LinkedList
// ---------------------------------------------------------------------------

/// Different ways to construct a LinkedList.
fn creating_lists() {
    section("Creating lists");

    // Empty list; the element type is given explicitly.
    let empty: LinkedList<i32> = LinkedList::new();
    println!("empty: {empty:?}, len = {}, is_empty = {}", empty.len(), empty.is_empty());

    // From<[T; N]> is the most convenient way to build a small list.
    let numbers = LinkedList::from([1, 2, 3, 4, 5]);
    println!("from array: {numbers:?}");

    // Collect from any iterator.
    let squares: LinkedList<u32> = (1..=5).map(|n| n * n).collect();
    println!("collected squares: {squares:?}");

    // Extend an existing list with the contents of an iterator.
    let mut words = LinkedList::from(["alpha"]);
    words.extend(["beta", "gamma"]);
    println!("extended: {words:?}");

    // Default gives an empty list as well.
    let default: LinkedList<char> = LinkedList::default();
    println!("default: {default:?}");

    // Note: unlike Vec, there is no with_capacity. Every element is its own
    // heap allocation, created on demand.
}

/// Push and pop at both ends - all O(1).
fn adding_and_removing() {
    section("Adding and removing at the ends");

    let mut list = LinkedList::new();
    list.push_back(2);
    list.push_back(3);
    list.push_front(1);
    list.push_front(0);
    println!("after pushes: {list:?}");

    println!("pop_front -> {:?}", list.pop_front());
    println!("pop_back  -> {:?}", list.pop_back());
    println!("remaining: {list:?}");

    // Popping an empty list returns None instead of panicking.
    let mut empty: LinkedList<i32> = LinkedList::new();
    println!("pop on empty -> {:?}", empty.pop_front());

    // clear drops all nodes.
    list.clear();
    println!("after clear: {list:?}, len = {}", list.len());
}

/// Only the two ends can be accessed directly; there is no indexing.
fn accessing_elements() {
    section("Accessing elements");

    let mut list = LinkedList::from([10, 20, 30, 40]);

    println!("front = {:?}, back = {:?}", list.front(), list.back());

    // Mutable access to the ends.
    if let Some(first) = list.front_mut() {
        *first += 1;
    }
    if let Some(last) = list.back_mut() {
        *last *= 2;
    }
    println!("after front_mut/back_mut: {list:?}");

    // There is no list[2]. Reaching the n-th element means walking the list,
    // which is O(n) - one of the main trade-offs versus Vec.
    let third = list.iter().nth(2);
    println!("third element via iter().nth(2): {third:?}");
}

/// Borrowing, mutating and consuming iteration, in both directions.
fn iterating() {
    section("Iterating");

    let mut list = LinkedList::from(["one", "two", "three"]);

    // Shared borrow; the list is a DoubleEndedIterator, so rev() works.
    let forward: Vec<_> = list.iter().collect();
    let backward: Vec<_> = list.iter().rev().collect();
    println!("forward:  {forward:?}");
    println!("backward: {backward:?}");

    // Mutable borrow: change elements in place.
    let mut lengths: LinkedList<usize> = list.iter().map(|s| s.len()).collect();
    for len in lengths.iter_mut() {
        *len *= 10;
    }
    println!("lengths * 10: {lengths:?}");

    // `for x in &list` is sugar for list.iter().
    for (i, word) in (&list).into_iter().enumerate() {
        println!("  [{i}] {word}");
    }

    // Consuming iteration moves the elements out; the list is gone afterwards.
    list.push_back("four");
    let upper: Vec<String> = list.into_iter().map(str::to_uppercase).collect();
    println!("consumed into: {upper:?}");
}

/// Finding elements - all linear scans.
fn searching() {
    section("Searching");

    let list = LinkedList::from([3, 8, 15, 4, 42, 23]);

    println!("contains(&42) = {}", list.contains(&42));
    println!("contains(&7)  = {}", list.contains(&7));

    let first_even = list.iter().find(|&&n| n % 2 == 0);
    println!("first even: {first_even:?}");

    let pos = list.iter().position(|&n| n == 15);
    println!("position of 15: {pos:?}");

    let max = list.iter().max();
    let sum: i32 = list.iter().sum();
    println!("max = {max:?}, sum = {sum}");
}

/// The operations that make linked lists interesting: cheap joining.
fn splitting_and_appending() {
    section("Splitting and appending");

    let mut a = LinkedList::from([1, 2, 3]);
    let mut b = LinkedList::from([4, 5, 6]);

    // append moves all nodes of b into a in O(1) - just relinks pointers.
    // b is left empty but still usable.
    a.append(&mut b);
    println!("a after append: {a:?}");
    println!("b after append: {b:?} (empty)");

    // split_off(at) returns everything from index `at` onward. It must walk
    // to that index (O(n)), but no elements are copied or reallocated.
    let tail = a.split_off(4);
    println!("a after split_off(4): {a:?}");
    println!("returned tail:        {tail:?}");
}

/// Stable Rust has no LinkedList::remove(index), but split_off + append does it.
fn removing_from_the_middle() {
    section("Removing / inserting in the middle");

    let mut list = LinkedList::from(['a', 'b', 'X', 'c', 'd']);
    let removed = remove_at(&mut list, 2);
    println!("removed {removed:?}, list now {list:?}");

    insert_at(&mut list, 2, 'Y');
    println!("after insert_at(2, 'Y'): {list:?}");

    // To remove all elements matching a predicate, rebuilding is simplest.
    let mut numbers: LinkedList<i32> = (1..=10).collect();
    numbers = numbers.into_iter().filter(|n| n % 3 != 0).collect();
    println!("without multiples of 3: {numbers:?}");
}

/// Remove and return the element at `index`, or None if out of bounds.
fn remove_at<T>(list: &mut LinkedList<T>, index: usize) -> Option<T> {
    if index >= list.len() {
        return None;
    }
    let mut tail = list.split_off(index);
    let removed = tail.pop_front();
    list.append(&mut tail);
    removed
}

/// Insert `value` so that it ends up at position `index`.
fn insert_at<T>(list: &mut LinkedList<T>, index: usize, value: T) {
    let index = index.min(list.len());
    let mut tail = list.split_off(index);
    list.push_back(value);
    list.append(&mut tail);
}

/// LinkedList has no sort(), so we go through a Vec.
fn sorting_and_deduplicating() {
    section("Sorting and deduplicating");

    let list = LinkedList::from([5, 3, 9, 3, 1, 5, 7]);

    let mut v: Vec<_> = list.into_iter().collect();
    v.sort_unstable();
    v.dedup();
    let sorted: LinkedList<_> = v.into_iter().collect();
    println!("sorted + deduped: {sorted:?}");
}

/// LinkedList implements PartialEq, Ord, Hash, Clone, ... when T does.
fn comparing_lists() {
    section("Comparing and cloning");

    let a = LinkedList::from([1, 2, 3]);
    let b: LinkedList<_> = (1..=3).collect();
    let c = LinkedList::from([1, 2, 4]);

    println!("a == b: {}", a == b);
    println!("a <  c: {} (lexicographic)", a < c);

    // clone performs a deep copy: every node is reallocated.
    let mut d = a.clone();
    d.push_back(4);
    println!("a = {a:?}, cloned+modified d = {d:?}");
}

// ---------------------------------------------------------------------------
// Part 2: a hand-written singly-linked list
// ---------------------------------------------------------------------------

/// A link is either empty or owns the next node on the heap.
type Link<T> = Option<Box<Node<T>>>;

struct Node<T> {
    value: T,
    next: Link<T>,
}

/// A singly-linked stack: push and pop work at the head in O(1).
pub struct Stack<T> {
    head: Link<T>,
    len: usize,
}

impl<T> Stack<T> {
    pub fn new() -> Self {
        Stack { head: None, len: 0 }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.head.is_none()
    }

    pub fn push(&mut self, value: T) {
        // take() moves the old head out, leaving None behind, so the new node
        // can own it without fighting the borrow checker.
        let new_node = Box::new(Node {
            value,
            next: self.head.take(),
        });
        self.head = Some(new_node);
        self.len += 1;
    }

    pub fn pop(&mut self) -> Option<T> {
        self.head.take().map(|node| {
            self.head = node.next;
            self.len -= 1;
            node.value
        })
    }

    pub fn peek(&self) -> Option<&T> {
        self.head.as_ref().map(|node| &node.value)
    }

    pub fn peek_mut(&mut self) -> Option<&mut T> {
        self.head.as_mut().map(|node| &mut node.value)
    }

    /// Reverse the list in place by relinking nodes - no allocation.
    pub fn reverse(&mut self) {
        let mut prev: Link<T> = None;
        let mut current = self.head.take();
        while let Some(mut node) = current {
            current = node.next.take();
            node.next = prev;
            prev = Some(node);
        }
        self.head = prev;
    }

    pub fn iter(&self) -> Iter<'_, T> {
        Iter {
            next: self.head.as_deref(),
        }
    }

    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        IterMut {
            next: self.head.as_deref_mut(),
        }
    }
}

impl<T> Default for Stack<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// The default (recursive) drop of Box<Node> could overflow the stack for very
/// long lists, so we unlink the nodes one at a time in a loop.
impl<T> Drop for Stack<T> {
    fn drop(&mut self) {
        let mut current = self.head.take();
        while let Some(mut node) = current {
            current = node.next.take();
        }
    }
}

/// Consuming iterator: yields owned values.
pub struct IntoIter<T>(Stack<T>);

impl<T> Iterator for IntoIter<T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        self.0.pop()
    }
}

impl<T> IntoIterator for Stack<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;
    fn into_iter(self) -> IntoIter<T> {
        IntoIter(self)
    }
}

/// Borrowing iterator: yields &T.
pub struct Iter<'a, T> {
    next: Option<&'a Node<T>>,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;
    fn next(&mut self) -> Option<&'a T> {
        self.next.map(|node| {
            self.next = node.next.as_deref();
            &node.value
        })
    }
}

impl<'a, T> IntoIterator for &'a Stack<T> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;
    fn into_iter(self) -> Iter<'a, T> {
        self.iter()
    }
}

/// Mutable iterator: yields &mut T. take() guarantees each node is handed
/// out at most once, which is what makes this safe.
pub struct IterMut<'a, T> {
    next: Option<&'a mut Node<T>>,
}

impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = &'a mut T;
    fn next(&mut self) -> Option<&'a mut T> {
        self.next.take().map(|node| {
            self.next = node.next.as_deref_mut();
            &mut node.value
        })
    }
}

/// Collecting pushes each item, so the last item ends up on top.
impl<T> FromIterator<T> for Stack<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut stack = Stack::new();
        for value in iter {
            stack.push(value);
        }
        stack
    }
}

/// Prints the list as `a -> b -> c -> None`.
impl<T: fmt::Display> fmt::Display for Stack<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for value in self {
            write!(f, "{value} -> ")?;
        }
        write!(f, "None")
    }
}

fn custom_stack_basics() {
    section("Custom Stack: push, pop, peek");

    let mut stack = Stack::new();
    println!("new: {stack}, is_empty = {}", stack.is_empty());

    stack.push(1);
    stack.push(2);
    stack.push(3);
    println!("after pushing 1, 2, 3: {stack} (len = {})", stack.len());

    println!("peek -> {:?}", stack.peek());
    if let Some(top) = stack.peek_mut() {
        *top = 30;
    }
    println!("after peek_mut: {stack}");

    println!("pop -> {:?}", stack.pop());
    println!("pop -> {:?}", stack.pop());
    println!("pop -> {:?}", stack.pop());
    println!("pop -> {:?} (empty)", stack.pop());

    // A long list is dropped without overflowing the call stack thanks to
    // the iterative Drop implementation.
    let big: Stack<u32> = (0..1_000_000).collect();
    println!("built a list with {} nodes; dropping it now", big.len());
    drop(big);
}

fn custom_stack_iterators() {
    section("Custom Stack: iterators");

    let mut stack: Stack<i32> = (1..=5).collect();
    println!("collected 1..=5: {stack}");

    let doubled: Vec<i32> = stack.iter().map(|n| n * 2).collect();
    println!("iter().map(*2): {doubled:?}");

    for n in stack.iter_mut() {
        *n += 100;
    }
    println!("after iter_mut (+100): {stack}");

    let owned: Vec<i32> = stack.into_iter().collect();
    println!("into_iter (consumed): {owned:?}");
}

fn custom_stack_reverse() {
    section("Custom Stack: in-place reverse");

    let mut stack: Stack<char> = "rust".chars().collect();
    println!("before: {stack}");
    stack.reverse();
    println!("after:  {stack}");
}

// ---------------------------------------------------------------------------
// Practical use cases
// ---------------------------------------------------------------------------

/// A music playlist where whole blocks of songs are moved around.
/// split_off + append relink nodes instead of copying elements.
fn playlist_splicing() {
    section("Use case: playlist splicing");

    let mut playlist = LinkedList::from(["Intro", "Song A", "Song B", "Song C", "Outro"]);
    let mut queue_next = LinkedList::from(["Request 1", "Request 2"]);
    println!("playlist: {playlist:?}");

    // Play the first song.
    if let Some(now_playing) = playlist.pop_front() {
        println!("now playing: {now_playing}");
    }

    // Insert the requested songs right after the current position.
    queue_next.append(&mut playlist);
    playlist = queue_next;
    println!("with requests queued: {playlist:?}");

    // Move the last two songs (e.g. "Song C", "Outro") to the front.
    let mut last_two = playlist.split_off(playlist.len() - 2);
    last_two.append(&mut playlist);
    playlist = last_two;
    println!("after moving the last two to the front: {playlist:?}");
}

/// Classic interview problem: merge two sorted singly-linked lists by
/// relinking existing nodes (no new allocations).
fn merge_sorted_lists() {
    section("Use case: merging two sorted lists");

    // Collecting into Stack reverses the order, so feed it descending input
    // to get ascending lists.
    let a: Stack<i32> = [1, 4, 7, 10].into_iter().rev().collect();
    let b: Stack<i32> = [2, 3, 8, 11, 12].into_iter().rev().collect();
    println!("a: {a}");
    println!("b: {b}");

    let merged = merge(a, b);
    println!("merged: {merged}");
}

fn merge<T: Ord>(mut a: Stack<T>, mut b: Stack<T>) -> Stack<T> {
    let mut result = Stack::new();
    let mut left = a.head.take();
    let mut right = b.head.take();
    let len = a.len + b.len;

    // `tail` always points at the empty link at the end of `result`, where
    // the next node gets attached.
    let mut tail = &mut result.head;
    loop {
        let next = match (left.take(), right.take()) {
            (Some(mut l), Some(r)) if l.value <= r.value => {
                left = l.next.take();
                right = Some(r);
                l
            }
            (Some(l), Some(mut r)) => {
                right = r.next.take();
                left = Some(l);
                r
            }
            // One side is exhausted: attach the rest of the other one.
            (rest @ Some(_), None) | (None, rest @ Some(_)) => {
                *tail = rest;
                break;
            }
            (None, None) => break,
        };
        tail = &mut tail.insert(next).next;
    }

    result.len = len;
    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_pop_is_lifo() {
        let mut s = Stack::new();
        s.push(1);
        s.push(2);
        assert_eq!(s.len(), 2);
        assert_eq!(s.pop(), Some(2));
        assert_eq!(s.pop(), Some(1));
        assert_eq!(s.pop(), None);
        assert!(s.is_empty());
    }

    #[test]
    fn reverse_reverses() {
        let mut s: Stack<i32> = (1..=4).collect();
        s.reverse();
        assert_eq!(s.iter().copied().collect::<Vec<_>>(), vec![1, 2, 3, 4]);
    }

    #[test]
    fn merge_keeps_order_and_len() {
        let a: Stack<i32> = [5, 3, 1].into_iter().collect();
        let b: Stack<i32> = [6, 4, 2].into_iter().collect();
        let m = merge(a, b);
        assert_eq!(m.len(), 6);
        assert_eq!(m.into_iter().collect::<Vec<_>>(), vec![1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn remove_and_insert_at() {
        let mut list = LinkedList::from([1, 2, 3]);
        assert_eq!(remove_at(&mut list, 1), Some(2));
        assert_eq!(remove_at(&mut list, 5), None);
        insert_at(&mut list, 1, 9);
        assert_eq!(list, LinkedList::from([1, 9, 3]));
    }
}
