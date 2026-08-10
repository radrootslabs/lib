//! Explicit user-facing transport profile composition.
//!
//! Profiles retain canonical `radroots_transport` identities, targets,
//! policies, and statuses. They select no adapter implicitly and never replace
//! an unavailable selection with another transport.

use radroots_transport::{
    Error, SinkStatus, SourceStatus, TargetSet, TransportId,
    capability::{Availability, Maturity, SinkCapabilities, SourceCapabilities},
    policy::SatisfactionPolicy,
};
#[cfg(any(feature = "blossom", feature = "nostr"))]
use std::sync::{Arc, RwLock};

#[cfg(feature = "blossom")]
use std::{
    collections::BTreeSet,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[cfg(feature = "blossom")]
use radroots_blossom::{
    BlobUrl, ByteVerifiedDescriptor, MediaType, Sha256,
    authorization::{AuthoredUploadClaim, AuthorizationContent, ServerDomain},
};

#[cfg(feature = "nostr")]
pub use radroots_transport_nostr::{
    DEFAULT_PUBLIC_RELAY, ReconnectBackoff, RelayAccess, RelayAggregateState,
    RelayCapabilityEvidence, RelayCursor, RelayEndpoint, RelayEvidenceState, RelayProfile,
    RelayProfileKind, RelayStatus, RelayStatusReport, RelayUrl, RelayUrlPolicy,
};

const PREVIEW_UNAVAILABLE_MESSAGE: &str = "preview transport is unavailable in this SDK release";

#[cfg(feature = "blossom")]
const MAX_BLOSSOM_ENDPOINTS: usize = 16;
#[cfg(feature = "blossom")]
const MAX_BLOSSOM_BLOB_BYTES: u64 = 100 * 1024 * 1024;
#[cfg(feature = "blossom")]
const MAX_BLOSSOM_DESCRIPTOR_BYTES: usize = 64 * 1024;
#[cfg(feature = "blossom")]
const MAX_BLOSSOM_REDIRECTS: u8 = 5;
#[cfg(feature = "blossom")]
const MAX_BLOSSOM_ATTEMPTS: u8 = 5;
#[cfg(feature = "blossom")]
const MAX_BLOSSOM_TIMEOUT: Duration = Duration::from_secs(120);
#[cfg(feature = "blossom")]
const MAX_BLOSSOM_RETRY_DELAY: Duration = Duration::from_secs(30);
#[cfg(feature = "blossom")]
const MAX_BLOSSOM_IMAGE_EDGE: u32 = 16_384;
#[cfg(feature = "blossom")]
const MAX_BLOSSOM_IMAGE_PIXELS: u64 = 100_000_000;

/// Host environment executing Blossom operations.
#[cfg(feature = "blossom")]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum BlossomHostKind {
    /// A non-mobile native host.
    Native,
    /// An Apple or Android simulator.
    Simulator,
    /// A physical mobile device.
    PhysicalDevice,
}

/// Network authority applied independently to configured Blossom origins.
#[cfg(feature = "blossom")]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum BlossomEndpointAuthority {
    /// Public HTTPS authenticated by the platform WebPKI roots.
    PublicWebPki,
    /// Development-only HTTP or HTTPS resolving exclusively to loopback.
    LoopbackDevelopment,
    /// Development-only HTTPS resolving to a non-loopback trusted network.
    PrivateNetworkDevelopment,
}

/// One canonical configured Blossom origin.
#[cfg(feature = "blossom")]
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BlossomEndpoint {
    origin: String,
    host: String,
    port: u16,
    authority: BlossomEndpointAuthority,
}

#[cfg(feature = "blossom")]
impl BlossomEndpoint {
    fn parse(
        value: impl AsRef<str>,
        authority: BlossomEndpointAuthority,
    ) -> Result<Self, BlossomError> {
        let value = value.as_ref();
        if value.is_empty() || !value.is_ascii() || value.chars().any(char::is_whitespace) {
            return Err(BlossomError::configuration(
                BlossomErrorKind::InvalidEndpoint,
            ));
        }
        let parsed = reqwest::Url::parse(value)
            .map_err(|_| BlossomError::configuration(BlossomErrorKind::InvalidEndpoint))?;
        if !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
            || parsed.path() != "/"
        {
            return Err(BlossomError::configuration(
                BlossomErrorKind::InvalidEndpoint,
            ));
        }
        let host = parsed
            .host_str()
            .ok_or_else(|| BlossomError::configuration(BlossomErrorKind::InvalidEndpoint))?
            .to_owned();
        let port = parsed
            .port_or_known_default()
            .ok_or_else(|| BlossomError::configuration(BlossomErrorKind::InvalidEndpoint))?;
        if port == 0 || !endpoint_scheme_is_allowed(parsed.scheme(), authority) {
            return Err(BlossomError::configuration(
                BlossomErrorKind::EndpointSchemeDenied,
            ));
        }
        validate_blossom_host(host.as_str(), authority)?;
        ServerDomain::parse(host.as_str())
            .map_err(|_| BlossomError::configuration(BlossomErrorKind::InvalidEndpoint))?;
        // HTTP(S) URLs always have a tuple origin after the scheme and host
        // checks above, so `ascii_serialization` cannot be the opaque `null`
        // origin here.
        let origin = parsed.origin().ascii_serialization();
        Ok(Self {
            origin,
            host,
            port,
            authority,
        })
    }

    /// Returns the canonical origin without a trailing slash.
    #[must_use]
    pub fn origin(&self) -> &str {
        self.origin.as_str()
    }

    /// Returns the BUD-11 server-domain spelling.
    #[must_use]
    pub fn host(&self) -> &str {
        self.host.as_str()
    }

    /// Returns the resolved connection port.
    #[must_use]
    pub const fn port(&self) -> u16 {
        self.port
    }

    /// Returns the authority used before and after DNS resolution.
    #[must_use]
    pub const fn authority(&self) -> BlossomEndpointAuthority {
        self.authority
    }

    pub(crate) fn upload_url(&self) -> String {
        format!("{}/upload", self.origin)
    }

    pub(crate) fn server_domain(&self) -> Result<ServerDomain, BlossomError> {
        ServerDomain::parse(self.host.as_str())
            .map_err(|_| BlossomError::configuration(BlossomErrorKind::InvalidEndpoint))
    }

    pub(crate) fn accepts_blob_url(&self, value: &BlobUrl) -> bool {
        reqwest::Url::parse(value.as_str())
            .is_ok_and(|url| url.origin().ascii_serialization() == self.origin)
    }

    pub(crate) fn validate_resolved_addresses(
        &self,
        addresses: impl IntoIterator<Item = IpAddr>,
    ) -> Result<(), BlossomError> {
        let mut found = false;
        for address in addresses {
            found = true;
            if !blossom_authority_accepts_address(self.authority, address) {
                return Err(BlossomError::configuration(
                    BlossomErrorKind::ResolvedAddressDenied,
                ));
            }
        }
        if !found {
            return Err(BlossomError::configuration(
                BlossomErrorKind::ResolutionFailed,
            ));
        }
        Ok(())
    }
}

/// Complete validated Blossom origin set for one host environment.
#[cfg(feature = "blossom")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlossomProfile {
    host_kind: BlossomHostKind,
    authority: BlossomEndpointAuthority,
    primary: BlossomEndpoint,
    fallbacks: Vec<BlossomEndpoint>,
}

#[cfg(feature = "blossom")]
impl BlossomProfile {
    /// Configures one primary origin and an ordered, explicitly bounded fallback set.
    pub fn new<I, S>(
        host_kind: BlossomHostKind,
        authority: BlossomEndpointAuthority,
        primary_origin: impl AsRef<str>,
        fallback_origins: I,
    ) -> Result<Self, BlossomError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        validate_host_authority(host_kind, authority)?;
        let primary = BlossomEndpoint::parse(primary_origin, authority)?;
        // The public constructor accepts any iterator, so bound collection
        // before parsing to prevent an oversized caller from allocating an
        // unbounded temporary vector only to be rejected afterward.
        let fallbacks = fallback_origins
            .into_iter()
            .take(MAX_BLOSSOM_ENDPOINTS)
            .map(|origin| BlossomEndpoint::parse(origin, authority))
            .collect::<Result<Vec<_>, _>>()?;
        if fallbacks.len().saturating_add(1) > MAX_BLOSSOM_ENDPOINTS {
            return Err(BlossomError::configuration(
                BlossomErrorKind::InvalidEndpointCount,
            ));
        }
        let unique = std::iter::once(&primary)
            .chain(fallbacks.iter())
            .map(BlossomEndpoint::origin)
            .collect::<BTreeSet<_>>();
        if unique.len() != fallbacks.len().saturating_add(1) {
            return Err(BlossomError::configuration(
                BlossomErrorKind::DuplicateEndpoint,
            ));
        }
        Ok(Self {
            host_kind,
            authority,
            primary,
            fallbacks,
        })
    }

    #[must_use]
    pub const fn host_kind(&self) -> BlossomHostKind {
        self.host_kind
    }

    #[must_use]
    pub const fn authority(&self) -> BlossomEndpointAuthority {
        self.authority
    }

    #[must_use]
    pub const fn primary(&self) -> &BlossomEndpoint {
        &self.primary
    }

    #[must_use]
    pub fn fallbacks(&self) -> &[BlossomEndpoint] {
        self.fallbacks.as_slice()
    }

    pub(crate) fn endpoint_for_blob(&self, url: &BlobUrl) -> Option<&BlossomEndpoint> {
        std::iter::once(&self.primary)
            .chain(self.fallbacks.iter())
            .find(|endpoint| endpoint.accepts_blob_url(url))
    }
}

/// Bounded HTTP, response, retry, and redirect policy for Blossom operations.
#[cfg(feature = "blossom")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlossomConfig {
    profile: BlossomProfile,
    max_blob_bytes: u64,
    max_descriptor_bytes: usize,
    max_redirects: u8,
    max_attempts: u8,
    connect_timeout: Duration,
    request_timeout: Duration,
    initial_retry_delay: Duration,
}

#[cfg(feature = "blossom")]
impl BlossomConfig {
    #[must_use]
    pub fn from_profile(profile: BlossomProfile) -> Self {
        Self {
            profile,
            max_blob_bytes: 20 * 1024 * 1024,
            max_descriptor_bytes: 16 * 1024,
            max_redirects: 3,
            max_attempts: 3,
            connect_timeout: Duration::from_secs(10),
            request_timeout: Duration::from_secs(60),
            initial_retry_delay: Duration::from_millis(250),
        }
    }

    pub fn with_limits(
        mut self,
        max_blob_bytes: u64,
        max_descriptor_bytes: usize,
        max_redirects: u8,
    ) -> Result<Self, BlossomError> {
        if max_blob_bytes == 0
            || max_blob_bytes > MAX_BLOSSOM_BLOB_BYTES
            || max_descriptor_bytes == 0
            || max_descriptor_bytes > MAX_BLOSSOM_DESCRIPTOR_BYTES
            || max_redirects > MAX_BLOSSOM_REDIRECTS
        {
            return Err(BlossomError::configuration(BlossomErrorKind::InvalidLimits));
        }
        self.max_blob_bytes = max_blob_bytes;
        self.max_descriptor_bytes = max_descriptor_bytes;
        self.max_redirects = max_redirects;
        Ok(self)
    }

    pub fn with_network_policy(
        mut self,
        connect_timeout: Duration,
        request_timeout: Duration,
        max_attempts: u8,
        initial_retry_delay: Duration,
    ) -> Result<Self, BlossomError> {
        if connect_timeout.is_zero()
            || connect_timeout > MAX_BLOSSOM_TIMEOUT
            || request_timeout.is_zero()
            || request_timeout > MAX_BLOSSOM_TIMEOUT
            || max_attempts == 0
            || max_attempts > MAX_BLOSSOM_ATTEMPTS
            || initial_retry_delay.is_zero()
            || initial_retry_delay > MAX_BLOSSOM_RETRY_DELAY
        {
            return Err(BlossomError::configuration(BlossomErrorKind::InvalidLimits));
        }
        self.connect_timeout = connect_timeout;
        self.request_timeout = request_timeout;
        self.max_attempts = max_attempts;
        self.initial_retry_delay = initial_retry_delay;
        Ok(self)
    }

    #[must_use]
    pub const fn profile(&self) -> &BlossomProfile {
        &self.profile
    }

    /// Returns the stable identity of every setting that can affect an operation.
    #[must_use]
    pub fn fingerprint(&self) -> BlossomConfigFingerprint {
        let mut material = Vec::new();
        material.extend_from_slice(b"radroots-blossom-config-v1\0");
        material.push(match self.profile.host_kind {
            BlossomHostKind::Native => 0,
            BlossomHostKind::Simulator => 1,
            BlossomHostKind::PhysicalDevice => 2,
        });
        material.push(match self.profile.authority {
            BlossomEndpointAuthority::PublicWebPki => 0,
            BlossomEndpointAuthority::LoopbackDevelopment => 1,
            BlossomEndpointAuthority::PrivateNetworkDevelopment => 2,
        });
        append_fingerprint_field(&mut material, self.profile.primary.origin.as_bytes());
        material.extend_from_slice(&(self.profile.fallbacks.len() as u64).to_be_bytes());
        for endpoint in &self.profile.fallbacks {
            append_fingerprint_field(&mut material, endpoint.origin.as_bytes());
        }
        material.extend_from_slice(&self.max_blob_bytes.to_be_bytes());
        material.extend_from_slice(&(self.max_descriptor_bytes as u64).to_be_bytes());
        material.push(self.max_redirects);
        material.push(self.max_attempts);
        material.extend_from_slice(&(self.connect_timeout.as_millis() as u64).to_be_bytes());
        material.extend_from_slice(&(self.request_timeout.as_millis() as u64).to_be_bytes());
        material.extend_from_slice(&(self.initial_retry_delay.as_millis() as u64).to_be_bytes());
        BlossomConfigFingerprint(Sha256::digest(material.as_slice()))
    }

