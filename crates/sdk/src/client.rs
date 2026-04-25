#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};
use core::fmt;
#[cfg(feature = "std")]
use std::{string::String, vec::Vec};

#[cfg(feature = "radrootsd-client")]
use crate::adapters::radrootsd;
#[cfg(all(
    feature = "identity-models",
    feature = "relay-client",
    feature = "signing"
))]
use crate::adapters::relay;
use crate::config::SignerConfig;
use crate::config::{RadrootsSdkConfig, SdkConfigError, SdkTransportMode};
#[cfg(all(
    feature = "identity-models",
    feature = "relay-client",
    feature = "signing"
))]
use crate::identity::RadrootsIdentity;
use crate::{
    NostrTags, RadrootsNostrEvent, RadrootsNostrEventPtr, RadrootsProfile, RadrootsProfileType,
    RadrootsTradeEnvelope, TradeListingValidateResult, WireEventParts, farm, listing, profile,
    trade,
};
#[cfg(any(
    feature = "radrootsd-client",
    all(
        feature = "identity-models",
        feature = "relay-client",
        feature = "signing"
    )
))]
use core::time::Duration;
#[cfg(feature = "radrootsd-client")]
use radroots_events::kinds::{KIND_FARM, KIND_LISTING};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SdkPublishReceipt {
    pub transport: SdkTransportMode,
    pub event_kind: Option<u32>,
    pub event_id: Option<String>,
    pub transport_receipt: SdkTransportReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SdkTransportReceipt {
    RelayDirect(SdkRelayPublishReceipt),
    Radrootsd(SdkRadrootsdPublishReceipt),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SdkRelayPublishReceipt {
    pub acknowledged_relays: Vec<String>,
    pub failed_relays: Vec<SdkRelayFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SdkRelayFailure {
    pub relay_url: String,
    pub error: String,
}

#[derive(Clone, PartialEq, Eq, Default)]
pub struct SdkRadrootsdPublishReceipt {
    pub accepted: bool,
    pub deduplicated: bool,
    pub job_id: Option<String>,
    pub status: Option<String>,
    pub signer_mode: Option<String>,
    pub signer_session_id: Option<String>,
    pub event_addr: Option<String>,
    pub relay_count: Option<usize>,
    pub acknowledged_relay_count: Option<usize>,
}

impl fmt::Debug for SdkRadrootsdPublishReceipt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("SdkRadrootsdPublishReceipt");
        debug.field("accepted", &self.accepted);
        debug.field("deduplicated", &self.deduplicated);
        debug.field("job_id", &self.job_id);
        debug.field("status", &self.status);
        debug.field(
            "signer_mode",
            &self.signer_mode.as_ref().map(|_| "<redacted>"),
        );
        debug.field(
            "signer_session_id",
            &self.signer_session_id.as_ref().map(|_| "<redacted>"),
        );
        debug.field("event_addr", &self.event_addr);
        debug.field("relay_count", &self.relay_count);
        debug.field("acknowledged_relay_count", &self.acknowledged_relay_count);
        debug.finish()
    }
}

#[cfg(feature = "radrootsd-client")]
impl SdkRadrootsdPublishReceipt {
    pub fn job(&self) -> Option<SdkRadrootsdBridgeJobRef> {
        self.job_id
            .as_ref()
            .map(|job_id| SdkRadrootsdBridgeJobRef::new(job_id.clone()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SdkPublishError {
    Config(SdkConfigError),
    Encode(String),
    UnsupportedTransport {
        transport: SdkTransportMode,
        operation: &'static str,
    },
    UnsupportedSignerMode {
        transport: SdkTransportMode,
        signer: SignerConfig,
        required: SignerConfig,
        operation: &'static str,
    },
    Relay(String),
    RelayNotAcknowledged {
        transport: SdkTransportMode,
        failed_relays: Vec<SdkRelayFailure>,
    },
    Radrootsd(String),
}

impl From<SdkConfigError> for SdkPublishError {
    fn from(value: SdkConfigError) -> Self {
        Self::Config(value)
    }
}

impl core::fmt::Display for SdkPublishError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Config(err) => write!(f, "{err}"),
            Self::Encode(message) => write!(f, "{message}"),
            Self::UnsupportedTransport {
                transport,
                operation,
            } => {
                write!(
                    f,
                    "{operation} requires a different sdk transport mode than {transport:?}"
                )
            }
            Self::UnsupportedSignerMode {
                transport,
                signer,
                required,
                operation,
            } => write!(
                f,
                "{operation} requires signer mode `{required}` for {transport:?} transport, got `{signer}`"
            ),
            Self::Relay(message) => write!(f, "{message}"),
            Self::RelayNotAcknowledged {
                transport,
                failed_relays,
            } => {
                if failed_relays.is_empty() {
                    write!(f, "{transport:?} publish was not acknowledged by any relay")
                } else {
                    let summary = failed_relays
                        .iter()
                        .map(|failure| format!("{}: {}", failure.relay_url, failure.error))
                        .collect::<Vec<_>>()
                        .join(", ");
                    write!(
                        f,
                        "{transport:?} publish was not acknowledged by any relay: {summary}"
                    )
                }
            }
            Self::Radrootsd(message) => write!(f, "{message}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for SdkPublishError {}

#[cfg(feature = "radrootsd-client")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SdkRadrootsdSessionError {
    Config(SdkConfigError),
    UnsupportedTransport {
        transport: SdkTransportMode,
        operation: &'static str,
    },
    Radrootsd(String),
}

#[cfg(feature = "radrootsd-client")]
impl From<SdkConfigError> for SdkRadrootsdSessionError {
    fn from(value: SdkConfigError) -> Self {
        Self::Config(value)
    }
}

#[cfg(feature = "radrootsd-client")]
impl fmt::Display for SdkRadrootsdSessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(err) => write!(f, "{err}"),
            Self::UnsupportedTransport {
                transport,
                operation,
            } => {
                write!(
                    f,
                    "{operation} requires a different sdk transport mode than {transport:?}"
                )
            }
            Self::Radrootsd(message) => write!(f, "{message}"),
        }
    }
}

#[cfg(all(feature = "radrootsd-client", feature = "std"))]
impl std::error::Error for SdkRadrootsdSessionError {}

#[cfg(feature = "radrootsd-client")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SdkRadrootsdBridgeError {
    Config(SdkConfigError),
    UnsupportedTransport {
        transport: SdkTransportMode,
        operation: &'static str,
    },
    Radrootsd(String),
}

#[cfg(feature = "radrootsd-client")]
impl From<SdkConfigError> for SdkRadrootsdBridgeError {
    fn from(value: SdkConfigError) -> Self {
        Self::Config(value)
    }
}

#[cfg(feature = "radrootsd-client")]
impl fmt::Display for SdkRadrootsdBridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(err) => write!(f, "{err}"),
            Self::UnsupportedTransport {
                transport,
                operation,
            } => write!(
                f,
                "{operation} requires a different sdk transport mode than {transport:?}"
            ),
            Self::Radrootsd(message) => write!(f, "{message}"),
        }
    }
}

#[cfg(all(feature = "radrootsd-client", feature = "std"))]
impl std::error::Error for SdkRadrootsdBridgeError {}

#[cfg(feature = "radrootsd-client")]
#[derive(Clone, PartialEq, Eq)]
pub struct SdkRadrootsdSignerSessionRef {
    session_id: String,
}

#[cfg(feature = "radrootsd-client")]
impl fmt::Debug for SdkRadrootsdSignerSessionRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SdkRadrootsdSignerSessionRef")
            .field("session_id", &"<redacted>")
            .finish()
    }
}

#[cfg(feature = "radrootsd-client")]
impl SdkRadrootsdSignerSessionRef {
    pub fn from_session_id(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
        }
    }

    pub(crate) fn session_id(&self) -> &str {
        self.session_id.as_str()
    }
}

#[cfg(feature = "radrootsd-client")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SdkRadrootsdBridgeJobRef {
    job_id: String,
}

#[cfg(feature = "radrootsd-client")]
impl SdkRadrootsdBridgeJobRef {
    pub fn new(job_id: impl Into<String>) -> Self {
        Self {
            job_id: job_id.into(),
        }
    }

    pub fn job_id(&self) -> &str {
        self.job_id.as_str()
    }
}

#[cfg(feature = "radrootsd-client")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SdkRadrootsdBridgeStatus {
    pub enabled: bool,
    pub ready: bool,
    pub auth_mode: String,
    pub signer_mode: String,
    pub default_signer_mode: String,
    pub supported_signer_modes: Vec<String>,
    pub available_nip46_signer_sessions: usize,
    pub relay_count: usize,
    pub delivery_policy: radrootsd::SdkRadrootsdBridgeDeliveryPolicy,
    pub delivery_quorum: Option<usize>,
    pub publish_max_attempts: usize,
    pub publish_initial_backoff_millis: u64,
    pub publish_max_backoff_millis: u64,
    pub job_status_retention: usize,
    pub retained_jobs: usize,
    pub retained_idempotency_keys: usize,
    pub accepted_jobs: usize,
    pub published_jobs: usize,
    pub failed_jobs: usize,
    pub recovered_failed_jobs: usize,
    pub methods: Vec<String>,
}

#[cfg(feature = "radrootsd-client")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SdkRadrootsdBridgeJobView {
    job: SdkRadrootsdBridgeJobRef,
    pub command: String,
    pub idempotency_key: Option<String>,
    pub status: radrootsd::SdkRadrootsdBridgeJobStatus,
    pub terminal: bool,
    pub recovered_after_restart: bool,
    pub requested_at_unix: u64,
    pub completed_at_unix: Option<u64>,
    pub signer_mode: String,
    pub signer_session_id: Option<String>,
    pub event_kind: u32,
    pub event_id: Option<String>,
    pub event_addr: Option<String>,
    pub delivery_policy: radrootsd::SdkRadrootsdBridgeDeliveryPolicy,
    pub delivery_quorum: Option<usize>,
    pub relay_count: usize,
    pub acknowledged_relay_count: usize,
    pub required_acknowledged_relay_count: usize,
    pub attempt_count: usize,
    pub attempt_summaries: Vec<String>,
    pub relay_results: Vec<radrootsd::SdkRadrootsdBridgeRelayPublishResult>,
    pub relay_outcome_summary: String,
}

