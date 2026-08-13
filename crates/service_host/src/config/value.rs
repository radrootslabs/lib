//! Validated service-neutral configuration leaf values.

use core::fmt;
use core::str::FromStr;
use core::time::Duration;
use std::error::Error;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::operations::{
    OperationsBindPolicy, OperationsConfigError, OperationsListenAddress, OperationsListenerConfig,
    OperationsTransportLimits,
};

const NANOSECONDS_PER_MICROSECOND: u64 = 1_000;
const NANOSECONDS_PER_MILLISECOND: u64 = 1_000_000;
const NANOSECONDS_PER_SECOND: u64 = 1_000_000_000;
const NANOSECONDS_PER_MINUTE: u64 = 60 * NANOSECONDS_PER_SECOND;
const NANOSECONDS_PER_HOUR: u64 = 60 * NANOSECONDS_PER_MINUTE;
const NANOSECONDS_PER_DAY: u64 = 24 * NANOSECONDS_PER_HOUR;

const BYTES_PER_KIBIBYTE: u64 = 1024;
const BYTES_PER_MEBIBYTE: u64 = 1024 * BYTES_PER_KIBIBYTE;
const BYTES_PER_GIBIBYTE: u64 = 1024 * BYTES_PER_MEBIBYTE;
const BYTES_PER_TEBIBYTE: u64 = 1024 * BYTES_PER_GIBIBYTE;

/// A positive duration represented by at most `u64::MAX` nanoseconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PositiveDuration(Duration);

impl PositiveDuration {
    /// Validates a positive, canonically representable duration.
    pub fn new(value: Duration) -> Result<Self, PositiveDurationError> {
        if value.is_zero() {
            return Err(PositiveDurationError::Zero);
        }
        if value.as_nanos() > u128::from(u64::MAX) {
            return Err(PositiveDurationError::Overflow);
        }
        Ok(Self(value))
    }

    /// Returns the validated standard duration.
    #[must_use]
    pub const fn duration(self) -> Duration {
        self.0
    }

    /// Returns the exact total nanoseconds.
    #[must_use]
    pub fn nanoseconds(self) -> u64 {
        u64::try_from(self.0.as_nanos()).expect("validated positive duration fits u64 nanoseconds")
    }
}

impl FromStr for PositiveDuration {
    type Err = PositiveDurationError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let nanoseconds = parse_human_quantity(
            value,
            &[
                ("ms", NANOSECONDS_PER_MILLISECOND),
                ("us", NANOSECONDS_PER_MICROSECOND),
                ("ns", 1),
                ("d", NANOSECONDS_PER_DAY),
                ("h", NANOSECONDS_PER_HOUR),
                ("m", NANOSECONDS_PER_MINUTE),
                ("s", NANOSECONDS_PER_SECOND),
            ],
        )
        .map_err(PositiveDurationError::from_quantity)?;
        Self::new(Duration::from_nanos(nanoseconds))
    }
}

impl fmt::Display for PositiveDuration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        format_human_quantity(
            formatter,
            self.nanoseconds(),
            &[
                ("d", NANOSECONDS_PER_DAY),
                ("h", NANOSECONDS_PER_HOUR),
                ("m", NANOSECONDS_PER_MINUTE),
                ("s", NANOSECONDS_PER_SECOND),
                ("ms", NANOSECONDS_PER_MILLISECOND),
                ("us", NANOSECONDS_PER_MICROSECOND),
                ("ns", 1),
            ],
        )
    }
}

impl Serialize for PositiveDuration {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for PositiveDuration {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(D::Error::custom)
    }
}

/// Safe positive-duration parse failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PositiveDurationError {
    Invalid,
    Zero,
    Overflow,
}

impl PositiveDurationError {
    const fn from_quantity(error: HumanQuantityError) -> Self {
        match error {
            HumanQuantityError::Invalid => Self::Invalid,
            HumanQuantityError::Zero => Self::Zero,
            HumanQuantityError::Overflow => Self::Overflow,
        }
    }
}

impl fmt::Display for PositiveDurationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("positive duration is invalid")
    }
}

