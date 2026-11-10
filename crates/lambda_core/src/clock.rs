//! Injectable clocks for poll / cyclic schedulers.
//!
//! Production uses [`SystemClock`]. Tests use [`FakeClock`] and advance time
//! explicitly — never sleep the real 5 s / 60 s intervals.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Monotonic time source for scheduler due-times.
pub trait Clock {
    fn now(&self) -> Instant;
}

/// Wall-clock based monotonic clock (`Instant::now`).
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

/// Controllable clock for unit tests (manual ticks via [`FakeClock::advance`]).
///
/// Cheap to clone: clones share the same elapsed counter.
#[derive(Debug, Clone)]
pub struct FakeClock {
    origin: Instant,
    elapsed: Arc<Mutex<Duration>>,
}

impl FakeClock {
    pub fn new() -> Self {
        Self {
            origin: Instant::now(),
            elapsed: Arc::new(Mutex::new(Duration::ZERO)),
        }
    }

    /// Move fake time forward by `by` (does not sleep).
    pub fn advance(&self, by: Duration) {
        let mut elapsed = self.elapsed.lock().expect("fake clock lock");
        *elapsed = elapsed.checked_add(by).unwrap_or(Duration::MAX);
    }

    pub fn elapsed(&self) -> Duration {
        *self.elapsed.lock().expect("fake clock lock")
    }
}

impl Default for FakeClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for FakeClock {
    fn now(&self) -> Instant {
        self.origin + self.elapsed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_clock_advances_without_sleep() {
        let clock = FakeClock::new();
        let t0 = clock.now();
        clock.advance(Duration::from_millis(50));
        assert_eq!(clock.now().duration_since(t0), Duration::from_millis(50));
        clock.advance(Duration::from_secs(5));
        assert_eq!(clock.elapsed(), Duration::from_millis(5050));
    }

    #[test]
    fn fake_clock_clones_share_elapsed() {
        let a = FakeClock::new();
        let b = a.clone();
        a.advance(Duration::from_millis(10));
        assert_eq!(b.elapsed(), Duration::from_millis(10));
    }
}
