//! Read-optimised word index shared by members, groups, pages, and posts.
//! Each index takes a lowercase word to the ids containing it.

use std::collections::HashMap;
use std::sync::RwLock;

use crate::id::{GroupId, MemberId, PageId, PostId};
use crate::locks::{rd, wr};

/// One searchable field's inverted index. Owns its lock, so callers never
/// handle a guard to add to or read from it.
pub struct WordIndex<T> {
    entries: RwLock<HashMap<String, Vec<T>>>,
}

impl<T> Default for WordIndex<T> {
    fn default() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
        }
    }
}

impl<T: Copy + Ord> WordIndex<T> {
    /// Files `id` under every word of `text`, skipping words it already carries
    /// so result lists stay deduped.
    pub fn add(&self, id: T, text: &str) {
        let mut entries = wr(&self.entries);
        for word in text.split_whitespace() {
            let ids = entries.entry(word.to_lowercase()).or_default();
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
    }

    /// Ids under any word starting with `query`, sorted and deduped.
    pub fn look(&self, query: &str) -> Vec<T> {
        let query = query.to_lowercase();
        let mut ids: Vec<T> = rd(&self.entries)
            .iter()
            .filter(|(word, _)| word.starts_with(&query))
            .flat_map(|(_, ids)| ids.iter().copied())
            .collect();
        ids.sort();
        ids.dedup();
        ids
    }
}

/// The concrete search back-end, one index per searchable entity.
#[derive(Default)]
pub struct SearchIndex {
    pub members: WordIndex<MemberId>,
    pub groups: WordIndex<GroupId>,
    pub pages: WordIndex<PageId>,
    pub posts: WordIndex<PostId>,
}

impl SearchIndex {
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lookup_case_insensitive() {
        let idx = SearchIndex::new();
        idx.members.add(MemberId(1), "Alice Smith");
        assert_eq!(idx.members.look("alice"), vec![MemberId(1)]);
        assert_eq!(idx.members.look("ALICE"), vec![MemberId(1)]);
    }

    #[test]
    fn test_unknown_word() {
        let idx = SearchIndex::new();
        idx.members.add(MemberId(1), "Alice");
        assert!(idx.members.look("bob").is_empty());
    }

    #[test]
    fn test_shared_word() {
        let idx = SearchIndex::new();
        idx.posts.add(PostId(1), "learn rust the hard way");
        idx.posts.add(PostId(2), "write rust daily");
        assert_eq!(idx.posts.look("rust"), vec![PostId(1), PostId(2)]);
    }

    #[test]
    fn test_repeat_add() {
        let idx = SearchIndex::new();
        idx.posts.add(PostId(1), "rust rust rust");
        idx.posts.add(PostId(1), "rust");
        assert_eq!(idx.posts.look("rust"), vec![PostId(1)]);
    }
}