#[cfg(feature = "radrootsd-client")]
impl SdkRadrootsdBridgeJobView {
    pub fn job(&self) -> &SdkRadrootsdBridgeJobRef {
        &self.job
    }
}

#[cfg(feature = "radrootsd-client")]
impl From<radrootsd::SdkRadrootsdBridgeStatusResponse> for SdkRadrootsdBridgeStatus {
    fn from(value: radrootsd::SdkRadrootsdBridgeStatusResponse) -> Self {
        Self {
            enabled: value.enabled,
            ready: value.ready,
            auth_mode: value.auth_mode,
            signer_mode: value.signer_mode,
            default_signer_mode: value.default_signer_mode,
            supported_signer_modes: value.supported_signer_modes,
            available_nip46_signer_sessions: value.available_nip46_signer_sessions,
            relay_count: value.relay_count,
            delivery_policy: value.delivery_policy,
            delivery_quorum: value.delivery_quorum,
            publish_max_attempts: value.publish_max_attempts,
            publish_initial_backoff_millis: value.publish_initial_backoff_millis,
            publish_max_backoff_millis: value.publish_max_backoff_millis,
            job_status_retention: value.job_status_retention,
            retained_jobs: value.retained_jobs,
            retained_idempotency_keys: value.retained_idempotency_keys,
            accepted_jobs: value.accepted_jobs,
            published_jobs: value.published_jobs,
            failed_jobs: value.failed_jobs,
            recovered_failed_jobs: value.recovered_failed_jobs,
            methods: value.methods,
        }
    }
}

#[cfg(feature = "radrootsd-client")]
impl From<radrootsd::SdkRadrootsdBridgeJobView> for SdkRadrootsdBridgeJobView {
    fn from(value: radrootsd::SdkRadrootsdBridgeJobView) -> Self {
        Self {
            job: SdkRadrootsdBridgeJobRef::new(value.job_id),
            command: value.command,
            idempotency_key: value.idempotency_key,
            status: value.status,
            terminal: value.terminal,
            recovered_after_restart: value.recovered_after_restart,
            requested_at_unix: value.requested_at_unix,
            completed_at_unix: value.completed_at_unix,
            signer_mode: value.signer_mode,
            signer_session_id: value.signer_session_id,
            event_kind: value.event_kind,
            event_id: value.event_id,
            event_addr: value.event_addr,
            delivery_policy: value.delivery_policy,
            delivery_quorum: value.delivery_quorum,
            relay_count: value.relay_count,
            acknowledged_relay_count: value.acknowledged_relay_count,
            required_acknowledged_relay_count: value.required_acknowledged_relay_count,
            attempt_count: value.attempt_count,
            attempt_summaries: value.attempt_summaries,
            relay_results: value.relay_results,
            relay_outcome_summary: value.relay_outcome_summary,
        }
    }
}

#[cfg(feature = "radrootsd-client")]
#[derive(Clone, PartialEq, Eq)]
pub struct SdkRadrootsdSignerSessionHandle {
    session: SdkRadrootsdSignerSessionRef,
    mode: radrootsd::SdkRadrootsdSignerSessionMode,
    remote_signer_pubkey: String,
    client_pubkey: String,
    relays: Vec<String>,
}

#[cfg(feature = "radrootsd-client")]
#[derive(Clone, PartialEq, Eq)]
pub struct SdkRadrootsdSignerSessionView {
    session: SdkRadrootsdSignerSessionRef,
    pub role: radrootsd::SdkRadrootsdSignerSessionRole,
    pub client_pubkey: String,
    pub signer_pubkey: String,
    pub user_pubkey: Option<String>,
    pub relays: Vec<String>,
    pub permissions: Vec<String>,
    pub name: Option<String>,
    pub url: Option<String>,
    pub image: Option<String>,
    pub auth_required: bool,
    pub authorized: bool,
    pub auth_url: Option<String>,
    pub expires_in_secs: Option<u64>,
    pub signer_authority: Option<radrootsd::SdkRadrootsdSignerAuthority>,
}

#[cfg(feature = "radrootsd-client")]
impl fmt::Debug for SdkRadrootsdSignerSessionView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("SdkRadrootsdSignerSessionView");
        debug.field("session", &self.session);
        debug.field("role", &self.role);
        debug.field("client_pubkey", &self.client_pubkey);
        debug.field("signer_pubkey", &self.signer_pubkey);
        debug.field("user_pubkey", &self.user_pubkey);
        debug.field("relays", &self.relays);
        debug.field("permissions", &self.permissions);
        debug.field("name", &self.name);
        debug.field("url", &self.url);
        debug.field("image", &self.image);
        debug.field("auth_required", &self.auth_required);
        debug.field("authorized", &self.authorized);
        debug.field("auth_url", &self.auth_url);
        debug.field("expires_in_secs", &self.expires_in_secs);
        debug.field("signer_authority", &self.signer_authority);
        debug.finish()
    }
}

#[cfg(feature = "radrootsd-client")]
impl SdkRadrootsdSignerSessionView {
    pub fn session(&self) -> &SdkRadrootsdSignerSessionRef {
        &self.session
    }
}

#[cfg(feature = "radrootsd-client")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SdkRadrootsdSignerSessionAuthorizeResult {
    pub authorized: bool,
    pub replayed: bool,
}

#[cfg(feature = "radrootsd-client")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SdkRadrootsdSignerSessionPublicKeyResult {
    pub pubkey: String,
}

#[cfg(feature = "radrootsd-client")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SdkRadrootsdSignerSessionRequireAuthResult {
    pub required: bool,
}

#[cfg(feature = "radrootsd-client")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SdkRadrootsdSignerSessionCloseResult {
    pub closed: bool,
}

#[cfg(feature = "radrootsd-client")]
impl From<radrootsd::SdkRadrootsdSignerSessionViewResponse> for SdkRadrootsdSignerSessionView {
    fn from(value: radrootsd::SdkRadrootsdSignerSessionViewResponse) -> Self {
        Self {
            session: SdkRadrootsdSignerSessionRef {
                session_id: value.session_id,
            },
            role: value.role,
            client_pubkey: value.client_pubkey,
            signer_pubkey: value.signer_pubkey,
            user_pubkey: value.user_pubkey,
            relays: value.relays,
            permissions: value.permissions,
            name: value.name,
            url: value.url,
            image: value.image,
            auth_required: value.auth_required,
            authorized: value.authorized,
            auth_url: value.auth_url,
            expires_in_secs: value.expires_in_secs,
            signer_authority: value.signer_authority,
        }
    }
}

#[cfg(feature = "radrootsd-client")]
impl From<radrootsd::SdkRadrootsdSignerSessionAuthorizeResponse>
    for SdkRadrootsdSignerSessionAuthorizeResult
{
    fn from(value: radrootsd::SdkRadrootsdSignerSessionAuthorizeResponse) -> Self {
        Self {
            authorized: value.authorized,
            replayed: value.replayed,
        }
    }
}

#[cfg(feature = "radrootsd-client")]
impl From<radrootsd::SdkRadrootsdSignerSessionPublicKeyResponse>
    for SdkRadrootsdSignerSessionPublicKeyResult
{
    fn from(value: radrootsd::SdkRadrootsdSignerSessionPublicKeyResponse) -> Self {
        Self {
            pubkey: value.pubkey,
        }
    }
}

#[cfg(feature = "radrootsd-client")]
impl From<radrootsd::SdkRadrootsdSignerSessionRequireAuthResponse>
    for SdkRadrootsdSignerSessionRequireAuthResult
{
    fn from(value: radrootsd::SdkRadrootsdSignerSessionRequireAuthResponse) -> Self {
        Self {
            required: value.required,
        }
    }
}

#[cfg(feature = "radrootsd-client")]
impl From<radrootsd::SdkRadrootsdSignerSessionCloseResponse>
    for SdkRadrootsdSignerSessionCloseResult
{
    fn from(value: radrootsd::SdkRadrootsdSignerSessionCloseResponse) -> Self {
        Self {
            closed: value.closed,
        }
    }
}

#[cfg(feature = "radrootsd-client")]
impl fmt::Debug for SdkRadrootsdSignerSessionHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("SdkRadrootsdSignerSessionHandle");
        debug.field("session", &self.session);
        debug.field("mode", &self.mode);
        debug.field("remote_signer_pubkey", &self.remote_signer_pubkey);
        debug.field("client_pubkey", &self.client_pubkey);
        debug.field("relays", &self.relays);
        debug.finish()
    }
}

#[cfg(feature = "radrootsd-client")]
impl SdkRadrootsdSignerSessionHandle {
    pub fn session(&self) -> &SdkRadrootsdSignerSessionRef {
        &self.session
    }

    pub fn mode(&self) -> radrootsd::SdkRadrootsdSignerSessionMode {
        self.mode
    }

    pub fn remote_signer_pubkey(&self) -> &str {
        self.remote_signer_pubkey.as_str()
    }

    pub fn client_pubkey(&self) -> &str {
        self.client_pubkey.as_str()
    }

    pub fn relays(&self) -> &[String] {
        self.relays.as_slice()
    }
}

#[cfg(feature = "radrootsd-client")]
impl From<radrootsd::SdkRadrootsdSignerSessionConnectResponse> for SdkRadrootsdSignerSessionHandle {
    fn from(value: radrootsd::SdkRadrootsdSignerSessionConnectResponse) -> Self {
        Self {
            session: SdkRadrootsdSignerSessionRef {
                session_id: value.session_id,
            },
            mode: value.mode,
            remote_signer_pubkey: value.remote_signer_pubkey,
            client_pubkey: value.client_pubkey,
            relays: value.relays,
        }
    }
}

