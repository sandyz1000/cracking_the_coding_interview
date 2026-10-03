// # Tokio Threadpool Offloading & Backpressure Dispatcher
//
// ## Prompt:
// You run a Tokio async reactor receiving 10,000 network requests/second, but running an inference tensor 
// or image transform takes 5 ms of pure CPU time. Design the actor pipeline to route tasks without blocking 
// the async reactor.
//
// ## Key Focus:
// * Why calling compute-heavy code directly inside async tasks causes executor starvation.
// * Proper use of `tokio::task::spawn_blocking`, Rayon thread pools, or custom bounded MPSC worker queues.
// * Backpressure handling (dropping packets vs. rejecting vs. blocking sender)

// ## Solution
//
// Why not run the 5 ms of CPU work inside the async task: a Tokio worker thread only switches
// tasks at `.await` points. Compute that never awaits pins the thread for 5 ms, and at 10k
// requests/s a handful of workers are saturated, so every other task (timers, I/O, heartbeats)
// stalls behind it. `main` measures this: heartbeat lag is ~90 ms inline and ~2 ms via the pool.
//
// Pipeline: async handler -> admission (`Semaphore`) -> job queue -> N dedicated OS threads ->
// `oneshot` reply back to the awaiting handler.
//   * CPU work runs on its own fixed-size set of worker threads, so it can never starve the
//     reactor, and concurrency is capped at the core count instead of growing with load.
//   * Backpressure: a semaphore of `max_pending` permits (queued + running). `Policy::Reject`
//     fails fast with `Overloaded` (shed load, e.g. HTTP 503) and `Policy::Wait` awaits a free
//     permit, pushing backpressure to the caller without blocking any thread. Waiting is
//     cancel-safe, so a caller can bound it with `tokio::time::timeout`. Because admission is
//     bounded, the queue behind it never grows without limit.
//   * A job's permit is released when it finishes, before the result is sent back.
//   * A panicking job is caught: that request gets `Failed` and the worker thread survives.
//   * Not used: `spawn_blocking` has a large, shared pool with a hidden queue and no admission
//     control; Rayon is a fine alternative for data-parallel work but still needs the same
//     semaphore in front of it.

use std::future::Future;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};
use tokio::sync::{Semaphore, oneshot};

type Job = Box<dyn FnOnce() + Send>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Policy {
    Reject,
    Wait,
}

#[derive(Debug, PartialEq, Eq)]
enum Error {
    Overloaded,
    Failed,
}

// Admission is a semaphore of `max_pending` permits (queued + running), so the job queue
// itself can be unbounded without ever growing past that bound.
struct Pool {
    jobs: mpsc::Sender<Job>,
    admit: Arc<Semaphore>,
}

impl Pool {
    fn new(workers: usize, max_pending: usize) -> Self {
        assert!(workers > 0 && max_pending > 0);
        let (jobs, rx) = mpsc::channel::<Job>();
        // ponytail: workers share one Mutex<Receiver>; use an MPMC queue if dequeue contention shows up
        let rx = Arc::new(Mutex::new(rx));
        for _ in 0..workers {
            let rx = Arc::clone(&rx);
            std::thread::spawn(move || {
                loop {
                    let next = rx.lock().unwrap().recv();
                    let Ok(job) = next else { break };
                    // A panicking job must not take a worker thread down with it.
                    let _ = catch_unwind(AssertUnwindSafe(job));
                }
            });
        }
        Self {
            jobs,
            admit: Arc::new(Semaphore::new(max_pending)),
        }
    }

    async fn submit<F, R>(&self, policy: Policy, work: F) -> Result<R, Error>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        let admit = Arc::clone(&self.admit);
        let permit = match policy {
            Policy::Reject => admit.try_acquire_owned().map_err(|_| Error::Overloaded)?,
            Policy::Wait => admit.acquire_owned().await.map_err(|_| Error::Failed)?,
        };
        let (tx, rx) = oneshot::channel();
        self.jobs
            .send(Box::new(move || {
                let out = work();
                // Released before replying so a caller holding its result can resubmit at once.
                drop(permit);
                let _ = tx.send(out);
            }))
            .map_err(|_| Error::Failed)?;
        rx.await.map_err(|_| Error::Failed)
    }
}

fn burn(duration: Duration) {
    let start = Instant::now();
    while start.elapsed() < duration {
        std::hint::spin_loop();
    }
}

async fn heartbeat(stop: Arc<AtomicBool>) -> Duration {
    const PERIOD: Duration = Duration::from_millis(10);
    let mut worst = Duration::ZERO;
    while !stop.load(Ordering::Relaxed) {
        let start = Instant::now();
        tokio::time::sleep(PERIOD).await;
        worst = worst.max(start.elapsed().saturating_sub(PERIOD));
    }
    worst
}