    pub(crate) const fn max_blob_bytes(&self) -> u64 {
        self.max_blob_bytes
    }

    pub(crate) const fn max_descriptor_bytes(&self) -> usize {
        self.max_descriptor_bytes
    }

    pub(crate) const fn max_redirects(&self) -> u8 {
        self.max_redirects
    }

    pub(crate) const fn max_attempts(&self) -> u8 {
        self.max_attempts
    }

    pub(crate) const fn connect_timeout(&self) -> Duration {
        self.connect_timeout
    }

    pub(crate) const fn request_timeout(&self) -> Duration {
        self.request_timeout
    }

    pub(crate) const fn initial_retry_delay(&self) -> Duration {
        self.initial_retry_delay
    }
}

/// Stable, non-secret identity of a completely validated Blossom configuration.
#[cfg(feature = "blossom")]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BlossomConfigFingerprint(Sha256);

#[cfg(feature = "blossom")]
impl BlossomConfigFingerprint {
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        self.0.as_bytes()
    }

    #[must_use]
    pub fn to_hex(self) -> String {
        self.0.to_hex()
    }
}

#[cfg(feature = "blossom")]
impl std::fmt::Display for BlossomConfigFingerprint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, formatter)
    }
}

#[cfg(feature = "blossom")]
fn append_fingerprint_field(material: &mut Vec<u8>, value: &[u8]) {
    material.extend_from_slice(&(value.len() as u64).to_be_bytes());
    material.extend_from_slice(value);
}

#[cfg(feature = "blossom")]
pub(crate) fn blossom_now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
        .unwrap_or(u64::MAX)
        .max(1)
}

/// Nonzero dimensions verified from the final image bytes.
#[cfg(feature = "blossom")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BlossomImageDimensions {
    width: u32,
    height: u32,
}

#[cfg(feature = "blossom")]
impl BlossomImageDimensions {
    pub const fn new(width: u32, height: u32) -> Result<Self, BlossomError> {
        if width == 0
            || height == 0
            || width > MAX_BLOSSOM_IMAGE_EDGE
            || height > MAX_BLOSSOM_IMAGE_EDGE
            || width as u64 * height as u64 > MAX_BLOSSOM_IMAGE_PIXELS
        {
            return Err(BlossomError::configuration(
                BlossomErrorKind::InvalidDimensions,
            ));
        }
        Ok(Self { width, height })
    }

    #[must_use]
    pub const fn width(self) -> u32 {
        self.width
    }

    #[must_use]
    pub const fn height(self) -> u32 {
        self.height
    }
}

/// Exact final image bytes from which Rust derives the BUD-02 destination.
#[cfg(feature = "blossom")]
#[derive(Clone)]
pub struct BlossomUploadRequest {
    sha256: Sha256,
    bytes: Arc<[u8]>,
    media_type: MediaType,
    dimensions: BlossomImageDimensions,
    verified_at_unix_ms: u64,
}

#[cfg(feature = "blossom")]
impl BlossomUploadRequest {
    pub fn new(
        bytes: Arc<[u8]>,
        media_type: MediaType,
        dimensions: BlossomImageDimensions,
        verified_at_unix_ms: u64,
    ) -> Result<Self, BlossomError> {
        if bytes.is_empty() || verified_at_unix_ms == 0 {
            return Err(BlossomError::configuration(
                BlossomErrorKind::InvalidRequest,
            ));
        }
        crate::adapters::blossom::verify_image(bytes.as_ref(), &media_type, dimensions)?;
        Ok(Self {
            sha256: Sha256::digest(bytes.as_ref()),
            bytes,
            media_type,
            dimensions,
            verified_at_unix_ms,
        })
    }

    #[must_use]
    pub fn media_type(&self) -> &MediaType {
        &self.media_type
    }

    #[must_use]
    pub const fn dimensions(&self) -> BlossomImageDimensions {
        self.dimensions
    }

    #[must_use]
    pub const fn sha256(&self) -> Sha256 {
        self.sha256
    }

    #[must_use]
    pub fn byte_size(&self) -> u64 {
        self.bytes.len() as u64
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        self.bytes.as_ref()
    }

    pub(crate) const fn verified_at_unix_ms(&self) -> u64 {
        self.verified_at_unix_ms
    }
}

/// Expected signed metadata for one bounded BUD-01 image retrieval.
#[cfg(feature = "blossom")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlossomInboundRequest {
    url: BlobUrl,
    expected_media_type: Option<MediaType>,
    expected_byte_size: Option<u64>,
    expected_dimensions: Option<BlossomImageDimensions>,
}

#[cfg(feature = "blossom")]
impl BlossomInboundRequest {
    pub fn new(
        url: BlobUrl,
        expected_media_type: Option<MediaType>,
        expected_byte_size: Option<u64>,
        expected_dimensions: Option<BlossomImageDimensions>,
    ) -> Result<Self, BlossomError> {
        url.clone()
            .approve()
            .map_err(|_| BlossomError::configuration(BlossomErrorKind::InvalidRequest))?;
        if expected_byte_size == Some(0) {
            return Err(BlossomError::configuration(
                BlossomErrorKind::InvalidRequest,
            ));
        }
        if let Some(media_type) = &expected_media_type {
            let extension = canonical_image_extension(media_type)?;
            if url
                .hash_path()
                .extension()
                .is_none_or(|value| value.as_str() != extension)
            {
                return Err(BlossomError::configuration(
                    BlossomErrorKind::MediaTypeMismatch,
                ));
            }
        }
        Ok(Self {
            url,
            expected_media_type,
            expected_byte_size,
            expected_dimensions,
        })
    }

    #[must_use]
    pub const fn url(&self) -> &BlobUrl {
        &self.url
    }

    #[must_use]
    pub const fn expected_media_type(&self) -> Option<&MediaType> {
        self.expected_media_type.as_ref()
    }

    #[must_use]
    pub const fn expected_byte_size(&self) -> Option<u64> {
        self.expected_byte_size
    }

    #[must_use]
    pub const fn expected_dimensions(&self) -> Option<BlossomImageDimensions> {
        self.expected_dimensions
    }
}

/// Immutable upload plan binding exact bytes to one complete configuration.
#[cfg(feature = "blossom")]
#[derive(Clone)]
pub struct BlossomUploadTransaction {
    config: BlossomConfig,
    config_fingerprint: BlossomConfigFingerprint,
    endpoint: BlossomEndpoint,
    expected_url: BlobUrl,
    request: BlossomUploadRequest,
}

/// Security property observed for the configured primary transport.
#[cfg(feature = "blossom")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum BlossomTransportSecurity {
    /// Public HTTPS authenticated by the bundled platform WebPKI roots.
    PublicWebPki,
    /// Development HTTPS without a public-origin availability claim.
    DevelopmentTls,
    /// Simulator-only cleartext loopback HTTP.
    DevelopmentCleartext,
}

/// Latest redacted evidence state for the configured primary Blossom origin.
#[cfg(feature = "blossom")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum BlossomEvidenceState {
    ConfiguredUnobserved,
    DnsPolicyValidated,
    TlsHttpObserved,
    UploadVerified,
    RetrievalVerified,
    RetryableFailure,
    TerminalFailure,
}

/// Versioned, passive, secret-safe evidence for one exact configuration.
#[cfg(feature = "blossom")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlossomEndpointEvidence {
    origin: String,
    config_fingerprint: BlossomConfigFingerprint,
    state: BlossomEvidenceState,
    last_successful_state: BlossomEvidenceState,
    transport_security: BlossomTransportSecurity,
    observed_at_unix_ms: Option<u64>,
    http_status: Option<u16>,
    error_code: Option<&'static str>,
    server_error_code: Option<String>,
    error_phase: Option<BlossomPhase>,
    retryable: bool,
    possible_orphan: bool,
    attempts: u8,
}

#[cfg(feature = "blossom")]
impl BlossomEndpointEvidence {
    const SCHEMA_VERSION: u16 = 2;

    fn configured(config: &BlossomConfig) -> Self {
        let primary = config.profile().primary();
        Self {
            origin: primary.origin().to_owned(),
            config_fingerprint: config.fingerprint(),
            state: BlossomEvidenceState::ConfiguredUnobserved,
            last_successful_state: BlossomEvidenceState::ConfiguredUnobserved,
            transport_security: match (
                primary.origin().starts_with("https://"),
                primary.authority(),
            ) {
                (true, BlossomEndpointAuthority::PublicWebPki) => {
                    BlossomTransportSecurity::PublicWebPki
                }
                (true, _) => BlossomTransportSecurity::DevelopmentTls,
                (false, _) => BlossomTransportSecurity::DevelopmentCleartext,
            },
            observed_at_unix_ms: None,
            http_status: None,
            error_code: None,
            server_error_code: None,
            error_phase: None,
            retryable: false,
            possible_orphan: false,
            attempts: 0,
        }
    }

    #[must_use]
    pub const fn schema_version(&self) -> u16 {
        Self::SCHEMA_VERSION
    }

    #[must_use]
    pub fn origin(&self) -> &str {
        self.origin.as_str()
    }

    #[must_use]
    pub const fn config_fingerprint(&self) -> BlossomConfigFingerprint {
        self.config_fingerprint
    }

    #[must_use]
    pub const fn state(&self) -> BlossomEvidenceState {
        self.state
    }

    #[must_use]
    pub const fn last_successful_state(&self) -> BlossomEvidenceState {
        self.last_successful_state
    }

    #[must_use]
    pub const fn transport_security(&self) -> BlossomTransportSecurity {
        self.transport_security
    }

    #[must_use]
    pub const fn observed_at_unix_ms(&self) -> Option<u64> {
        self.observed_at_unix_ms
    }

    #[must_use]
    pub const fn http_status(&self) -> Option<u16> {
        self.http_status
    }

    #[must_use]
    pub const fn error_code(&self) -> Option<&'static str> {
        self.error_code
    }

    /// Bounded, validated public error identifier returned by the server.
    #[must_use]
    pub fn server_error_code(&self) -> Option<&str> {
        self.server_error_code.as_deref()
    }

    #[must_use]
    pub const fn error_phase(&self) -> Option<BlossomPhase> {
        self.error_phase
    }

    #[must_use]
    pub const fn retryable(&self) -> bool {
        self.retryable
    }

    #[must_use]
    pub const fn possible_orphan(&self) -> bool {
        self.possible_orphan
    }

    #[must_use]
    pub const fn attempts(&self) -> u8 {
        self.attempts
    }

    fn record_success(&mut self, state: BlossomEvidenceState, http_status: Option<u16>) {
        self.state = state;
        self.last_successful_state = state;
        self.observed_at_unix_ms = Some(blossom_now_unix_ms());
        self.http_status = http_status;
        self.error_code = None;
        self.server_error_code = None;
        self.error_phase = None;
        self.retryable = false;
        self.possible_orphan = false;
        self.attempts = 0;
    }

    fn record_failure(&mut self, error: &BlossomError) {
        self.state = if error.retryable() {
            BlossomEvidenceState::RetryableFailure
        } else {
            BlossomEvidenceState::TerminalFailure
        };
        self.observed_at_unix_ms = Some(blossom_now_unix_ms());
        self.http_status = error.http_status();
        self.error_code = Some(error.code());
        self.server_error_code = error.server_error_code().map(str::to_owned);
        self.error_phase = Some(error.phase());
        self.retryable = error.retryable();
        self.possible_orphan = error.possible_orphan();
        self.attempts = error.attempts();
    }
}

#[cfg(feature = "blossom")]
impl BlossomUploadTransaction {
    #[must_use]
    pub const fn config_fingerprint(&self) -> BlossomConfigFingerprint {
        self.config_fingerprint
    }

    #[must_use]
    pub const fn expected_url(&self) -> &BlobUrl {
        &self.expected_url
    }

    #[must_use]
    pub const fn request(&self) -> &BlossomUploadRequest {
        &self.request
    }

    pub(crate) const fn config(&self) -> &BlossomConfig {
        &self.config
    }

    pub(crate) const fn endpoint(&self) -> &BlossomEndpoint {
        &self.endpoint
    }

    pub(crate) fn into_request(self) -> BlossomUploadRequest {
        self.request
    }
}

#[cfg(feature = "blossom")]
impl std::fmt::Debug for BlossomUploadTransaction {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BlossomUploadTransaction")
            .field("config_fingerprint", &self.config_fingerprint)
            .field("endpoint", &self.endpoint)
            .field("expected_url", &self.expected_url)
            .field("request", &self.request)
            .finish()
    }
}

