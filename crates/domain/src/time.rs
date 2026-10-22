use crate::UnixSeconds;
use core::fmt;

/// Pinned IANA data identity shared with the retained calendar source.
pub const TIME_ZONE_DATABASE_VERSION: &str = "2026c";

/// Exact canonical IANA identifier, without ambient clock or conversion effects.
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct TimeZoneId(&'static str);
impl TimeZoneId {
    /// Admits a bounded exact name from the pinned dataset. Case, whitespace and
    /// unknown names are never normalized or replaced with a guessed zone.
    pub fn from_name(value: &str) -> Result<Self, TimeError> {
        if jiff_tzdb::VERSION != Some(TIME_ZONE_DATABASE_VERSION) {
            return Err(TimeError::UnsupportedDatabase);
        }
        if value.is_empty() || value.len() > 80 {
            return Err(TimeError::InvalidTimeZone);
        }
        let (canonical, _) = jiff_tzdb::get(value).ok_or(TimeError::InvalidTimeZone)?;
        if canonical != value {
            return Err(TimeError::InvalidTimeZone);
        }
        Ok(Self(canonical))
    }
    /// Returns the exact validated name, not a claim about an ambient zone.
    pub const fn name(self) -> &'static str {
        self.0
    }
}
impl fmt::Debug for TimeZoneId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("TimeZoneId(<redacted>)")
    }
}

/// Ordered absolute instants and an explicit display zone. No local wall-time
/// ambiguity, fold or offset is guessed; civil conversion is a separate job.
#[derive(Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Window {
    start: UnixSeconds,
    end: UnixSeconds,
    timezone: TimeZoneId,
}
impl Window {
    /// Requires a strictly later end, preserving the existing calendar rule.
    pub fn new(
        start: UnixSeconds,
        end: UnixSeconds,
        timezone: TimeZoneId,
    ) -> Result<Self, TimeError> {
        if end <= start {
            return Err(TimeError::InvalidWindow);
        }
        Ok(Self {
            start,
            end,
            timezone,
        })
    }
    /// Returns the exact absolute start without observing current time.
    pub const fn start(self) -> UnixSeconds {
        self.start
    }
    /// Returns the exact strictly later end.
    pub const fn end(self) -> UnixSeconds {
        self.end
    }
    /// Returns the explicit validated display-zone identity.
    pub const fn timezone(self) -> TimeZoneId {
        self.timezone
    }
}
impl fmt::Debug for Window {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Window(<redacted>)")
    }
}

/// Input-free shared time-value failure, with no ambient fallback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum TimeError {
    /// An unknown, noncanonical or excessive IANA name was supplied.
    InvalidTimeZone,
    /// End is not strictly after start.
    InvalidWindow,
    /// Compiled data does not match the explicitly pinned version.
    UnsupportedDatabase,
}
impl fmt::Display for TimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidTimeZone => "invalid time zone",
            Self::InvalidWindow => "invalid absolute window",
            Self::UnsupportedDatabase => "unsupported time zone database",
        })
    }
}
impl core::error::Error for TimeError {}
