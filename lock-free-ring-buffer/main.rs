// # 1. Lock-Free Single-Producer Single-Consumer (SPSC) Ring Buffer
//
// ## Prompt:
// Implement a fixed-capacity, cache-line-aligned SPSC bounded ring buffer for streaming sensor
// measurements between an ingestion thread and an inference thread.
//
// ## Key Focus:
//   * Use atomic indices (`AtomicUsize`) with explicit `Ordering::Acquire`, `Ordering::Release`,
// and `Ordering::Relaxed`.
//   * Avoid false sharing using padding/alignment (`#[repr(align(64))]`).
//   * Ensure zero heap reallocations after construction.

// ## Solution
//
// Vyukov-style SPSC ring: two monotonically increasing indices, no shared size counter.
//   * `tail` is written only by the producer and `head` only by the consumer. Each side reads
//     the other's index but never writes it, so no CAS and no lock is needed.
//   * Capacity is a power of two: slot = index & mask (no division), and the occupied count is
//     `tail.wrapping_sub(head)`, so the indices may overflow harmlessly. All `capacity` slots
//     are usable (no sacrificed slot to tell full from empty).
//   * Orderings: the owner loads its own index Relaxed (nobody else writes it). The other
//     side's index is loaded Acquire and stored Release. Release on `tail` publishes the slot
//     write to the consumer; Release on `head` tells the producer the slot was read and may be
//     reused.
//   * `head` and `tail` live in separate `#[repr(align(64))]` wrappers so the two threads do not
//     fight over one cache line (false sharing). 64 is x86_64; Apple Silicon needs 128.
//   * The slots are a boxed slice allocated once in `new`: zero allocations afterwards. `Cell`
//     allows writing through `&self`; the one `unsafe impl Sync` rests on the head/tail
//     invariant that producer and consumer never touch the same slot at the same time.
//   * Contract: exactly one thread calls `en_queue` and exactly one calls `de_queue`. Handing
//     out Producer/Consumer handles would make the compiler enforce that.

use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

#[repr(align(64))]
struct Padded<T>(T);

struct ThreadedRingBuffer {
    mask: usize,
    store: Box<[Cell<i32>]>,
    head: Padded<AtomicUsize>,
    tail: Padded<AtomicUsize>,
}

// SAFETY: `head`/`tail` are monotonic indices whose gap is kept in `0..=capacity`, so the
// producer only ever writes slots outside `[head, tail)` and the consumer only reads inside it.
// Producer and consumer therefore never touch the same slot concurrently, and the Release/Acquire
// pairs on the two indices publish the non-atomic `store` writes between them.
unsafe impl Sync for ThreadedRingBuffer {}

impl ThreadedRingBuffer {
    fn new(k: u32) -> Self {
        assert!(
            k.is_power_of_two(),
            "capacity must be a non-zero power of two, got {k}"
        );
        let capacity = k as usize;
        Self {
            mask: capacity - 1,
            store: vec![Cell::new(0); capacity].into_boxed_slice(),
            head: Padded(AtomicUsize::new(0)),
            tail: Padded(AtomicUsize::new(0)),
        }
    }

    fn capacity(&self) -> usize {
        self.mask + 1
    }

    /// Producer thread only. Returns `false` when full; the value is dropped.
    fn en_queue(&self, value: i32) -> bool {
        let tail = self.tail.0.load(Ordering::Relaxed);
        // Acquire pairs with the consumer's Release store in `de_queue`, so a slot is only
        // reused after the consumer's read of it is visible.
        if tail.wrapping_sub(self.head.0.load(Ordering::Acquire)) >= self.capacity() {
            return false;
        }
        self.store[tail & self.mask].set(value);
        // Release publishes the slot write before the consumer can observe the new `tail`.
        self.tail.0.store(tail.wrapping_add(1), Ordering::Release);
        true
    }