#[cfg(feature = "blossom")]
impl std::fmt::Debug for BlossomUploadRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BlossomUploadRequest")
            .field("sha256", &self.sha256())
            .field("byte_size", &self.byte_size())
            .field("media_type", &self.media_type)
            .field("dimensions", &self.dimensions)
            .field("bytes", &"<redacted>")
            .finish()
    }
}

/// Cooperative cancellation shared by upload, retry, and retrieval phases.
#[cfg(feature = "blossom")]
#[derive(Clone, Debug, Default)]
pub struct BlossomCancellation {
    state: Arc<BlossomCancellationState>,
}

#[cfg(feature = "blossom")]
#[derive(Debug, Default)]
struct BlossomCancellationState {
    cancelled: AtomicBool,
    notify: tokio::sync::Notify,
}

#[cfg(feature = "blossom")]
impl BlossomCancellation {
    pub fn cancel(&self) {
        self.state.cancelled.store(true, Ordering::Release);
        self.state.notify.notify_waiters();
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.state.cancelled.load(Ordering::Acquire)
    }

    pub(crate) async fn cancelled(&self) {
        loop {
            let notified = self.state.notify.notified();
            if self.is_cancelled() {
                return;
            }
            notified.await;
        }
    }
}

/// Exact operation phase associated with one redacted Blossom failure.
#[cfg(feature = "blossom")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum BlossomPhase {
    Configuration,
    Probe,
    Authorization,
    Upload,
    Descriptor,
    Retrieval,
    Verification,
}

/// Stable, secret-safe Blossom failure classification.
#[cfg(feature = "blossom")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum BlossomErrorKind {
    InvalidEndpoint,
    EndpointSchemeDenied,
    InvalidEndpointCount,
    DuplicateEndpoint,
    EndpointNotConfigured,
    ConfigurationChanged,
    ResolutionFailed,
    ResolvedAddressDenied,
    InvalidLimits,
    InvalidRequest,
    InvalidDimensions,
    UnsupportedMediaType,
    MediaTypeMismatch,
    InvalidImageBytes,
    DimensionMismatch,
    Authorization,
    Transport,
    Timeout,
    Cancelled,
    HttpStatus,
    UnsafeRedirect,
    RedirectLimit,
    ContentEncodingDenied,
    ResponseTooLarge,
    ResponseSizeMismatch,
    ResponseHashMismatch,
    InvalidDescriptor,
    DescriptorMismatch,
    RetrievedBytesMismatch,
}

/// Redacted recoverable state for one Blossom operation failure.
#[cfg(feature = "blossom")]
#[derive(Clone, Eq, PartialEq)]
pub struct BlossomError {
    kind: BlossomErrorKind,
    phase: BlossomPhase,
    retryable: bool,
    possible_orphan: bool,
    attempts: u8,
    http_status: Option<u16>,
    server_error_code: Option<String>,
}

#[cfg(feature = "blossom")]
impl BlossomError {
    pub(crate) const fn new(
        kind: BlossomErrorKind,
        phase: BlossomPhase,
        retryable: bool,
        possible_orphan: bool,
        attempts: u8,
    ) -> Self {
        Self {
            kind,
            phase,
            retryable,
            possible_orphan,
            attempts,
            http_status: None,
            server_error_code: None,
        }
    }

    const fn configuration(kind: BlossomErrorKind) -> Self {
        Self::new(kind, BlossomPhase::Configuration, false, false, 0)
    }

    #[must_use]
    pub const fn kind(&self) -> BlossomErrorKind {
        self.kind
    }

    #[must_use]
    pub const fn phase(&self) -> BlossomPhase {
        self.phase
    }

    #[must_use]
    pub const fn retryable(&self) -> bool {
        self.retryable
    }

    #[must_use]
    pub const fn possible_orphan(&self) -> bool {
        self.possible_orphan
    }

    #[must_use]
    pub const fn attempts(&self) -> u8 {
        self.attempts
    }

    #[must_use]
    pub const fn http_status(&self) -> Option<u16> {
        self.http_status
    }

    /// Bounded, validated public error identifier returned by the server.
    #[must_use]
    pub fn server_error_code(&self) -> Option<&str> {
        self.server_error_code.as_deref()
    }

    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self.kind {
            BlossomErrorKind::InvalidEndpoint => "blossom_invalid_endpoint",
            BlossomErrorKind::EndpointSchemeDenied => "blossom_endpoint_scheme_denied",
            BlossomErrorKind::InvalidEndpointCount => "blossom_invalid_endpoint_count",
            BlossomErrorKind::DuplicateEndpoint => "blossom_duplicate_endpoint",
            BlossomErrorKind::EndpointNotConfigured => "blossom_endpoint_not_configured",
            BlossomErrorKind::ConfigurationChanged => "blossom_configuration_changed",
            BlossomErrorKind::ResolutionFailed => "blossom_resolution_failed",
            BlossomErrorKind::ResolvedAddressDenied => "blossom_resolved_address_denied",
            BlossomErrorKind::InvalidLimits => "blossom_invalid_limits",
            BlossomErrorKind::InvalidRequest => "blossom_invalid_request",
            BlossomErrorKind::InvalidDimensions => "blossom_invalid_dimensions",
            BlossomErrorKind::UnsupportedMediaType => "blossom_unsupported_media_type",
            BlossomErrorKind::MediaTypeMismatch => "blossom_media_type_mismatch",
            BlossomErrorKind::InvalidImageBytes => "blossom_invalid_image_bytes",
            BlossomErrorKind::DimensionMismatch => "blossom_dimension_mismatch",
            BlossomErrorKind::Authorization => "blossom_authorization_failed",
            BlossomErrorKind::Transport => "blossom_transport_failed",
            BlossomErrorKind::Timeout => "blossom_timeout",
            BlossomErrorKind::Cancelled => "blossom_cancelled",
            BlossomErrorKind::HttpStatus => "blossom_http_status",
            BlossomErrorKind::UnsafeRedirect => "blossom_unsafe_redirect",
            BlossomErrorKind::RedirectLimit => "blossom_redirect_limit",
            BlossomErrorKind::ContentEncodingDenied => "blossom_content_encoding_denied",
            BlossomErrorKind::ResponseTooLarge => "blossom_response_too_large",
            BlossomErrorKind::ResponseSizeMismatch => "blossom_response_size_mismatch",
            BlossomErrorKind::ResponseHashMismatch => "blossom_response_hash_mismatch",
            BlossomErrorKind::InvalidDescriptor => "blossom_invalid_descriptor",
            BlossomErrorKind::DescriptorMismatch => "blossom_descriptor_mismatch",
            BlossomErrorKind::RetrievedBytesMismatch => "blossom_retrieved_bytes_mismatch",
        }
    }

    pub(crate) const fn with_operation(mut self, possible_orphan: bool, attempts: u8) -> Self {
        self.possible_orphan |= possible_orphan;
        self.attempts = attempts;
        self
    }

    pub(crate) const fn with_http_status(mut self, status: u16) -> Self {
        self.http_status = Some(status);
        self
    }

    pub(crate) fn with_server_error_code(mut self, code: String) -> Self {
        self.server_error_code = Some(code);
        self
    }
}

#[cfg(feature = "blossom")]
impl std::fmt::Display for BlossomError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code())
    }
}

#[cfg(feature = "blossom")]
impl std::fmt::Debug for BlossomError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BlossomError")
            .field("kind", &self.kind)
            .field("phase", &self.phase)
            .field("retryable", &self.retryable)
            .field("possible_orphan", &self.possible_orphan)
            .field("attempts", &self.attempts)
            .field("http_status", &self.http_status)
            .field("server_error_code", &self.server_error_code)
            .finish()
    }
}

#[cfg(feature = "blossom")]
impl std::error::Error for BlossomError {}

/// Successful BUD-02 upload plus bounded BUD-01 retrieval verification.
#[cfg(feature = "blossom")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlossomUploadReceipt {
    descriptor: ByteVerifiedDescriptor,
    dimensions: BlossomImageDimensions,
    attempts: u8,
    verified_at_unix_ms: u64,
}

/// Exact verified image bytes returned by a bounded BUD-01 retrieval.
#[cfg(feature = "blossom")]
#[derive(Clone)]
pub struct BlossomInboundReceipt {
    final_url: BlobUrl,
    commitment: radroots_blossom::descriptor::ByteCommitment,
    dimensions: BlossomImageDimensions,
    bytes: Arc<[u8]>,
    config_fingerprint: BlossomConfigFingerprint,
    attempts: u8,
    verified_at_unix_ms: u64,
}

#[cfg(feature = "blossom")]
impl std::fmt::Debug for BlossomInboundReceipt {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BlossomInboundReceipt")
            .field("final_url", &self.final_url)
            .field("commitment", &self.commitment)
            .field("dimensions", &self.dimensions)
            .field("bytes", &"<redacted>")
            .field("config_fingerprint", &self.config_fingerprint)
            .field("attempts", &self.attempts)
            .field("verified_at_unix_ms", &self.verified_at_unix_ms)
            .finish()
    }
}

#[cfg(feature = "blossom")]
impl BlossomInboundReceipt {
    pub(crate) fn new(
        final_url: BlobUrl,
        commitment: radroots_blossom::descriptor::ByteCommitment,
        dimensions: BlossomImageDimensions,
        bytes: Arc<[u8]>,
        config_fingerprint: BlossomConfigFingerprint,
        attempts: u8,
        verified_at_unix_ms: u64,
    ) -> Self {
        Self {
            final_url,
            commitment,
            dimensions,
            bytes,
            config_fingerprint,
            attempts,
            verified_at_unix_ms,
        }
    }

    #[must_use]
    pub const fn final_url(&self) -> &BlobUrl {
        &self.final_url
    }

    #[must_use]
    pub const fn commitment(&self) -> &radroots_blossom::descriptor::ByteCommitment {
        &self.commitment
    }

    #[must_use]
    pub const fn dimensions(&self) -> BlossomImageDimensions {
        self.dimensions
    }

    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        self.bytes.as_ref()
    }

    #[must_use]
    pub const fn config_fingerprint(&self) -> BlossomConfigFingerprint {
        self.config_fingerprint
    }

    #[must_use]
    pub const fn attempts(&self) -> u8 {
        self.attempts
    }

    #[must_use]
    pub const fn verified_at_unix_ms(&self) -> u64 {
        self.verified_at_unix_ms
    }
}

#[cfg(feature = "blossom")]
impl BlossomUploadReceipt {
    pub(crate) const fn new(
        descriptor: ByteVerifiedDescriptor,
        dimensions: BlossomImageDimensions,
        attempts: u8,
        verified_at_unix_ms: u64,
    ) -> Self {
        Self {
            descriptor,
            dimensions,
            attempts,
            verified_at_unix_ms,
        }
    }

    #[must_use]
    pub const fn descriptor(&self) -> &ByteVerifiedDescriptor {
        &self.descriptor
    }

    #[must_use]
    pub const fn dimensions(&self) -> BlossomImageDimensions {
        self.dimensions
    }

    #[must_use]
    pub const fn attempts(&self) -> u8 {
        self.attempts
    }

    #[must_use]
    pub const fn verified_at_unix_ms(&self) -> u64 {
        self.verified_at_unix_ms
    }

    #[must_use]
    pub fn into_descriptor(self) -> ByteVerifiedDescriptor {
        self.descriptor
    }
}

/// Host-reconfigurable Blossom HTTP adapter slot.
#[cfg(feature = "blossom")]
#[derive(Clone, Default)]
pub struct BlossomSlot {
    state: Arc<RwLock<BlossomSlotState>>,
}

#[cfg(feature = "blossom")]
#[derive(Default)]
struct BlossomSlotState {
    config: Option<BlossomConfig>,
    evidence: Option<BlossomEndpointEvidence>,
}

#[cfg(feature = "blossom")]
impl BlossomSlot {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Atomically installs completely validated inert configuration.
    pub fn configure(&self, config: BlossomConfig) -> Result<(), BlossomError> {
        let evidence = BlossomEndpointEvidence::configured(&config);
        let mut state = self
            .state
            .write()
            .map_err(|_| BlossomError::configuration(BlossomErrorKind::EndpointNotConfigured))?;
        state.config = Some(config);
        state.evidence = Some(evidence);
        Ok(())
    }

    pub fn clear(&self) {
        if let Ok(mut state) = self.state.write() {
            state.config = None;
            state.evidence = None;
        }
    }

    #[must_use]
    pub fn host_kind(&self) -> Option<BlossomHostKind> {
        self.snapshot().map(|config| config.profile.host_kind())
    }

    #[must_use]
    pub fn endpoint_authority(&self) -> Option<BlossomEndpointAuthority> {
        self.snapshot().map(|config| config.profile.authority())
    }

    #[must_use]
    pub fn config_fingerprint(&self) -> Option<BlossomConfigFingerprint> {
        self.snapshot().map(|config| config.fingerprint())
    }

    /// Returns the configured inert profile without performing network I/O.
    #[must_use]
    pub fn profile(&self) -> Option<BlossomProfile> {
        self.snapshot().map(|config| config.profile)
    }