async fn with_heartbeat(label: &str, work: impl Future<Output = ()>) {
    let stop = Arc::new(AtomicBool::new(false));
    let beat = tokio::spawn(heartbeat(Arc::clone(&stop)));
    let start = Instant::now();
    work.await;
    stop.store(true, Ordering::Relaxed);
    println!(
        "{label:<10} total {:>9.1?}   worst heartbeat lag {:>9.1?}",
        start.elapsed(),
        beat.await.unwrap()
    );
}

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() {
    const REQUESTS: usize = 40;
    const CPU: Duration = Duration::from_millis(5);

    with_heartbeat("inline", async {
        let tasks: Vec<_> = (0..REQUESTS)
            .map(|_| tokio::spawn(async { burn(CPU) }))
            .collect();
        for task in tasks {
            task.await.unwrap();
        }
    })
    .await;

    let pool = Arc::new(Pool::new(2, 8));
    with_heartbeat("pool/wait", async {
        let tasks: Vec<_> = (0..REQUESTS)
            .map(|_| {
                let pool = Arc::clone(&pool);
                tokio::spawn(async move { pool.submit(Policy::Wait, || burn(CPU)).await.unwrap() })
            })
            .collect();
        for task in tasks {
            task.await.unwrap();
        }
    })
    .await;

    let tasks: Vec<_> = (0..REQUESTS)
        .map(|_| {
            let pool = Arc::clone(&pool);
            tokio::spawn(async move { pool.submit(Policy::Reject, || burn(CPU)).await })
        })
        .collect();
    let mut accepted = 0;
    for task in tasks {
        accepted += usize::from(task.await.unwrap().is_ok());
    }
    println!(
        "pool/reject accepted {accepted}, rejected {} of {REQUESTS}",
        REQUESTS - accepted
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    type Gate = Arc<Mutex<mpsc::Receiver<()>>>;

    fn gate() -> (mpsc::Sender<()>, Gate) {
        let (tx, rx) = mpsc::channel();
        (tx, Arc::new(Mutex::new(rx)))
    }

    fn wait_at(gate: &Gate) -> impl FnOnce() -> u32 + Send + 'static {
        let gate = Arc::clone(gate);
        move || {
            gate.lock().unwrap().recv().unwrap();
            7
        }
    }

    async fn until_full(pool: &Pool) {
        while pool.admit.available_permits() > 0 {
            tokio::task::yield_now().await;
        }
    }

    #[tokio::test]
    async fn work_runs_off_the_reactor_and_returns_result() {
        let pool = Pool::new(2, 4);
        let result = pool
            .submit(Policy::Wait, || {
                (tokio::runtime::Handle::try_current().is_err(), 6 * 7)
            })
            .await;
        assert_eq!(result, Ok((true, 42)));
    }

    #[tokio::test]
    async fn reject_policy_sheds_when_full_and_recovers() {
        let pool = Arc::new(Pool::new(1, 2));
        let (release, gate) = gate();
        let held: Vec<_> = (0..2)
            .map(|_| {
                let (pool, work) = (Arc::clone(&pool), wait_at(&gate));
                tokio::spawn(async move { pool.submit(Policy::Wait, work).await })
            })
            .collect();
        until_full(&pool).await;

        assert_eq!(pool.submit(Policy::Reject, || 1).await, Err(Error::Overloaded));

        for _ in 0..2 {
            release.send(()).unwrap();
        }
        for task in held {
            assert_eq!(task.await.unwrap(), Ok(7));
        }
        assert_eq!(pool.admit.available_permits(), 2);
        assert_eq!(pool.submit(Policy::Reject, || 1).await, Ok(1));
    }

    #[tokio::test]
    async fn wait_policy_blocks_only_the_caller_and_is_cancel_safe() {
        let pool = Arc::new(Pool::new(1, 1));
        let (release, gate) = gate();
        let held = {
            let (pool, work) = (Arc::clone(&pool), wait_at(&gate));
            tokio::spawn(async move { pool.submit(Policy::Wait, work).await })
        };
        until_full(&pool).await;

        let timed_out = tokio::time::timeout(
            Duration::from_millis(20),
            pool.submit(Policy::Wait, || 1),
        )
        .await;
        assert!(timed_out.is_err(), "no capacity, so the caller waits");
        assert_eq!(pool.admit.available_permits(), 0, "giving up leaks no permit");

        release.send(()).unwrap();
        assert_eq!(held.await.unwrap(), Ok(7));
        assert_eq!(pool.submit(Policy::Wait, || 1).await, Ok(1));
    }

    #[tokio::test]
    async fn panicking_job_fails_alone_and_pool_survives() {
        let pool = Pool::new(1, 2);
        assert_eq!(
            pool.submit(Policy::Wait, || -> u32 { panic!("boom") }).await,
            Err(Error::Failed)
        );
        assert_eq!(pool.admit.available_permits(), 2);
        assert_eq!(pool.submit(Policy::Wait, || 5).await, Ok(5));
    }
}
