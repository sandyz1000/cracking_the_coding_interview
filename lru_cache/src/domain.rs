use std::collections::HashMap;
use crate::doubly_linked::{DoublyLinked, NodeRef};

// 
pub struct LruCache {
    linked: DoublyLinked,
    items: HashMap<i64, NodeRef>,
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
        // If key is present, fetch the value and move the node to head else return -1
        if let Some(node) = self.items.get(&key) {
            self.linked.move_to_head(node.clone());
            let s = node.borrow().value.clone();
            return s;
        } else {
            return -1;
        }
    }

    // Check if the item is in the cache then update the value and reorder the position of the node
    // Else, 
    pub fn put(&mut self, key: i64, val: i64) {
        if let Some(node) = self.items.get(&key) {
            node.borrow_mut().value = val;
            self.linked.move_to_head(node.clone());
        } else {
            if self.capacity == self.items.len() as u64 {
                // remove the tail item first
            }

            
        }
    }
}
