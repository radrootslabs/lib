//! Validated connection limits for one service-owned SQLite database.

use core::{fmt, time::Duration};
use std::error::Error;

const MIN_BUSY_TIMEOUT: Duration = Duration::from_millis(1);
const DEFAULT_BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_BUSY_TIMEOUT: Duration = Duration::from_secs(60);
const MIN_CONNECTIONS: u32 = 1;
const DEFAULT_MAX_CONNECTIONS: u32 = 8;
const MAX_CONNECTIONS: u32 = 8;

/// Validated limits applied to every connection in a service SQLite pool.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ServiceSqliteConnectionOptions {
    busy_timeout: Duration,
    max_connections: u32,
}

impl ServiceSqliteConnectionOptions {
    /// Returns the reviewed service defaults: five seconds and eight connections.
    #[must_use]
    pub const fn reviewed() -> Self {
        Self {
            busy_timeout: DEFAULT_BUSY_TIMEOUT,
            max_connections: DEFAULT_MAX_CONNECTIONS,
        }
    }

    /// Validates an integral-millisecond busy timeout and bounded pool size.
    pub fn new(
        busy_timeout: Duration,
        max_connections: u32,
    ) -> Result<Self, ServiceSqliteConnectionOptionsError> {
        let timeout_nanos = busy_timeout.as_nanos();
        if !timeout_nanos.is_multiple_of(1_000_000) {
            return Err(ServiceSqliteConnectionOptionsError::BusyTimeoutNotMilliseconds);
        }
        if busy_timeout < MIN_BUSY_TIMEOUT {
            return Err(ServiceSqliteConnectionOptionsError::BusyTimeoutTooSmall);
        }
        if busy_timeout > MAX_BUSY_TIMEOUT {
            return Err(ServiceSqliteConnectionOptionsError::BusyTimeoutTooLarge);
        }
        if max_connections < MIN_CONNECTIONS {
            return Err(ServiceSqliteConnectionOptionsError::PoolTooSmall);
        }
        if max_connections > MAX_CONNECTIONS {
            return Err(ServiceSqliteConnectionOptionsError::PoolTooLarge);
        }
        Ok(Self {
            busy_timeout,
            max_connections,
        })
    }

    /// Returns the exact busy timeout applied to SQLite and pool acquisition.
    #[must_use]
    pub const fn busy_timeout(self) -> Duration {
        self.busy_timeout
    }

    /// Returns the maximum number of connections in the pool.
    #[must_use]
    pub const fn max_connections(self) -> u32 {
        self.max_connections
    }

    #[allow(
        dead_code,
        reason = "Step 056 keeps pragma verification private until the Step 061 host boundary"
    )]
    pub(crate) fn busy_timeout_milliseconds(self) -> i64 {
        i64::try_from(self.busy_timeout.as_millis())
            .expect("validated busy timeout always fits in i64 milliseconds")
    }
}

impl Default for ServiceSqliteConnectionOptions {
    fn default() -> Self {
        Self::reviewed()
    }
}

/// Path-free failure returned when SQLite connection limits are invalid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceSqliteConnectionOptionsError {
    BusyTimeoutTooSmall,
    BusyTimeoutTooLarge,
    BusyTimeoutNotMilliseconds,
    PoolTooSmall,
    PoolTooLarge,
}

impl fmt::Display for ServiceSqliteConnectionOptionsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::BusyTimeoutTooSmall => "SQLite busy timeout must be at least one millisecond",
            Self::BusyTimeoutTooLarge => "SQLite busy timeout must not exceed sixty seconds",
            Self::BusyTimeoutNotMilliseconds => {
                "SQLite busy timeout must be an integral number of milliseconds"
            }
            Self::PoolTooSmall => "SQLite connection pool must contain at least one connection",
            Self::PoolTooLarge => "SQLite connection pool must not exceed eight connections",
        })
    }
}

impl Error for ServiceSqliteConnectionOptionsError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reviewed_defaults_and_complete_boundary_inventory_are_exact() {
        let reviewed = ServiceSqliteConnectionOptions::reviewed();
        assert_eq!(reviewed.busy_timeout(), Duration::from_secs(5));
        assert_eq!(reviewed.max_connections(), 8);
        assert_eq!(ServiceSqliteConnectionOptions::default(), reviewed);

        for timeout in [
            Duration::from_millis(1),
            Duration::from_secs(5),
            Duration::from_secs(60),
        ] {
            for max_connections in 1..=8 {
                assert_eq!(
                    ServiceSqliteConnectionOptions::new(timeout, max_connections),
                    Ok(ServiceSqliteConnectionOptions {
                        busy_timeout: timeout,
                        max_connections,
                    })
                );
            }
        }
    }

    #[test]
    fn zero_fractional_oversized_and_extreme_values_fail_closed() {
        let vectors = [
            (
                Duration::ZERO,
                8,
                ServiceSqliteConnectionOptionsError::BusyTimeoutTooSmall,
            ),
            (
                Duration::from_nanos(1),
                8,
                ServiceSqliteConnectionOptionsError::BusyTimeoutNotMilliseconds,
            ),
            (
                Duration::from_micros(1_500),
                8,
                ServiceSqliteConnectionOptionsError::BusyTimeoutNotMilliseconds,
            ),
            (
                Duration::from_millis(60_001),
                8,
                ServiceSqliteConnectionOptionsError::BusyTimeoutTooLarge,
            ),
            (
                Duration::MAX,
                8,
                ServiceSqliteConnectionOptionsError::BusyTimeoutNotMilliseconds,
            ),
            (
                Duration::from_secs(5),
                0,
                ServiceSqliteConnectionOptionsError::PoolTooSmall,
            ),
            (
                Duration::from_secs(5),
                9,
                ServiceSqliteConnectionOptionsError::PoolTooLarge,
            ),
            (
                Duration::from_secs(5),
                u32::MAX,
                ServiceSqliteConnectionOptionsError::PoolTooLarge,
            ),
        ];

        for (timeout, max_connections, expected) in vectors {
            let error = ServiceSqliteConnectionOptions::new(timeout, max_connections)
                .expect_err("invalid options must fail");
            assert_eq!(error, expected);
            assert!(!error.to_string().contains(&max_connections.to_string()));
        }
    }
}
