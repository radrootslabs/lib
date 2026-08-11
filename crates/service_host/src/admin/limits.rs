use core::{fmt, time::Duration};
use std::error::Error;

const MAX_HEADER_COUNT: u32 = 64;
const MAX_HEADER_BYTES: u32 = 32 * 1024;
const MAX_REQUEST_BODY_UTF8_BYTES: u32 = 65_536;
const MAX_RESPONSE_BODY_UTF8_BYTES: u32 = 1_048_576;
const MAX_CONCURRENT_CONNECTIONS: u32 = 64;
const MAX_REQUEST_DEADLINE: Duration = Duration::from_secs(30);
const MAX_IDLE_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_QUERY_ITEMS: u32 = 200;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdminTransportLimitField {
    HeaderCount,
    HeaderBytes,
    RequestBodyUtf8Bytes,
    ResponseBodyUtf8Bytes,
    ConcurrentConnections,
    RequestDeadline,
    IdleTimeout,
    QueryItems,
}

impl AdminTransportLimitField {
    /// Returns the hard maximum in items, bytes, or milliseconds as appropriate.
    #[must_use]
    pub const fn maximum(self) -> u64 {
        match self {
            Self::HeaderCount => MAX_HEADER_COUNT as u64,
            Self::HeaderBytes => MAX_HEADER_BYTES as u64,
            Self::RequestBodyUtf8Bytes => MAX_REQUEST_BODY_UTF8_BYTES as u64,
            Self::ResponseBodyUtf8Bytes => MAX_RESPONSE_BODY_UTF8_BYTES as u64,
            Self::ConcurrentConnections => MAX_CONCURRENT_CONNECTIONS as u64,
            Self::RequestDeadline => MAX_REQUEST_DEADLINE.as_millis() as u64,
            Self::IdleTimeout => MAX_IDLE_TIMEOUT.as_millis() as u64,
            Self::QueryItems => MAX_QUERY_ITEMS as u64,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdminTransportLimitsError {
    Zero { field: AdminTransportLimitField },
    ExceedsMaximum { field: AdminTransportLimitField },
}

impl fmt::Display for AdminTransportLimitsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("admin transport limit is outside its supported positive bounds")
    }
}

impl Error for AdminTransportLimitsError {}

/// Unvalidated values read from a service-owned configuration model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdminTransportLimitValues {
    pub header_count: u32,
    pub header_bytes: u32,
    pub request_body_utf8_bytes: u32,
    pub response_body_utf8_bytes: u32,
    pub concurrent_connections: u32,
    pub request_deadline: Duration,
    pub idle_timeout: Duration,
    pub query_items: u32,
}

/// Validated local-admin resource policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdminTransportLimits {
    values: AdminTransportLimitValues,
}

impl AdminTransportLimits {
    pub const DEFAULT: Self = Self {
        values: AdminTransportLimitValues {
            header_count: 32,
            header_bytes: 16 * 1024,
            request_body_utf8_bytes: MAX_REQUEST_BODY_UTF8_BYTES,
            response_body_utf8_bytes: MAX_RESPONSE_BODY_UTF8_BYTES,
            concurrent_connections: 32,
            request_deadline: Duration::from_secs(15),
            idle_timeout: Duration::from_secs(30),
            query_items: 100,
        },
    };

    pub fn new(values: AdminTransportLimitValues) -> Result<Self, AdminTransportLimitsError> {
        validate_u32(
            AdminTransportLimitField::HeaderCount,
            values.header_count,
            MAX_HEADER_COUNT,
        )?;
        validate_u32(
            AdminTransportLimitField::HeaderBytes,
            values.header_bytes,
            MAX_HEADER_BYTES,
        )?;
        validate_u32(
            AdminTransportLimitField::RequestBodyUtf8Bytes,
            values.request_body_utf8_bytes,
            MAX_REQUEST_BODY_UTF8_BYTES,
        )?;
        validate_u32(
            AdminTransportLimitField::ResponseBodyUtf8Bytes,
            values.response_body_utf8_bytes,
            MAX_RESPONSE_BODY_UTF8_BYTES,
        )?;
        validate_u32(
            AdminTransportLimitField::ConcurrentConnections,
            values.concurrent_connections,
            MAX_CONCURRENT_CONNECTIONS,
        )?;
        validate_duration(
            AdminTransportLimitField::RequestDeadline,
            values.request_deadline,
            MAX_REQUEST_DEADLINE,
        )?;
        validate_duration(
            AdminTransportLimitField::IdleTimeout,
            values.idle_timeout,
            MAX_IDLE_TIMEOUT,
        )?;
        validate_u32(
            AdminTransportLimitField::QueryItems,
            values.query_items,
            MAX_QUERY_ITEMS,
        )?;
        Ok(Self { values })
    }

    #[must_use]
    pub const fn values(self) -> AdminTransportLimitValues {
        self.values
    }

    #[must_use]
    pub const fn header_count(self) -> u32 {
        self.values.header_count
    }

    #[must_use]
    pub const fn header_bytes(self) -> u32 {
        self.values.header_bytes
    }

    #[must_use]
    pub const fn request_body_utf8_bytes(self) -> u32 {
        self.values.request_body_utf8_bytes
    }

    #[must_use]
    pub const fn response_body_utf8_bytes(self) -> u32 {
        self.values.response_body_utf8_bytes
    }

    #[must_use]
    pub const fn concurrent_connections(self) -> u32 {
        self.values.concurrent_connections
    }

    #[must_use]
    pub const fn request_deadline(self) -> Duration {
        self.values.request_deadline
    }

    #[must_use]
    pub const fn idle_timeout(self) -> Duration {
        self.values.idle_timeout
    }

