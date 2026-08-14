use std::collections::HashMap;
use crate::doubly_linked::{DoublyLinked, LinkedNode};

// 
pub struct LruCache {
    linked: DoublyLinked,
    items: HashMap<i64, LinkedNode>,
    capacity: u64
}

impl LruCache {
    pub fn new(capacity: u64) -> Self {
        Self {
            linked: DoublyLinked::new(),
            items: HashMap::new(),
            capacity,
        }
    }

    pub fn get(&mut self, key: i64) -> i64 {
        todo!()
    }

    // Check if the item is in the cache then update the value and reorder the position of the node
    // Else, 
    pub fn put(&mut self, key: i64, val: i64) {
        if self.items.contains_key(&key) {
            todo!()
        } else {
            if self.capacity == self.items.len() as u64 {
                // remove the tail item first
            }

            
        }
    }
}