impl Error for PositiveDurationError {}

/// A positive exact byte quantity using canonical binary human units.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ByteLimit(u64);

impl ByteLimit {
    /// Validates a positive byte quantity.
    pub const fn new(bytes: u64) -> Result<Self, ByteLimitError> {
        if bytes == 0 {
            Err(ByteLimitError::Zero)
        } else {
            Ok(Self(bytes))
        }
    }

    /// Returns the exact byte quantity.
    #[must_use]
    pub const fn bytes(self) -> u64 {
        self.0
    }
}

impl FromStr for ByteLimit {
    type Err = ByteLimitError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let bytes = parse_human_quantity(
            value,
            &[
                ("KiB", BYTES_PER_KIBIBYTE),
                ("MiB", BYTES_PER_MEBIBYTE),
                ("GiB", BYTES_PER_GIBIBYTE),
                ("TiB", BYTES_PER_TEBIBYTE),
                ("B", 1),
            ],
        )
        .map_err(ByteLimitError::from_quantity)?;
        Self::new(bytes)
    }
}

impl fmt::Display for ByteLimit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        format_human_quantity(
            formatter,
            self.bytes(),
            &[
                ("TiB", BYTES_PER_TEBIBYTE),
                ("GiB", BYTES_PER_GIBIBYTE),
                ("MiB", BYTES_PER_MEBIBYTE),
                ("KiB", BYTES_PER_KIBIBYTE),
                ("B", 1),
            ],
        )
    }
}

impl Serialize for ByteLimit {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for ByteLimit {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(D::Error::custom)
    }
}

/// Safe byte-limit parse failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ByteLimitError {
    Invalid,
    Zero,
    Overflow,
}

impl ByteLimitError {
    const fn from_quantity(error: HumanQuantityError) -> Self {
        match error {
            HumanQuantityError::Invalid => Self::Invalid,
            HumanQuantityError::Zero => Self::Zero,
            HumanQuantityError::Overflow => Self::Overflow,
        }
    }
}

impl fmt::Display for ByteLimitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("byte limit is invalid")
    }
}

impl Error for ByteLimitError {}

/// A positive count bounded by its compile-time service-owned maximum.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct BoundedCount<const MAXIMUM: u32>(u32);

impl<const MAXIMUM: u32> BoundedCount<MAXIMUM> {
    /// Validates one positive count against the compile-time maximum.
    pub const fn new(value: u32) -> Result<Self, BoundedCountError> {
        if MAXIMUM == 0 {
            Err(BoundedCountError::InvalidMaximum)
        } else if value == 0 {
            Err(BoundedCountError::Zero)
        } else if value > MAXIMUM {
            Err(BoundedCountError::ExceedsMaximum)
        } else {
            Ok(Self(value))
        }
    }

    /// Returns the exact count.
    #[must_use]
    pub const fn value(self) -> u32 {
        self.0
    }

    /// Returns the compile-time maximum.
    #[must_use]
    pub const fn maximum() -> u32 {
        MAXIMUM
    }
}

impl<const MAXIMUM: u32> fmt::Display for BoundedCount<MAXIMUM> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl<const MAXIMUM: u32> Serialize for BoundedCount<MAXIMUM> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u32(self.0)
    }
}

impl<'de, const MAXIMUM: u32> Deserialize<'de> for BoundedCount<MAXIMUM> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u64::deserialize(deserializer)?;
        let value = u32::try_from(value)
            .map_err(|_| D::Error::custom(BoundedCountError::ExceedsMaximum))?;
        Self::new(value).map_err(D::Error::custom)
    }
}

/// Safe bounded-count validation failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoundedCountError {
    InvalidMaximum,
    Zero,
    ExceedsMaximum,
}

impl fmt::Display for BoundedCountError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("bounded count is outside its supported positive range")
    }
}

impl Error for BoundedCountError {}

/// Closed v1 logging encoding for safe structured service logs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LoggingFormat {
    #[default]
    Json,
}

