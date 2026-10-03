// # Intrusive In-Memory LRU Cache without `Rc<RefCell<T>>`
//
// ## Prompt:
// Build a high-throughput, thread-safe LRU cache with fixed capacity. Avoid recursive reference cycles 
// and explain why standard doubly-linked lists with `Rc<RefCell<...>>` are avoided in performance-critical Rust code.
//
// ## Key Focus:
// * Implementing index-based links inside a contiguous `Vec<Node<K, V>>`.
// * Granular locking (`parking_lot::RwLock` or striped locks) vs single coarse-grained `Mutex`.
//
// ## How it works
// - Lru<K, V> is a single-shard LRU. Nodes live in a Vec<Node<K, V>>, and prev/next are usize indices with
// NIL = usize::MAX as the end marker. A HashMap<K, usize> maps each key to its slot.
// - When the cache is full, put evicts the tail and reuses its slot with mem::replace. The Vec never grows
// past capacity, so there are no allocations after warm-up.
// - ShardedLru holds N independent Mutex<Lru> shards and picks one with RandomState::hash_one(key) % N.
// This is the striped-lock option from the prompt.
//
// ## Why not Rc<RefCell<Node>> (the prompt asks for this explanation)
// - Every node is a separate heap allocation, and nodes point at each other through scattered pointers.
// Index links in one Vec stay contiguous, which is much friendlier to the cache.
// - A doubly linked list built from Rc forms cycles (prev and next point at each other), so nothing is
// ever dropped. You need Weak for the back link, which adds more overhead and upgrade() calls.
// - Each RefCell adds a runtime borrow flag and a possible panic. Rc is also !Send, so it can't be used
// across threads at all. With indices, the borrow checker covers everything at compile time.
//
// ## Why Mutex and not RwLock
// - A cache hit moves the entry to the front of the list, so get is a write. A read lock would not be
// enough.
// - Striping is what cuts contention. I left a short comment on this in the file.

// ## Solution
//
// `Lru` is one single-threaded LRU; `ShardedLru` stripes keys across many of them.
//   * Nodes live in a `Vec<Node>` allocated up front and are linked by `usize` indices
//     (`prev`/`next`, `NIL` = none) into a doubly linked list ordered by recency: `head` is the
//     most recent, `tail` the least. `HashMap<K, usize>` maps a key to its slot, so get/put are
//     O(1): look up the slot, unlink it, relink at the head.
//   * When full, `put` evicts `tail` and overwrites that slot in place, so the `Vec` never
//     grows or reallocates and slots are reused.
//   * Why not `Rc<RefCell<Node>>`: one heap allocation per node scattered across memory (poor
//     cache locality); `prev`/`next` form reference cycles so nothing is freed without `Weak`
//     and `upgrade()` calls; `RefCell` adds a runtime borrow flag and a possible panic; and
//     `Rc` is `!Send`, so it cannot cross threads. Indices into a `Vec` have none of these costs
//     and are checked by the ordinary borrow checker.
//   * Concurrency: keys are hashed to one of N shards, each behind its own `Mutex`, so threads
//     touching different shards never contend. Recency is tracked per shard, so global eviction
//     is only approximately LRU. A global exact LRU would need one lock for the whole cache.

use std::collections::HashMap;
use std::hash::{BuildHasher, Hash, RandomState};
use std::sync::Mutex;

const NIL: usize = usize::MAX;

struct Node<K, V> {
    key: K,
    value: V,
    prev: usize,
    next: usize,
}

struct Lru<K, V> {
    capacity: usize,
    map: HashMap<K, usize>,
    nodes: Vec<Node<K, V>>,
    head: usize,
    tail: usize,
}

