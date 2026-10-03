// Asynchronous Multi-Stream Time-Window Aligner (Sensor Fusion)
//
// ## Prompt:
// Implement an asynchronous component that consumes two streams (`StreamA` and `StreamB`) with arbitrary 
// arrival times and emits matched pairs `(A, B)` whose timestamps fall within a configurable tolerance 
// window $\pm \Delta t$.
//
// ## Key Focus:
// * `tokio::select!` handling with cancellation safety.
// * Eviction policy for stale/unpaired frames to prevent unbounded heap growth.
// * Handling clock skew, jitter, and out-of-order deliveries.

// ## Solution
//
// `Aligner` is the synchronous core and `align` is the thin async shell around it.
//   * Matching: each stream has a buffer sorted by timestamp. An arriving frame takes the
//     nearest unmatched frame from the other buffer within `+-tolerance` (inclusive) and the
//     pair is emitted. Otherwise it is buffered. A frame is used at most once.
//   * Out-of-order delivery: buffers are kept sorted via binary-search insertion, so late
//     frames still land in the right place and can still match.
//   * Eviction: each stream tracks a watermark, the highest timestamp seen. A buffered frame is
//     dead once the other stream's watermark minus `max_lateness` has passed its match window,
//     because nothing arriving later can fall inside it. `max_lateness` is the jitter budget:
//     frames later than that are dropped rather than waited for.
//   * Memory bound: a stalled stream never advances its watermark, so `max_buffered` caps each
//     buffer and drops the oldest frames. Closing a stream drops the frames waiting on it.
//   * Clock skew: `skew_b` shifts B's timestamps onto A's clock on arrival.
//   * Async shell: `tokio::select!` over the two receivers. `mpsc::recv` is cancel-safe and all
//     state changes happen in the handlers, so a branch losing the race never loses a frame.
//     The output `send` is awaited outside the select, which gives backpressure from the
//     consumer without risking a half-sent pair. The loop ends when both inputs have closed.

use std::collections::VecDeque;
use std::time::Duration;
use tokio::sync::mpsc::{self, Receiver, Sender};

#[derive(Debug, Clone, PartialEq)]
struct Frame<T> {
    ts: i64,
    payload: T,
}

type Pair<A, B> = (Frame<A>, Frame<B>);

#[derive(Debug, Clone, Copy)]
struct Config {
    tolerance: i64,
    max_lateness: i64,
    skew_b: i64,
    max_buffered: usize,
}

struct Aligner<A, B> {
    cfg: Config,
    a: VecDeque<Frame<A>>,
    b: VecDeque<Frame<B>>,
    wm_a: i64,
    wm_b: i64,
}

fn take_nearest<T>(buf: &mut VecDeque<Frame<T>>, ts: i64, tolerance: i64) -> Option<Frame<T>> {
    let (lo, hi) = (ts.saturating_sub(tolerance), ts.saturating_add(tolerance));
    let start = buf.partition_point(|f| f.ts < lo);
    let best = (start..buf.len())
        .take_while(|&i| buf[i].ts <= hi)
        .min_by_key(|&i| buf[i].ts.abs_diff(ts))?;
    buf.remove(best)
}

fn insert_sorted<T>(buf: &mut VecDeque<Frame<T>>, frame: Frame<T>, cap: usize) {
    let at = buf.partition_point(|f| f.ts <= frame.ts);
    buf.insert(at, frame);
    if buf.len() > cap {
        buf.pop_front();
    }
}

fn drop_before<T>(buf: &mut VecDeque<Frame<T>>, cutoff: i64) {
    while buf.front().is_some_and(|f| f.ts < cutoff) {
        buf.pop_front();
    }
}

impl<A, B> Aligner<A, B> {
    fn new(cfg: Config) -> Self {
        Self {
            cfg,
            a: VecDeque::new(),
            b: VecDeque::new(),
            wm_a: i64::MIN,
            wm_b: i64::MIN,
        }
    }

    fn push_a(&mut self, frame: Frame<A>) -> Option<Pair<A, B>> {
        self.wm_a = self.wm_a.max(frame.ts);
        let pair = match take_nearest(&mut self.b, frame.ts, self.cfg.tolerance) {
            Some(b) => Some((frame, b)),
            None => {
                insert_sorted(&mut self.a, frame, self.cfg.max_buffered);
                None
            }
        };
        self.evict();
        pair
    }

