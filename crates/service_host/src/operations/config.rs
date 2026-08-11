//! Pure configuration for the optional TCP operations listener.

use core::{fmt, str::FromStr, time::Duration};
use std::error::Error;
use std::net::SocketAddr;

use serde::de::Error as _;
use serde::ser::{Error as _, SerializeStruct};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

const MAX_HEADER_COUNT: u32 = 64;
const MAX_HEADER_BYTES: u32 = 32 * 1024;
const MAX_RESPONSE_BODY_UTF8_BYTES: u32 = 1_048_576;
const MAX_CONCURRENT_CONNECTIONS: u32 = 64;
const MAX_REQUEST_DEADLINE: Duration = Duration::from_secs(30);
const MAX_IDLE_TIMEOUT: Duration = Duration::from_secs(60);

/// Network scope explicitly authorized for the operations listener.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OperationsBindPolicy {
    /// Only an IPv4 or IPv6 loopback address may be selected.
    #[default]
    LoopbackOnly,
    /// A non-loopback, wildcard, or loopback address may be selected.
    Public,
}

/// A parsed TCP socket address with an explicit nonzero port.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OperationsListenAddress(SocketAddr);

impl OperationsListenAddress {
    pub fn new(address: SocketAddr) -> Result<Self, OperationsListenAddressError> {
        if address.port() == 0 {
            Err(OperationsListenAddressError::PortZero)
        } else {
            Ok(Self(address))
        }
    }

    #[must_use]
    pub const fn socket_addr(self) -> SocketAddr {
        self.0
    }

    #[must_use]
    pub const fn is_loopback(self) -> bool {
        self.0.ip().is_loopback()
    }
}

impl FromStr for OperationsListenAddress {
    type Err = OperationsListenAddressError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let address = value
            .parse::<SocketAddr>()
            .map_err(|_| OperationsListenAddressError::Invalid)?;
        Self::new(address)
    }
}

impl fmt::Display for OperationsListenAddress {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl Serialize for OperationsListenAddress {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

/// A safe parse failure for an operations listen address.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperationsListenAddressError {
    Invalid,
    PortZero,
}

impl fmt::Display for OperationsListenAddressError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("operations listen address is invalid")
    }
}

impl Error for OperationsListenAddressError {}

/// One bounded operations transport setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperationsTransportLimitField {
    HeaderCount,
    HeaderBytes,
    ResponseBodyUtf8Bytes,
    ConcurrentConnections,
    RequestDeadline,
    IdleTimeout,
}

impl OperationsTransportLimitField {
    /// Returns the hard maximum in items, bytes, or milliseconds as appropriate.
    #[must_use]
    pub const fn maximum(self) -> u64 {
        match self {
            Self::HeaderCount => MAX_HEADER_COUNT as u64,
            Self::HeaderBytes => MAX_HEADER_BYTES as u64,
            Self::ResponseBodyUtf8Bytes => MAX_RESPONSE_BODY_UTF8_BYTES as u64,
            Self::ConcurrentConnections => MAX_CONCURRENT_CONNECTIONS as u64,
            Self::RequestDeadline => MAX_REQUEST_DEADLINE.as_millis() as u64,
            Self::IdleTimeout => MAX_IDLE_TIMEOUT.as_millis() as u64,
        }
    }
}

/// A safe validation failure for operations transport limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperationsTransportLimitsError {
    Zero {
        field: OperationsTransportLimitField,
    },
    ExceedsMaximum {
        field: OperationsTransportLimitField,
    },
}

impl fmt::Display for OperationsTransportLimitsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("operations transport limit is outside its supported positive bounds")
    }
}

impl Error for OperationsTransportLimitsError {}

/// Unvalidated values read from a service-owned configuration model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OperationsTransportLimitValues {
    pub header_count: u32,
    pub header_bytes: u32,
    pub response_body_utf8_bytes: u32,
    pub concurrent_connections: u32,
    pub request_deadline: Duration,
    pub idle_timeout: Duration,
}

/// Validated resource policy for the cached operations listener.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OperationsTransportLimits {
    values: OperationsTransportLimitValues,
}

impl OperationsTransportLimits {
    pub const DEFAULT: Self = Self {
        values: OperationsTransportLimitValues {
            header_count: 32,
            header_bytes: 16 * 1024,
            response_body_utf8_bytes: MAX_RESPONSE_BODY_UTF8_BYTES,
            concurrent_connections: 32,
            request_deadline: Duration::from_secs(15),
            idle_timeout: Duration::from_secs(30),
        },
    };

