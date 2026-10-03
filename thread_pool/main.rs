//! A thread pool whose `spawn` returns a handle carrying the task's result.
//!
//! Two behaviours beyond the minimum: a panicking task is caught by the worker
//! and re-raised on whoever joins it, and dropping the pool drains the queue
//! before the workers exit.

use std::any::Any;
use std::panic::{self, AssertUnwindSafe};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// The queue holds erased closures: every task has its own return type, but the
/// result channel is captured inside the closure, so `T` never reaches here.
type Job = Box<dyn FnOnce() + Send + 'static>;

type Panic = Box<dyn Any + Send + 'static>;

pub struct Task<T> {
    outcome: Receiver<thread::Result<T>>,
}

impl<T> Task<T> {
    /// Blocks until the task finishes, re-raising its panic on this thread.
    pub fn join(self) -> T {
        match self.try_join() {
            Ok(value) => value,
            Err(payload) => panic::resume_unwind(payload),
        }
    }

    /// Blocks until the task finishes, handing back the panic instead.
    pub fn try_join(self) -> thread::Result<T> {
        self.outcome
            .recv()
            .unwrap_or_else(|_| Err(Box::new("worker died before finishing the task") as Panic))
    }
}

pub struct ThreadPool {
    /// `None` only while `Drop` is closing the queue.
    sender: Option<Sender<Job>>,
    workers: Vec<JoinHandle<()>>,
}

impl ThreadPool {
    pub fn new(size: usize) -> Self {
        assert!(size > 0, "a thread pool needs at least one worker");
        let (sender, receiver) = channel::<Job>();
        // Receiver is Send but not Sync, so the workers share one behind a Mutex.
        let receiver = Arc::new(Mutex::new(receiver));
        let workers = (0..size)
            .map(|_| {
                let receiver = Arc::clone(&receiver);
                thread::spawn(move || run_worker(&receiver))
            })
            .collect();
        Self {
            sender: Some(sender),
            workers,
        }
    }

    pub fn spawn<F, T>(&self, task: F) -> Task<T>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let (done, outcome) = channel();
        let job: Job = Box::new(move || {
            let result = panic::catch_unwind(AssertUnwindSafe(task));
            // The caller may have dropped its Task; that is not an error.
            let _ = done.send(result);
        });
        if let Some(sender) = &self.sender {
            let _ = sender.send(job);
        }
        Task { outcome }
    }
}

fn run_worker(receiver: &Mutex<Receiver<Job>>) {
    loop {
        // The guard is scoped to this statement. Holding it across `job()`
        // would let one worker monopolise the queue and serialise the pool.
        let job = receiver
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .recv();
        match job {
            Ok(job) => job(),
            Err(_) => break,
        }
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        // Closing the queue lets each worker drain what is already queued and
        // then see the disconnect. Joining before this would deadlock.
        drop(self.sender.take());
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

fn main() {
    let pool = ThreadPool::new(10);
    let started = Instant::now();
    let tasks: Vec<_> = (0..10)
        .map(|i| {
            pool.spawn(move || {
                std::thread::sleep(Duration::from_millis(200));
                i * i
            })
        })
        .collect();
    for (i, task) in tasks.into_iter().enumerate() {
        println!("task {i} => {}", task.join());
    }
    println!("finished in {:?}", started.elapsed());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_runs_parallel() {
        let pool = ThreadPool::new(10);
        let started = Instant::now();
        let tasks: Vec<_> = (0..10)
            .map(|i| {
                pool.spawn(move || {
                    thread::sleep(Duration::from_millis(200));
                    i * i
                })
            })
            .collect();
        let results: Vec<i32> = tasks.into_iter().map(Task::join).collect();
        assert_eq!(results, vec![0, 1, 4, 9, 16, 25, 36, 49, 64, 81]);
        assert!(
            started.elapsed() < Duration::from_millis(600),
            "10 threads should overlap, took {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn test_panic_reported() {
        let pool = ThreadPool::new(2);
        let task = pool.spawn(|| -> i32 { panic!("task blew up") });
        let payload = task.try_join().expect_err("the panic must surface");
        assert_eq!(payload.downcast_ref::<&str>(), Some(&"task blew up"));
    }

    #[test]
    #[should_panic(expected = "task blew up")]
    fn test_panic_reraised() {
        let pool = ThreadPool::new(2);
        pool.spawn(|| -> i32 { panic!("task blew up") }).join();
    }

    #[test]
    fn test_worker_survives() {
        let pool = ThreadPool::new(1);
        assert!(pool.spawn(|| -> i32 { panic!("boom") }).try_join().is_err());
        // The single worker must still be alive to run the next task.
        assert_eq!(pool.spawn(|| 7).join(), 7);
    }

    #[test]
    fn test_drop_drains() {
        let ran = Arc::new(AtomicUsize::new(0));
        {
            let pool = ThreadPool::new(2);
            for _ in 0..20 {
                let ran = Arc::clone(&ran);
                pool.spawn(move || {
                    thread::sleep(Duration::from_millis(10));
                    ran.fetch_add(1, Ordering::SeqCst);
                });
            }
        }
        // Drop returned only after every queued task finished.
        assert_eq!(ran.load(Ordering::SeqCst), 20);
    }

    #[test]
    fn test_dropped_handle() {
        let pool = ThreadPool::new(2);
        let ran = Arc::new(AtomicUsize::new(0));
        let inner = Arc::clone(&ran);
        drop(pool.spawn(move || inner.fetch_add(1, Ordering::SeqCst)));
        drop(pool);
        assert_eq!(ran.load(Ordering::SeqCst), 1);
    }
}