#[cfg(feature = "radrootsd-client")]
#[derive(Clone, PartialEq, Eq)]
pub struct SdkRadrootsdProfilePublishOptions {
    session: SdkRadrootsdSignerSessionRef,
    idempotency_key: Option<String>,
    signer_authority: Option<radrootsd::SdkRadrootsdSignerAuthority>,
}

#[cfg(feature = "radrootsd-client")]
impl SdkRadrootsdProfilePublishOptions {
    pub fn from_signer_session(session: &SdkRadrootsdSignerSessionHandle) -> Self {
        Self {
            session: session.session().clone(),
            idempotency_key: None,
            signer_authority: None,
        }
    }

    pub fn from_signer_session_ref(session: &SdkRadrootsdSignerSessionRef) -> Self {
        Self {
            session: session.clone(),
            idempotency_key: None,
            signer_authority: None,
        }
    }

    pub fn with_idempotency_key(mut self, idempotency_key: impl Into<String>) -> Self {
        self.idempotency_key = Some(idempotency_key.into());
        self
    }

    pub fn with_signer_authority(
        mut self,
        signer_authority: radrootsd::SdkRadrootsdSignerAuthority,
    ) -> Self {
        self.signer_authority = Some(signer_authority);
        self
    }

    pub fn session(&self) -> &SdkRadrootsdSignerSessionRef {
        &self.session
    }

    pub fn idempotency_key(&self) -> Option<&str> {
        self.idempotency_key.as_deref()
    }

    pub fn signer_authority(&self) -> Option<&radrootsd::SdkRadrootsdSignerAuthority> {
        self.signer_authority.as_ref()
    }
}

#[cfg(feature = "radrootsd-client")]
impl fmt::Debug for SdkRadrootsdProfilePublishOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("SdkRadrootsdProfilePublishOptions");
        debug.field("session", &self.session);
        debug.field("idempotency_key", &self.idempotency_key);
        debug.field("signer_authority", &self.signer_authority);
        debug.finish()
    }
}

#[cfg(feature = "radrootsd-client")]
#[derive(Clone, PartialEq, Eq)]
pub struct SdkRadrootsdFarmPublishOptions {
    session: SdkRadrootsdSignerSessionRef,
    idempotency_key: Option<String>,
    signer_authority: Option<radrootsd::SdkRadrootsdSignerAuthority>,
}

#[cfg(feature = "radrootsd-client")]
impl SdkRadrootsdFarmPublishOptions {
    pub fn from_signer_session(session: &SdkRadrootsdSignerSessionHandle) -> Self {
        Self {
            session: session.session().clone(),
            idempotency_key: None,
            signer_authority: None,
        }
    }

    pub fn from_signer_session_ref(session: &SdkRadrootsdSignerSessionRef) -> Self {
        Self {
            session: session.clone(),
            idempotency_key: None,
            signer_authority: None,
        }
    }

    pub fn with_idempotency_key(mut self, idempotency_key: impl Into<String>) -> Self {
        self.idempotency_key = Some(idempotency_key.into());
        self
    }

    pub fn with_signer_authority(
        mut self,
        signer_authority: radrootsd::SdkRadrootsdSignerAuthority,
    ) -> Self {
        self.signer_authority = Some(signer_authority);
        self
    }

    pub fn session(&self) -> &SdkRadrootsdSignerSessionRef {
        &self.session
    }

    pub fn idempotency_key(&self) -> Option<&str> {
        self.idempotency_key.as_deref()
    }

    pub fn signer_authority(&self) -> Option<&radrootsd::SdkRadrootsdSignerAuthority> {
        self.signer_authority.as_ref()
    }
}

#[cfg(feature = "radrootsd-client")]
impl fmt::Debug for SdkRadrootsdFarmPublishOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("SdkRadrootsdFarmPublishOptions");
        debug.field("session", &self.session);
        debug.field("idempotency_key", &self.idempotency_key);
        debug.field("signer_authority", &self.signer_authority);
        debug.finish()
    }
}

#[cfg(feature = "radrootsd-client")]
#[derive(Clone, PartialEq, Eq)]
pub struct SdkRadrootsdListingPublishOptions {
    session: SdkRadrootsdSignerSessionRef,
    idempotency_key: Option<String>,
    signer_authority: Option<radrootsd::SdkRadrootsdSignerAuthority>,
}

#[cfg(feature = "radrootsd-client")]
impl SdkRadrootsdListingPublishOptions {
    pub fn from_signer_session(session: &SdkRadrootsdSignerSessionHandle) -> Self {
        Self {
            session: session.session().clone(),
            idempotency_key: None,
            signer_authority: None,
        }
    }

    pub fn from_signer_session_ref(session: &SdkRadrootsdSignerSessionRef) -> Self {
        Self {
            session: session.clone(),
            idempotency_key: None,
            signer_authority: None,
        }
    }

    pub fn with_idempotency_key(mut self, idempotency_key: impl Into<String>) -> Self {
        self.idempotency_key = Some(idempotency_key.into());
        self
    }

    pub fn with_signer_authority(
        mut self,
        signer_authority: radrootsd::SdkRadrootsdSignerAuthority,
    ) -> Self {
        self.signer_authority = Some(signer_authority);
        self
    }

    pub fn session(&self) -> &SdkRadrootsdSignerSessionRef {
        &self.session
    }

    pub fn idempotency_key(&self) -> Option<&str> {
        self.idempotency_key.as_deref()
    }

    pub fn signer_authority(&self) -> Option<&radrootsd::SdkRadrootsdSignerAuthority> {
        self.signer_authority.as_ref()
    }
}

#[cfg(feature = "radrootsd-client")]
impl fmt::Debug for SdkRadrootsdListingPublishOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("SdkRadrootsdListingPublishOptions");
        debug.field("session", &self.session);
        debug.field("idempotency_key", &self.idempotency_key);
        debug.field("signer_authority", &self.signer_authority);
        debug.finish()
    }
}

#[cfg(feature = "radrootsd-client")]
#[derive(Clone, PartialEq, Eq)]
pub struct SdkRadrootsdOrderRequestPublishOptions {
    session: SdkRadrootsdSignerSessionRef,
    idempotency_key: Option<String>,
    signer_authority: Option<radrootsd::SdkRadrootsdSignerAuthority>,
}

#[cfg(feature = "radrootsd-client")]
impl SdkRadrootsdOrderRequestPublishOptions {
    pub fn from_signer_session(session: &SdkRadrootsdSignerSessionHandle) -> Self {
        Self {
            session: session.session().clone(),
            idempotency_key: None,
            signer_authority: None,
        }
    }

    pub fn from_signer_session_ref(session: &SdkRadrootsdSignerSessionRef) -> Self {
        Self {
            session: session.clone(),
            idempotency_key: None,
            signer_authority: None,
        }
    }

    pub fn with_idempotency_key(mut self, idempotency_key: impl Into<String>) -> Self {
        self.idempotency_key = Some(idempotency_key.into());
        self
    }

    pub fn with_signer_authority(
        mut self,
        signer_authority: radrootsd::SdkRadrootsdSignerAuthority,
    ) -> Self {
        self.signer_authority = Some(signer_authority);
        self
    }

    pub fn session(&self) -> &SdkRadrootsdSignerSessionRef {
        &self.session
    }

    pub fn idempotency_key(&self) -> Option<&str> {
        self.idempotency_key.as_deref()
    }

    pub fn signer_authority(&self) -> Option<&radrootsd::SdkRadrootsdSignerAuthority> {
        self.signer_authority.as_ref()
    }
}

#[cfg(feature = "radrootsd-client")]
impl fmt::Debug for SdkRadrootsdOrderRequestPublishOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("SdkRadrootsdOrderRequestPublishOptions");
        debug.field("session", &self.session);
        debug.field("idempotency_key", &self.idempotency_key);
        debug.field("signer_authority", &self.signer_authority);
        debug.finish()
    }
}

#[cfg(feature = "radrootsd-client")]
#[derive(Clone, PartialEq, Eq)]
pub struct SdkRadrootsdPublicTradePublishOptions {
    session: SdkRadrootsdSignerSessionRef,
    idempotency_key: Option<String>,
}

#[cfg(feature = "radrootsd-client")]
impl SdkRadrootsdPublicTradePublishOptions {
    pub fn from_signer_session(session: &SdkRadrootsdSignerSessionHandle) -> Self {
        Self {
            session: session.session().clone(),
            idempotency_key: None,
        }
    }

    pub fn from_signer_session_ref(session: &SdkRadrootsdSignerSessionRef) -> Self {
        Self {
            session: session.clone(),
            idempotency_key: None,
        }
    }

    pub fn with_idempotency_key(mut self, idempotency_key: impl Into<String>) -> Self {
        self.idempotency_key = Some(idempotency_key.into());
        self
    }

    pub fn session(&self) -> &SdkRadrootsdSignerSessionRef {
        &self.session
    }

    pub fn idempotency_key(&self) -> Option<&str> {
        self.idempotency_key.as_deref()
    }
}

