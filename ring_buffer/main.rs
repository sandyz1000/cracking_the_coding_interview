// To design a circular queue, it need to have a vector of fixed capacity, a head that point
// to first item in queue and the curr_size of the queue
struct CircularQueue {
    capacity: i64,
    head: i64,
    curr_size: i64,
    items: Vec<i64>,
}

impl CircularQueue {
    fn new(capacity: i64) -> Self {
        // -1 indicate that queue is empty
        Self {
            capacity,
            head: 0,
            curr_size: 0,
            items: vec![0; capacity as usize],
        }
    }

    pub(crate) fn push(&mut self, item: i64) -> Option<bool> {
        // Check if there are space for the new item
        if self.is_full() {
            return None;
        }
        let idx = (self.head + self.curr_size) % self.capacity;
        self.curr_size += 1;
        self.items[idx as usize] = item;
        Some(true)
    }

    // This popped from the front
    pub(crate) fn pop(&mut self) -> Option<i64> {
        if self.is_empty() {
            return None;
        }
        let idx = self.head;
        self.head = (self.head + 1) % self.capacity;
        self.curr_size -= 1;
        Some(self.items[idx as usize])
    }

    pub(crate) fn front(&self) -> Option<i64> {
        if self.is_empty() {
            return None;
        }
        let val = self.items[self.head as usize];
        Some(val)
    }

    pub(crate) fn back(&self) -> Option<i64> {
        if self.is_empty() {
            return None;
        }
        let idx = (self.head + self.curr_size - 1) % self.capacity;
        Some(self.items[idx as usize])
    }

    fn is_empty(&self) -> bool {
        self.curr_size == 0
    }

    fn is_full(&self) -> bool {
        self.curr_size == self.capacity
    }
}

fn main() {
    println!("Hello world")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a queue with its backing store already populated, so the read
    /// paths can be exercised independently of `push`.
    fn seeded(capacity: i64, items: Vec<i64>, head: i64, curr_size: i64) -> CircularQueue {
        CircularQueue {
            capacity,
            head,
            curr_size,
            items,
        }
    }

    #[test]
    fn test_new_empty() {
        let mut q = CircularQueue::new(4);
        assert!(q.is_empty());
        assert!(!q.is_full());
        assert_eq!(q.front(), None);
        assert_eq!(q.back(), None);
        assert_eq!(q.pop(), None);
    }

    #[test]
    fn test_push_stores() {
        let mut q = CircularQueue::new(3);
        assert_eq!(q.push(10), Some(true));
        assert_eq!(q.front(), Some(10));
    }

    #[test]
    fn test_push_fills() {
        let mut q = CircularQueue::new(2);
        assert_eq!(q.push(1), Some(true));
        assert_eq!(q.push(2), Some(true));
        assert!(q.is_full());
        assert_eq!(q.push(3), None, "a full queue rejects a push");
    }

    #[test]
    fn test_back_last() {
        // Logical contents 10, 20, 30 laid out from index 0.
        let q = seeded(5, vec![10, 20, 30, 0, 0], 0, 3);
        assert_eq!(q.front(), Some(10));
        assert_eq!(q.back(), Some(30), "back is the most recently pushed item");
    }

    #[test]
    fn test_back_wrapped() {
        // head=3, size=5: logical order 1,2,3,4,5 stored wrapped.
        let q = seeded(5, vec![4, 5, 1, 2, 3], 3, 5);
        assert_eq!(q.front(), Some(2));
        assert_eq!(q.back(), Some(1), "back must not wrap round to the front");
    }

    #[test]
    fn test_pop_order() {
        let mut q = seeded(5, vec![10, 20, 30, 0, 0], 0, 3);
        assert_eq!(q.pop(), Some(10));
        assert_eq!(q.pop(), Some(20));
        assert_eq!(q.pop(), Some(30));
        assert_eq!(q.pop(), None);
        assert!(q.is_empty());
    }

    #[test]
    fn test_pop_wrapped() {
        let mut q = seeded(3, vec![30, 0, 20], 2, 2);
        assert_eq!(q.pop(), Some(20));
        assert_eq!(q.pop(), Some(30), "head wraps past the end of the store");
        assert_eq!(q.pop(), None);
    }

    #[test]
    fn test_wrap_cycle() {
        // Drive a full lap through push/pop so the store is reused in place.
        let mut q = CircularQueue::new(3);
        for value in [1, 2, 3] {
            assert_eq!(q.push(value), Some(true));
        }
        assert_eq!(q.front(), Some(1));
        assert_eq!(q.back(), Some(3));

        assert_eq!(q.pop(), Some(1));
        assert_eq!(q.pop(), Some(2));
        assert_eq!(q.push(4), Some(true));
        assert_eq!(q.push(5), Some(true));
        assert!(q.is_full());

        assert_eq!(q.front(), Some(3));
        assert_eq!(q.back(), Some(5));
        assert_eq!(q.pop(), Some(3));
        assert_eq!(q.pop(), Some(4));
        assert_eq!(q.pop(), Some(5));
        assert_eq!(q.pop(), None);
        assert!(q.is_empty());
    }
}