impl FromStr for LoggingFormat {
    type Err = LoggingFormatError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "json" => Ok(Self::Json),
            _ => Err(LoggingFormatError::Unsupported),
        }
    }
}

impl fmt::Display for LoggingFormat {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Json => "json",
        })
    }
}

impl Serialize for LoggingFormat {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for LoggingFormat {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(D::Error::custom)
    }
}

/// Safe unsupported logging-format failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoggingFormatError {
    Unsupported,
}

impl fmt::Display for LoggingFormatError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("logging format is unsupported")
    }
}

impl Error for LoggingFormatError {}

/// Explicitly disabled or explicitly addressed operations binding.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OptionalOperationsBind {
    Disabled,
    Listen(OperationsListenAddress),
}

impl OptionalOperationsBind {
    /// Returns a disabled operations binding.
    #[must_use]
    pub const fn disabled() -> Self {
        Self::Disabled
    }

    /// Returns an explicitly addressed operations binding.
    #[must_use]
    pub const fn listen(address: OperationsListenAddress) -> Self {
        Self::Listen(address)
    }

    /// Returns whether an operations binding is explicitly enabled.
    #[must_use]
    pub const fn is_enabled(self) -> bool {
        matches!(self, Self::Listen(_))
    }

    /// Returns the selected address when enabled.
    #[must_use]
    pub const fn address(self) -> Option<OperationsListenAddress> {
        match self {
            Self::Disabled => None,
            Self::Listen(address) => Some(address),
        }
    }

    /// Applies the existing network-scope and transport-limit authority.
    pub fn into_listener_config(
        self,
        bind_policy: OperationsBindPolicy,
        limits: OperationsTransportLimits,
    ) -> Result<OperationsListenerConfig, OperationsConfigError> {
        match self {
            Self::Disabled => Ok(OperationsListenerConfig::disabled()),
            Self::Listen(address) => {
                OperationsListenerConfig::enabled(address, bind_policy, limits)
            }
        }
    }
}

impl Default for OptionalOperationsBind {
    fn default() -> Self {
        Self::disabled()
    }
}

impl FromStr for OptionalOperationsBind {
    type Err = OptionalOperationsBindError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value == "disabled" {
            return Ok(Self::Disabled);
        }
        value
            .parse()
            .map(Self::Listen)
            .map_err(|_| OptionalOperationsBindError::Invalid)
    }
}

impl fmt::Debug for OptionalOperationsBind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OptionalOperationsBind")
            .field("enabled", &self.is_enabled())
            .field("listen", &self.address().map(|_| "[redacted]"))
            .finish()
    }
}

impl fmt::Display for OptionalOperationsBind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disabled => formatter.write_str("disabled"),
            Self::Listen(address) => address.fmt(formatter),
        }
    }
}

impl Serialize for OptionalOperationsBind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for OptionalOperationsBind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(D::Error::custom)
    }
}

/// Safe optional operations-bind parse failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptionalOperationsBindError {
    Invalid,
}

impl fmt::Display for OptionalOperationsBindError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("optional operations bind is invalid")
    }
}

impl Error for OptionalOperationsBindError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HumanQuantityError {
    Invalid,
    Zero,
    Overflow,
}

fn parse_human_quantity(value: &str, units: &[(&str, u64)]) -> Result<u64, HumanQuantityError> {
    let Some((number, multiplier)) = units.iter().find_map(|(unit, multiplier)| {
        value.strip_suffix(unit).map(|number| (number, *multiplier))
    }) else {
        return Err(HumanQuantityError::Invalid);
    };
    if number.is_empty()
        || !number.bytes().all(|byte| byte.is_ascii_digit())
        || (number.len() > 1 && number.starts_with('0'))
    {
        return Err(HumanQuantityError::Invalid);
    }
    let quantity = number
        .parse::<u64>()
        .map_err(|_| HumanQuantityError::Overflow)?;
    if quantity == 0 {
        return Err(HumanQuantityError::Zero);
    }
    quantity
        .checked_mul(multiplier)
        .ok_or(HumanQuantityError::Overflow)
}