#[cfg(feature = "radrootsd-client")]
impl fmt::Debug for SdkRadrootsdPublicTradePublishOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("SdkRadrootsdPublicTradePublishOptions");
        debug.field("session", &self.session);
        debug.field("idempotency_key", &self.idempotency_key);
        debug.finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSdkClient {
    config: RadrootsSdkConfig,
    resolved_transport_target: SdkResolvedTransportTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SdkResolvedTransportTarget {
    RelayDirect { relay_urls: Vec<String> },
    Radrootsd { endpoint: String },
}

impl RadrootsSdkClient {
    pub fn from_config(config: RadrootsSdkConfig) -> Result<Self, SdkConfigError> {
        let resolved_transport_target = match config.transport {
            SdkTransportMode::RelayDirect => SdkResolvedTransportTarget::RelayDirect {
                relay_urls: config.resolved_relay_urls()?,
            },
            SdkTransportMode::Radrootsd => SdkResolvedTransportTarget::Radrootsd {
                endpoint: config.resolved_radrootsd_endpoint()?,
            },
        };
        Ok(Self {
            config,
            resolved_transport_target,
        })
    }

    pub fn config(&self) -> &RadrootsSdkConfig {
        &self.config
    }

    pub fn transport(&self) -> SdkTransportMode {
        self.config.transport
    }

    pub fn signer(&self) -> SignerConfig {
        self.config.signer
    }

    pub fn resolved_transport_target(&self) -> &SdkResolvedTransportTarget {
        &self.resolved_transport_target
    }

    pub fn profile(&self) -> ProfileClient<'_> {
        ProfileClient { client: self }
    }

    pub fn farm(&self) -> FarmClient<'_> {
        FarmClient { client: self }
    }

    pub fn listing(&self) -> ListingClient<'_> {
        ListingClient { client: self }
    }

    pub fn trade(&self) -> TradeClient<'_> {
        TradeClient { client: self }
    }

    #[cfg(feature = "radrootsd-client")]
    pub fn radrootsd(&self) -> RadrootsdClient<'_> {
        RadrootsdClient { client: self }
    }

    #[cfg(any(
        feature = "radrootsd-client",
        all(
            feature = "identity-models",
            feature = "relay-client",
            feature = "signing"
        )
    ))]
    fn require_signer_mode(
        &self,
        required: SignerConfig,
        operation: &'static str,
    ) -> Result<(), SdkPublishError> {
        let signer = self.signer();
        if signer == required {
            return Ok(());
        }
        Err(SdkPublishError::UnsupportedSignerMode {
            transport: self.transport(),
            signer,
            required,
            operation,
        })
    }

    #[cfg(all(
        feature = "identity-models",
        feature = "relay-client",
        feature = "signing"
    ))]
    async fn publish_parts_via_relay_with_identity(
        &self,
        identity: &RadrootsIdentity,
        parts: WireEventParts,
        operation: &'static str,
    ) -> Result<SdkPublishReceipt, SdkPublishError> {
        if self.transport() != SdkTransportMode::RelayDirect {
            return Err(SdkPublishError::UnsupportedTransport {
                transport: self.transport(),
                operation,
            });
        }
        self.require_signer_mode(SignerConfig::LocalIdentity, operation)?;

        let event_kind = u32::from(parts.kind);
        let relay_urls = match &self.resolved_transport_target {
            SdkResolvedTransportTarget::RelayDirect { relay_urls } => relay_urls.clone(),
            SdkResolvedTransportTarget::Radrootsd { .. } => {
                return Err(SdkPublishError::UnsupportedTransport {
                    transport: self.transport(),
                    operation,
                });
            }
        };
        let client = relay::connected_client_from_identity(
            identity,
            &relay_urls,
            Duration::from_millis(self.config.network.timeout_ms),
        )
        .await
        .map_err(|err| SdkPublishError::Relay(err.to_string()))?;
        let output = relay::publish_parts(&client, parts)
            .await
            .map_err(|err| SdkPublishError::Relay(err.to_string()))?;
        sdk_publish_receipt_from_relay_output(event_kind, output)
    }

    #[cfg(feature = "radrootsd-client")]
    async fn publish_listing_via_radrootsd(
        &self,
        request: &radrootsd::SdkRadrootsdListingPublishRequest,
    ) -> Result<SdkPublishReceipt, SdkPublishError> {
        if self.transport() != SdkTransportMode::Radrootsd {
            return Err(SdkPublishError::UnsupportedTransport {
                transport: self.transport(),
                operation: "listing.publish_via_radrootsd",
            });
        }
        self.require_signer_mode(SignerConfig::Nip46, "listing.publish_via_radrootsd")?;

        let endpoint = match &self.resolved_transport_target {
            SdkResolvedTransportTarget::Radrootsd { endpoint } => endpoint.as_str(),
            SdkResolvedTransportTarget::RelayDirect { .. } => {
                return Err(SdkPublishError::UnsupportedTransport {
                    transport: self.transport(),
                    operation: "listing.publish_via_radrootsd",
                });
            }
        };
        let response = radrootsd::publish_listing(
            endpoint,
            &self.config.radrootsd.auth,
            request,
            Duration::from_millis(self.config.network.timeout_ms),
        )
        .await
        .map_err(|err| SdkPublishError::Radrootsd(err.to_string()))?;
        Ok(sdk_publish_receipt_from_radrootsd_bridge_response(response))
    }

    #[cfg(feature = "radrootsd-client")]
    async fn publish_profile_via_radrootsd(
        &self,
        request: &radrootsd::SdkRadrootsdProfilePublishRequest,
    ) -> Result<SdkPublishReceipt, SdkPublishError> {
        if self.transport() != SdkTransportMode::Radrootsd {
            return Err(SdkPublishError::UnsupportedTransport {
                transport: self.transport(),
                operation: "profile.publish_via_radrootsd",
            });
        }
        self.require_signer_mode(SignerConfig::Nip46, "profile.publish_via_radrootsd")?;

        let endpoint = match &self.resolved_transport_target {
            SdkResolvedTransportTarget::Radrootsd { endpoint } => endpoint.as_str(),
            SdkResolvedTransportTarget::RelayDirect { .. } => {
                return Err(SdkPublishError::UnsupportedTransport {
                    transport: self.transport(),
                    operation: "profile.publish_via_radrootsd",
                });
            }
        };
        let response = radrootsd::publish_profile(
            endpoint,
            &self.config.radrootsd.auth,
            request,
            Duration::from_millis(self.config.network.timeout_ms),
        )
        .await
        .map_err(|err| SdkPublishError::Radrootsd(err.to_string()))?;
        Ok(sdk_publish_receipt_from_radrootsd_bridge_response(response))
    }

    #[cfg(feature = "radrootsd-client")]
    async fn publish_farm_via_radrootsd(
        &self,
        request: &radrootsd::SdkRadrootsdFarmPublishRequest,
    ) -> Result<SdkPublishReceipt, SdkPublishError> {
        if self.transport() != SdkTransportMode::Radrootsd {
            return Err(SdkPublishError::UnsupportedTransport {
                transport: self.transport(),
                operation: "farm.publish_via_radrootsd",
            });
        }
        self.require_signer_mode(SignerConfig::Nip46, "farm.publish_via_radrootsd")?;

        let endpoint = match &self.resolved_transport_target {
            SdkResolvedTransportTarget::Radrootsd { endpoint } => endpoint.as_str(),
            SdkResolvedTransportTarget::RelayDirect { .. } => {
                return Err(SdkPublishError::UnsupportedTransport {
                    transport: self.transport(),
                    operation: "farm.publish_via_radrootsd",
                });
            }
        };
        let response = radrootsd::publish_farm(
            endpoint,
            &self.config.radrootsd.auth,
            request,
            Duration::from_millis(self.config.network.timeout_ms),
        )
        .await
        .map_err(|err| SdkPublishError::Radrootsd(err.to_string()))?;
        Ok(sdk_publish_receipt_from_radrootsd_bridge_response(response))
    }

    #[cfg(feature = "radrootsd-client")]
    async fn publish_order_request_via_radrootsd(
        &self,
        request: &radrootsd::SdkRadrootsdOrderRequestPublishRequest,
    ) -> Result<SdkPublishReceipt, SdkPublishError> {
        if self.transport() != SdkTransportMode::Radrootsd {
            return Err(SdkPublishError::UnsupportedTransport {
                transport: self.transport(),
                operation: "trade.publish_order_request_via_radrootsd",
            });
        }
        self.require_signer_mode(
            SignerConfig::Nip46,
            "trade.publish_order_request_via_radrootsd",
        )?;

        let endpoint = match &self.resolved_transport_target {
            SdkResolvedTransportTarget::Radrootsd { endpoint } => endpoint.as_str(),
            SdkResolvedTransportTarget::RelayDirect { .. } => {
                return Err(SdkPublishError::UnsupportedTransport {
                    transport: self.transport(),
                    operation: "trade.publish_order_request_via_radrootsd",
                });
            }
        };
        let response = radrootsd::publish_order_request(
            endpoint,
            &self.config.radrootsd.auth,
            request,
            Duration::from_millis(self.config.network.timeout_ms),
        )
        .await
        .map_err(|err| SdkPublishError::Radrootsd(err.to_string()))?;
        Ok(sdk_publish_receipt_from_radrootsd_bridge_response(response))
    }

    #[cfg(feature = "radrootsd-client")]
    async fn publish_public_trade_via_radrootsd(
        &self,
        request: &radrootsd::SdkRadrootsdPublicTradePublishRequest,
        signer_session: &SdkRadrootsdSignerSessionRef,
        idempotency_key: Option<&str>,
    ) -> Result<SdkPublishReceipt, SdkPublishError> {
        if self.transport() != SdkTransportMode::Radrootsd {
            return Err(SdkPublishError::UnsupportedTransport {
                transport: self.transport(),
                operation: "trade.publish_public_message_via_radrootsd",
            });
        }
        self.require_signer_mode(
            SignerConfig::Nip46,
            "trade.publish_public_message_via_radrootsd",
        )?;

        let endpoint = match &self.resolved_transport_target {
            SdkResolvedTransportTarget::Radrootsd { endpoint } => endpoint.as_str(),
            SdkResolvedTransportTarget::RelayDirect { .. } => {
                return Err(SdkPublishError::UnsupportedTransport {
                    transport: self.transport(),
                    operation: "trade.publish_public_message_via_radrootsd",
                });
            }
        };
        let response = radrootsd::publish_public_trade(
            endpoint,
            &self.config.radrootsd.auth,
            request,
            signer_session.session_id(),
            idempotency_key,
            Duration::from_millis(self.config.network.timeout_ms),
        )
        .await
        .map_err(|err| SdkPublishError::Radrootsd(err.to_string()))?;
        Ok(sdk_publish_receipt_from_radrootsd_bridge_response(response))
    }

    #[cfg(feature = "radrootsd-client")]
    async fn connect_radrootsd_signer_session(
        &self,
        request: &radrootsd::SdkRadrootsdSignerSessionConnectRequest,
    ) -> Result<SdkRadrootsdSignerSessionHandle, SdkRadrootsdSessionError> {
        if self.transport() != SdkTransportMode::Radrootsd {
            return Err(SdkRadrootsdSessionError::UnsupportedTransport {
                transport: self.transport(),
                operation: "radrootsd.signer_sessions.connect",
            });
        }

        let endpoint = self.require_radrootsd_endpoint("radrootsd.signer_sessions.connect")?;
        let response = radrootsd::connect_signer_session(
            endpoint,
            &self.config.radrootsd.auth,
            request,
            Duration::from_millis(self.config.network.timeout_ms),
        )
        .await
        .map_err(|err| SdkRadrootsdSessionError::Radrootsd(err.to_string()))?;
        Ok(response.into())
    }

    #[cfg(feature = "radrootsd-client")]
    fn require_radrootsd_endpoint(
        &self,
        operation: &'static str,
    ) -> Result<&str, SdkRadrootsdSessionError> {
        match &self.resolved_transport_target {
            SdkResolvedTransportTarget::Radrootsd { endpoint } => Ok(endpoint.as_str()),
            SdkResolvedTransportTarget::RelayDirect { .. } => {
                Err(SdkRadrootsdSessionError::UnsupportedTransport {
                    transport: self.transport(),
                    operation,
                })
            }
        }
    }

    #[cfg(feature = "radrootsd-client")]
    async fn radrootsd_signer_session_status(
        &self,
        session: &SdkRadrootsdSignerSessionRef,
    ) -> Result<SdkRadrootsdSignerSessionView, SdkRadrootsdSessionError> {
        if self.transport() != SdkTransportMode::Radrootsd {
            return Err(SdkRadrootsdSessionError::UnsupportedTransport {
                transport: self.transport(),
                operation: "radrootsd.signer_sessions.status",
            });
        }

        let response = radrootsd::signer_session_status(
            self.require_radrootsd_endpoint("radrootsd.signer_sessions.status")?,
            &self.config.radrootsd.auth,
            session.session_id(),
            Duration::from_millis(self.config.network.timeout_ms),
        )
        .await
        .map_err(|err| SdkRadrootsdSessionError::Radrootsd(err.to_string()))?;
        Ok(response.into())
    }

    #[cfg(feature = "radrootsd-client")]
    async fn radrootsd_list_signer_sessions(
        &self,
    ) -> Result<Vec<SdkRadrootsdSignerSessionView>, SdkRadrootsdSessionError> {
        if self.transport() != SdkTransportMode::Radrootsd {
            return Err(SdkRadrootsdSessionError::UnsupportedTransport {
                transport: self.transport(),
                operation: "radrootsd.signer_sessions.list",
            });
        }

        let response = radrootsd::list_signer_sessions(
            self.require_radrootsd_endpoint("radrootsd.signer_sessions.list")?,
            &self.config.radrootsd.auth,
            Duration::from_millis(self.config.network.timeout_ms),
        )
        .await
        .map_err(|err| SdkRadrootsdSessionError::Radrootsd(err.to_string()))?;
        Ok(response.into_iter().map(Into::into).collect())
    }

    #[cfg(feature = "radrootsd-client")]
    async fn authorize_radrootsd_signer_session(
        &self,
        session: &SdkRadrootsdSignerSessionRef,
    ) -> Result<SdkRadrootsdSignerSessionAuthorizeResult, SdkRadrootsdSessionError> {
        if self.transport() != SdkTransportMode::Radrootsd {
            return Err(SdkRadrootsdSessionError::UnsupportedTransport {
                transport: self.transport(),
                operation: "radrootsd.signer_sessions.authorize",
            });
        }

        let response = radrootsd::authorize_signer_session(
            self.require_radrootsd_endpoint("radrootsd.signer_sessions.authorize")?,
            &self.config.radrootsd.auth,
            session.session_id(),
            Duration::from_millis(self.config.network.timeout_ms),
        )
        .await
        .map_err(|err| SdkRadrootsdSessionError::Radrootsd(err.to_string()))?;
        Ok(response.into())
    }

    #[cfg(feature = "radrootsd-client")]
    async fn get_radrootsd_signer_session_public_key(
        &self,
        session: &SdkRadrootsdSignerSessionRef,
    ) -> Result<SdkRadrootsdSignerSessionPublicKeyResult, SdkRadrootsdSessionError> {
        if self.transport() != SdkTransportMode::Radrootsd {
            return Err(SdkRadrootsdSessionError::UnsupportedTransport {
                transport: self.transport(),
                operation: "radrootsd.signer_sessions.get_public_key",
            });
        }

        let response = radrootsd::get_signer_session_public_key(
            self.require_radrootsd_endpoint("radrootsd.signer_sessions.get_public_key")?,
            &self.config.radrootsd.auth,
            session.session_id(),
            Duration::from_millis(self.config.network.timeout_ms),
        )
        .await
        .map_err(|err| SdkRadrootsdSessionError::Radrootsd(err.to_string()))?;
        Ok(response.into())
    }

    #[cfg(feature = "radrootsd-client")]
    async fn require_radrootsd_signer_session_auth(
        &self,
        session: &SdkRadrootsdSignerSessionRef,
        auth_url: &str,
    ) -> Result<SdkRadrootsdSignerSessionRequireAuthResult, SdkRadrootsdSessionError> {
        if self.transport() != SdkTransportMode::Radrootsd {
            return Err(SdkRadrootsdSessionError::UnsupportedTransport {
                transport: self.transport(),
                operation: "radrootsd.signer_sessions.require_auth",
            });
        }

        let response = radrootsd::require_signer_session_auth(
            self.require_radrootsd_endpoint("radrootsd.signer_sessions.require_auth")?,
            &self.config.radrootsd.auth,
            session.session_id(),
            auth_url,
            Duration::from_millis(self.config.network.timeout_ms),
        )
        .await
        .map_err(|err| SdkRadrootsdSessionError::Radrootsd(err.to_string()))?;
        Ok(response.into())
    }

    #[cfg(feature = "radrootsd-client")]
    async fn close_radrootsd_signer_session(
        &self,
        session: &SdkRadrootsdSignerSessionRef,
    ) -> Result<SdkRadrootsdSignerSessionCloseResult, SdkRadrootsdSessionError> {
        if self.transport() != SdkTransportMode::Radrootsd {
            return Err(SdkRadrootsdSessionError::UnsupportedTransport {
                transport: self.transport(),
                operation: "radrootsd.signer_sessions.close",
            });
        }

        let response = radrootsd::close_signer_session(
            self.require_radrootsd_endpoint("radrootsd.signer_sessions.close")?,
            &self.config.radrootsd.auth,
            session.session_id(),
            Duration::from_millis(self.config.network.timeout_ms),
        )
        .await
        .map_err(|err| SdkRadrootsdSessionError::Radrootsd(err.to_string()))?;
        Ok(response.into())
    }

    #[cfg(feature = "radrootsd-client")]
    fn require_radrootsd_bridge_endpoint(
        &self,
        operation: &'static str,
    ) -> Result<&str, SdkRadrootsdBridgeError> {
        match &self.resolved_transport_target {
            SdkResolvedTransportTarget::Radrootsd { endpoint } => Ok(endpoint.as_str()),
            SdkResolvedTransportTarget::RelayDirect { .. } => {
                Err(SdkRadrootsdBridgeError::UnsupportedTransport {
                    transport: self.transport(),
                    operation,
                })
            }
        }
    }

    #[cfg(feature = "radrootsd-client")]
    async fn radrootsd_bridge_status(
        &self,
    ) -> Result<SdkRadrootsdBridgeStatus, SdkRadrootsdBridgeError> {
        if self.transport() != SdkTransportMode::Radrootsd {
            return Err(SdkRadrootsdBridgeError::UnsupportedTransport {
                transport: self.transport(),
                operation: "radrootsd.bridge.status",
            });
        }

        let response = radrootsd::bridge_status(
            self.require_radrootsd_bridge_endpoint("radrootsd.bridge.status")?,
            &self.config.radrootsd.auth,
            Duration::from_millis(self.config.network.timeout_ms),
        )
        .await
        .map_err(|err| SdkRadrootsdBridgeError::Radrootsd(err.to_string()))?;
        Ok(response.into())
    }

    #[cfg(feature = "radrootsd-client")]
    async fn radrootsd_bridge_job_status(
        &self,
        job: &SdkRadrootsdBridgeJobRef,
    ) -> Result<SdkRadrootsdBridgeJobView, SdkRadrootsdBridgeError> {
        if self.transport() != SdkTransportMode::Radrootsd {
            return Err(SdkRadrootsdBridgeError::UnsupportedTransport {
                transport: self.transport(),
                operation: "radrootsd.bridge.job",
            });
        }

        let response = radrootsd::bridge_job_status(
            self.require_radrootsd_bridge_endpoint("radrootsd.bridge.job")?,
            &self.config.radrootsd.auth,
            job.job_id(),
            Duration::from_millis(self.config.network.timeout_ms),
        )
        .await
        .map_err(|err| SdkRadrootsdBridgeError::Radrootsd(err.to_string()))?;
        Ok(response.into())
    }

    #[cfg(feature = "radrootsd-client")]
    async fn radrootsd_bridge_jobs(
        &self,
    ) -> Result<Vec<SdkRadrootsdBridgeJobView>, SdkRadrootsdBridgeError> {
        if self.transport() != SdkTransportMode::Radrootsd {
            return Err(SdkRadrootsdBridgeError::UnsupportedTransport {
                transport: self.transport(),
                operation: "radrootsd.bridge.jobs",
            });
        }

        let response = radrootsd::list_bridge_jobs(
            self.require_radrootsd_bridge_endpoint("radrootsd.bridge.jobs")?,
            &self.config.radrootsd.auth,
            Duration::from_millis(self.config.network.timeout_ms),
        )
        .await
        .map_err(|err| SdkRadrootsdBridgeError::Radrootsd(err.to_string()))?;
        Ok(response.into_iter().map(Into::into).collect())
    }
}

