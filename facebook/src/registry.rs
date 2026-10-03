//! An id-allocating store: the counter lives with the map it feeds, so an id
//! can only ever be drawn for the registry it is inserted into.

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::locks::wr;

pub struct Registry<Id, T> {
    pub items: RwLock<HashMap<Id, T>>,
    next: AtomicU64,
}

impl<Id, T> Default for Registry<Id, T> {
    fn default() -> Self {
        Self {
            items: RwLock::new(HashMap::new()),
            next: AtomicU64::new(0),
        }
    }
}

impl<Id, T> Registry<Id, T>
where
    Id: From<u64> + Eq + Hash + Copy,
    T: Clone,
{
    /// Allocate the next id, build the entity from it, and store it.
    pub fn insert(&self, build: impl FnOnce(Id) -> T) -> T {
        let id = Id::from(self.next.fetch_add(1, Ordering::Relaxed) + 1);
        let value = build(id);
        wr(&self.items).insert(id, value.clone());
        value
    }
}