    // B is moved onto A's clock here, so emitted B frames carry the corrected timestamp.
    fn push_b(&mut self, mut frame: Frame<B>) -> Option<Pair<A, B>> {
        frame.ts = frame.ts.saturating_add(self.cfg.skew_b);
        self.wm_b = self.wm_b.max(frame.ts);
        let pair = match take_nearest(&mut self.a, frame.ts, self.cfg.tolerance) {
            Some(a) => Some((a, frame)),
            None => {
                insert_sorted(&mut self.b, frame, self.cfg.max_buffered);
                None
            }
        };
        self.evict();
        pair
    }

    fn close_a(&mut self) {
        self.wm_a = i64::MAX;
        self.evict();
    }

    fn close_b(&mut self) {
        self.wm_b = i64::MAX;
        self.evict();
    }

    // A buffered frame is dead once the other stream's watermark, minus the lateness we are
    // willing to tolerate, has passed its match window. `max_buffered` is the backstop for a
    // stalled stream, whose watermark never advances and so never triggers this.
    fn evict(&mut self) {
        let slack = self.cfg.max_lateness.saturating_add(self.cfg.tolerance);
        drop_before(&mut self.a, self.wm_b.saturating_sub(slack));
        drop_before(&mut self.b, self.wm_a.saturating_sub(slack));
    }
}

async fn align<A, B>(
    cfg: Config,
    mut rx_a: Receiver<Frame<A>>,
    mut rx_b: Receiver<Frame<B>>,
    tx: Sender<Pair<A, B>>,
) {
    let mut aligner = Aligner::new(cfg);
    let (mut a_open, mut b_open) = (true, true);
    loop {
        // `recv` is cancel-safe and all state changes happen in the handlers, so losing the
        // race in `select!` never drops a frame. The send stays outside for the same reason.
        let pair = tokio::select! {
            frame = rx_a.recv(), if a_open => match frame {
                Some(frame) => aligner.push_a(frame),
                None => {
                    a_open = false;
                    aligner.close_a();
                    None
                }
            },
            frame = rx_b.recv(), if b_open => match frame {
                Some(frame) => aligner.push_b(frame),
                None => {
                    b_open = false;
                    aligner.close_b();
                    None
                }
            },
            else => break,
        };
        if let Some(pair) = pair
            && tx.send(pair).await.is_err()
        {
            break;
        }
    }
}