#[cfg(feature = "radrootsd-client")]
#[derive(Clone, PartialEq, Eq)]
pub struct SdkRadrootsdPublicTradeMessage {
    request: radrootsd::SdkRadrootsdPublicTradePublishRequest,
}

#[cfg(feature = "radrootsd-client")]
impl SdkRadrootsdPublicTradeMessage {
    pub fn order_response(
        route: &radrootsd::SdkRadrootsdPublicTradeRoute,
        chain: &radrootsd::SdkRadrootsdTradeChain,
        payload: trade::RadrootsTradeOrderResponse,
    ) -> Result<Self, radrootsd::SdkRadrootsdPublicTradePublishValidationError> {
        Ok(Self {
            request: radrootsd::SdkRadrootsdPublicTradePublishRequest::order_response(
                route, chain, payload,
            )?,
        })
    }

    pub fn order_revision(
        route: &radrootsd::SdkRadrootsdPublicTradeRoute,
        listing_event: RadrootsNostrEventPtr,
        chain: &radrootsd::SdkRadrootsdTradeChain,
        payload: trade::RadrootsTradeOrderRevision,
    ) -> Result<Self, radrootsd::SdkRadrootsdPublicTradePublishValidationError> {
        Ok(Self {
            request: radrootsd::SdkRadrootsdPublicTradePublishRequest::order_revision(
                route,
                listing_event,
                chain,
                payload,
            )?,
        })
    }

