use std::cell::RefCell;
use std::rc::Rc;

pub(crate) type NodeRef = Rc<RefCell<LinkedNode>>;

#[derive(Debug)]
pub struct LinkedNode {
    #[warn(unused)]
    pub(crate) key: i64,
    pub(crate) value: i64,
    next: Option<NodeRef>,
    prev: Option<NodeRef>,
}

impl LinkedNode {
    pub fn new(key: i64, value: i64) -> Self {
        Self {
            key,
            value,
            prev: None,
            next: None,
        }
    }
}

#[derive(Debug)]
pub struct DoublyLinked {
    head: Option<NodeRef>,
    tail: Option<NodeRef>,
}

impl DoublyLinked {
    pub fn new() -> Self {
        Self {
            head: None,
            tail: None,
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
            }
            // If this is the tail node
            (Some(n1), None) => {
                n1.borrow_mut().next = None;
                self.tail = Some(n1);
            }
            // If this is the head node
            (None, Some(n2)) => {
                n2.borrow_mut().prev = None;
                self.head = Some(n2);
            }
            // If this is the only node present
            (None, None) => {
                self.head = None;
                self.tail = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(key: i64) -> NodeRef {
        Rc::new(RefCell::new(LinkedNode::new(key, key * 10)))
    }

    /// Three nodes, head -> 1 -> 2 -> 3 <- tail.
    fn list_of_three() -> (DoublyLinked, NodeRef, NodeRef, NodeRef) {
        let (a, b, c) = (node(1), node(2), node(3));
        let mut list = DoublyLinked::new();
        list.add_to_head(c.clone());
        list.add_to_head(b.clone());
        list.add_to_head(a.clone());
        list.tail = Some(c.clone());
        (list, a, b, c)
    }

    /// Bails out rather than looping forever if the links form a cycle.
    fn forward(list: &DoublyLinked) -> Vec<i64> {
        let mut keys = Vec::new();
        let mut cur = list.head.clone();
        while let Some(n) = cur {
            if keys.len() > 8 {
                keys.push(-1);
                break;
            }
            keys.push(n.borrow().key);
            cur = n.borrow().next.clone();
        }
        keys
    }

    fn backward(list: &DoublyLinked) -> Vec<i64> {
        let mut keys = Vec::new();
        let mut cur = list.tail.clone();
        while let Some(n) = cur {
            if keys.len() > 8 {
                keys.push(-1);
                break;
            }
            keys.push(n.borrow().key);
            cur = n.borrow().prev.clone();
        }
        keys
    }

    #[test]
    fn test_move_tail() {
        let (mut list, _, _, c) = list_of_three();
        list.move_to_head(c);
        assert_eq!(forward(&list), vec![3, 1, 2]);
        assert_eq!(backward(&list), vec![2, 1, 3]);
    }

    #[test]
    fn test_move_middle() {
        let (mut list, _, b, _) = list_of_three();
        list.move_to_head(b);
        assert_eq!(forward(&list), vec![2, 1, 3]);
        assert_eq!(backward(&list), vec![3, 1, 2]);
    }

    #[test]
    fn test_move_head() {
        let (mut list, a, _, _) = list_of_three();
        list.move_to_head(a);
        assert_eq!(forward(&list), vec![1, 2, 3]);
    }

    #[test]
    fn test_remove_head() {
        let (mut list, a, _, _) = list_of_three();
        list.remove(a);
        assert_eq!(forward(&list), vec![2, 3]);
        assert_eq!(backward(&list), vec![3, 2]);
    }
}
