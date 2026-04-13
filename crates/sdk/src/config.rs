#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};
use core::fmt;
#[cfg(feature = "std")]
use std::{string::String, vec::Vec};

pub const RADROOTS_SDK_PRODUCTION_RELAY_URL: &str = "wss://radroots.org";
pub const RADROOTS_SDK_STAGING_RELAY_URL: &str = "wss://staging.radroots.org";
pub const RADROOTS_SDK_LOCAL_RELAY_URL: &str = "ws://127.0.0.1:8080";

pub const RADROOTS_SDK_PRODUCTION_RADROOTSD_ENDPOINT: &str = "https://rpc.radroots.org/jsonrpc";
pub const RADROOTS_SDK_STAGING_RADROOTSD_ENDPOINT: &str =
    "https://rpc.staging.radroots.org/jsonrpc";
pub const RADROOTS_SDK_LOCAL_RADROOTSD_ENDPOINT: &str = "http://127.0.0.1:7070";

pub const RADROOTS_SDK_DEFAULT_TIMEOUT_MS: u64 = 10_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSdkConfig {
    pub environment: SdkEnvironment,
    pub transport: SdkTransportMode,
    pub relay: RelayConfig,
    pub radrootsd: RadrootsdConfig,
    pub signer: SignerConfig,
    pub network: NetworkConfig,
}

impl RadrootsSdkConfig {
    pub fn production() -> Self {
        Self::for_environment(SdkEnvironment::Production)
    }

    pub fn staging() -> Self {
        Self::for_environment(SdkEnvironment::Staging)
    }

    pub fn local() -> Self {
        Self::for_environment(SdkEnvironment::Local)
    }

    pub fn custom() -> Self {
        Self::for_environment(SdkEnvironment::Custom)
    }

    pub fn for_environment(environment: SdkEnvironment) -> Self {
        Self {
            environment,
            transport: SdkTransportMode::RelayDirect,
            relay: RelayConfig::default(),
            radrootsd: RadrootsdConfig::default(),
            signer: SignerConfig::default(),
            network: NetworkConfig::default(),
        }
    }

    pub fn resolved_relay_urls(&self) -> Result<Vec<String>, SdkConfigError> {
        self.relay.resolved_urls(self.environment)
    }

    pub fn resolved_radrootsd_endpoint(&self) -> Result<String, SdkConfigError> {
        self.radrootsd.resolved_endpoint(self.environment)
    }
}

impl Default for RadrootsSdkConfig {
    fn default() -> Self {
        Self::production()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SdkEnvironment {
    Production,
    Staging,
    Local,
    Custom,
}

impl SdkEnvironment {
    pub fn default_relay_urls(self) -> Option<Vec<String>> {
        match self {
            Self::Production => Some(vec![RADROOTS_SDK_PRODUCTION_RELAY_URL.to_owned()]),
            Self::Staging => Some(vec![RADROOTS_SDK_STAGING_RELAY_URL.to_owned()]),
            Self::Local => Some(vec![RADROOTS_SDK_LOCAL_RELAY_URL.to_owned()]),
            Self::Custom => None,
        }
    }

    pub fn default_radrootsd_endpoint(self) -> Option<&'static str> {
        match self {
            Self::Production => Some(RADROOTS_SDK_PRODUCTION_RADROOTSD_ENDPOINT),
            Self::Staging => Some(RADROOTS_SDK_STAGING_RADROOTSD_ENDPOINT),
            Self::Local => Some(RADROOTS_SDK_LOCAL_RADROOTSD_ENDPOINT),
            Self::Custom => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SdkTransportMode {
    RelayDirect,
    Radrootsd,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RelayConfig {
    pub urls: Vec<String>,
}

impl RelayConfig {
    pub fn resolved_urls(
        &self,
        environment: SdkEnvironment,
    ) -> Result<Vec<String>, SdkConfigError> {
        if self.urls.is_empty() {
            return environment
                .default_relay_urls()
                .ok_or(SdkConfigError::MissingCustomRelayUrls);
        }

        normalize_relay_urls(&self.urls)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsdConfig {
    pub endpoint: Option<String>,
    pub auth: RadrootsdAuth,
}

impl RadrootsdConfig {
    pub fn resolved_endpoint(&self, environment: SdkEnvironment) -> Result<String, SdkConfigError> {
        match self.endpoint.as_deref() {
            Some(endpoint) => normalize_radrootsd_endpoint(endpoint),
            None => environment
                .default_radrootsd_endpoint()
                .map(str::to_owned)
                .ok_or(SdkConfigError::MissingCustomRadrootsdEndpoint),
        }
    }
}

impl Default for RadrootsdConfig {
    fn default() -> Self {
        Self {
            endpoint: None,
            auth: RadrootsdAuth::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RadrootsdAuth {
    #[default]
    None,
    BearerToken(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SignerConfig {
    #[default]
    DraftOnly,
    LocalIdentity,
    Nip46,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkConfig {
    pub timeout_ms: u64,
    pub retry_policy: RetryPolicy,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            timeout_ms: RADROOTS_SDK_DEFAULT_TIMEOUT_MS,
            retry_policy: RetryPolicy::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RetryPolicy {
    #[default]
    None,
    Fixed {
        max_attempts: u32,
        backoff_ms: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SdkConfigError {
    MissingCustomRelayUrls,
    MissingCustomRadrootsdEndpoint,
    EmptyRelayUrl,
    InvalidRelayUrl(String),
    EmptyRadrootsdEndpoint,
    InvalidRadrootsdEndpoint(String),
}

impl fmt::Display for SdkConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCustomRelayUrls => {
                f.write_str("custom sdk environment requires explicit relay urls")
            }
            Self::MissingCustomRadrootsdEndpoint => {
                f.write_str("custom sdk environment requires an explicit radrootsd endpoint")
            }
            Self::EmptyRelayUrl => f.write_str("relay url must not be empty"),
            Self::InvalidRelayUrl(value) => {
                write!(f, "relay url must use ws or wss, got `{value}`")
            }
            Self::EmptyRadrootsdEndpoint => f.write_str("radrootsd endpoint must not be empty"),
            Self::InvalidRadrootsdEndpoint(value) => {
                write!(
                    f,
                    "radrootsd endpoint must use http or https, got `{value}`"
                )
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for SdkConfigError {}

impl fmt::Display for SignerConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DraftOnly => f.write_str("draft_only"),
            Self::LocalIdentity => f.write_str("local_identity"),
            Self::Nip46 => f.write_str("nip46"),
        }
    }
}

fn normalize_relay_urls(values: &[String]) -> Result<Vec<String>, SdkConfigError> {
    let mut normalized = Vec::new();
    for value in values {
        let relay = normalize_relay_url(value.as_str())?;
        if !normalized.iter().any(|existing| existing == &relay) {
            normalized.push(relay);
        }
    }
    Ok(normalized)
}

fn normalize_relay_url(value: &str) -> Result<String, SdkConfigError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(SdkConfigError::EmptyRelayUrl);
    }
    if !(trimmed.starts_with("ws://") || trimmed.starts_with("wss://")) {
        return Err(SdkConfigError::InvalidRelayUrl(trimmed.to_owned()));
    }
    Ok(trimmed.to_owned())
}

fn normalize_radrootsd_endpoint(value: &str) -> Result<String, SdkConfigError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(SdkConfigError::EmptyRadrootsdEndpoint);
    }
    if !(trimmed.starts_with("http://") || trimmed.starts_with("https://")) {
        return Err(SdkConfigError::InvalidRadrootsdEndpoint(trimmed.to_owned()));
    }
    Ok(trimmed.to_owned())
}
