use std::{cell::RefCell, collections::HashMap, rc::Rc};

type NodeRef = Rc<RefCell<LinkedNode>>;

#[derive(Debug, Clone)]
struct LinkedNode {
    val: i32,
    prev: Option<NodeRef>,
    next: Option<NodeRef>,
}

impl LinkedNode {
    fn new(val: i32) -> NodeRef {
        Rc::new(RefCell::new(Self {
            val: val,
            prev: None,
            next: None,
        }))
    }
}

#[derive(Debug, Clone)]
struct DoublyLinkedList {
    head: Option<NodeRef>,
    tail: Option<NodeRef>,
}

impl DoublyLinkedList {
    fn new() -> Self {
        Self {
            head: None,
            tail: None,
        }
    }

    fn add_to_front(&mut self, node: NodeRef) {}

    fn add_to_back(&mut self, node: NodeRef) {}

    fn move_to_head(&mut self, node: NodeRef) {}

    fn move_to_tail(&mut self, node: NodeRef) {}

    fn remove(&mut self, node: NodeRef) {
        let (prev, next) = (
            node.borrow_mut().next.clone(),
            node.borrow_mut().prev.clone(),
        );
        match (prev, next) {
            (Some(p1), Some(n1)) => {}
            (Some(p1), None) => {}
            (None, Some(n1)) => {}
            (None, None) => {}
        }
    }
}

#[derive(Debug)]
pub struct LruCache {
    dll: DoublyLinkedList,
    cache: HashMap<i32, NodeRef>,
}

impl LruCache {
    fn get(&mut self, key: i32) -> i32 {
        0
    }

    fn set(&mut self, key: i32, val: i32) {}
}

fn main() {
    println!("Implementing LRU!!");
}
