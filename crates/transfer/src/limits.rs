//! Separate stalled-frame, meaningful-progress and total-operation bounds.
use std::time::{Duration, Instant};

pub(crate) const FRAME_BUDGET: Duration = Duration::from_secs(30);
const IDLE_BUDGET: Duration = Duration::from_secs(90);
const DATA_BUDGET: Duration = Duration::from_secs(24 * 60 * 60);

pub(crate) struct Clock {
    hard: Instant,
    idle: Instant,
}
impl Clock {
    pub(crate) fn bounded(now: Instant, duration: Duration) -> Self {
        Self {
            hard: now + duration,
            idle: now + duration,
        }
    }
    pub(crate) fn data(now: Instant) -> Self {
        Self {
            hard: now + DATA_BUDGET,
            idle: now + IDLE_BUDGET,
        }
    }
    pub(crate) fn expired(&self, now: Instant) -> bool {
        now >= self.hard || now >= self.idle
    }
    pub(crate) fn frame_deadline(&self, now: Instant) -> Instant {
        self.hard.min(self.idle).min(now + FRAME_BUDGET)
    }
    pub(crate) fn progress(&mut self, now: Instant) {
        self.idle = self.hard.min(now + IDLE_BUDGET);
    }
}

pub(crate) struct ControlBudget {
    since: Instant,
    count: usize,
}
impl ControlBudget {
    pub(crate) fn new(now: Instant) -> Self {
        Self {
            since: now,
            count: 0,
        }
    }
    pub(crate) fn record(&mut self, now: Instant) -> bool {
        if now.duration_since(self.since) >= Duration::from_secs(60) {
            self.since = now;
            self.count = 0;
        }
        self.count += 1;
        self.count <= 8192
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn progress_allows_a_long_transfer_without_removing_frame_or_idle_bounds() {
        let start = Instant::now();
        let mut clock = Clock::data(start);
        for second in (30..=3600).step_by(30) {
            let now = start + Duration::from_secs(second);
            assert!(!clock.expired(now));
            clock.progress(now);
            assert!(clock.frame_deadline(now) <= now + FRAME_BUDGET);
        }
        assert!(clock.expired(start + Duration::from_secs(3690)));
        clock.progress(start + DATA_BUDGET);
        assert!(clock.expired(start + DATA_BUDGET));
    }
    #[test]
    fn a_control_flood_is_bounded_per_window_not_per_file_length() {
        let start = Instant::now();
        let mut budget = ControlBudget::new(start);
        for _ in 0..8192 {
            assert!(budget.record(start));
        }
        assert!(!budget.record(start));
        assert!(budget.record(start + Duration::from_secs(60)));
        let clock = Clock::data(start);
        // Control handling has no capability to renew the progress clock.
        assert!(clock.expired(start + IDLE_BUDGET));
    }
}