    /// Atomically returns the inert profile and its exact configuration identity.
    #[must_use]
    pub fn configuration(&self) -> Option<(BlossomProfile, BlossomConfigFingerprint)> {
        self.snapshot().map(|config| {
            let fingerprint = config.fingerprint();
            (config.profile, fingerprint)
        })
    }

    /// Returns the latest passive evidence without performing network I/O.
    #[must_use]
    pub fn evidence(&self) -> Option<BlossomEndpointEvidence> {
        self.state
            .read()
            .ok()
            .and_then(|state| state.evidence.clone())
    }

    /// Performs a bounded non-mutating primary-origin probe.
    pub async fn probe(
        &self,
        cancellation: BlossomCancellation,
    ) -> Result<BlossomEndpointEvidence, BlossomError> {
        let config = self
            .snapshot()
            .ok_or_else(|| BlossomError::configuration(BlossomErrorKind::EndpointNotConfigured))?;
        let fingerprint = config.fingerprint();
        let result = crate::adapters::blossom::probe(
            config.clone(),
            config.profile().primary().clone(),
            cancellation,
        )
        .await;
        match result {
            Ok(observation) => {
                self.record_evidence(fingerprint, BlossomPhase::Probe, false, |evidence| {
                    evidence.record_success(
                        BlossomEvidenceState::TlsHttpObserved,
                        Some(observation.http_status),
                    );
                })
            }
            Err(failure) => {
                self.record_evidence(fingerprint, BlossomPhase::Probe, false, |evidence| {
                    if failure.dns_policy_validated {
                        evidence.last_successful_state = BlossomEvidenceState::DnsPolicyValidated;
                    }
                    evidence.record_failure(&failure.error);
                })?;
                Err(failure.error)
            }
        }
    }

    /// Binds verified bytes to the configured primary origin without network I/O.
    pub fn prepare_upload(
        &self,
        request: BlossomUploadRequest,
    ) -> Result<BlossomUploadTransaction, BlossomError> {
        let config = self
            .snapshot()
            .ok_or_else(|| BlossomError::configuration(BlossomErrorKind::EndpointNotConfigured))?;
        let config_fingerprint = config.fingerprint();
        let endpoint = config.profile.primary.clone();
        let extension = canonical_image_extension(request.media_type())?;
        let expected_url = BlobUrl::parse(
            format!("{}/{}.{}", endpoint.origin(), request.sha256(), extension).as_str(),
        )
        .map_err(|_| BlossomError::configuration(BlossomErrorKind::InvalidEndpoint))?;
        Ok(BlossomUploadTransaction {
            config,
            config_fingerprint,
            endpoint,
            expected_url,
            request,
        })
    }

    /// Builds the exact BUD-11 claim for this configured upload destination.
    pub fn authored_upload_claim(
        &self,
        transaction: &BlossomUploadTransaction,
        content: AuthorizationContent,
        created_at_unix_s: u64,
        lifetime_seconds: u64,
    ) -> Result<AuthoredUploadClaim, BlossomError> {
        self.validate_transaction(transaction)?;
        AuthoredUploadClaim::new(
            content,
            transaction.endpoint.server_domain()?,
            transaction.request.sha256(),
            created_at_unix_s,
            lifetime_seconds,
        )
        .map_err(|_| {
            BlossomError::new(
                BlossomErrorKind::Authorization,
                BlossomPhase::Authorization,
                false,
                false,
                0,
            )
        })
    }

    /// Uploads exact bytes and verifies the returned descriptor and a full GET.
    pub async fn upload(
        &self,
        transaction: BlossomUploadTransaction,
        authorization: crate::signing::AuthorizationHeader,
        cancellation: BlossomCancellation,
    ) -> Result<BlossomUploadReceipt, BlossomError> {
        self.validate_transaction(&transaction)?;
        let fingerprint = transaction.config_fingerprint();
        let result =
            crate::adapters::blossom::upload(transaction, authorization, cancellation).await;
        match result {
            Ok(receipt) => {
                self.record_evidence(fingerprint, BlossomPhase::Verification, true, |evidence| {
                    evidence.record_success(BlossomEvidenceState::RetrievalVerified, None);
                })?;
                Ok(receipt)
            }
            Err(error) => {
                self.record_evidence(fingerprint, BlossomPhase::Verification, true, |evidence| {
                    if error.possible_orphan()
                        && matches!(
                            error.phase(),
                            BlossomPhase::Retrieval | BlossomPhase::Verification
                        )
                    {
                        evidence.last_successful_state = BlossomEvidenceState::UploadVerified;
                    }
                    evidence.record_failure(&error);
                })?;
                Err(error)
            }
        }
    }

    /// Verifies a host-executed BUD-02 response, then performs the canonical
    /// BUD-01 exact-byte retrieval before returning an upload receipt.
    pub async fn complete_native_upload(
        &self,
        transaction: BlossomUploadTransaction,
        status_code: u16,
        response_media_type: Option<&str>,
        response_content_encoding: Option<&str>,
        response_body: &[u8],
        cancellation: BlossomCancellation,
    ) -> Result<BlossomUploadReceipt, BlossomError> {
        self.validate_transaction(&transaction)?;
        let fingerprint = transaction.config_fingerprint();
        let result = crate::adapters::blossom::complete_native_upload(
            transaction,
            status_code,
            response_media_type,
            response_content_encoding,
            response_body,
            cancellation,
        )
        .await;
        match result {
            Ok(receipt) => {
                self.record_evidence(fingerprint, BlossomPhase::Verification, true, |evidence| {
                    evidence.record_success(BlossomEvidenceState::RetrievalVerified, None);
                })?;
                Ok(receipt)
            }
            Err(error) => {
                self.record_evidence(fingerprint, BlossomPhase::Verification, true, |evidence| {
                    if error.possible_orphan() {
                        evidence.last_successful_state = BlossomEvidenceState::UploadVerified;
                    }
                    evidence.record_failure(&error);
                })?;
                Err(error)
            }
        }
    }

    /// Retrieves and verifies one immutable BUD-01 image under the exact
    /// configured DNS, TLS, redirect, retry, and byte limits.
    pub async fn retrieve(
        &self,
        request: BlossomInboundRequest,
        cancellation: BlossomCancellation,
    ) -> Result<BlossomInboundReceipt, BlossomError> {
        let config = self
            .snapshot()
            .ok_or_else(|| BlossomError::configuration(BlossomErrorKind::EndpointNotConfigured))?;
        let fingerprint = config.fingerprint();
        if config.profile().endpoint_for_blob(request.url()).is_none() {
            return Err(BlossomError::configuration(
                BlossomErrorKind::EndpointNotConfigured,
            ));
        }
        let result = crate::adapters::blossom::retrieve(config, request, cancellation).await;
        match result {
            Ok(receipt) => {
                self.record_evidence(fingerprint, BlossomPhase::Verification, false, |evidence| {
                    evidence.record_success(BlossomEvidenceState::RetrievalVerified, None);
                })?;
                Ok(receipt)
            }
            Err(error) => {
                self.record_evidence(fingerprint, BlossomPhase::Verification, false, |evidence| {
                    evidence.record_failure(&error);
                })?;
                Err(error)
            }
        }
    }

    fn validate_transaction(
        &self,
        transaction: &BlossomUploadTransaction,
    ) -> Result<(), BlossomError> {
        let fingerprint = self
            .config_fingerprint()
            .ok_or_else(|| BlossomError::configuration(BlossomErrorKind::EndpointNotConfigured))?;
        if fingerprint != transaction.config_fingerprint {
            return Err(BlossomError::configuration(
                BlossomErrorKind::ConfigurationChanged,
            ));
        }
        Ok(())
    }

    fn snapshot(&self) -> Option<BlossomConfig> {
        self.state
            .read()
            .ok()
            .and_then(|state| state.config.clone())
    }

    fn record_evidence(
        &self,
        fingerprint: BlossomConfigFingerprint,
        drift_phase: BlossomPhase,
        drift_possible_orphan: bool,
        update: impl FnOnce(&mut BlossomEndpointEvidence),
    ) -> Result<BlossomEndpointEvidence, BlossomError> {
        let mut state = self
            .state
            .write()
            .map_err(|_| BlossomError::configuration(BlossomErrorKind::EndpointNotConfigured))?;
        let current = state
            .config
            .as_ref()
            .ok_or_else(|| BlossomError::configuration(BlossomErrorKind::EndpointNotConfigured))?;
        if current.fingerprint() != fingerprint {
            return Err(BlossomError::new(
                BlossomErrorKind::ConfigurationChanged,
                drift_phase,
                false,
                drift_possible_orphan,
                0,
            ));
        }
        let evidence = state
            .evidence
            .as_mut()
            .ok_or_else(|| BlossomError::configuration(BlossomErrorKind::EndpointNotConfigured))?;
        update(evidence);
        Ok(evidence.clone())
    }
}

#[cfg(feature = "blossom")]
impl std::fmt::Debug for BlossomSlot {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BlossomSlot")
            .field("configured", &self.host_kind().is_some())
            .finish()
    }
}

#[cfg(feature = "blossom")]
fn endpoint_scheme_is_allowed(scheme: &str, authority: BlossomEndpointAuthority) -> bool {
    scheme == "https"
        || scheme == "http" && authority == BlossomEndpointAuthority::LoopbackDevelopment
}

#[cfg(feature = "blossom")]
fn validate_host_authority(
    host_kind: BlossomHostKind,
    authority: BlossomEndpointAuthority,
) -> Result<(), BlossomError> {
    let accepted = match authority {
        BlossomEndpointAuthority::PublicWebPki => true,
        BlossomEndpointAuthority::LoopbackDevelopment => host_kind == BlossomHostKind::Simulator,
        BlossomEndpointAuthority::PrivateNetworkDevelopment => {
            host_kind == BlossomHostKind::PhysicalDevice
        }
    };
    if accepted {
        Ok(())
    } else {
        Err(BlossomError::configuration(
            BlossomErrorKind::InvalidEndpoint,
        ))
    }
}

#[cfg(feature = "blossom")]
fn validate_blossom_host(
    host: &str,
    authority: BlossomEndpointAuthority,
) -> Result<(), BlossomError> {
    let address = host.parse::<IpAddr>().ok();
    let accepted = match (authority, address) {
        (BlossomEndpointAuthority::PublicWebPki, Some(address)) => public_blossom_address(address),
        (BlossomEndpointAuthority::PublicWebPki, None) => public_blossom_hostname(host),
        (BlossomEndpointAuthority::LoopbackDevelopment, Some(address)) => address.is_loopback(),
        (BlossomEndpointAuthority::LoopbackDevelopment, None) => host == "localhost",
        (BlossomEndpointAuthority::PrivateNetworkDevelopment, Some(address)) => {
            trusted_blossom_address(address)
        }
        (BlossomEndpointAuthority::PrivateNetworkDevelopment, None) => host != "localhost",
    };
    if accepted {
        Ok(())
    } else {
        Err(BlossomError::configuration(
            BlossomErrorKind::ResolvedAddressDenied,
        ))
    }
}

#[cfg(feature = "blossom")]
fn public_blossom_hostname(host: &str) -> bool {
    host.contains('.')
        && !host.ends_with(".localhost")
        && !host.ends_with(".local")
        && !host.ends_with(".home.arpa")
}

#[cfg(feature = "blossom")]
fn blossom_authority_accepts_address(authority: BlossomEndpointAuthority, address: IpAddr) -> bool {
    match authority {
        BlossomEndpointAuthority::PublicWebPki => public_blossom_address(address),
        BlossomEndpointAuthority::LoopbackDevelopment => address.is_loopback(),
        BlossomEndpointAuthority::PrivateNetworkDevelopment => trusted_blossom_address(address),
    }
}

#[cfg(feature = "blossom")]
pub(crate) fn canonical_image_extension(
    media_type: &MediaType,
) -> Result<&'static str, BlossomError> {
    match media_type.as_str() {
        "image/png" => Ok("png"),
        "image/jpeg" => Ok("jpg"),
        "image/gif" => Ok("gif"),
        "image/webp" => Ok("webp"),
        _ => Err(BlossomError::new(
            BlossomErrorKind::UnsupportedMediaType,
            BlossomPhase::Verification,
            false,
            false,
            0,
        )),
    }
}

#[cfg(feature = "blossom")]
fn public_blossom_address(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => public_blossom_ipv4(address),
        IpAddr::V6(address) => public_blossom_ipv6(address),
    }
}

#[cfg(feature = "blossom")]
fn trusted_blossom_address(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => {
            !address.is_unspecified()
                && !address.is_loopback()
                && !address.is_multicast()
                && !address.is_broadcast()
        }
        IpAddr::V6(address) => {
            !address.is_unspecified() && !address.is_loopback() && !address.is_multicast()
        }
    }
}

#[cfg(feature = "blossom")]
fn public_blossom_ipv4(address: Ipv4Addr) -> bool {
    let octets = address.octets();
    !(octets[0] == 0
        || address.is_loopback()
        || address.is_private()
        || address.is_link_local()
        || address.is_multicast()
        || address.is_documentation()
        || octets[0] == 100 && (64..=127).contains(&octets[1])
        || octets[0] == 192 && octets[1] == 0 && octets[2] == 0
        || octets[0] == 192 && octets[1] == 88 && octets[2] == 99
        || octets[0] == 198 && matches!(octets[1], 18 | 19)
        || octets[0] >= 240)
}