#[tokio::main]
async fn main() {
    let cfg = Config {
        tolerance: 5,
        max_lateness: 20,
        skew_b: 0,
        max_buffered: 1024,
    };
    let (tx_a, rx_a) = mpsc::channel(64);
    let (tx_b, rx_b) = mpsc::channel(64);
    let (tx_out, mut rx_out) = mpsc::channel(64);
    tokio::spawn(align(cfg, rx_a, rx_b, tx_out));

    tokio::spawn(async move {
        for i in 0..20i64 {
            tx_a.send(Frame { ts: i * 10, payload: i }).await.unwrap();
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    });
    tokio::spawn(async move {
        for i in 0..20i64 {
            let j = i ^ 1;
            tx_b.send(Frame { ts: j * 10 + 3, payload: j }).await.unwrap();
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    });

    while let Some((a, b)) = rx_out.recv().await {
        println!("A#{} @{}  <->  B#{} @{}", a.payload, a.ts, b.payload, b.ts);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> Config {
        Config {
            tolerance: 5,
            max_lateness: 10,
            skew_b: 0,
            max_buffered: 64,
        }
    }

    fn frame(ts: i64) -> Frame<i64> {
        Frame { ts, payload: ts }
    }

    #[test]
    fn matches_within_tolerance_inclusive() {
        let mut al = Aligner::new(cfg());
        assert!(al.push_a(frame(100)).is_none());
        let (a, b) = al.push_b(frame(105)).expect("edge of the window matches");
        assert_eq!((a.ts, b.ts), (100, 105));
        assert!(al.a.is_empty() && al.b.is_empty());
    }

    #[test]
    fn rejects_outside_tolerance() {
        let mut al = Aligner::new(cfg());
        al.push_a(frame(100));
        assert!(al.push_b(frame(106)).is_none());
        assert_eq!((al.a.len(), al.b.len()), (1, 1));
    }

    #[test]
    fn picks_nearest_and_pairs_one_to_one() {
        let mut al = Aligner::new(cfg());
        al.push_a(frame(96));
        al.push_a(frame(99));
        let (a, _) = al.push_b(frame(100)).unwrap();
        assert_eq!(a.ts, 99);
        let (a, _) = al.push_b(frame(100)).unwrap();
        assert_eq!(a.ts, 96, "each frame is used at most once");
        assert!(al.push_b(frame(100)).is_none());
    }

    #[test]
    fn out_of_order_arrivals_still_match() {
        let mut al = Aligner::new(cfg());
        al.push_b(frame(200));
        al.push_b(frame(100));
        al.push_b(frame(150));
        let (a, b) = al.push_a(frame(100)).unwrap();
        assert_eq!((a.ts, b.ts), (100, 100));
        let (a, b) = al.push_a(frame(151)).unwrap();
        assert_eq!((a.ts, b.ts), (151, 150));
    }

    #[test]
    fn frame_later_than_max_lateness_is_dropped() {
        let mut al = Aligner::new(cfg());
        al.push_b(frame(100));
        al.push_a(frame(151));
        assert!(al.push_a(frame(98)).is_none(), "100 was already evicted");
    }

    #[test]
    fn evicts_once_other_stream_moves_past_window() {
        let mut al = Aligner::new(cfg());
        al.push_a(frame(0));
        al.push_b(frame(15));
        assert_eq!(al.a.len(), 1, "within lateness + tolerance, still waiting");
        al.push_b(frame(16));
        assert_eq!(al.a.len(), 0, "0 < 16 - (10 + 5)");
    }

    #[test]
    fn stalled_stream_is_bounded_by_capacity() {
        let mut al: Aligner<i64, i64> = Aligner::new(Config {
            max_buffered: 16,
            ..cfg()
        });
        for ts in 0..1_000 {
            al.push_a(frame(ts));
        }
        assert_eq!(al.a.len(), 16);
        assert_eq!(al.a.front().unwrap().ts, 984, "oldest frames are dropped first");
    }

    #[test]
    fn skew_moves_b_onto_a_clock() {
        let mut al = Aligner::new(Config {
            skew_b: -1_000,
            ..cfg()
        });
        al.push_a(frame(0));
        let (a, b) = al.push_b(frame(1_002)).unwrap();
        assert_eq!((a.ts, b.ts), (0, 2));
    }

    #[test]
    fn closing_a_stream_releases_the_other_buffer() {
        let mut al: Aligner<i64, i64> = Aligner::new(cfg());
        al.push_b(frame(10));
        al.close_a();
        assert!(al.b.is_empty());
        assert!(al.push_b(frame(20)).is_none());
        assert!(al.b.is_empty(), "nothing left to match against");
    }

    #[tokio::test]
    async fn async_pipeline_pairs_interleaved_streams() {
        let (tx_a, rx_a) = mpsc::channel(8);
        let (tx_b, rx_b) = mpsc::channel(8);
        let (tx_out, mut rx_out) = mpsc::channel(8);
        // Arrival order across the two channels is nondeterministic, so give lateness room.
        let cfg = Config {
            max_lateness: 10_000,
            ..cfg()
        };
        let task = tokio::spawn(align(cfg, rx_a, rx_b, tx_out));

        for ts in [300, 100, 200] {
            tx_a.send(frame(ts)).await.unwrap();
        }
        for ts in [203, 101, 303, 999] {
            tx_b.send(frame(ts)).await.unwrap();
        }
        drop((tx_a, tx_b));

        let mut pairs = Vec::new();
        while let Some((a, b)) = rx_out.recv().await {
            pairs.push((a.ts, b.ts));
        }
        pairs.sort();
        assert_eq!(pairs, [(100, 101), (200, 203), (300, 303)]);
        task.await.unwrap();
    }
}