    pub fn order_revision_accept(
        route: &radrootsd::SdkRadrootsdPublicTradeRoute,
        chain: &radrootsd::SdkRadrootsdTradeChain,
        payload: trade::RadrootsTradeOrderRevisionResponse,
    ) -> Result<Self, radrootsd::SdkRadrootsdPublicTradePublishValidationError> {
        Ok(Self {
            request: radrootsd::SdkRadrootsdPublicTradePublishRequest::order_revision_accept(
                route, chain, payload,
            )?,
        })
    }

    pub fn order_revision_decline(
        route: &radrootsd::SdkRadrootsdPublicTradeRoute,
        chain: &radrootsd::SdkRadrootsdTradeChain,
        payload: trade::RadrootsTradeOrderRevisionResponse,
    ) -> Result<Self, radrootsd::SdkRadrootsdPublicTradePublishValidationError> {
        Ok(Self {
            request: radrootsd::SdkRadrootsdPublicTradePublishRequest::order_revision_decline(
                route, chain, payload,
            )?,
        })
    }

    pub fn question(
        route: &radrootsd::SdkRadrootsdPublicTradeRoute,
        chain: &radrootsd::SdkRadrootsdTradeChain,
        payload: trade::RadrootsTradeQuestion,
    ) -> Result<Self, radrootsd::SdkRadrootsdPublicTradePublishValidationError> {
        Ok(Self {
            request: radrootsd::SdkRadrootsdPublicTradePublishRequest::question(
                route, chain, payload,
            )?,
        })
    }

    pub fn answer(
        route: &radrootsd::SdkRadrootsdPublicTradeRoute,
        chain: &radrootsd::SdkRadrootsdTradeChain,
        payload: trade::RadrootsTradeAnswer,
    ) -> Result<Self, radrootsd::SdkRadrootsdPublicTradePublishValidationError> {
        Ok(Self {
            request: radrootsd::SdkRadrootsdPublicTradePublishRequest::answer(
                route, chain, payload,
            )?,
        })
    }

    pub fn discount_request(
        route: &radrootsd::SdkRadrootsdPublicTradeRoute,
        listing_event: RadrootsNostrEventPtr,
        chain: &radrootsd::SdkRadrootsdTradeChain,
        payload: trade::RadrootsTradeDiscountRequest,
    ) -> Result<Self, radrootsd::SdkRadrootsdPublicTradePublishValidationError> {
        Ok(Self {
            request: radrootsd::SdkRadrootsdPublicTradePublishRequest::discount_request(
                route,
                listing_event,
                chain,
                payload,
            )?,
        })
    }

    pub fn discount_offer(
        route: &radrootsd::SdkRadrootsdPublicTradeRoute,
        listing_event: RadrootsNostrEventPtr,
        chain: &radrootsd::SdkRadrootsdTradeChain,
        payload: trade::RadrootsTradeDiscountOffer,
    ) -> Result<Self, radrootsd::SdkRadrootsdPublicTradePublishValidationError> {
        Ok(Self {
            request: radrootsd::SdkRadrootsdPublicTradePublishRequest::discount_offer(
                route,
                listing_event,
                chain,
                payload,
            )?,
        })
    }

    pub fn discount_accept(
        route: &radrootsd::SdkRadrootsdPublicTradeRoute,
        chain: &radrootsd::SdkRadrootsdTradeChain,
        payload: trade::RadrootsTradeDiscountDecision,
    ) -> Result<Self, radrootsd::SdkRadrootsdPublicTradePublishValidationError> {
        Ok(Self {
            request: radrootsd::SdkRadrootsdPublicTradePublishRequest::discount_accept(
                route, chain, payload,
            )?,
        })
    }

    pub fn discount_decline(
        route: &radrootsd::SdkRadrootsdPublicTradeRoute,
        chain: &radrootsd::SdkRadrootsdTradeChain,
        payload: trade::RadrootsTradeDiscountDecision,
    ) -> Result<Self, radrootsd::SdkRadrootsdPublicTradePublishValidationError> {
        Ok(Self {
            request: radrootsd::SdkRadrootsdPublicTradePublishRequest::discount_decline(
                route, chain, payload,
            )?,
        })
    }

    pub fn cancel(
        route: &radrootsd::SdkRadrootsdPublicTradeRoute,
        chain: &radrootsd::SdkRadrootsdTradeChain,
        payload: trade::RadrootsTradeListingCancel,
    ) -> Result<Self, radrootsd::SdkRadrootsdPublicTradePublishValidationError> {
        Ok(Self {
            request: radrootsd::SdkRadrootsdPublicTradePublishRequest::cancel(
                route, chain, payload,
            )?,
        })
    }

    pub fn fulfillment_update(
        route: &radrootsd::SdkRadrootsdPublicTradeRoute,
        chain: &radrootsd::SdkRadrootsdTradeChain,
        payload: trade::RadrootsTradeFulfillmentUpdate,
    ) -> Result<Self, radrootsd::SdkRadrootsdPublicTradePublishValidationError> {
        Ok(Self {
            request: radrootsd::SdkRadrootsdPublicTradePublishRequest::fulfillment_update(
                route, chain, payload,
            )?,
        })
    }

    pub fn receipt(
        route: &radrootsd::SdkRadrootsdPublicTradeRoute,
        chain: &radrootsd::SdkRadrootsdTradeChain,
        payload: trade::RadrootsTradeReceipt,
    ) -> Result<Self, radrootsd::SdkRadrootsdPublicTradePublishValidationError> {
        Ok(Self {
            request: radrootsd::SdkRadrootsdPublicTradePublishRequest::receipt(
                route, chain, payload,
            )?,
        })
    }

    pub(crate) fn as_request(&self) -> &radrootsd::SdkRadrootsdPublicTradePublishRequest {
        &self.request
    }
}

#[cfg(feature = "radrootsd-client")]
impl fmt::Debug for SdkRadrootsdPublicTradeMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SdkRadrootsdPublicTradeMessage")
            .field("request", &self.request)
            .finish()
    }
}

#[cfg(feature = "radrootsd-client")]
#[derive(Debug, Clone, Copy)]
pub struct RadrootsdClient<'a> {
    client: &'a RadrootsSdkClient,
}

#[cfg(feature = "radrootsd-client")]
impl<'a> RadrootsdClient<'a> {
    pub fn sdk(&self) -> &'a RadrootsSdkClient {
        self.client
    }

    pub fn transport(&self) -> SdkTransportMode {
        self.client.transport()
    }

    pub fn signer(&self) -> SignerConfig {
        self.client.signer()
    }

    pub fn signer_sessions(&self) -> RadrootsdSignerSessionClient<'a> {
        RadrootsdSignerSessionClient {
            client: self.client,
        }
    }

    pub fn bridge(&self) -> RadrootsdBridgeClient<'a> {
        RadrootsdBridgeClient {
            client: self.client,
        }
    }
}

#[cfg(feature = "radrootsd-client")]
#[derive(Debug, Clone, Copy)]
pub struct RadrootsdSignerSessionClient<'a> {
    client: &'a RadrootsSdkClient,
}

#[cfg(feature = "radrootsd-client")]
impl<'a> RadrootsdSignerSessionClient<'a> {
    pub fn sdk(&self) -> &'a RadrootsSdkClient {
        self.client
    }

    pub fn transport(&self) -> SdkTransportMode {
        self.client.transport()
    }

    pub fn signer(&self) -> SignerConfig {
        self.client.signer()
    }

    pub async fn connect(
        &self,
        request: &radrootsd::SdkRadrootsdSignerSessionConnectRequest,
    ) -> Result<SdkRadrootsdSignerSessionHandle, SdkRadrootsdSessionError> {
        self.client.connect_radrootsd_signer_session(request).await
    }

    pub async fn connect_bunker(
        &self,
        url: impl Into<String>,
    ) -> Result<SdkRadrootsdSignerSessionHandle, SdkRadrootsdSessionError> {
        let request = radrootsd::SdkRadrootsdSignerSessionConnectRequest::bunker(url);
        self.connect(&request).await
    }

    pub async fn connect_nostrconnect(
        &self,
        url: impl Into<String>,
        client_secret_key: impl Into<String>,
    ) -> Result<SdkRadrootsdSignerSessionHandle, SdkRadrootsdSessionError> {
        let request = radrootsd::SdkRadrootsdSignerSessionConnectRequest::nostrconnect(
            url,
            client_secret_key,
        );
        self.connect(&request).await
    }

    pub async fn status(
        &self,
        session: &SdkRadrootsdSignerSessionRef,
    ) -> Result<SdkRadrootsdSignerSessionView, SdkRadrootsdSessionError> {
        self.client.radrootsd_signer_session_status(session).await
    }

    pub async fn list(
        &self,
    ) -> Result<Vec<SdkRadrootsdSignerSessionView>, SdkRadrootsdSessionError> {
        self.client.radrootsd_list_signer_sessions().await
    }

    pub async fn authorize(
        &self,
        session: &SdkRadrootsdSignerSessionRef,
    ) -> Result<SdkRadrootsdSignerSessionAuthorizeResult, SdkRadrootsdSessionError> {
        self.client
            .authorize_radrootsd_signer_session(session)
            .await
    }

    pub async fn get_public_key(
        &self,
        session: &SdkRadrootsdSignerSessionRef,
    ) -> Result<SdkRadrootsdSignerSessionPublicKeyResult, SdkRadrootsdSessionError> {
        self.client
            .get_radrootsd_signer_session_public_key(session)
            .await
    }

    pub async fn require_auth(
        &self,
        session: &SdkRadrootsdSignerSessionRef,
        auth_url: impl AsRef<str>,
    ) -> Result<SdkRadrootsdSignerSessionRequireAuthResult, SdkRadrootsdSessionError> {
        self.client
            .require_radrootsd_signer_session_auth(session, auth_url.as_ref())
            .await
    }

    pub async fn close(
        &self,
        session: &SdkRadrootsdSignerSessionRef,
    ) -> Result<SdkRadrootsdSignerSessionCloseResult, SdkRadrootsdSessionError> {
        self.client.close_radrootsd_signer_session(session).await
    }
}