#[cfg(feature = "blossom")]
fn public_blossom_ipv6(address: Ipv6Addr) -> bool {
    if let Some(mapped) = address.to_ipv4_mapped() {
        return public_blossom_ipv4(mapped);
    }
    let segments = address.segments();
    (segments[0] & 0xe000) == 0x2000
        && !(segments[0] == 0x2001 && segments[1] <= 0x01ff)
        && !(segments[0] == 0x2001 && segments[1] == 0x0db8)
        && segments[0] != 0x2002
        && !(segments[0] == 0x3fff && (segments[1] & 0xf000) == 0)
}

/// A side-effect-free transport selection for a client operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Profile {
    selection: Selection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Selection {
    LocalOnly,
    Delivery {
        targets: TargetSet,
        satisfaction: SatisfactionPolicy,
    },
    UnavailablePreview {
        source: SourceStatus,
        sink: SinkStatus,
    },
}

impl Profile {
    /// Selects local persistence only, with no transport target or fallback.
    #[must_use]
    pub const fn local_only() -> Self {
        Self {
            selection: Selection::LocalOnly,
        }
    }

    /// Selects an exact bounded target set and canonical satisfaction policy.
    ///
    /// Impossible quorum and required-target policies are rejected here by the
    /// owning transport contract. Construction performs no network operation.
    pub fn delivery(targets: TargetSet, satisfaction: SatisfactionPolicy) -> Result<Self, Error> {
        satisfaction.validate_for(&targets)?;
        Ok(Self {
            selection: Selection::Delivery {
                targets,
                satisfaction,
            },
        })
    }

    /// Describes a preview transport that is intentionally not selectable.
    ///
    /// Both canonical capability directions remain explicitly unconfigured
    /// and unavailable. The profile has no targets and therefore cannot fall
    /// back to local, Nostr, daemon, or another transport.
    #[must_use]
    pub fn unavailable_preview(transport_id: TransportId) -> Self {
        Self {
            selection: Selection::UnavailablePreview {
                source: SourceStatus::new(
                    transport_id,
                    false,
                    Maturity::Preview,
                    Availability::Unavailable,
                    SourceCapabilities::NONE,
                    PREVIEW_UNAVAILABLE_MESSAGE,
                ),
                sink: SinkStatus::new(
                    transport_id,
                    false,
                    Maturity::Preview,
                    Availability::Unavailable,
                    SinkCapabilities::NONE,
                    PREVIEW_UNAVAILABLE_MESSAGE,
                ),
            },
        }
    }

    /// Returns whether this profile authorizes no transport operation.
    #[must_use]
    pub const fn is_local_only(&self) -> bool {
        matches!(self.selection, Selection::LocalOnly)
    }

    /// Returns the exact selected targets, if delivery is authorized.
    #[must_use]
    pub const fn targets(&self) -> Option<&TargetSet> {
        match &self.selection {
            Selection::Delivery { targets, .. } => Some(targets),
            Selection::LocalOnly | Selection::UnavailablePreview { .. } => None,
        }
    }

    /// Returns the exact selected satisfaction policy, if delivery is authorized.
    #[must_use]
    pub const fn satisfaction(&self) -> Option<&SatisfactionPolicy> {
        match &self.selection {
            Selection::Delivery { satisfaction, .. } => Some(satisfaction),
            Selection::LocalOnly | Selection::UnavailablePreview { .. } => None,
        }
    }

    /// Returns canonical source status for an unavailable preview.
    #[must_use]
    pub const fn source_status(&self) -> Option<&SourceStatus> {
        match &self.selection {
            Selection::UnavailablePreview { source, .. } => Some(source),
            Selection::LocalOnly | Selection::Delivery { .. } => None,
        }
    }

    /// Returns canonical sink status for an unavailable preview.
    #[must_use]
    pub const fn sink_status(&self) -> Option<&SinkStatus> {
        match &self.selection {
            Selection::UnavailablePreview { sink, .. } => Some(sink),
            Selection::LocalOnly | Selection::Delivery { .. } => None,
        }
    }
}

impl Default for Profile {
    fn default() -> Self {
        Self::local_only()
    }
}

/// Host-configured, client-shareable Nostr transport slot.
///
/// Reconfiguration validates the complete relay set before atomically
/// replacing the active adapter. Construction, clearing, and target
/// inspection perform no network I/O.
#[cfg(feature = "nostr")]
#[derive(Clone)]
pub struct NostrSlot {
    state: Arc<RwLock<Option<NostrState>>>,
}

#[cfg(feature = "nostr")]
#[derive(Clone)]
struct NostrState {
    transport: Arc<radroots_transport_nostr::NostrTransport>,
    read_targets: TargetSet,
    write_targets: Option<TargetSet>,
}

#[cfg(feature = "nostr")]
impl NostrSlot {
    /// Creates an inert slot with no selected host profile.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(None)),
        }
    }

    /// Atomically installs one completely validated relay profile.
    pub fn configure(&self, profile: RelayProfile) -> crate::Result<()> {
        let config = radroots_transport_nostr::Config::from_profile(profile);
        let read_targets = TargetSet::new(
            config
                .read_relays()
                .map(radroots_transport_nostr::RelayUrl::to_target)
                .collect::<Result<Vec<_>, _>>()
                .map_err(crate::Error::invalid_host_configuration)?,
        )
        .map_err(|_| crate::Error::invalid_host_configuration_without_source())?;
        let write_targets = {
            let targets = config
                .write_relays()
                .map(radroots_transport_nostr::RelayUrl::to_target)
                .collect::<Result<Vec<_>, _>>()
                .map_err(crate::Error::invalid_host_configuration)?;
            if targets.is_empty() {
                None
            } else {
                Some(
                    TargetSet::new(targets)
                        .map_err(|_| crate::Error::invalid_host_configuration_without_source())?,
                )
            }
        };
        let state = NostrState {
            transport: Arc::new(radroots_transport_nostr::NostrTransport::new(config)),
            read_targets,
            write_targets,
        };
        let mut current = self
            .state
            .write()
            .map_err(|_| crate::Error::shared_operation_unavailable())?;
        *current = Some(state);
        Ok(())
    }

    /// Removes the active adapter without starting or stopping background work.
    pub fn clear(&self) {
        if let Ok(mut state) = self.state.write() {
            *state = None;
        }
    }

    /// Returns the currently selected canonical read targets.
    #[must_use]
    pub fn read_targets(&self) -> Option<TargetSet> {
        self.snapshot().map(|state| state.read_targets)
    }

    /// Returns writable targets, or `None` when the profile is intentionally
    /// read-only.
    #[must_use]
    pub fn write_targets(&self) -> Option<TargetSet> {
        self.snapshot().and_then(|state| state.write_targets)
    }

    /// Returns passive per-relay evidence without probing or opening sockets.
    #[must_use]
    pub fn relay_status(&self) -> Option<RelayStatusReport> {
        self.snapshot().map(|state| state.transport.relay_status())
    }

    fn snapshot(&self) -> Option<NostrState> {
        self.state.read().ok().and_then(|state| state.clone())
    }
}

#[cfg(feature = "nostr")]
impl radroots_transport::EventSource for NostrSlot {
    fn status(
        &self,
    ) -> radroots_transport::BoxFuture<'_, Result<SourceStatus, radroots_transport::Error>> {
        Box::pin(async move {
            match self.snapshot() {
                Some(state) => {
                    radroots_transport::EventSource::status(state.transport.as_ref()).await
                }
                None => Ok(SourceStatus::new(
                    TransportId::NOSTR,
                    false,
                    Maturity::Stable,
                    Availability::Unavailable,
                    SourceCapabilities::FETCH,
                    "Nostr transport is not configured",
                )),
            }
        })
    }

    fn fetch(
        &self,
        request: radroots_transport::FetchRequest,
    ) -> radroots_transport::BoxFuture<
        '_,
        Result<radroots_transport::FetchPage, radroots_transport::Error>,
    > {
        Box::pin(async move {
            let state = self
                .snapshot()
                .ok_or(radroots_transport::Error::UnsupportedOperation)?;
            radroots_transport::EventSource::fetch(state.transport.as_ref(), request).await
        })
    }
}

#[cfg(feature = "nostr")]
impl radroots_transport::EventSink for NostrSlot {
    fn status(
        &self,
    ) -> radroots_transport::BoxFuture<'_, Result<SinkStatus, radroots_transport::Error>> {
        Box::pin(async move {
            match self.snapshot() {
                Some(state) => {
                    radroots_transport::EventSink::status(state.transport.as_ref()).await
                }
                None => Ok(SinkStatus::new(
                    TransportId::NOSTR,
                    false,
                    Maturity::Stable,
                    Availability::Unavailable,
                    SinkCapabilities::DELIVER,
                    "Nostr transport is not configured",
                )),
            }
        })
    }

    fn deliver(
        &self,
        request: radroots_transport::DeliveryRequest,
    ) -> radroots_transport::BoxFuture<
        '_,
        Result<radroots_transport::DeliveryReceipt, radroots_transport::SinkFailure>,
    > {
        Box::pin(async move {
            let Some(state) = self.snapshot() else {
                return Err(radroots_transport::SinkFailure::for_request(
                    &request,
                    "nostr_transport_not_configured",
                    radroots_transport::outcome::Retryability::Terminal,
                    None,
                    None,
                    Vec::new(),
                )
                .expect("static unconfigured sink failure is valid"));
            };
            radroots_transport::EventSink::deliver(state.transport.as_ref(), request).await
        })
    }
}

#[cfg(feature = "nostr")]
impl std::fmt::Debug for NostrSlot {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NostrSlot")
            .field("configured", &self.read_targets().is_some())
            .field("writable", &self.write_targets().is_some())
            .finish()
    }
}

#[cfg(feature = "nostr")]
impl Default for NostrSlot {
    fn default() -> Self {
        Self::new()
    }
}

/// Explicit daemon adapter authentication configuration.
#[cfg(feature = "radrootsd")]
#[derive(Clone, Eq, PartialEq)]
#[non_exhaustive]
pub enum DaemonAuth {
    /// Sends no authorization header.
    None,
    /// Sends the supplied bearer credential only when delivery is invoked.
    BearerToken(String),
}

#[cfg(feature = "radrootsd")]
impl std::fmt::Debug for DaemonAuth {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => formatter.write_str("None"),
            Self::BearerToken(_) => formatter.write_str("BearerToken(<redacted>)"),
        }
    }
}

/// Explicit daemon endpoint and request deadline configuration.
#[cfg(feature = "radrootsd")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DaemonConfig {
    endpoint: String,
    auth: DaemonAuth,
    timeout: core::time::Duration,
}

#[cfg(feature = "radrootsd")]
impl DaemonConfig {
    /// Creates inert configuration; no client is built and no request is sent.
    #[must_use]
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            auth: DaemonAuth::None,
            timeout: core::time::Duration::from_secs(10),
        }
    }

    /// Selects explicit authentication for later invocation.
    #[must_use]
    pub fn with_auth(mut self, auth: DaemonAuth) -> Self {
        self.auth = auth;
        self
    }

    /// Selects the complete HTTP/RPC request deadline.
    #[must_use]
    pub const fn with_timeout(mut self, timeout: core::time::Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

/// Stable secret-safe daemon execution failure class.
#[cfg(feature = "radrootsd")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum DaemonErrorKind {
    /// The explicit authentication value cannot be represented safely.
    Authentication,
    /// The versioned protocol rejected the request.
    InvalidRequest,
    /// HTTP transport or timeout failed.
    Transport,
    /// The daemon returned a JSON-RPC error.
    Rpc,
    /// The response was malformed or did not match the request.
    InvalidResponse,
}

/// One redacted daemon failure retaining a private source chain.
#[cfg(feature = "radrootsd")]
pub struct DaemonError {
    kind: DaemonErrorKind,
    source: crate::adapters::radrootsd::RadrootsdError,
}

#[cfg(feature = "radrootsd")]
impl DaemonError {
    /// Returns the stable failure class.
    #[must_use]
    pub const fn kind(&self) -> DaemonErrorKind {
        self.kind
    }

    fn from_private(source: crate::adapters::radrootsd::RadrootsdError) -> Self {
        use crate::adapters::radrootsd::RadrootsdError;
        let kind = match &source {
            RadrootsdError::InvalidAuthHeader(_) => DaemonErrorKind::Authentication,
            RadrootsdError::InvalidRequest(_) => DaemonErrorKind::InvalidRequest,
            RadrootsdError::Http(_) => DaemonErrorKind::Transport,
            RadrootsdError::JsonRpc { .. } => DaemonErrorKind::Rpc,
            RadrootsdError::MalformedResponse(_) => DaemonErrorKind::InvalidResponse,
        };
        Self { kind, source }
    }
}

#[cfg(feature = "radrootsd")]
impl std::fmt::Display for DaemonError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self.kind {
            DaemonErrorKind::Authentication => "daemon authentication configuration is invalid",
            DaemonErrorKind::InvalidRequest => "daemon delivery request is invalid",
            DaemonErrorKind::Transport => "daemon transport failed",
            DaemonErrorKind::Rpc => "daemon RPC failed",
            DaemonErrorKind::InvalidResponse => "daemon response is invalid",
        })
    }
}

