//! Injected wall and monotonic clock contracts.

use core::fmt;
use std::{error::Error, time::Duration};

/// A nonnegative whole-second UTC timestamp relative to the Unix epoch.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnixTimeSeconds(u64);

impl UnixTimeSeconds {
    /// Creates an already validated Unix timestamp.
    #[must_use]
    pub const fn new(seconds: u64) -> Self {
        Self(seconds)
    }

    /// Returns whole seconds since the Unix epoch.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// A process-local monotonic observation relative to one clock origin.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct MonotonicTime(Duration);

impl MonotonicTime {
    /// Creates a monotonic observation from a clock-relative duration.
    #[must_use]
    pub const fn from_duration_since_origin(elapsed: Duration) -> Self {
        Self(elapsed)
    }

    /// Returns the clock-relative duration.
    #[must_use]
    pub const fn duration_since_origin(self) -> Duration {
        self.0
    }

    /// Computes a deadline without wrapping on duration overflow.
    pub fn checked_deadline_after(
        self,
        duration: Duration,
    ) -> Result<MonotonicDeadline, MonotonicClockError> {
        self.0
            .checked_add(duration)
            .map(|elapsed| MonotonicDeadline(Self(elapsed)))
            .ok_or(MonotonicClockError::DeadlineOverflow)
    }
}

/// A deadline in the same process-local clock domain as `MonotonicTime`.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct MonotonicDeadline(MonotonicTime);

impl MonotonicDeadline {
    /// Returns true when the supplied observation reaches or passes the deadline.
    #[must_use]
    pub fn is_reached_at(self, now: MonotonicTime) -> bool {
        now.0 >= self.0.0
    }

    /// Returns the deadline as a clock-relative observation.
    #[must_use]
    pub const fn time(self) -> MonotonicTime {
        self.0
    }
}

/// Wall-clock adapter failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WallClockError {
    BeforeUnixEpoch,
}

impl fmt::Display for WallClockError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BeforeUnixEpoch => formatter.write_str("wall clock is before the Unix epoch"),
        }
    }
}

impl Error for WallClockError {}

/// Monotonic clock arithmetic failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MonotonicClockError {
    DeadlineOverflow,
}

impl fmt::Display for MonotonicClockError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DeadlineOverflow => formatter.write_str("monotonic deadline overflows"),
        }
    }
}

impl Error for MonotonicClockError {}

/// Injected source of restart-stable wall UTC observations.
pub trait WallClock: Send + Sync {
    /// Returns the current whole-second UTC timestamp.
    fn now_utc(&self) -> Result<UnixTimeSeconds, WallClockError>;
}

/// Injected source of process-local monotonic observations.
pub trait MonotonicClock: Send + Sync {
    /// Returns the current observation in this clock's domain.
    fn now_monotonic(&self) -> MonotonicTime;

    /// Computes a deadline relative to the current observation.
    fn deadline_after(&self, duration: Duration) -> Result<MonotonicDeadline, MonotonicClockError> {
        self.now_monotonic().checked_deadline_after(duration)
    }
}

/// Production wall clock backed by `SystemTime`.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemWallClock;

impl WallClock for SystemWallClock {
    fn now_utc(&self) -> Result<UnixTimeSeconds, WallClockError> {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| UnixTimeSeconds::new(duration.as_secs()))
            .map_err(|_| WallClockError::BeforeUnixEpoch)
    }
}

/// Production monotonic clock with an instance-local origin.
#[derive(Clone, Copy, Debug)]
pub struct SystemMonotonicClock {
    origin: std::time::Instant,
}

impl SystemMonotonicClock {
    /// Captures a new private monotonic origin.
    #[must_use]
    pub fn new() -> Self {
        Self {
            origin: std::time::Instant::now(),
        }
    }
}

impl Default for SystemMonotonicClock {
    fn default() -> Self {
        Self::new()
    }
}