    #[must_use]
    pub const fn query_items(self) -> u32 {
        self.values.query_items
    }
}

impl Default for AdminTransportLimits {
    fn default() -> Self {
        Self::DEFAULT
    }
}

fn validate_u32(
    field: AdminTransportLimitField,
    value: u32,
    maximum: u32,
) -> Result<(), AdminTransportLimitsError> {
    if value == 0 {
        Err(AdminTransportLimitsError::Zero { field })
    } else if value > maximum {
        Err(AdminTransportLimitsError::ExceedsMaximum { field })
    } else {
        Ok(())
    }
}

fn validate_duration(
    field: AdminTransportLimitField,
    value: Duration,
    maximum: Duration,
) -> Result<(), AdminTransportLimitsError> {
    if value.is_zero() {
        Err(AdminTransportLimitsError::Zero { field })
    } else if value > maximum {
        Err(AdminTransportLimitsError::ExceedsMaximum { field })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIELDS: [AdminTransportLimitField; 8] = [
        AdminTransportLimitField::HeaderCount,
        AdminTransportLimitField::HeaderBytes,
        AdminTransportLimitField::RequestBodyUtf8Bytes,
        AdminTransportLimitField::ResponseBodyUtf8Bytes,
        AdminTransportLimitField::ConcurrentConnections,
        AdminTransportLimitField::RequestDeadline,
        AdminTransportLimitField::IdleTimeout,
        AdminTransportLimitField::QueryItems,
    ];

    fn maximum_values() -> AdminTransportLimitValues {
        AdminTransportLimitValues {
            header_count: MAX_HEADER_COUNT,
            header_bytes: MAX_HEADER_BYTES,
            request_body_utf8_bytes: MAX_REQUEST_BODY_UTF8_BYTES,
            response_body_utf8_bytes: MAX_RESPONSE_BODY_UTF8_BYTES,
            concurrent_connections: MAX_CONCURRENT_CONNECTIONS,
            request_deadline: MAX_REQUEST_DEADLINE,
            idle_timeout: MAX_IDLE_TIMEOUT,
            query_items: MAX_QUERY_ITEMS,
        }
    }

    fn with_field(
        mut values: AdminTransportLimitValues,
        field: AdminTransportLimitField,
        value: u64,
    ) -> AdminTransportLimitValues {
        match field {
            AdminTransportLimitField::HeaderCount => values.header_count = value as u32,
            AdminTransportLimitField::HeaderBytes => values.header_bytes = value as u32,
            AdminTransportLimitField::RequestBodyUtf8Bytes => {
                values.request_body_utf8_bytes = value as u32;
            }
            AdminTransportLimitField::ResponseBodyUtf8Bytes => {
                values.response_body_utf8_bytes = value as u32;
            }
            AdminTransportLimitField::ConcurrentConnections => {
                values.concurrent_connections = value as u32;
            }
            AdminTransportLimitField::RequestDeadline => {
                values.request_deadline = Duration::from_millis(value);
            }
            AdminTransportLimitField::IdleTimeout => {
                values.idle_timeout = Duration::from_millis(value);
            }
            AdminTransportLimitField::QueryItems => values.query_items = value as u32,
        }
        values
    }

    #[test]
    fn exact_positive_boundaries_are_accepted_for_every_field() {
        assert_eq!(
            AdminTransportLimits::new(maximum_values())
                .unwrap()
                .values(),
            maximum_values()
        );
        for field in FIELDS {
            let minimum = with_field(maximum_values(), field, 1);
            assert!(AdminTransportLimits::new(minimum).is_ok(), "{field:?}");
        }
    }

    #[test]
    fn hard_maximum_inventory_is_exact() {
        assert_eq!(
            FIELDS.map(AdminTransportLimitField::maximum),
            [64, 32_768, 65_536, 1_048_576, 64, 30_000, 60_000, 200]
        );
    }

    #[test]
    fn zero_and_just_over_maximum_fail_for_every_field() {
        for field in FIELDS {
            assert_eq!(
                AdminTransportLimits::new(with_field(maximum_values(), field, 0)),
                Err(AdminTransportLimitsError::Zero { field })
            );
            assert_eq!(
                AdminTransportLimits::new(
                    with_field(maximum_values(), field, field.maximum() + 1,)
                ),
                Err(AdminTransportLimitsError::ExceedsMaximum { field })
            );
        }
    }

    #[test]
    fn extreme_duration_and_integer_inputs_fail_without_overflow() {
        let mut durations = maximum_values();
        durations.request_deadline = Duration::MAX;
        assert_eq!(
            AdminTransportLimits::new(durations),
            Err(AdminTransportLimitsError::ExceedsMaximum {
                field: AdminTransportLimitField::RequestDeadline,
            })
        );

        let mut integers = maximum_values();
        integers.response_body_utf8_bytes = u32::MAX;
        assert_eq!(
            AdminTransportLimits::new(integers),
            Err(AdminTransportLimitsError::ExceedsMaximum {
                field: AdminTransportLimitField::ResponseBodyUtf8Bytes,
            })
        );
    }

    #[test]
    fn defaults_are_stable_safe_and_within_hard_bounds() {
        let limits = AdminTransportLimits::default();
        assert_eq!(
            limits.values(),
            AdminTransportLimitValues {
                header_count: 32,
                header_bytes: 16 * 1024,
                request_body_utf8_bytes: 65_536,
                response_body_utf8_bytes: 1_048_576,
                concurrent_connections: 32,
                request_deadline: Duration::from_secs(15),
                idle_timeout: Duration::from_secs(30),
                query_items: 100,
            }
        );
        assert!(AdminTransportLimits::new(limits.values()).is_ok());
    }
}