    pub fn new(
        values: OperationsTransportLimitValues,
    ) -> Result<Self, OperationsTransportLimitsError> {
        validate_u32(
            OperationsTransportLimitField::HeaderCount,
            values.header_count,
            MAX_HEADER_COUNT,
        )?;
        validate_u32(
            OperationsTransportLimitField::HeaderBytes,
            values.header_bytes,
            MAX_HEADER_BYTES,
        )?;
        validate_u32(
            OperationsTransportLimitField::ResponseBodyUtf8Bytes,
            values.response_body_utf8_bytes,
            MAX_RESPONSE_BODY_UTF8_BYTES,
        )?;
        validate_u32(
            OperationsTransportLimitField::ConcurrentConnections,
            values.concurrent_connections,
            MAX_CONCURRENT_CONNECTIONS,
        )?;
        validate_duration(
            OperationsTransportLimitField::RequestDeadline,
            values.request_deadline,
            MAX_REQUEST_DEADLINE,
        )?;
        validate_duration(
            OperationsTransportLimitField::IdleTimeout,
            values.idle_timeout,
            MAX_IDLE_TIMEOUT,
        )?;
        Ok(Self { values })
    }

    #[must_use]
    pub const fn values(self) -> OperationsTransportLimitValues {
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
}

impl Default for OperationsTransportLimits {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl Serialize for OperationsTransportLimits {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("OperationsTransportLimits", 6)?;
        state.serialize_field("header_count", &self.header_count())?;
        state.serialize_field("header_bytes", &self.header_bytes())?;
        state.serialize_field("response_body_utf8_bytes", &self.response_body_utf8_bytes())?;
        state.serialize_field("concurrent_connections", &self.concurrent_connections())?;
        let request_deadline_ms =
            u64::try_from(self.request_deadline().as_millis()).map_err(S::Error::custom)?;
        let idle_timeout_ms =
            u64::try_from(self.idle_timeout().as_millis()).map_err(S::Error::custom)?;
        state.serialize_field("request_deadline_ms", &request_deadline_ms)?;
        state.serialize_field("idle_timeout_ms", &idle_timeout_ms)?;
        state.end()
    }
}

/// A field that is forbidden when the operations listener is disabled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperationsConfigField {
    Listen,
    BindPolicy,
    Limits,
}

/// A safe semantic configuration failure for the operations listener.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperationsConfigError {
    MissingListen,
    DisabledField { field: OperationsConfigField },
    Listen(OperationsListenAddressError),
    PublicBindRequiresPolicy,
    Limits(OperationsTransportLimitsError),
}

impl fmt::Display for OperationsConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("operations listener configuration is invalid")
    }
}

impl Error for OperationsConfigError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Listen(error) => Some(error),
            Self::Limits(error) => Some(error),
            Self::MissingListen | Self::DisabledField { .. } | Self::PublicBindRequiresPolicy => {
                None
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OperationsListenerState {
    Disabled,
    Enabled {
        listen: OperationsListenAddress,
        bind_policy: OperationsBindPolicy,
        limits: OperationsTransportLimits,
    },
}

/// Pure, validated configuration for the optional cached operations listener.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct OperationsListenerConfig {
    state: OperationsListenerState,
}

impl OperationsListenerConfig {
    #[must_use]
    pub const fn disabled() -> Self {
        Self {
            state: OperationsListenerState::Disabled,
        }
    }

    pub fn enabled(
        listen: OperationsListenAddress,
        bind_policy: OperationsBindPolicy,
        limits: OperationsTransportLimits,
    ) -> Result<Self, OperationsConfigError> {
        if !listen.is_loopback() && bind_policy != OperationsBindPolicy::Public {
            return Err(OperationsConfigError::PublicBindRequiresPolicy);
        }
        Ok(Self {
            state: OperationsListenerState::Enabled {
                listen,
                bind_policy,
                limits,
            },
        })
    }

    #[must_use]
    pub const fn is_enabled(self) -> bool {
        matches!(self.state, OperationsListenerState::Enabled { .. })
    }

    #[must_use]
    pub const fn listen(self) -> Option<OperationsListenAddress> {
        match self.state {
            OperationsListenerState::Disabled => None,
            OperationsListenerState::Enabled { listen, .. } => Some(listen),
        }
    }

