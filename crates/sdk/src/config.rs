#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};
use core::fmt;
#[cfg(feature = "std")]
use std::{env, string::String, vec::Vec};

pub const RADROOTS_SDK_PRODUCTION_RELAY_URL: &str = "wss://radroots.org";
pub const RADROOTS_SDK_STAGING_RELAY_URL: &str = "wss://staging.radroots.org";
pub const RADROOTS_SDK_LOCAL_RELAY_URL: &str = "ws://127.0.0.1:8080";

pub const RADROOTS_SDK_PRODUCTION_RADROOTSD_ENDPOINT: &str = "https://rpc.radroots.org/jsonrpc";
pub const RADROOTS_SDK_STAGING_RADROOTSD_ENDPOINT: &str =
    "https://rpc.staging.radroots.org/jsonrpc";
pub const RADROOTS_SDK_LOCAL_RADROOTSD_ENDPOINT: &str = "http://127.0.0.1:7070";

pub const RADROOTS_SDK_DEFAULT_TIMEOUT_MS: u64 = 10_000;

#[cfg(feature = "std")]
const LOCAL_RELAY_SCHEME_ENV: &str = "NOSTR_RS_RELAY_PUBLIC_SCHEME";
#[cfg(feature = "std")]
const LOCAL_RELAY_HOST_ENV: &str = "NOSTR_RS_RELAY_PUBLIC_HOST";
#[cfg(feature = "std")]
const LOCAL_RELAY_PORT_ENV: &str = "NOSTR_RS_RELAY_PUBLIC_PORT";
#[cfg(feature = "std")]
const LOCAL_RADROOTSD_ENDPOINT_ENV: &str = "RADROOTSD_RPC_URL";
#[cfg(feature = "std")]
const LOCAL_RADROOTSD_HOST_ENV: &str = "RADROOTSD_RPC_HOST";
#[cfg(feature = "std")]
const LOCAL_RADROOTSD_PORT_ENV: &str = "RADROOTSD_RPC_PORT";

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
            if environment == SdkEnvironment::Local {
                #[cfg(feature = "std")]
                if let Some(local_url) = resolve_local_relay_url_from_env() {
                    return Ok(vec![normalize_relay_url(local_url.as_str())?]);
                }
            }
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
            None => {
                if environment == SdkEnvironment::Local {
                    #[cfg(feature = "std")]
                    if let Some(endpoint) = resolve_local_radrootsd_endpoint_from_env() {
                        return normalize_radrootsd_endpoint(endpoint.as_str());
                    }
                }

                environment
                    .default_radrootsd_endpoint()
                    .map(str::to_owned)
                    .ok_or(SdkConfigError::MissingCustomRadrootsdEndpoint)
            }
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

#[derive(Clone, PartialEq, Eq, Default)]
pub enum RadrootsdAuth {
    #[default]
    None,
    BearerToken(String),
}

impl fmt::Debug for RadrootsdAuth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => f.write_str("None"),
            Self::BearerToken(_) => f.write_str("BearerToken(\"<redacted>\")"),
        }
    }
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
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            timeout_ms: RADROOTS_SDK_DEFAULT_TIMEOUT_MS,
        }
    }
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

    let rest = if let Some(rest) = trimmed.strip_prefix("ws://") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("wss://") {
        rest
    } else {
        return Err(SdkConfigError::InvalidRelayUrl(trimmed.to_owned()));
    };

    if relay_authority_is_invalid(rest) {
        return Err(SdkConfigError::InvalidRelayUrl(trimmed.to_owned()));
    }

    Ok(trimmed.to_owned())
}

fn relay_authority_is_invalid(rest: &str) -> bool {
    let authority_end = rest
        .char_indices()
        .find(|(_, ch)| matches!(ch, '/' | '?' | '#'))
        .map(|(index, _)| index)
        .unwrap_or(rest.len());
    let authority = &rest[..authority_end];

    if authority.is_empty() || authority.chars().any(char::is_whitespace) {
        return true;
    }
    if authority.contains('@') {
        return true;
    }

    if let Some(after_open) = authority.strip_prefix('[') {
        let Some(close_index) = after_open.find(']') else {
            return true;
        };
        let host = &after_open[..close_index];
        let after_host = &after_open[close_index + 1..];
        if host.is_empty() {
            return true;
        }
        return relay_port_suffix_is_invalid(after_host);
    }

    let colon_count = authority.bytes().filter(|byte| *byte == b':').count();
    match colon_count {
        0 => false,
        1 => {
            let Some((host, port)) = authority.split_once(':') else {
                return true;
            };
            host.is_empty() || relay_port_is_invalid(port)
        }
        _ => true,
    }
}

fn relay_port_suffix_is_invalid(after_host: &str) -> bool {
    if after_host.is_empty() {
        return false;
    }
    let Some(port) = after_host.strip_prefix(':') else {
        return true;
    };
    relay_port_is_invalid(port)
}

fn relay_port_is_invalid(port: &str) -> bool {
    port.is_empty() || !port.bytes().all(|byte| byte.is_ascii_digit())
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

#[cfg(feature = "std")]
fn resolve_local_relay_url_from_env() -> Option<String> {
    let scheme = read_trimmed_env(LOCAL_RELAY_SCHEME_ENV)?;
    let host = read_trimmed_env(LOCAL_RELAY_HOST_ENV)?;
    let port = read_trimmed_env(LOCAL_RELAY_PORT_ENV)?;
    Some(format!("{scheme}://{host}:{port}"))
}

#[cfg(feature = "std")]
fn resolve_local_radrootsd_endpoint_from_env() -> Option<String> {
    if let Some(endpoint) = read_trimmed_env(LOCAL_RADROOTSD_ENDPOINT_ENV) {
        return Some(endpoint);
    }

    let host = read_trimmed_env(LOCAL_RADROOTSD_HOST_ENV)?;
    let port = read_trimmed_env(LOCAL_RADROOTSD_PORT_ENV)?;
    Some(format!("http://{host}:{port}"))
}

#[cfg(feature = "std")]
fn read_trimmed_env(key: &str) -> Option<String> {
    let value = env::var(key).ok()?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_owned())
}