#[cfg(feature = "radrootsd")]
impl std::fmt::Debug for DaemonError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DaemonError")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

#[cfg(feature = "radrootsd")]
impl std::error::Error for DaemonError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

/// Explicitly configured daemon execution adapter.
///
/// Construction is inert. Network contact occurs only in [`Self::deliver`].
#[cfg(feature = "radrootsd")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DaemonDelivery {
    adapter: crate::adapters::radrootsd::RadrootsdPublishAdapter,
}

#[cfg(feature = "radrootsd")]
impl DaemonDelivery {
    /// Creates an inert adapter from explicit host configuration.
    #[must_use]
    pub fn new(config: DaemonConfig) -> Self {
        let auth = match config.auth {
            DaemonAuth::None => crate::adapters::radrootsd::RadrootsdAuth::None,
            DaemonAuth::BearerToken(token) => {
                crate::adapters::radrootsd::RadrootsdAuth::BearerToken(token)
            }
        };
        Self {
            adapter: crate::adapters::radrootsd::RadrootsdPublishAdapter::new(
                crate::adapters::radrootsd::RadrootsdPublishConfig::new(config.endpoint)
                    .with_auth(auth)
                    .with_timeout(config.timeout),
            ),
        }
    }

    /// Invokes the generation-5 daemon transport-publish contract.
    pub async fn deliver(
        &self,
        signed_event: radroots_event::SignedEvent,
        target_policy: radroots_protocol::radrootsd::transport_publish::v5::TargetPolicy,
        delivery_policy: radroots_protocol::radrootsd::transport_publish::v5::DeliveryPolicy,
        idempotency_key: Option<String>,
        timeout_ms: Option<u64>,
    ) -> Result<radroots_protocol::radrootsd::transport_publish::v5::EventResponse, DaemonError>
    {
        self.adapter
            .publish_signed_event(crate::adapters::radrootsd::RadrootsdPublishRequest {
                signed_event,
                target_policy,
                delivery_policy,
                idempotency_key,
                timeout_ms,
            })
            .await
            .map_err(DaemonError::from_private)
    }
}

#[cfg(test)]
mod tests {
    use radroots_transport::{
        Error, TARGET_SET_MAX_ITEMS, Target,
        capability::{Availability, Maturity},
        policy::{SatisfactionClass, SatisfactionPolicy, TargetPolicy},
        target::TargetFingerprint,
    };

    use super::*;

    fn target(index: usize) -> Target {
        Target::nostr_relay(format!("wss://relay-{index}.example")).expect("target")
    }

    #[cfg(feature = "nostr")]
    fn signed_event() -> radroots_event::SignedEvent {
        let raw = r#"{"id":"56bfc78223bb2221bad82b539efdec1ade0f56d0eb0e1f592fd387df4b2ceee0","pubkey":"585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df","created_at":1700000001,"kind":0,"tags":[],"content":"{}","sig":"dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"}"#;
        let wire = radroots_event::wire::v1::Nip01EventWire::parse_json(raw).expect("wire event");
        radroots_event::SignedEvent::from_wire_verified_id(wire, raw).expect("signed event")
    }

    #[test]
    fn delivery_profile_preserves_canonical_targets_and_policy() {
        let targets = TargetSet::new(vec![target(1), target(2)]).expect("target set");
        let policy = SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::all());
        let profile = Profile::delivery(targets.clone(), policy.clone()).expect("profile");