fn format_human_quantity(
    formatter: &mut fmt::Formatter<'_>,
    value: u64,
    units: &[(&str, u64)],
) -> fmt::Result {
    let (unit, multiplier) = units
        .iter()
        .find(|(_, multiplier)| value.is_multiple_of(*multiplier))
        .expect("unit inventory ends in multiplier one");
    write!(formatter, "{}{unit}", value / multiplier)
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
    #[serde(deny_unknown_fields)]
    struct Values {
        duration: PositiveDuration,
        bytes: ByteLimit,
        count: BoundedCount<64>,
        logging: LoggingFormat,
        operations_bind: OptionalOperationsBind,
    }

    #[test]
    fn human_duration_units_parse_and_display_canonically() {
        for (source, nanoseconds, display) in [
            ("1ns", 1, "1ns"),
            ("1us", 1_000, "1us"),
            ("1ms", 1_000_000, "1ms"),
            ("1500ms", 1_500_000_000, "1500ms"),
            ("1s", 1_000_000_000, "1s"),
            ("60s", 60_000_000_000, "1m"),
            ("1m", 60_000_000_000, "1m"),
            ("1h", 3_600_000_000_000, "1h"),
            ("1d", 86_400_000_000_000, "1d"),
        ] {
            let value: PositiveDuration = source.parse().unwrap();
            assert_eq!(value.nanoseconds(), nanoseconds);
            assert_eq!(value.to_string(), display);
            assert_eq!(value.duration().as_nanos(), u128::from(nanoseconds));
        }
    }

    #[test]
    fn duration_zero_invalid_and_overflow_inputs_fail_closed() {
        for source in ["", "1", "s", "0s", "01s", "+1s", "-1s", "1.5s", "1S", " 1s"] {
            assert!(source.parse::<PositiveDuration>().is_err(), "{source}");
        }
        assert_eq!(
            PositiveDuration::new(Duration::ZERO),
            Err(PositiveDurationError::Zero)
        );
        assert_eq!(
            "18446744073709551616ns"
                .parse::<PositiveDuration>()
                .unwrap_err(),
            PositiveDurationError::Overflow
        );
        assert_eq!(
            "18446744073709551615d"
                .parse::<PositiveDuration>()
                .unwrap_err(),
            PositiveDurationError::Overflow
        );
        assert_eq!(
            PositiveDuration::new(Duration::MAX).unwrap_err(),
            PositiveDurationError::Overflow
        );
        let maximum = PositiveDuration::new(Duration::from_nanos(u64::MAX)).unwrap();
        assert_eq!(maximum.nanoseconds(), u64::MAX);
        assert_eq!(maximum.to_string(), format!("{}ns", u64::MAX));
    }

    #[test]
    fn binary_byte_units_parse_and_display_canonically() {
        for (source, bytes, display) in [
            ("1B", 1, "1B"),
            ("1KiB", 1024, "1KiB"),
            ("1536B", 1536, "1536B"),
            ("1MiB", 1_048_576, "1MiB"),
            ("1GiB", 1_073_741_824, "1GiB"),
            ("1TiB", 1_099_511_627_776, "1TiB"),
        ] {
            let value: ByteLimit = source.parse().unwrap();
            assert_eq!(value.bytes(), bytes);
            assert_eq!(value.to_string(), display);
        }
    }

    #[test]
    fn byte_zero_invalid_and_overflow_inputs_fail_closed() {
        for source in [
            "", "1", "B", "0B", "01B", "+1B", "-1B", "1.5KiB", "1KB", " 1B",
        ] {
            assert!(source.parse::<ByteLimit>().is_err(), "{source}");
        }
        assert_eq!(ByteLimit::new(0), Err(ByteLimitError::Zero));
        assert_eq!(
            "18446744073709551616B".parse::<ByteLimit>().unwrap_err(),
            ByteLimitError::Overflow
        );
        assert_eq!(
            "18446744073709551615KiB".parse::<ByteLimit>().unwrap_err(),
            ByteLimitError::Overflow
        );
        let maximum = ByteLimit::new(u64::MAX).unwrap();
        assert_eq!(maximum.bytes(), u64::MAX);
        assert_eq!(maximum.to_string(), format!("{}B", u64::MAX));
    }

    #[test]
    fn bounded_count_rejects_zero_invalid_maximum_overflow_and_just_over() {
        assert_eq!(BoundedCount::<64>::new(1).unwrap().value(), 1);
        assert_eq!(BoundedCount::<64>::new(64).unwrap().value(), 64);
        assert_eq!(BoundedCount::<64>::maximum(), 64);
        assert_eq!(
            BoundedCount::<64>::new(0).unwrap_err(),
            BoundedCountError::Zero
        );
        assert_eq!(
            BoundedCount::<64>::new(65).unwrap_err(),
            BoundedCountError::ExceedsMaximum
        );
        assert_eq!(
            BoundedCount::<0>::new(1).unwrap_err(),
            BoundedCountError::InvalidMaximum
        );
        assert!(toml::from_str::<Values>(
            "duration='1s'\nbytes='1KiB'\ncount=4294967296\nlogging='json'\noperations_bind='disabled'",
        )
        .is_err());
    }

    #[test]
    fn serde_and_display_are_exact_for_every_common_leaf() {
        let source = concat!(
            "duration = \"2m\"\n",
            "bytes = \"8MiB\"\n",
            "count = 32\n",
            "logging = \"json\"\n",
            "operations_bind = \"127.0.0.1:9100\"\n",
        );
        let values: Values = toml::from_str(source).unwrap();
        assert_eq!(values.duration.to_string(), "2m");
        assert_eq!(values.bytes.to_string(), "8MiB");
        assert_eq!(values.count.to_string(), "32");
        assert_eq!(values.logging.to_string(), "json");
        assert_eq!(values.operations_bind.to_string(), "127.0.0.1:9100");
        assert_eq!(toml::to_string(&values).unwrap(), source);
    }

    #[test]
    fn logging_format_is_closed_to_structured_json() {
        assert_eq!(LoggingFormat::default(), LoggingFormat::Json);
        assert_eq!("json".parse(), Ok(LoggingFormat::Json));
        for source in ["", "JSON", "text", "pretty", "compact"] {
            assert_eq!(
                source.parse::<LoggingFormat>(),
                Err(LoggingFormatError::Unsupported)
            );
        }
    }

    #[test]
    fn optional_operations_bind_is_explicit_redacted_and_policy_checked() {
        let disabled: OptionalOperationsBind = "disabled".parse().unwrap();
        assert!(!disabled.is_enabled());
        assert_eq!(disabled.address(), None);
        assert_eq!(disabled.to_string(), "disabled");
        assert_eq!(
            disabled
                .into_listener_config(
                    OperationsBindPolicy::LoopbackOnly,
                    OperationsTransportLimits::DEFAULT,
                )
                .unwrap(),
            OperationsListenerConfig::disabled()
        );

        let loopback: OptionalOperationsBind = "127.0.0.1:9100".parse().unwrap();
        assert!(loopback.is_enabled());
        assert_eq!(loopback.to_string(), "127.0.0.1:9100");
        assert!(!format!("{loopback:?}").contains("127.0.0.1"));
        assert!(
            loopback
                .into_listener_config(
                    OperationsBindPolicy::LoopbackOnly,
                    OperationsTransportLimits::DEFAULT,
                )
                .is_ok()
        );

        let public: OptionalOperationsBind = "0.0.0.0:9100".parse().unwrap();
        assert!(
            public
                .into_listener_config(
                    OperationsBindPolicy::LoopbackOnly,
                    OperationsTransportLimits::DEFAULT,
                )
                .is_err()
        );
        assert!(
            public
                .into_listener_config(
                    OperationsBindPolicy::Public,
                    OperationsTransportLimits::DEFAULT,
                )
                .is_ok()
        );

        for source in ["", "Disabled", "127.0.0.1:0", "localhost:9100"] {
            assert_eq!(
                source.parse::<OptionalOperationsBind>(),
                Err(OptionalOperationsBindError::Invalid)
            );
        }
    }
}