#[cfg(feature = "radrootsd-client")]
#[derive(Debug, Clone, Copy)]
pub struct RadrootsdBridgeClient<'a> {
    client: &'a RadrootsSdkClient,
}

#[cfg(feature = "radrootsd-client")]
impl<'a> RadrootsdBridgeClient<'a> {
    pub fn sdk(&self) -> &'a RadrootsSdkClient {
        self.client
    }

    pub fn transport(&self) -> SdkTransportMode {
        self.client.transport()
    }

    pub fn signer(&self) -> SignerConfig {
        self.client.signer()
    }

    pub async fn status(&self) -> Result<SdkRadrootsdBridgeStatus, SdkRadrootsdBridgeError> {
        self.client.radrootsd_bridge_status().await
    }

    pub async fn job(
        &self,
        job: &SdkRadrootsdBridgeJobRef,
    ) -> Result<SdkRadrootsdBridgeJobView, SdkRadrootsdBridgeError> {
        self.client.radrootsd_bridge_job_status(job).await
    }

    pub async fn jobs(&self) -> Result<Vec<SdkRadrootsdBridgeJobView>, SdkRadrootsdBridgeError> {
        self.client.radrootsd_bridge_jobs().await
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ProfileClient<'a> {
    client: &'a RadrootsSdkClient,
}

impl<'a> ProfileClient<'a> {
    pub fn sdk(&self) -> &'a RadrootsSdkClient {
        self.client
    }

    pub fn transport(&self) -> SdkTransportMode {
        self.client.transport()
    }

    pub fn signer(&self) -> SignerConfig {
        self.client.signer()
    }

    #[cfg(feature = "serde_json")]
    pub fn build_draft(
        &self,
        profile_value: &RadrootsProfile,
        profile_type: Option<RadrootsProfileType>,
    ) -> Result<WireEventParts, profile::ProfileEncodeError> {
        profile::build_draft(profile_value, profile_type)
    }

    #[cfg(feature = "radrootsd-client")]
    pub async fn publish_profile_via_radrootsd(
        &self,
        profile_value: &RadrootsProfile,
        profile_type: Option<RadrootsProfileType>,
        session: &SdkRadrootsdSignerSessionHandle,
    ) -> Result<SdkPublishReceipt, SdkPublishError> {
        self.publish_profile_via_radrootsd_with_options(
            profile_value,
            profile_type,
            &SdkRadrootsdProfilePublishOptions::from_signer_session(session),
        )
        .await
    }

    #[cfg(feature = "radrootsd-client")]
    pub async fn publish_profile_via_radrootsd_with_options(
        &self,
        profile_value: &RadrootsProfile,
        profile_type: Option<RadrootsProfileType>,
        options: &SdkRadrootsdProfilePublishOptions,
    ) -> Result<SdkPublishReceipt, SdkPublishError> {
        let request = radrootsd::SdkRadrootsdProfilePublishRequest {
            profile: profile_value.clone(),
            profile_type,
            signer_session_id: options.session().session_id().to_owned(),
            signer_authority: options.signer_authority().cloned(),
            idempotency_key: options.idempotency_key().map(str::to_owned),
        };
        self.client.publish_profile_via_radrootsd(&request).await
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FarmClient<'a> {
    client: &'a RadrootsSdkClient,
}

impl<'a> FarmClient<'a> {
    pub fn sdk(&self) -> &'a RadrootsSdkClient {
        self.client
    }

    pub fn transport(&self) -> SdkTransportMode {
        self.client.transport()
    }

    pub fn signer(&self) -> SignerConfig {
        self.client.signer()
    }

    #[cfg(feature = "serde_json")]
    pub fn build_draft(
        &self,
        farm_value: &farm::RadrootsFarm,
    ) -> Result<WireEventParts, farm::EventEncodeError> {
        farm::build_draft(farm_value)
    }

    #[cfg(feature = "radrootsd-client")]
    pub async fn publish_farm_via_radrootsd(
        &self,
        farm_value: &farm::RadrootsFarm,
        session: &SdkRadrootsdSignerSessionHandle,
    ) -> Result<SdkPublishReceipt, SdkPublishError> {
        self.publish_farm_via_radrootsd_with_options(
            farm_value,
            &SdkRadrootsdFarmPublishOptions::from_signer_session(session),
        )
        .await
    }

    #[cfg(feature = "radrootsd-client")]
    pub async fn publish_farm_via_radrootsd_with_options(
        &self,
        farm_value: &farm::RadrootsFarm,
        options: &SdkRadrootsdFarmPublishOptions,
    ) -> Result<SdkPublishReceipt, SdkPublishError> {
        let request = radrootsd::SdkRadrootsdFarmPublishRequest {
            farm: farm_value.clone(),
            kind: Some(KIND_FARM),
            signer_session_id: options.session().session_id().to_owned(),
            signer_authority: options.signer_authority().cloned(),
            idempotency_key: options.idempotency_key().map(str::to_owned),
        };
        self.client.publish_farm_via_radrootsd(&request).await
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ListingClient<'a> {
    client: &'a RadrootsSdkClient,
}

impl<'a> ListingClient<'a> {
    pub fn sdk(&self) -> &'a RadrootsSdkClient {
        self.client
    }

    pub fn transport(&self) -> SdkTransportMode {
        self.client.transport()
    }

    pub fn signer(&self) -> SignerConfig {
        self.client.signer()
    }

    pub fn build_tags(
        &self,
        listing_value: &listing::RadrootsListing,
    ) -> Result<NostrTags, listing::EventEncodeError> {
        listing::build_tags(listing_value)
    }

    #[cfg(feature = "serde_json")]
    pub fn build_draft(
        &self,
        listing_value: &listing::RadrootsListing,
    ) -> Result<listing::RadrootsListingDraft, listing::EventEncodeError> {
        listing::build_draft(listing_value)
    }

    #[cfg(feature = "serde_json")]
    pub fn parse_event(
        &self,
        event: &RadrootsNostrEvent,
    ) -> Result<listing::RadrootsListing, listing::RadrootsTradeListingParseError> {
        listing::parse_event(event)
    }

    #[cfg(all(
        feature = "identity-models",
        feature = "relay-client",
        feature = "signing"
    ))]
    pub async fn publish_with_identity(
        &self,
        identity: &RadrootsIdentity,
        listing_value: &listing::RadrootsListing,
    ) -> Result<SdkPublishReceipt, SdkPublishError> {
        let parts = listing::build_draft(listing_value)
            .map_err(|err| SdkPublishError::Encode(err.to_string()))?
            .into_wire_parts();
        self.client
            .publish_parts_via_relay_with_identity(identity, parts, "listing.publish_with_identity")
            .await
    }

    #[cfg(all(
        feature = "identity-models",
        feature = "relay-client",
        feature = "signing"
    ))]
    pub async fn publish_draft_with_identity(
        &self,
        identity: &RadrootsIdentity,
        draft: listing::RadrootsListingDraft,
    ) -> Result<SdkPublishReceipt, SdkPublishError> {
        self.client
            .publish_parts_via_relay_with_identity(
                identity,
                draft.into_wire_parts(),
                "listing.publish_draft_with_identity",
            )
            .await
    }

    #[cfg(feature = "radrootsd-client")]
    pub async fn publish_listing_via_radrootsd(
        &self,
        listing_value: &listing::RadrootsListing,
        session: &SdkRadrootsdSignerSessionHandle,
    ) -> Result<SdkPublishReceipt, SdkPublishError> {
        self.publish_listing_via_radrootsd_with_options(
            listing_value,
            &SdkRadrootsdListingPublishOptions::from_signer_session(session),
        )
        .await
    }

    #[cfg(feature = "radrootsd-client")]
    pub async fn publish_listing_via_radrootsd_with_options(
        &self,
        listing_value: &listing::RadrootsListing,
        options: &SdkRadrootsdListingPublishOptions,
    ) -> Result<SdkPublishReceipt, SdkPublishError> {
        let request = radrootsd::SdkRadrootsdListingPublishRequest {
            listing: listing_value.clone(),
            kind: Some(KIND_LISTING),
            signer_session_id: options.session().session_id().to_owned(),
            signer_authority: options.signer_authority().cloned(),
            idempotency_key: options.idempotency_key().map(str::to_owned),
        };
        self.client.publish_listing_via_radrootsd(&request).await
    }

    #[cfg(feature = "radrootsd-client")]
    pub async fn publish_draft_via_radrootsd(
        &self,
        draft: listing::RadrootsListingDraft,
        session: &SdkRadrootsdSignerSessionHandle,
    ) -> Result<SdkPublishReceipt, SdkPublishError> {
        self.publish_draft_via_radrootsd_with_options(
            draft,
            &SdkRadrootsdListingPublishOptions::from_signer_session(session),
        )
        .await
    }

    #[cfg(feature = "radrootsd-client")]
    pub async fn publish_draft_via_radrootsd_with_options(
        &self,
        draft: listing::RadrootsListingDraft,
        options: &SdkRadrootsdListingPublishOptions,
    ) -> Result<SdkPublishReceipt, SdkPublishError> {
        let parts = draft.into_wire_parts();
        let event = RadrootsNostrEvent {
            id: String::new(),
            author: String::new(),
            created_at: 0,
            kind: parts.kind,
            tags: parts.tags,
            content: parts.content,
            sig: String::new(),
        };
        let request = radrootsd::SdkRadrootsdListingPublishRequest::from_event(
            &event,
            options.session().session_id().to_owned(),
            options.signer_authority().cloned(),
            options.idempotency_key().map(str::to_owned),
        )
        .map_err(|err| SdkPublishError::Encode(err.to_string()))?;
        self.client.publish_listing_via_radrootsd(&request).await
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TradeClient<'a> {
    client: &'a RadrootsSdkClient,
}