    /// Consumer thread only. Returns `None` when empty.
    fn de_queue(&self) -> Option<i32> {
        let head = self.head.0.load(Ordering::Relaxed);
        // Acquire pairs with the producer's Release store in `en_queue`.
        if head == self.tail.0.load(Ordering::Acquire) {
            return None;
        }
        let value = self.store[head & self.mask].get();
        // Release publishes the slot read before the producer can observe the new `head`.
        self.head.0.store(head.wrapping_add(1), Ordering::Release);
        Some(value)
    }

    /// Advisory snapshot: exact only for the calling thread's own index.
    fn is_empty(&self) -> bool {
        self.head.0.load(Ordering::Acquire) == self.tail.0.load(Ordering::Acquire)
    }

    /// Advisory snapshot: exact only if no producer is running concurrently.
    fn is_full(&self) -> bool {
        let head = self.head.0.load(Ordering::Acquire);
        let tail = self.tail.0.load(Ordering::Acquire);
        tail.wrapping_sub(head) >= self.capacity()
    }
}

fn main() {
    const TOTAL: i32 = 1_000_000;
    let ring = ThreadedRingBuffer::new(1024);

    std::thread::scope(|s| {
        s.spawn(|| {
            for value in 0..TOTAL {
                while !ring.en_queue(value) {
                    std::hint::spin_loop();
                }
            }
        });
        s.spawn(|| {
            for expected in 0..TOTAL {
                loop {
                    if let Some(value) = ring.de_queue() {
                        assert_eq!(value, expected, "consumer observed a reordered value");
                        break;
                    }
                    std::hint::spin_loop();
                }
            }
        });
    });

    assert!(ring.is_empty());
    println!("streamed {TOTAL} measurements in order");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "power of two")]
    fn new_rejects_non_power_of_two() {
        ThreadedRingBuffer::new(3);
    }

    #[test]
    #[should_panic(expected = "power of two")]
    fn new_rejects_zero_capacity() {
        ThreadedRingBuffer::new(0);
    }

    #[test]
    fn new_empty() {
        let ring = ThreadedRingBuffer::new(4);
        assert!(ring.is_empty());
        assert!(!ring.is_full());
        assert_eq!(ring.de_queue(), None);
    }

    #[test]
    fn fills_to_capacity_then_rejects() {
        // All `capacity` slots are usable, unlike heapless which reserves one.
        let ring = ThreadedRingBuffer::new(4);
        for value in 0..4 {
            assert!(ring.en_queue(value));
        }
        assert!(ring.is_full());
        assert!(!ring.en_queue(4), "a full ring rejects the write");

        for value in 0..4 {
            assert_eq!(ring.de_queue(), Some(value));
        }
        assert!(ring.is_empty());
        assert_eq!(ring.de_queue(), None);
    }

    #[test]
    fn wrap_around_reuses_slots_in_order() {
        let ring = ThreadedRingBuffer::new(4);
        let mut expected = 0;
        for value in 0..20 {
            assert!(ring.en_queue(value));
            if value % 3 != 2 {
                continue;
            }
            for _ in 0..3 {
                assert_eq!(ring.de_queue(), Some(expected));
                expected += 1;
            }
        }
        while let Some(value) = ring.de_queue() {
            assert_eq!(value, expected);
            expected += 1;
        }
        assert_eq!(expected, 20);
    }

    #[test]
    fn concurrent_stream_is_fifo_and_lossless() {
        // Capacity 8 with 200k items hammers both the full and the empty path.
        const TOTAL: i32 = 200_000;
        let ring = ThreadedRingBuffer::new(8);
        let (produced, consumed) = std::thread::scope(|s| {
            let producer = s.spawn(|| {
                for value in 0..TOTAL {
                    while !ring.en_queue(value) {
                        std::hint::spin_loop();
                    }
                }
            });
            let consumer = s.spawn(|| {
                let mut expected = 0;
                while expected < TOTAL {
                    match ring.de_queue() {
                        Some(value) => {
                            assert_eq!(value, expected, "reordered value");
                            expected += 1;
                        }
                        None => std::hint::spin_loop(),
                    }
                }
            });
            (producer.join().is_ok(), consumer.join().is_ok())
        });
        assert!(produced && consumed);
        assert!(ring.is_empty(), "every enqueued value was consumed");
    }
}