impl<K: Hash + Eq + Clone, V: Clone> Lru<K, V> {
    fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "capacity must be non-zero");
        Self {
            capacity,
            map: HashMap::with_capacity(capacity),
            nodes: Vec::with_capacity(capacity),
            head: NIL,
            tail: NIL,
        }
    }

    fn get(&mut self, key: &K) -> Option<V> {
        let idx = *self.map.get(key)?;
        self.touch(idx);
        Some(self.nodes[idx].value.clone())
    }

    fn put(&mut self, key: K, value: V) {
        if let Some(&idx) = self.map.get(&key) {
            self.nodes[idx].value = value;
            self.touch(idx);
            return;
        }
        let node = Node { key: key.clone(), value, prev: NIL, next: NIL };
        // Both branches only choose the slot; the insert and `push_front` after it run for both.
        let idx = if self.nodes.len() < self.capacity {
            self.nodes.push(node);
            self.nodes.len() - 1
        } else {
            let idx = self.tail;
            self.detach(idx);
            let evicted = std::mem::replace(&mut self.nodes[idx], node);
            self.map.remove(&evicted.key);
            idx
        };
        self.map.insert(key, idx);
        self.push_front(idx);
    }

    fn touch(&mut self, idx: usize) {
        if self.head != idx {
            self.detach(idx);
            self.push_front(idx);
        }
    }

    fn detach(&mut self, idx: usize) {
        let (prev, next) = (self.nodes[idx].prev, self.nodes[idx].next);
        match prev {
            NIL => self.head = next,
            p => self.nodes[p].next = next,
        }
        match next {
            NIL => self.tail = prev,
            n => self.nodes[n].prev = prev,
        }
    }

    fn push_front(&mut self, idx: usize) {
        self.nodes[idx].prev = NIL;
        self.nodes[idx].next = self.head;
        match self.head {
            NIL => self.tail = idx,
            h => self.nodes[h].prev = idx,
        }
        self.head = idx;
    }
}

// A hit reorders the list, so `get` is a write: an `RwLock` read guard would not be enough
// and would only add overhead. Striping is what cuts contention. Recency is per shard, so
// eviction is approximately (not globally) least-recently-used.
struct ShardedLru<K, V> {
    hasher: RandomState,
    shards: Box<[Mutex<Lru<K, V>>]>,
}

impl<K: Hash + Eq + Clone, V: Clone> ShardedLru<K, V> {
    fn new(capacity: usize, shards: usize) -> Self {
        assert!(shards > 0 && capacity >= shards, "need capacity >= shards > 0");
        Self {
            hasher: RandomState::new(),
            shards: (0..shards)
                .map(|_| Mutex::new(Lru::new(capacity.div_ceil(shards))))
                .collect(),
        }
    }

    fn shard(&self, key: &K) -> &Mutex<Lru<K, V>> {
        let hash = self.hasher.hash_one(key) as usize;
        &self.shards[hash % self.shards.len()]
    }

    fn get(&self, key: &K) -> Option<V> {
        self.shard(key).lock().unwrap().get(key)
    }

    fn put(&self, key: K, value: V) {
        self.shard(&key).lock().unwrap().put(key, value);
    }
}

fn main() {
    let cache = ShardedLru::new(1024, 8);
    std::thread::scope(|s| {
        for t in 0..4u64 {
            let cache = &cache;
            s.spawn(move || {
                for i in 0..10_000u64 {
                    let key = (i * 4 + t) % 2048;
                    if cache.get(&key).is_none() {
                        cache.put(key, key * 2);
                    }
                }
            });
        }
    });
    println!("hit 7 -> {:?}", cache.get(&7));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evicts_least_recently_used() {
        let mut c = Lru::new(2);
        c.put(1, "a");
        c.put(2, "b");
        assert_eq!(c.get(&1), Some("a"));
        c.put(3, "c");
        assert_eq!(c.get(&2), None, "2 was the LRU entry");
        assert_eq!(c.get(&1), Some("a"));
        assert_eq!(c.get(&3), Some("c"));
    }

    #[test]
    fn put_existing_updates_and_refreshes() {
        let mut c = Lru::new(2);
        c.put(1, 10);
        c.put(2, 20);
        c.put(1, 11);
        c.put(3, 30);
        assert_eq!(c.get(&1), Some(11));
        assert_eq!(c.get(&2), None);
    }

    #[test]
    fn capacity_one_and_slot_reuse() {
        let mut c = Lru::new(1);
        for i in 0..10 {
            c.put(i, i);
            assert_eq!(c.get(&i), Some(i));
        }
        assert_eq!(c.nodes.len(), 1, "evicted slot is reused, never grown");
        assert_eq!(c.map.len(), 1);
    }

    #[test]
    fn sharded_bounded_under_contention() {
        let cache = ShardedLru::new(64, 4);
        std::thread::scope(|s| {
            for t in 0..4u64 {
                let cache = &cache;
                s.spawn(move || {
                    for i in 0..5_000u64 {
                        cache.put(i + t, i);
                        cache.get(&(i / 2));
                    }
                });
            }
        });
        let total: usize = cache.shards.iter().map(|m| m.lock().unwrap().map.len()).sum();
        assert!(total <= 64);
    }
}