impl<'a> TradeClient<'a> {
    pub fn sdk(&self) -> &'a RadrootsSdkClient {
        self.client
    }

    pub fn transport(&self) -> SdkTransportMode {
        self.client.transport()
    }

    pub fn signer(&self) -> SignerConfig {
        self.client.signer()
    }

    #[cfg(feature = "serde_json")]
    #[allow(clippy::too_many_arguments)]
    pub fn build_envelope_draft(
        &self,
        recipient_pubkey: impl Into<String>,
        message_type: trade::RadrootsTradeMessageType,
        listing_addr: impl Into<String>,
        order_id: Option<String>,
        listing_event: Option<&RadrootsNostrEventPtr>,
        root_event_id: Option<&str>,
        prev_event_id: Option<&str>,
        payload: &trade::RadrootsTradeMessagePayload,
    ) -> Result<WireEventParts, trade::EventEncodeError> {
        trade::build_envelope_draft(
            recipient_pubkey,
            message_type,
            listing_addr,
            order_id,
            listing_event,
            root_event_id,
            prev_event_id,
            payload,
        )
    }

    #[cfg(feature = "serde_json")]
    pub fn parse_envelope(
        &self,
        event: &RadrootsNostrEvent,
    ) -> Result<RadrootsTradeEnvelope, trade::RadrootsTradeEnvelopeParseError> {
        trade::parse_envelope(event)
    }

    #[cfg(feature = "serde_json")]
    pub fn parse_listing_address(
        &self,
        listing_addr: &str,
    ) -> Result<trade::RadrootsTradeListingAddress, trade::RadrootsTradeListingAddressError> {
        trade::parse_listing_address(listing_addr)
    }

    #[cfg(feature = "serde_json")]
    pub fn validate_listing_event(
        &self,
        event: &RadrootsNostrEvent,
    ) -> Result<TradeListingValidateResult, trade::RadrootsTradeListingValidationError> {
        trade::validate_listing_event(event)
    }

    #[cfg(feature = "radrootsd-client")]
    pub async fn publish_order_request_via_radrootsd(
        &self,
        order: &trade::RadrootsTradeOrder,
        session: &SdkRadrootsdSignerSessionHandle,
    ) -> Result<SdkPublishReceipt, SdkPublishError> {
        self.publish_order_request_via_radrootsd_with_options(
            order,
            &SdkRadrootsdOrderRequestPublishOptions::from_signer_session(session),
        )
        .await
    }

    #[cfg(feature = "radrootsd-client")]
    pub async fn publish_order_request_via_radrootsd_with_options(
        &self,
        order: &trade::RadrootsTradeOrder,
        options: &SdkRadrootsdOrderRequestPublishOptions,
    ) -> Result<SdkPublishReceipt, SdkPublishError> {
        let request = radrootsd::SdkRadrootsdOrderRequestPublishRequest {
            order: order.clone(),
            signer_session_id: options.session().session_id().to_owned(),
            signer_authority: options.signer_authority().cloned(),
            idempotency_key: options.idempotency_key().map(str::to_owned),
        };
        self.client
            .publish_order_request_via_radrootsd(&request)
            .await
    }

    #[cfg(feature = "radrootsd-client")]
    pub async fn publish_public_message_via_radrootsd(
        &self,
        message: &SdkRadrootsdPublicTradeMessage,
        session: &SdkRadrootsdSignerSessionHandle,
    ) -> Result<SdkPublishReceipt, SdkPublishError> {
        self.publish_public_message_via_radrootsd_with_options(
            message,
            &SdkRadrootsdPublicTradePublishOptions::from_signer_session(session),
        )
        .await
    }

    #[cfg(feature = "radrootsd-client")]
    pub async fn publish_public_message_via_radrootsd_with_options(
        &self,
        message: &SdkRadrootsdPublicTradeMessage,
        options: &SdkRadrootsdPublicTradePublishOptions,
    ) -> Result<SdkPublishReceipt, SdkPublishError> {
        self.client
            .publish_public_trade_via_radrootsd(
                message.as_request(),
                options.session(),
                options.idempotency_key(),
            )
            .await
    }
}

#[cfg(all(
    feature = "identity-models",
    feature = "relay-client",
    feature = "signing"
))]
fn sdk_publish_receipt_from_relay_output(
    event_kind: u32,
    output: relay::RelayOutput<relay::RelayEventId>,
) -> Result<SdkPublishReceipt, SdkPublishError> {
    let mut acknowledged_relays = output
        .success
        .into_iter()
        .map(|relay| relay.to_string())
        .collect::<Vec<_>>();
    acknowledged_relays.sort();

    let mut failed_relays = output
        .failed
        .into_iter()
        .map(|(relay_url, error)| SdkRelayFailure {
            relay_url: relay_url.to_string(),
            error,
        })
        .collect::<Vec<_>>();
    failed_relays.sort_by(|left, right| left.relay_url.cmp(&right.relay_url));

    if acknowledged_relays.is_empty() {
        return Err(SdkPublishError::RelayNotAcknowledged {
            transport: SdkTransportMode::RelayDirect,
            failed_relays,
        });
    }

    Ok(SdkPublishReceipt {
        transport: SdkTransportMode::RelayDirect,
        event_kind: Some(event_kind),
        event_id: Some(output.val.to_string()),
        transport_receipt: SdkTransportReceipt::RelayDirect(SdkRelayPublishReceipt {
            acknowledged_relays,
            failed_relays,
        }),
    })
}

#[cfg(feature = "radrootsd-client")]
fn sdk_publish_receipt_from_radrootsd_bridge_response(
    response: radrootsd::SdkRadrootsdBridgePublishResponse,
) -> SdkPublishReceipt {
    let job = response.job;
    SdkPublishReceipt {
        transport: SdkTransportMode::Radrootsd,
        event_kind: Some(job.event_kind),
        event_id: job.event_id.clone(),
        transport_receipt: SdkTransportReceipt::Radrootsd(SdkRadrootsdPublishReceipt {
            accepted: true,
            deduplicated: response.deduplicated,
            job_id: Some(job.job_id),
            status: Some(job.status),
            signer_mode: Some(job.signer_mode),
            signer_session_id: job.signer_session_id,
            event_addr: job.event_addr,
            relay_count: Some(job.relay_count),
            acknowledged_relay_count: Some(job.acknowledged_relay_count),
        }),
    }
}

#[cfg(all(
    test,
    feature = "identity-models",
    feature = "relay-client",
    feature = "signing"
))]
mod tests {
    use super::{
        SdkPublishError, SdkRelayFailure, SdkTransportMode, sdk_publish_receipt_from_relay_output,
    };
    use crate::adapters::relay::RelayOutput;
    use radroots_nostr::prelude::RadrootsNostrEventId;
    use std::collections::{HashMap, HashSet};

    #[test]
    fn relay_output_maps_to_normalized_publish_receipt() {
        let output = RelayOutput {
            val: RadrootsNostrEventId::parse(
                "5f3cf27d85c9571a2dca28269f6547f625364a7e06e5e853ee1bc74d2c4aa3d4",
            )
            .expect("event id"),
            success: HashSet::from([
                nostr::RelayUrl::parse("ws://127.0.0.1:8080").expect("relay a"),
                nostr::RelayUrl::parse("ws://127.0.0.1:8081").expect("relay b"),
            ]),
            failed: HashMap::from([(
                nostr::RelayUrl::parse("ws://127.0.0.1:8082").expect("relay c"),
                "timeout".to_owned(),
            )]),
        };

        let receipt = sdk_publish_receipt_from_relay_output(30402, output).expect("receipt");

        assert_eq!(receipt.transport, SdkTransportMode::RelayDirect);
        assert_eq!(receipt.event_kind, Some(30402));
        assert_eq!(
            receipt.event_id,
            Some("5f3cf27d85c9571a2dca28269f6547f625364a7e06e5e853ee1bc74d2c4aa3d4".to_owned())
        );
    }

    #[test]
    fn relay_output_without_acknowledgement_is_rejected() {
        let output = RelayOutput {
            val: RadrootsNostrEventId::parse(
                "5f3cf27d85c9571a2dca28269f6547f625364a7e06e5e853ee1bc74d2c4aa3d4",
            )
            .expect("event id"),
            success: HashSet::new(),
            failed: HashMap::from([(
                nostr::RelayUrl::parse("ws://127.0.0.1:8082").expect("relay c"),
                "blocked".to_owned(),
            )]),
        };

        let error = sdk_publish_receipt_from_relay_output(30402, output).expect_err("error");

        assert_eq!(
            error,
            SdkPublishError::RelayNotAcknowledged {
                transport: SdkTransportMode::RelayDirect,
                failed_relays: vec![SdkRelayFailure {
                    relay_url: "ws://127.0.0.1:8082".to_owned(),
                    error: "blocked".to_owned(),
                }],
            }
        );
    }
}