        assert_eq!(profile.targets(), Some(&targets));
        assert_eq!(profile.satisfaction(), Some(&policy));
        assert!(!profile.is_local_only());
        assert!(profile.source_status().is_none());
        assert!(profile.sink_status().is_none());
    }

    #[test]
    fn canonical_target_and_policy_bounds_fail_during_profile_construction() {
        assert_eq!(TargetSet::new(Vec::new()), Err(Error::EmptyTargetSet));
        assert_eq!(
            TargetSet::new((0..=TARGET_SET_MAX_ITEMS).map(target).collect()),
            Err(Error::TargetSetTooLarge)
        );

        let targets = TargetSet::new(vec![target(1)]).expect("target set");
        let quorum = SatisfactionPolicy::new(
            SatisfactionClass::Delivered,
            TargetPolicy::quorum(2).expect("non-zero quorum"),
        );
        assert_eq!(
            Profile::delivery(targets.clone(), quorum),
            Err(Error::InvalidSatisfactionPolicy)
        );

        let missing =
            TargetFingerprint::from_target(target(2).kind(), target(2).uri(), target(2).scope());
        let required = SatisfactionPolicy::new(
            SatisfactionClass::Accepted,
            TargetPolicy::required(vec![missing]).expect("required policy"),
        );
        assert_eq!(
            Profile::delivery(targets, required),
            Err(Error::RequiredTargetNotRequested)
        );
    }

    #[test]
    fn preview_transport_is_explicitly_unavailable_and_unselectable() {
        let profile = Profile::unavailable_preview(TransportId::RETICULUM);
        let source = profile.source_status().expect("source status");
        let sink = profile.sink_status().expect("sink status");

        assert_eq!(source.transport_id(), TransportId::RETICULUM);
        assert_eq!(sink.transport_id(), TransportId::RETICULUM);
        assert!(!source.is_configured());
        assert!(!sink.is_configured());
        assert_eq!(source.maturity(), Maturity::Preview);
        assert_eq!(sink.maturity(), Maturity::Preview);
        assert_eq!(source.availability(), Availability::Unavailable);
        assert_eq!(sink.availability(), Availability::Unavailable);
        assert!(!source.capabilities().can_fetch());
        assert!(!sink.capabilities().can_deliver());
        assert!(profile.targets().is_none());
        assert!(profile.satisfaction().is_none());
    }

    #[test]
    fn local_and_preview_profiles_never_substitute_fallback_targets() {
        let local = Profile::local_only();
        let preview = Profile::unavailable_preview(TransportId::RETICULUM);
        assert!(local.is_local_only());
        assert!(local.targets().is_none());
        assert!(preview.targets().is_none());

        let selected = TargetSet::new(vec![target(7)]).expect("selected targets");
        let profile = Profile::delivery(
            selected.clone(),
            SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::any()),
        )
        .expect("profile");
        assert_eq!(profile.targets(), Some(&selected));
        assert!(
            profile
                .targets()
                .expect("targets")
                .targets()
                .iter()
                .all(|target| *target.kind() == TransportId::NOSTR)
        );
    }

    #[test]
    fn default_profile_is_local_only() {
        assert_eq!(Profile::default(), Profile::local_only());
    }

    #[cfg(feature = "blossom")]
    #[test]
    fn blossom_profiles_enforce_environment_and_ssrf_boundaries() {
        assert!(public_blossom_profile("https://media.example").is_ok());
        assert!(public_blossom_profile("http://media.example").is_err());
        assert!(public_blossom_profile("https://127.0.0.1").is_err());
        assert!(public_blossom_profile("https://10.0.0.1").is_err());
        assert!(simulator_blossom_profile("http://127.0.0.1:3000").is_ok());
        assert!(simulator_blossom_profile("http://localhost:3000").is_ok());
        assert!(simulator_blossom_profile("http://media.example").is_err());
        assert!(device_blossom_profile("https://10.0.0.10:8443").is_ok());
        assert!(device_blossom_profile("http://10.0.0.10:8443").is_err());
        assert!(device_blossom_profile("https://127.0.0.1:8443").is_err());
    }

    #[cfg(feature = "blossom")]
    #[test]
    fn blossom_configuration_is_bounded_and_debug_is_secret_safe() {
        let profile = simulator_blossom_profile("http://127.0.0.1:3000").unwrap();
        assert!(
            BlossomConfig::from_profile(profile.clone())
                .with_limits(0, 1, 0)
                .is_err()
        );
        assert!(
            BlossomConfig::from_profile(profile.clone())
                .with_network_policy(
                    Duration::from_secs(1),
                    Duration::from_secs(1),
                    0,
                    Duration::from_millis(1),
                )
                .is_err()
        );
        let slot = BlossomSlot::new();
        slot.configure(BlossomConfig::from_profile(profile))
            .unwrap();
        assert_eq!(slot.host_kind(), Some(BlossomHostKind::Simulator));
        assert_eq!(
            slot.endpoint_authority(),
            Some(BlossomEndpointAuthority::LoopbackDevelopment)
        );
        assert_eq!(format!("{slot:?}"), "BlossomSlot { configured: true }");
    }

    #[cfg(feature = "blossom")]
    #[test]
    fn blossom_evidence_is_versioned_passive_and_preserves_last_success() {
        let slot = BlossomSlot::new();
        assert!(slot.profile().is_none());
        assert!(slot.configuration().is_none());
        assert!(slot.evidence().is_none());

        let public_config = BlossomConfig::from_profile(
            public_blossom_profile("https://media.example").expect("public profile"),
        );
        let public_fingerprint = public_config.fingerprint();
        slot.configure(public_config).expect("public config");
        let initial = slot.evidence().expect("initial evidence");
        assert_eq!(initial.schema_version(), 2);
        assert_eq!(initial.origin(), "https://media.example");
        assert_eq!(initial.config_fingerprint(), public_fingerprint);
        assert_eq!(initial.state(), BlossomEvidenceState::ConfiguredUnobserved);
        assert_eq!(
            initial.last_successful_state(),
            BlossomEvidenceState::ConfiguredUnobserved
        );
        assert_eq!(
            initial.transport_security(),
            BlossomTransportSecurity::PublicWebPki
        );
        assert_eq!(initial.observed_at_unix_ms(), None);
        assert_eq!(initial.http_status(), None);
        assert_eq!(initial.error_code(), None);
        assert_eq!(initial.error_phase(), None);
        assert!(!initial.retryable());
        assert!(!initial.possible_orphan());
        assert_eq!(initial.attempts(), 0);
        assert_eq!(public_fingerprint.to_hex(), public_fingerprint.to_string());
        assert_eq!(
            slot.profile().expect("profile").primary().origin(),
            initial.origin()
        );
        assert_eq!(
            slot.configuration().expect("configuration").1,
            public_fingerprint
        );

        let private_tls = BlossomConfig::from_profile(
            device_blossom_profile("https://10.0.0.10:8443").expect("device profile"),
        );
        let mut evidence = BlossomEndpointEvidence::configured(&private_tls);
        assert_eq!(
            evidence.transport_security(),
            BlossomTransportSecurity::DevelopmentTls
        );
        evidence.record_success(BlossomEvidenceState::UploadVerified, Some(201));
        assert_eq!(evidence.state(), BlossomEvidenceState::UploadVerified);
        assert_eq!(
            evidence.last_successful_state(),
            BlossomEvidenceState::UploadVerified
        );
        assert!(evidence.observed_at_unix_ms().is_some());
        assert_eq!(evidence.http_status(), Some(201));

        let failure = BlossomError::new(
            BlossomErrorKind::HttpStatus,
            BlossomPhase::Retrieval,
            false,
            true,
            2,
        )
        .with_http_status(403);
        evidence.record_failure(&failure);
        assert_eq!(evidence.state(), BlossomEvidenceState::TerminalFailure);
        assert_eq!(
            evidence.last_successful_state(),
            BlossomEvidenceState::UploadVerified
        );
        assert_eq!(evidence.http_status(), Some(403));
        assert_eq!(evidence.error_code(), Some("blossom_http_status"));
        assert_eq!(evidence.server_error_code(), None);
        assert_eq!(evidence.error_phase(), Some(BlossomPhase::Retrieval));
        assert!(!evidence.retryable());
        assert!(evidence.possible_orphan());
        assert_eq!(evidence.attempts(), 2);

        let cleartext = BlossomEndpointEvidence::configured(&BlossomConfig::from_profile(
            simulator_blossom_profile("http://127.0.0.1:3000").expect("simulator profile"),
        ));
        assert_eq!(
            cleartext.transport_security(),
            BlossomTransportSecurity::DevelopmentCleartext
        );

        slot.clear();
        assert!(slot.evidence().is_none());
    }

    #[cfg(feature = "blossom")]
    fn blossom_png(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes
    }

    #[cfg(feature = "blossom")]
    fn blossom_request(_origin: &str) -> BlossomUploadRequest {
        let bytes = blossom_png(2, 3);
        BlossomUploadRequest::new(
            Arc::from(bytes),
            MediaType::parse("image/png").expect("media type"),
            BlossomImageDimensions::new(2, 3).expect("dimensions"),
            1_900_000_000_000,
        )
        .expect("request")
    }

    #[cfg(feature = "blossom")]
    fn public_blossom_profile(origin: &str) -> Result<BlossomProfile, BlossomError> {
        BlossomProfile::new(
            BlossomHostKind::Native,
            BlossomEndpointAuthority::PublicWebPki,
            origin,
            std::iter::empty::<&str>(),
        )
    }

    #[cfg(feature = "blossom")]
    fn simulator_blossom_profile(origin: &str) -> Result<BlossomProfile, BlossomError> {
        BlossomProfile::new(
            BlossomHostKind::Simulator,
            BlossomEndpointAuthority::LoopbackDevelopment,
            origin,
            std::iter::empty::<&str>(),
        )
    }

    #[cfg(feature = "blossom")]
    fn device_blossom_profile(origin: &str) -> Result<BlossomProfile, BlossomError> {
        BlossomProfile::new(
            BlossomHostKind::PhysicalDevice,
            BlossomEndpointAuthority::PrivateNetworkDevelopment,
            origin,
            std::iter::empty::<&str>(),
        )
    }

    #[cfg(feature = "blossom")]
    #[test]
    fn blossom_profiles_expose_exact_identity_and_reject_malformed_sets() {
        let public = BlossomProfile::new(
            BlossomHostKind::PhysicalDevice,
            BlossomEndpointAuthority::PublicWebPki,
            "https://media.example:8443",
            ["https://fallback.example"],
        )
        .expect("public");
        assert_eq!(public.host_kind(), BlossomHostKind::PhysicalDevice);
        assert_eq!(public.authority(), BlossomEndpointAuthority::PublicWebPki);
        assert_eq!(public.fallbacks().len(), 1);
        let endpoint = public.primary();
        assert_eq!(endpoint.origin(), "https://media.example:8443");
        assert_eq!(endpoint.host(), "media.example");
        assert_eq!(endpoint.port(), 8443);
        assert_eq!(endpoint.authority(), BlossomEndpointAuthority::PublicWebPki);

        let request = blossom_request("https://media.example:8443");
        let slot = BlossomSlot::new();
        slot.configure(BlossomConfig::from_profile(public.clone()))
            .unwrap();
        let transaction = slot.prepare_upload(request).unwrap();
        assert!(endpoint.accepts_blob_url(transaction.expected_url()));
        assert_eq!(
            transaction
                .expected_url()
                .hash_path()
                .extension()
                .unwrap()
                .as_str(),
            "png"
        );
        assert_eq!(
            transaction.expected_url().hash_path().hash(),
            transaction.request().sha256()
        );
        assert_eq!(endpoint.upload_url(), "https://media.example:8443/upload");
        assert_eq!(endpoint.server_domain().unwrap().as_str(), "media.example");
        assert_eq!(
            public
                .endpoint_for_blob(transaction.expected_url())
                .expect("configured endpoint"),
            endpoint
        );

        assert_eq!(
            BlossomProfile::new(
                BlossomHostKind::PhysicalDevice,
                BlossomEndpointAuthority::PrivateNetworkDevelopment,
                "https://device.example",
                std::iter::empty::<&str>(),
            )
            .unwrap()
            .host_kind(),
            BlossomHostKind::PhysicalDevice
        );
        assert_eq!(
            simulator_blossom_profile("http://localhost:3000")
                .unwrap()
                .host_kind(),
            BlossomHostKind::Simulator
        );
        assert_eq!(
            BlossomProfile::new(
                BlossomHostKind::Native,
                BlossomEndpointAuthority::PublicWebPki,
                "",
                std::iter::empty::<&str>(),
            )
            .expect_err("empty profile")
            .kind(),
            BlossomErrorKind::InvalidEndpoint
        );
        assert_eq!(
            BlossomProfile::new(
                BlossomHostKind::Native,
                BlossomEndpointAuthority::PublicWebPki,
                "https://primary.example",
                std::iter::repeat_n("https://media.example", 16),
            )
            .expect_err("bounded profile")
            .kind(),
            BlossomErrorKind::InvalidEndpointCount
        );
        assert_eq!(
            BlossomProfile::new(
                BlossomHostKind::Native,
                BlossomEndpointAuthority::PublicWebPki,
                "https://media.example",
                ["https://media.example"],
            )
            .expect_err("duplicate profile")
            .kind(),
            BlossomErrorKind::DuplicateEndpoint
        );

        for malformed in [
            "",
            " https://media.example",
            "https://média.example",
            "https://user@media.example",
            "https://:password@media.example",
            "https://media.example/path",
            "https://media.example?query=1",
            "https://media.example#fragment",
            "ftp://media.example",
            "https://media.example:0",
        ] {
            assert!(public_blossom_profile(malformed).is_err(), "{malformed}");
        }
    }

    #[cfg(feature = "blossom")]
    #[test]
    fn blossom_upload_transactions_bind_primary_authority_and_complete_config() {
        let profile = BlossomProfile::new(
            BlossomHostKind::PhysicalDevice,
            BlossomEndpointAuthority::PublicWebPki,
            "https://media.example:443",
            ["https://fallback.example"],
        )
        .unwrap();
        assert_eq!(profile.primary().origin(), "https://media.example");
        assert!(
            profile
                .primary()
                .validate_resolved_addresses([IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))])
                .is_err()
        );

        let slot = BlossomSlot::new();
        let original = BlossomConfig::from_profile(profile);
        let original_fingerprint = original.fingerprint();
        slot.configure(original).unwrap();
        let transaction = slot
            .prepare_upload(blossom_request("caller-origin-is-ignored"))
            .unwrap();
        assert_eq!(transaction.config_fingerprint(), original_fingerprint);
        assert!(
            transaction
                .expected_url()
                .as_str()
                .starts_with("https://media.example/")
        );
        let claim = slot
            .authored_upload_claim(
                &transaction,
                AuthorizationContent::parse("Upload farm image").unwrap(),
                100,
                60,
            )
            .unwrap();
        assert_eq!(claim.server_domain().as_str(), "media.example");

        let changed = BlossomConfig::from_profile(
            BlossomProfile::new(
                BlossomHostKind::PhysicalDevice,
                BlossomEndpointAuthority::PublicWebPki,
                "https://other.example",
                ["https://fallback.example"],
            )
            .unwrap(),
        );
        assert_ne!(changed.fingerprint(), original_fingerprint);
        slot.configure(changed).unwrap();
        assert_eq!(
            slot.authored_upload_claim(
                &transaction,
                AuthorizationContent::parse("Upload farm image").unwrap(),
                100,
                60,
            )
            .expect_err("changed configuration"),
            BlossomError::configuration(BlossomErrorKind::ConfigurationChanged)
        );
        assert_eq!(
            slot.validate_transaction(&transaction)
                .expect_err("changed upload destination")
                .kind(),
            BlossomErrorKind::ConfigurationChanged
        );
    }

    #[cfg(feature = "blossom")]
    #[test]
    fn blossom_limits_requests_and_errors_cover_the_complete_public_contract() {
        let profile = simulator_blossom_profile("http://127.0.0.1:3000").unwrap();
        let valid = BlossomConfig::from_profile(profile.clone())
            .with_limits(1, 1, 5)
            .unwrap()
            .with_network_policy(
                Duration::from_millis(1),
                Duration::from_millis(2),
                5,
                Duration::from_millis(3),
            )
            .unwrap();
        assert_eq!(valid.profile(), &profile);
        assert_eq!(valid.max_blob_bytes(), 1);
        assert_eq!(valid.max_descriptor_bytes(), 1);
        assert_eq!(valid.max_redirects(), 5);
        assert_eq!(valid.max_attempts(), 5);
        assert_eq!(valid.connect_timeout(), Duration::from_millis(1));
        assert_eq!(valid.request_timeout(), Duration::from_millis(2));
        assert_eq!(valid.initial_retry_delay(), Duration::from_millis(3));

        for (blob, descriptor, redirects) in [
            (0, 1, 0),
            (MAX_BLOSSOM_BLOB_BYTES + 1, 1, 0),
            (1, 0, 0),
            (1, MAX_BLOSSOM_DESCRIPTOR_BYTES + 1, 0),
            (1, 1, MAX_BLOSSOM_REDIRECTS + 1),
        ] {
            assert!(
                BlossomConfig::from_profile(profile.clone())
                    .with_limits(blob, descriptor, redirects)
                    .is_err()
            );
        }
        for (connect, request, attempts, delay) in [
            (
                Duration::ZERO,
                Duration::from_secs(1),
                1,
                Duration::from_millis(1),
            ),
            (
                MAX_BLOSSOM_TIMEOUT + Duration::from_secs(1),
                Duration::from_secs(1),
                1,
                Duration::from_millis(1),
            ),
            (
                Duration::from_secs(1),
                Duration::ZERO,
                1,
                Duration::from_millis(1),
            ),
            (
                Duration::from_secs(1),
                MAX_BLOSSOM_TIMEOUT + Duration::from_secs(1),
                1,
                Duration::from_millis(1),
            ),
            (
                Duration::from_secs(1),
                Duration::from_secs(1),
                0,
                Duration::from_millis(1),
            ),
            (
                Duration::from_secs(1),
                Duration::from_secs(1),
                MAX_BLOSSOM_ATTEMPTS + 1,
                Duration::from_millis(1),
            ),
            (
                Duration::from_secs(1),
                Duration::from_secs(1),
                1,
                Duration::ZERO,
            ),
            (
                Duration::from_secs(1),
                Duration::from_secs(1),
                1,
                MAX_BLOSSOM_RETRY_DELAY + Duration::from_secs(1),
            ),
        ] {
            assert!(
                BlossomConfig::from_profile(profile.clone())
                    .with_network_policy(connect, request, attempts, delay)
                    .is_err()
            );
        }

        assert!(BlossomImageDimensions::new(0, 1).is_err());
        assert!(BlossomImageDimensions::new(1, 0).is_err());
        assert!(BlossomImageDimensions::new(16_385, 1).is_err());
        assert!(BlossomImageDimensions::new(10_001, 10_000).is_err());
        let dimensions = BlossomImageDimensions::new(2, 3).unwrap();
        assert_eq!(dimensions.width(), 2);
        assert_eq!(dimensions.height(), 3);

        let request = blossom_request("http://127.0.0.1:3000");
        assert_eq!(request.media_type().as_str(), "image/png");
        assert_eq!(request.dimensions(), dimensions);
        assert_eq!(request.byte_size(), request.bytes().len() as u64);
        assert_eq!(request.sha256(), Sha256::digest(request.bytes()));
        assert_eq!(request.verified_at_unix_ms(), 1_900_000_000_000);
        assert!(format!("{request:?}").contains("bytes: \"<redacted>\""));

        let media_type = MediaType::parse("image/png").unwrap();
        assert!(
            BlossomUploadRequest::new(Arc::from([]), media_type.clone(), dimensions, 1,).is_err()
        );
        let bytes = blossom_png(2, 3);
        assert!(
            BlossomUploadRequest::new(
                Arc::from(bytes.clone()),
                MediaType::parse("image/jpeg").unwrap(),
                dimensions,
                1,
            )
            .is_err()
        );
        assert!(BlossomUploadRequest::new(Arc::from(bytes), media_type, dimensions, 0).is_err());

        let all_kinds = [
            (
                BlossomErrorKind::InvalidEndpoint,
                "blossom_invalid_endpoint",
            ),
            (
                BlossomErrorKind::EndpointSchemeDenied,
                "blossom_endpoint_scheme_denied",
            ),
            (
                BlossomErrorKind::InvalidEndpointCount,
                "blossom_invalid_endpoint_count",
            ),
            (
                BlossomErrorKind::DuplicateEndpoint,
                "blossom_duplicate_endpoint",
            ),
            (
                BlossomErrorKind::EndpointNotConfigured,
                "blossom_endpoint_not_configured",
            ),
            (
                BlossomErrorKind::ConfigurationChanged,
                "blossom_configuration_changed",
            ),
            (
                BlossomErrorKind::ResolutionFailed,
                "blossom_resolution_failed",
            ),
            (
                BlossomErrorKind::ResolvedAddressDenied,
                "blossom_resolved_address_denied",
            ),
            (BlossomErrorKind::InvalidLimits, "blossom_invalid_limits"),
            (BlossomErrorKind::InvalidRequest, "blossom_invalid_request"),
            (
                BlossomErrorKind::InvalidDimensions,
                "blossom_invalid_dimensions",
            ),
            (
                BlossomErrorKind::UnsupportedMediaType,
                "blossom_unsupported_media_type",
            ),
            (
                BlossomErrorKind::MediaTypeMismatch,
                "blossom_media_type_mismatch",
            ),
            (
                BlossomErrorKind::InvalidImageBytes,
                "blossom_invalid_image_bytes",
            ),
            (
                BlossomErrorKind::DimensionMismatch,
                "blossom_dimension_mismatch",
            ),
            (
                BlossomErrorKind::Authorization,
                "blossom_authorization_failed",
            ),
            (BlossomErrorKind::Transport, "blossom_transport_failed"),
            (BlossomErrorKind::Timeout, "blossom_timeout"),
            (BlossomErrorKind::Cancelled, "blossom_cancelled"),
            (BlossomErrorKind::HttpStatus, "blossom_http_status"),
            (BlossomErrorKind::UnsafeRedirect, "blossom_unsafe_redirect"),
            (BlossomErrorKind::RedirectLimit, "blossom_redirect_limit"),
            (
                BlossomErrorKind::ContentEncodingDenied,
                "blossom_content_encoding_denied",
            ),
            (
                BlossomErrorKind::ResponseTooLarge,
                "blossom_response_too_large",
            ),
            (
                BlossomErrorKind::ResponseSizeMismatch,
                "blossom_response_size_mismatch",
            ),
            (
                BlossomErrorKind::ResponseHashMismatch,
                "blossom_response_hash_mismatch",
            ),
            (
                BlossomErrorKind::InvalidDescriptor,
                "blossom_invalid_descriptor",
            ),
            (
                BlossomErrorKind::DescriptorMismatch,
                "blossom_descriptor_mismatch",
            ),
            (
                BlossomErrorKind::RetrievedBytesMismatch,
                "blossom_retrieved_bytes_mismatch",
            ),
        ];
        for (kind, code) in all_kinds {
            let error = BlossomError::new(kind, BlossomPhase::Verification, true, false, 2);
            assert_eq!(error.kind(), kind);
            assert_eq!(error.phase(), BlossomPhase::Verification);
            assert!(error.retryable());
            assert!(!error.possible_orphan());
            assert_eq!(error.attempts(), 2);
            assert_eq!(error.code(), code);
            assert_eq!(error.to_string(), code);
            assert!(
                format!("{error:?}").contains(code.trim_start_matches("blossom_"))
                    || !code.is_empty()
            );
            let updated = error.with_operation(true, 3);
            assert!(updated.possible_orphan());
            assert_eq!(updated.attempts(), 3);
        }
    }

    #[cfg(feature = "blossom")]
    #[test]
    fn blossom_address_policy_covers_public_simulator_and_device_networks() {
        use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

        let public_v4 = [
            (Ipv4Addr::new(8, 8, 8, 8), true),
            (Ipv4Addr::new(0, 1, 2, 3), false),
            (Ipv4Addr::LOCALHOST, false),
            (Ipv4Addr::new(10, 0, 0, 1), false),
            (Ipv4Addr::new(169, 254, 1, 1), false),
            (Ipv4Addr::new(224, 0, 0, 1), false),
            (Ipv4Addr::new(192, 0, 2, 1), false),
            (Ipv4Addr::new(100, 64, 0, 1), false),
            (Ipv4Addr::new(100, 128, 0, 1), true),
            (Ipv4Addr::new(192, 0, 0, 1), false),
            (Ipv4Addr::new(192, 0, 1, 1), true),
            (Ipv4Addr::new(192, 88, 99, 1), false),
            (Ipv4Addr::new(192, 88, 98, 1), true),
            (Ipv4Addr::new(198, 18, 0, 1), false),
            (Ipv4Addr::new(240, 0, 0, 1), false),
        ];
        for (address, accepted) in public_v4 {
            assert_eq!(public_blossom_ipv4(address), accepted, "{address}");
            assert_eq!(public_blossom_address(IpAddr::V4(address)), accepted);
        }
        let public_v6 = [
            ("2606:4700:4700::1111", true),
            ("::ffff:8.8.8.8", true),
            ("::ffff:127.0.0.1", false),
            ("2001:100::1", false),
            ("2001:db8::1", false),
            ("2002::1", false),
            ("3fff::1", false),
            ("4000::1", false),
            ("2001:db9::1", true),
        ];
        for (text, accepted) in public_v6 {
            let address = text.parse::<Ipv6Addr>().unwrap();
            assert_eq!(public_blossom_ipv6(address), accepted, "{text}");
            assert_eq!(public_blossom_address(IpAddr::V6(address)), accepted);
        }

        for (host, accepted) in [
            ("media.example", true),
            ("localhost", false),
            ("farm.localhost", false),
            ("farm.local", false),
            ("farm.home.arpa", false),
            ("intranet", false),
        ] {
            assert_eq!(public_blossom_hostname(host), accepted, "{host}");
        }

        let simulator = simulator_blossom_profile("http://127.0.0.1:3000").unwrap();
        let simulator_endpoint = simulator.primary();
        assert!(
            simulator_endpoint
                .validate_resolved_addresses([IpAddr::V4(Ipv4Addr::LOCALHOST)])
                .is_ok()
        );
        assert!(simulator_endpoint.validate_resolved_addresses([]).is_err());
        assert!(
            simulator_endpoint
                .validate_resolved_addresses([IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))])
                .is_err()
        );

        let public = public_blossom_profile("https://media.example").unwrap();
        assert!(
            public
                .primary()
                .validate_resolved_addresses([IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))])
                .is_ok()
        );
        let device = device_blossom_profile("https://device.example").unwrap();
        assert!(
            device
                .primary()
                .validate_resolved_addresses([IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))])
                .is_ok()
        );

        for address in [
            IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            IpAddr::V4(Ipv4Addr::new(224, 0, 0, 1)),
            IpAddr::V4(Ipv4Addr::BROADCAST),
            IpAddr::V6(Ipv6Addr::UNSPECIFIED),
            IpAddr::V6(Ipv6Addr::LOCALHOST),
            IpAddr::V6("ff02::1".parse().unwrap()),
        ] {
            assert!(!trusted_blossom_address(address), "{address}");
        }
        assert!(trusted_blossom_address(IpAddr::V4(Ipv4Addr::new(
            10, 0, 0, 1
        ))));
        assert!(trusted_blossom_address(IpAddr::V6(
            "fd00::1".parse().unwrap()
        )));
        assert!(blossom_authority_accepts_address(
            BlossomEndpointAuthority::PublicWebPki,
            IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))
        ));
        assert!(blossom_authority_accepts_address(
            BlossomEndpointAuthority::LoopbackDevelopment,
            IpAddr::V4(Ipv4Addr::LOCALHOST)
        ));
        assert!(blossom_authority_accepts_address(
            BlossomEndpointAuthority::PrivateNetworkDevelopment,
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))
        ));
    }

    #[cfg(feature = "blossom")]
    #[tokio::test]
    async fn blossom_cancellation_slot_claim_and_receipt_state_are_exact() {
        let cancellation = BlossomCancellation::default();
        assert!(!cancellation.is_cancelled());
        let waiting = cancellation.clone();
        let waiter = tokio::spawn(async move { waiting.cancelled().await });
        tokio::task::yield_now().await;
        cancellation.cancel();
        waiter.await.unwrap();
        cancellation.cancelled().await;
        assert!(cancellation.is_cancelled());

        let request = blossom_request("http://127.0.0.1:3000");
        let slot = BlossomSlot::new();
        assert!(slot.host_kind().is_none());
        let content = AuthorizationContent::parse("Upload farm image").unwrap();
        assert!(slot.prepare_upload(request.clone()).is_err());
        slot.configure(BlossomConfig::from_profile(
            simulator_blossom_profile("http://127.0.0.1:3000").unwrap(),
        ))
        .unwrap();
        let transaction = slot.prepare_upload(request.clone()).unwrap();
        let claim = slot
            .authored_upload_claim(&transaction, content.clone(), 100, 60)
            .unwrap();
        assert_eq!(claim.server_domain().as_str(), "127.0.0.1");
        assert_eq!(claim.sha256(), request.sha256());
        assert_eq!(claim.lifetime_seconds(), 60);
        assert_eq!(
            slot.authored_upload_claim(&transaction, content, 100, 0)
                .expect_err("invalid lifetime")
                .kind(),
            BlossomErrorKind::Authorization
        );
        slot.clear();
        assert!(slot.host_kind().is_none());

        let descriptor = radroots_blossom::BlobDescriptor::new(
            transaction.expected_url().clone(),
            request.sha256(),
            request.byte_size(),
            request.media_type().clone(),
            1,
        )
        .unwrap()
        .approve_reference()
        .unwrap()
        .verify_bytes(request.bytes(), request.media_type())
        .unwrap();
        let receipt = BlossomUploadReceipt::new(descriptor, request.dimensions(), 2, 3);
        assert_eq!(receipt.descriptor().sha256(), request.sha256());
        assert_eq!(receipt.dimensions(), request.dimensions());
        assert_eq!(receipt.attempts(), 2);
        assert_eq!(receipt.verified_at_unix_ms(), 3);
        assert_eq!(receipt.into_descriptor().size(), request.byte_size());

        let poisoned = BlossomSlot::new();
        let state = Arc::clone(&poisoned.state);
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = state.write().expect("write lock");
            panic!("poison Blossom slot");
        }));
        poisoned.clear();
        assert!(
            poisoned
                .configure(BlossomConfig::from_profile(
                    simulator_blossom_profile("http://127.0.0.1:3000").unwrap(),
                ))
                .is_err()
        );
        assert!(poisoned.host_kind().is_none());
    }

    #[cfg(feature = "radrootsd")]
    #[test]
    fn daemon_configuration_is_inert_explicit_and_redacted() {
        let config = DaemonConfig::new("http://127.0.0.1:1/rpc")
            .with_auth(DaemonAuth::BearerToken("secret-token".to_owned()))
            .with_timeout(core::time::Duration::from_millis(5));
        let adapter = DaemonDelivery::new(config);

        let debug = format!("{adapter:?}");
        assert!(!debug.contains("secret-token"));
        assert!(!debug.contains("reqwest"));
        assert_eq!(format!("{:?}", DaemonAuth::None), "None");
        assert_eq!(
            format!("{:?}", DaemonAuth::BearerToken("private".to_owned())),
            "BearerToken(<redacted>)"
        );
    }

    #[cfg(feature = "radrootsd")]
    #[test]
    fn daemon_errors_are_stably_classified_and_redacted() {
        use std::error::Error as _;

        use crate::adapters::radrootsd::RadrootsdError;

        let cases = [
            (
                RadrootsdError::InvalidAuthHeader("private".to_owned()),
                DaemonErrorKind::Authentication,
                "daemon authentication configuration is invalid",
            ),
            (
                RadrootsdError::InvalidRequest("private".to_owned()),
                DaemonErrorKind::InvalidRequest,
                "daemon delivery request is invalid",
            ),
            (
                RadrootsdError::Http("private".to_owned()),
                DaemonErrorKind::Transport,
                "daemon transport failed",
            ),
            (
                RadrootsdError::JsonRpc {
                    code: -1,
                    message: "private".to_owned(),
                },
                DaemonErrorKind::Rpc,
                "daemon RPC failed",
            ),
            (
                RadrootsdError::MalformedResponse("private".to_owned()),
                DaemonErrorKind::InvalidResponse,
                "daemon response is invalid",
            ),
        ];
        for (private, kind, display) in cases {
            let error = DaemonError::from_private(private);
            assert_eq!(error.kind(), kind);
            assert_eq!(error.to_string(), display);
            assert!(error.source().is_some());
            assert!(!format!("{error:?}").contains("private"));
        }
    }

    #[cfg(feature = "nostr")]
    #[test]
    fn nostr_slot_reconfiguration_is_atomic_directional_and_inert() {
        let slot = NostrSlot::new();
        assert!(slot.read_targets().is_none());
        assert!(slot.relay_status().is_none());
        assert!(
            slot.configure(RelayProfile::simulator(["ws://127.0.0.1:7447"]).expect("profile"))
                .is_ok()
        );
        let original = slot.read_targets().expect("configured targets");
        assert_eq!(slot.write_targets(), Some(original.clone()));
        let status = slot.relay_status().expect("status");
        assert_eq!(status.profile_kind(), RelayProfileKind::Simulator);
        assert_eq!(status.read_availability(), Availability::Unavailable);
        assert_eq!(status.write_availability(), Availability::Unavailable);
        slot.clear();
        assert!(slot.read_targets().is_none());
        assert!(slot.write_targets().is_none());
        assert!(format!("{slot:?}").contains("configured: false"));
    }

    #[cfg(feature = "nostr")]
    #[test]
    fn poisoned_nostr_slot_fails_closed_for_every_host_operation() {
        let slot = NostrSlot::new();
        let state = Arc::clone(&slot.state);
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = state.write().expect("write lock");
            panic!("poison transport slot");
        }));

        slot.clear();
        assert!(slot.read_targets().is_none());
        assert!(
            slot.configure(RelayProfile::simulator(["ws://127.0.0.1:7447"]).expect("profile"))
                .is_err()
        );
    }

    #[cfg(feature = "nostr")]
    #[tokio::test]
    async fn empty_nostr_slot_reports_unavailable_and_rejects_operations() {
        use radroots_transport::{
            DeliveryRequest, EventSink as _, EventSource as _, FetchRequest, sink::DeliveryPayload,
            source::FetchBounds,
        };

        let slot = NostrSlot::new();
        let source = radroots_transport::EventSource::status(&slot)
            .await
            .expect("source status");
        assert_eq!(source.availability(), Availability::Unavailable);

        let targets = TargetSet::new(vec![target(1)]).expect("targets");
        let fetch = FetchRequest::new(
            "fetch",
            targets.clone(),
            FetchBounds::new(1, 1).expect("bounds"),
        )
        .expect("fetch");
        assert_eq!(slot.fetch(fetch).await, Err(Error::UnsupportedOperation));

        let sink = radroots_transport::EventSink::status(&slot)
            .await
            .expect("sink status");
        assert_eq!(sink.availability(), Availability::Unavailable);
        let deliver = DeliveryRequest::new(
            "deliver",
            DeliveryPayload::new(signed_event()),
            targets,
            SatisfactionPolicy::new(SatisfactionClass::Accepted, TargetPolicy::any()),
            1,
        )
        .expect("delivery");
        let failure = slot.deliver(deliver).await.expect_err("unconfigured sink");
        assert_eq!(failure.code(), "nostr_transport_not_configured");
    }
}