impl MonotonicClock for SystemMonotonicClock {
    fn now_monotonic(&self) -> MonotonicTime {
        MonotonicTime::from_duration_since_origin(self.origin.elapsed())
    }
}

#[cfg(test)]
mod tests {
    use core::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    struct FakeClock {
        wall_seconds: AtomicU64,
        monotonic_millis: AtomicU64,
    }

    impl FakeClock {
        fn new(wall_seconds: u64, monotonic_millis: u64) -> Self {
            Self {
                wall_seconds: AtomicU64::new(wall_seconds),
                monotonic_millis: AtomicU64::new(monotonic_millis),
            }
        }

        fn advance(&self, duration: Duration) {
            self.wall_seconds
                .fetch_add(duration.as_secs(), Ordering::Relaxed);
            let millis = u64::try_from(duration.as_millis()).expect("test duration fits u64");
            self.monotonic_millis.fetch_add(millis, Ordering::Relaxed);
        }
    }

    impl WallClock for FakeClock {
        fn now_utc(&self) -> Result<UnixTimeSeconds, WallClockError> {
            Ok(UnixTimeSeconds::new(
                self.wall_seconds.load(Ordering::Relaxed),
            ))
        }
    }

    impl MonotonicClock for FakeClock {
        fn now_monotonic(&self) -> MonotonicTime {
            MonotonicTime::from_duration_since_origin(Duration::from_millis(
                self.monotonic_millis.load(Ordering::Relaxed),
            ))
        }
    }

    #[test]
    fn fake_clocks_advance_without_hidden_system_reads() {
        let clock = FakeClock::new(1_000, 40);
        let deadline = clock
            .deadline_after(Duration::from_millis(25))
            .expect("deadline");

        assert_eq!(clock.now_utc().expect("wall time").get(), 1_000);
        assert!(!deadline.is_reached_at(clock.now_monotonic()));
        clock.advance(Duration::from_millis(25));
        assert!(deadline.is_reached_at(clock.now_monotonic()));
        assert_eq!(clock.now_utc().expect("wall time").get(), 1_000);
        clock.advance(Duration::from_secs(2));
        assert_eq!(clock.now_utc().expect("wall time").get(), 1_002);
    }

    #[test]
    fn deadline_comparison_is_inclusive_and_overflow_is_rejected() {
        let now = MonotonicTime::from_duration_since_origin(Duration::from_secs(5));
        let deadline = now
            .checked_deadline_after(Duration::from_secs(2))
            .expect("deadline");
        assert!(!deadline.is_reached_at(now));
        assert!(
            deadline.is_reached_at(MonotonicTime::from_duration_since_origin(
                Duration::from_secs(7)
            ))
        );
        assert!(
            deadline.is_reached_at(MonotonicTime::from_duration_since_origin(
                Duration::from_secs(8)
            ))
        );

        assert_eq!(
            MonotonicTime::from_duration_since_origin(Duration::MAX)
                .checked_deadline_after(Duration::from_nanos(1)),
            Err(MonotonicClockError::DeadlineOverflow)
        );
    }

    #[test]
    fn production_clock_adapters_smoke_test() {
        assert!(SystemWallClock.now_utc().expect("system wall time").get() > 0);

        let monotonic = SystemMonotonicClock::new();
        let first = monotonic.now_monotonic();
        let second = monotonic.now_monotonic();
        assert!(second >= first);
        assert!(monotonic.deadline_after(Duration::from_secs(1)).is_ok());

        let default_monotonic = SystemMonotonicClock::default();
        assert!(
            default_monotonic.now_monotonic().duration_since_origin() <= Duration::from_secs(1)
        );
        assert_eq!(
            WallClockError::BeforeUnixEpoch.to_string(),
            "wall clock is before the Unix epoch"
        );
        assert_eq!(
            MonotonicClockError::DeadlineOverflow.to_string(),
            "monotonic deadline overflows"
        );
    }
}