    #[must_use]
    pub const fn bind_policy(self) -> Option<OperationsBindPolicy> {
        match self.state {
            OperationsListenerState::Disabled => None,
            OperationsListenerState::Enabled { bind_policy, .. } => Some(bind_policy),
        }
    }

    #[must_use]
    pub const fn limits(self) -> Option<OperationsTransportLimits> {
        match self.state {
            OperationsListenerState::Disabled => None,
            OperationsListenerState::Enabled { limits, .. } => Some(limits),
        }
    }
}

impl Default for OperationsListenerConfig {
    fn default() -> Self {
        Self::disabled()
    }
}

impl fmt::Debug for OperationsListenerConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OperationsListenerConfig")
            .field("enabled", &self.is_enabled())
            .field("listen", &self.listen().map(|_| "[redacted]"))
            .field("bind_policy", &self.bind_policy())
            .field("limits", &self.limits())
            .finish()
    }
}

impl<'de> Deserialize<'de> for OperationsListenerConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = OperationsListenerWire::deserialize(deserializer)?;
        if !wire.enabled {
            if wire.listen.is_some() {
                return Err(D::Error::custom(OperationsConfigError::DisabledField {
                    field: OperationsConfigField::Listen,
                }));
            }
            if wire.bind_policy.is_some() {
                return Err(D::Error::custom(OperationsConfigError::DisabledField {
                    field: OperationsConfigField::BindPolicy,
                }));
            }
            if wire.limits.is_some() {
                return Err(D::Error::custom(OperationsConfigError::DisabledField {
                    field: OperationsConfigField::Limits,
                }));
            }
            return Ok(Self::disabled());
        }

        let listen = wire
            .listen
            .ok_or_else(|| D::Error::custom(OperationsConfigError::MissingListen))?
            .parse()
            .map_err(|error| D::Error::custom(OperationsConfigError::Listen(error)))?;
        let limits = wire
            .limits
            .map(OperationsTransportLimitWire::validate)
            .transpose()
            .map_err(|error| D::Error::custom(OperationsConfigError::Limits(error)))?
            .unwrap_or_default();
        Self::enabled(listen, wire.bind_policy.unwrap_or_default(), limits)
            .map_err(D::Error::custom)
    }
}

