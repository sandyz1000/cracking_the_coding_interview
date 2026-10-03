use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

// Reuse the Part A types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateLimitStatus {
    Allowed { remaining: u64 },
    Limited { retry_after: Duration },
}

pub trait Clock: Send + Sync {
    fn now(&self) -> Instant;
}

#[derive(Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

#[derive(Debug)]
pub struct SlidingWindow {
    max_requests: u64,
    window: Duration,
    timestamps: VecDeque<Instant>,
}

impl SlidingWindow {
    pub fn new(max_requests: u64, window: Duration) -> Self {
        Self {
            max_requests,
            window,
            timestamps: VecDeque::with_capacity(max_requests as usize + 1),
        }
    }

    pub fn check(&mut self, now: Instant) -> RateLimitStatus {
        let window_start = now - self.window;

        while let Some(&oldest) = self.timestamps.front() {
            if oldest < window_start {
                self.timestamps.pop_front();
            } else {
                break;
            }
        }

        let current = self.timestamps.len() as u64;
        if current < self.max_requests {
            self.timestamps.push_back(now);
            RateLimitStatus::Allowed {
                remaining: self.max_requests - (current + 1),
            }
        } else {
            let oldest = *self.timestamps.front().unwrap();
            let expiry = oldest + self.window;
            let retry_after = expiry.saturating_duration_since(now);
            RateLimitStatus::Limited { retry_after }
        }
    }
}




#[derive(Debug)]
pub struct RequestContext<'a> {
    pub user_id: &'a str,
    pub experience: &'a str,
}

#[derive(Debug)]
pub enum MultiLimitDecision {
    Allowed,
    LimitedByGlobal(Duration),
    LimitedByUser { user_id: String, retry_after: Duration },
    LimitedByExperience { experience: String, retry_after: Duration },
}

pub struct MultiDimLimiter<C: Clock> {
    clock: C,
    window: Duration,

    // Limits
    global_limit: SlidingWindow,
    per_user_limit: u64,
    per_exp_limit: u64,

    // State
    per_user: HashMap<String, SlidingWindow>,
    per_experience: HashMap<String, SlidingWindow>,
}

impl<C: Clock> MultiDimLimiter<C> {
    pub fn new(
        clock: C,
        window: Duration,
        global_r: u64,
        per_user_u: u64,
        per_experience_x: u64,
    ) -> Self {
        Self {
            clock,
            window,
            global_limit: SlidingWindow::new(global_r, window),
            per_user_limit: per_user_u,
            per_exp_limit: per_experience_x,
            per_user: HashMap::new(),
            per_experience: HashMap::new(),
        }
    }

    fn get_or_create<'a>(
        map: &'a mut HashMap<String, SlidingWindow>,
        key: &str,
        max_requests: u64,
        window: Duration,
    ) -> &'a mut SlidingWindow {
        map.entry(key.to_string())
            .or_insert_with(|| SlidingWindow::new(max_requests, window))
    }

    pub fn check(&mut self, ctx: RequestContext<'_>) -> MultiLimitDecision {
        let now = self.clock.now();

        // 1) Global
        match self.global_limit.check(now) {
            RateLimitStatus::Allowed { .. } => { /* continue */ }
            RateLimitStatus::Limited { retry_after } => {
                return MultiLimitDecision::LimitedByGlobal(retry_after);
            }
        }

        // 2) Per user
        let user_limiter = Self::get_or_create(
            &mut self.per_user,
            ctx.user_id,
            self.per_user_limit,
            self.window,
        );

        match user_limiter.check(now) {
            RateLimitStatus::Allowed { .. } => { /* continue */ }
            RateLimitStatus::Limited { retry_after } => {
                return MultiLimitDecision::LimitedByUser {
                    user_id: ctx.user_id.to_string(),
                    retry_after,
                };
            }
        }

        // 3) Per experience
        let exp_limiter = Self::get_or_create(
            &mut self.per_experience,
            ctx.experience,
            self.per_exp_limit,
            self.window,
        );

        match exp_limiter.check(now) {
            RateLimitStatus::Allowed { .. } => MultiLimitDecision::Allowed,
            RateLimitStatus::Limited { retry_after } => {
                MultiLimitDecision::LimitedByExperience {
                    experience: ctx.experience.to_string(),
                    retry_after,
                }
            }
        }
    }
}
