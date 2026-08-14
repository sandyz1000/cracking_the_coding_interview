use std::rc::Rc;
use std::cell::RefCell;

type NodeRef = Rc<RefCell<LinkedNode>>;

#[derive(Debug)]
pub struct LinkedNode {
    key: i64,
    value: i64,
    next: Option<NodeRef>,
    prev: Option<NodeRef>,
}

impl LinkedNode {
    pub fn new(key: i64, value: i64) -> Self {
        Self { key, value, prev: None, next: None }
    }
}

#[derive(Debug)]
pub struct DoublyLinked {
    head: Option<NodeRef>,
    tail: Option<NodeRef>
}

impl DoublyLinked {
    pub fn new() -> Self {
        Self {
            head: None,
            tail: None
        }
    }

    pub(crate) fn move_to_head(&mut self, node: NodeRef) {
        if self.head.is_none() {
            return;
        }
        // Check if the node is not the head
        let head_node = self.head.as_ref().unwrap();
        if !Rc::ptr_eq(head_node, &node) {
            // Remove the node and move to head
            self.remove(node.clone());
            self.add_to_head(node);
        }
    }

    pub(crate) fn move_to_tail(&mut self, node: NodeRef) {
        if self.tail.is_none() {
            return;
        }
        // Check if the node is not the head
        let tail_node = self.tail.as_ref().unwrap();
        if !Rc::ptr_eq(tail_node, &node) {
            // Remove the node and move to head
            self.remove(node.clone());
            self.add_to_tail(node);
        }
    }

    pub(crate) fn add_to_head(&mut self, node: NodeRef) {
        let mut curr = self.head.take();
        node.borrow_mut().next = curr.clone();
        if let Some(n1) = curr.as_mut() {
            n1.borrow_mut().prev = Some(node.clone());
        }
        self.head = Some(node);
    }

    pub(crate) fn add_to_tail(&mut self, node: NodeRef) {
        let mut curr = self.tail.take();
        node.borrow_mut().prev = curr.clone();
        if let Some(n2) = curr.as_mut() {
            n2.borrow_mut().next = Some(node.clone());
        }
        self.tail = Some(node);
    }

    pub(crate) fn remove(&mut self, node: NodeRef) {
        let prev = node.borrow_mut().prev.take();
        let next = node.borrow_mut().next.take();
        match (prev, next) {
            // If this is the mid node
            (Some(n1), Some(n2)) => {
                n1.borrow_mut().next = Some(n2.clone());
                n2.borrow_mut().prev = Some(n1.clone());
            },
            // If this is the tail node
            (Some(n1), None) => {
                self.tail = Some(n1);
            },
            // If this is the head node
            (None, Some(n2)) => {
                self.head = Some(n2);
            },
            // If this is the only node present
            (None, None) => {
                self.head = None;
                self.tail = None;
            }
        }
    }
}