impl Serialize for OperationsListenerConfig {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.state {
            OperationsListenerState::Disabled => {
                let mut state = serializer.serialize_struct("OperationsListenerConfig", 1)?;
                state.serialize_field("enabled", &false)?;
                state.end()
            }
            OperationsListenerState::Enabled {
                listen,
                bind_policy,
                limits,
            } => {
                let mut state = serializer.serialize_struct("OperationsListenerConfig", 4)?;
                state.serialize_field("enabled", &true)?;
                state.serialize_field("listen", &listen)?;
                state.serialize_field("bind_policy", &bind_policy)?;
                state.serialize_field("limits", &limits)?;
                state.end()
            }
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OperationsListenerWire {
    enabled: bool,
    #[serde(default)]
    listen: Option<String>,
    #[serde(default)]
    bind_policy: Option<OperationsBindPolicy>,
    #[serde(default)]
    limits: Option<OperationsTransportLimitWire>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct OperationsTransportLimitWire {
    header_count: u32,
    header_bytes: u32,
    response_body_utf8_bytes: u32,
    concurrent_connections: u32,
    request_deadline_ms: u64,
    idle_timeout_ms: u64,
}

impl OperationsTransportLimitWire {
    fn validate(self) -> Result<OperationsTransportLimits, OperationsTransportLimitsError> {
        OperationsTransportLimits::new(OperationsTransportLimitValues {
            header_count: self.header_count,
            header_bytes: self.header_bytes,
            response_body_utf8_bytes: self.response_body_utf8_bytes,
            concurrent_connections: self.concurrent_connections,
            request_deadline: Duration::from_millis(self.request_deadline_ms),
            idle_timeout: Duration::from_millis(self.idle_timeout_ms),
        })
    }
}

impl Default for OperationsTransportLimitWire {
    fn default() -> Self {
        let values = OperationsTransportLimits::DEFAULT.values();
        Self {
            header_count: values.header_count,
            header_bytes: values.header_bytes,
            response_body_utf8_bytes: values.response_body_utf8_bytes,
            concurrent_connections: values.concurrent_connections,
            request_deadline_ms: values.request_deadline.as_millis() as u64,
            idle_timeout_ms: values.idle_timeout.as_millis() as u64,
        }
    }
}

fn validate_u32(
    field: OperationsTransportLimitField,
    value: u32,
    maximum: u32,
) -> Result<(), OperationsTransportLimitsError> {
    if value == 0 {
        Err(OperationsTransportLimitsError::Zero { field })
    } else if value > maximum {
        Err(OperationsTransportLimitsError::ExceedsMaximum { field })
    } else {
        Ok(())
    }
}

fn validate_duration(
    field: OperationsTransportLimitField,
    value: Duration,
    maximum: Duration,
) -> Result<(), OperationsTransportLimitsError> {
    if value.is_zero() {
        Err(OperationsTransportLimitsError::Zero { field })
    } else if value > maximum {
        Err(OperationsTransportLimitsError::ExceedsMaximum { field })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIELDS: [OperationsTransportLimitField; 6] = [
        OperationsTransportLimitField::HeaderCount,
        OperationsTransportLimitField::HeaderBytes,
        OperationsTransportLimitField::ResponseBodyUtf8Bytes,
        OperationsTransportLimitField::ConcurrentConnections,
        OperationsTransportLimitField::RequestDeadline,
        OperationsTransportLimitField::IdleTimeout,
    ];

    #[test]
    fn valid_loopback_and_explicit_public_addresses_are_accepted() {
        let loopback: OperationsListenerConfig = toml::from_str(
            r#"
enabled = true
listen = "127.0.0.1:9100"
"#,
        )
        .expect("loopback config");
        assert!(loopback.is_enabled());
        assert_eq!(
            loopback.bind_policy(),
            Some(OperationsBindPolicy::LoopbackOnly)
        );
        assert_eq!(
            loopback.listen().expect("listen").to_string(),
            "127.0.0.1:9100"
        );
        assert_eq!(loopback.limits(), Some(OperationsTransportLimits::DEFAULT));

        let ipv6: OperationsListenerConfig = toml::from_str(
            r#"
enabled = true
listen = "[::1]:9100"
"#,
        )
        .expect("IPv6 loopback config");
        assert!(ipv6.listen().expect("listen").is_loopback());

        let public: OperationsListenerConfig = toml::from_str(
            r#"
enabled = true
listen = "0.0.0.0:9100"
bind_policy = "public"
"#,
        )
        .expect("explicit public config");
        assert_eq!(public.bind_policy(), Some(OperationsBindPolicy::Public));
    }

    #[test]
    fn invalid_missing_zero_port_and_implicit_public_addresses_fail() {
        for source in [
            "enabled = true",
            "enabled = true\nlisten = 'not-an-address'",
            "enabled = true\nlisten = '127.0.0.1:0'",
            "enabled = true\nlisten = '0.0.0.0:9100'",
            "enabled = true\nlisten = '192.0.2.10:9100'",
        ] {
            assert!(
                toml::from_str::<OperationsListenerConfig>(source).is_err(),
                "{source}"
            );
        }
    }

    #[test]
    fn disabled_mode_has_no_dormant_bind_or_limit_state() {
        let disabled: OperationsListenerConfig =
            toml::from_str("enabled = false").expect("disabled config");
        assert_eq!(disabled, OperationsListenerConfig::disabled());
        assert!(!disabled.is_enabled());
        assert_eq!(disabled.listen(), None);
        assert_eq!(disabled.bind_policy(), None);
        assert_eq!(disabled.limits(), None);

        for source in [
            "enabled = false\nlisten = '127.0.0.1:9100'",
            "enabled = false\nbind_policy = 'loopback_only'",
            "enabled = false\n[limits]",
        ] {
            assert!(
                toml::from_str::<OperationsListenerConfig>(source).is_err(),
                "{source}"
            );
        }
    }

    #[test]
    fn unknown_or_detailed_admin_options_are_rejected() {
        for source in [
            "enabled = false\ndetailed_status = true",
            "enabled = false\nadmin_routes = true",
            "enabled = false\nunknown = 1",
            "enabled = true\nlisten = '127.0.0.1:9100'\n[limits]\nunknown = 1",
        ] {
            assert!(
                toml::from_str::<OperationsListenerConfig>(source).is_err(),
                "{source}"
            );
        }
    }

    #[test]
    fn limit_inventory_defaults_and_boundaries_are_exact() {
        assert_eq!(
            FIELDS.map(OperationsTransportLimitField::maximum),
            [64, 32_768, 1_048_576, 64, 30_000, 60_000]
        );
        assert_eq!(
            OperationsTransportLimits::DEFAULT.values(),
            OperationsTransportLimitValues {
                header_count: 32,
                header_bytes: 16_384,
                response_body_utf8_bytes: 1_048_576,
                concurrent_connections: 32,
                request_deadline: Duration::from_millis(15_000),
                idle_timeout: Duration::from_millis(30_000),
            }
        );
        assert_eq!(
            OperationsTransportLimits::new(maximum_values())
                .expect("complete maximum tuple")
                .values(),
            maximum_values()
        );
        for field in FIELDS {
            assert_eq!(
                OperationsTransportLimits::new(with_field(maximum_values(), field, 0,)),
                Err(OperationsTransportLimitsError::Zero { field })
            );
            assert_eq!(
                OperationsTransportLimits::new(with_field(
                    maximum_values(),
                    field,
                    field.maximum() + 1,
                )),
                Err(OperationsTransportLimitsError::ExceedsMaximum { field })
            );
        }

        let mut extreme_request = maximum_values();
        extreme_request.request_deadline = Duration::MAX;
        assert_eq!(
            OperationsTransportLimits::new(extreme_request),
            Err(OperationsTransportLimitsError::ExceedsMaximum {
                field: OperationsTransportLimitField::RequestDeadline,
            })
        );
        let mut extreme_idle = maximum_values();
        extreme_idle.idle_timeout = Duration::MAX;
        assert_eq!(
            OperationsTransportLimits::new(extreme_idle),
            Err(OperationsTransportLimitsError::ExceedsMaximum {
                field: OperationsTransportLimitField::IdleTimeout,
            })
        );
        for field in ["request_deadline_ms", "idle_timeout_ms"] {
            let source = format!(
                "enabled = true\nlisten = '127.0.0.1:9100'\n[limits]\n{field} = 9223372036854775807"
            );
            assert!(
                toml::from_str::<OperationsListenerConfig>(&source).is_err(),
                "{field}"
            );
        }
    }

    #[test]
    fn serialization_is_exact_and_debug_redacts_the_bind_address() {
        let config: OperationsListenerConfig = toml::from_str(
            r#"
enabled = true
listen = "127.0.0.1:9100"
"#,
        )
        .expect("operations config");
        let encoded = toml::to_string(&config).expect("serialize operations config");
        assert_eq!(
            encoded,
            concat!(
                "enabled = true\n",
                "listen = \"127.0.0.1:9100\"\n",
                "bind_policy = \"loopback_only\"\n",
                "\n[limits]\n",
                "header_count = 32\n",
                "header_bytes = 16384\n",
                "response_body_utf8_bytes = 1048576\n",
                "concurrent_connections = 32\n",
                "request_deadline_ms = 15000\n",
                "idle_timeout_ms = 30000\n",
            )
        );
        assert!(!format!("{config:?}").contains("127.0.0.1"));
        assert_eq!(
            toml::from_str::<OperationsListenerConfig>(&encoded).expect("round trip"),
            config
        );
    }

    fn maximum_values() -> OperationsTransportLimitValues {
        OperationsTransportLimitValues {
            header_count: MAX_HEADER_COUNT,
            header_bytes: MAX_HEADER_BYTES,
            response_body_utf8_bytes: MAX_RESPONSE_BODY_UTF8_BYTES,
            concurrent_connections: MAX_CONCURRENT_CONNECTIONS,
            request_deadline: MAX_REQUEST_DEADLINE,
            idle_timeout: MAX_IDLE_TIMEOUT,
        }
    }

    fn with_field(
        mut values: OperationsTransportLimitValues,
        field: OperationsTransportLimitField,
        value: u64,
    ) -> OperationsTransportLimitValues {
        match field {
            OperationsTransportLimitField::HeaderCount => values.header_count = value as u32,
            OperationsTransportLimitField::HeaderBytes => values.header_bytes = value as u32,
            OperationsTransportLimitField::ResponseBodyUtf8Bytes => {
                values.response_body_utf8_bytes = value as u32;
            }
            OperationsTransportLimitField::ConcurrentConnections => {
                values.concurrent_connections = value as u32;
            }
            OperationsTransportLimitField::RequestDeadline => {
                values.request_deadline = Duration::from_millis(value);
            }
            OperationsTransportLimitField::IdleTimeout => {
                values.idle_timeout = Duration::from_millis(value);
            }
        }
        values
    }
}
