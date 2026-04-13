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
use radroots_events::kinds::KIND_LISTING;

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
#[derive(Clone, PartialEq, Eq)]
pub struct SdkRadrootsdSignerSessionHandle {
    session_id: String,
    mode: radrootsd::SdkRadrootsdSignerSessionMode,
    remote_signer_pubkey: String,
    client_pubkey: String,
    relays: Vec<String>,
}

#[cfg(feature = "radrootsd-client")]
impl fmt::Debug for SdkRadrootsdSignerSessionHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("SdkRadrootsdSignerSessionHandle");
        debug.field("session_id", &"<redacted>");
        debug.field("mode", &self.mode);
        debug.field("remote_signer_pubkey", &self.remote_signer_pubkey);
        debug.field("client_pubkey", &self.client_pubkey);
        debug.field("relays", &self.relays);
        debug.finish()
    }
}

#[cfg(feature = "radrootsd-client")]
impl SdkRadrootsdSignerSessionHandle {
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

    pub(crate) fn session_id(&self) -> &str {
        self.session_id.as_str()
    }
}

#[cfg(feature = "radrootsd-client")]
impl From<radrootsd::SdkRadrootsdSignerSessionConnectResponse> for SdkRadrootsdSignerSessionHandle {
    fn from(value: radrootsd::SdkRadrootsdSignerSessionConnectResponse) -> Self {
        Self {
            session_id: value.session_id,
            mode: value.mode,
            remote_signer_pubkey: value.remote_signer_pubkey,
            client_pubkey: value.client_pubkey,
            relays: value.relays,
        }
    }
}

#[cfg(feature = "radrootsd-client")]
#[derive(Clone, PartialEq, Eq)]
pub struct SdkRadrootsdListingPublishOptions {
    pub signer_session_id: String,
    pub idempotency_key: Option<String>,
}

#[cfg(feature = "radrootsd-client")]
impl SdkRadrootsdListingPublishOptions {
    pub fn new(signer_session_id: impl Into<String>) -> Self {
        Self {
            signer_session_id: signer_session_id.into(),
            idempotency_key: None,
        }
    }

    pub fn from_signer_session(session: &SdkRadrootsdSignerSessionHandle) -> Self {
        Self::new(session.session_id())
    }
}

#[cfg(feature = "radrootsd-client")]
impl fmt::Debug for SdkRadrootsdListingPublishOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("SdkRadrootsdListingPublishOptions");
        debug.field("signer_session_id", &"<redacted>");
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
        Ok(sdk_publish_receipt_from_radrootsd_listing_response(
            response,
        ))
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

        let endpoint = match &self.resolved_transport_target {
            SdkResolvedTransportTarget::Radrootsd { endpoint } => endpoint.as_str(),
            SdkResolvedTransportTarget::RelayDirect { .. } => {
                return Err(SdkRadrootsdSessionError::UnsupportedTransport {
                    transport: self.transport(),
                    operation: "radrootsd.signer_sessions.connect",
                });
            }
        };
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
        options: &SdkRadrootsdListingPublishOptions,
    ) -> Result<SdkPublishReceipt, SdkPublishError> {
        let request = radrootsd::SdkRadrootsdListingPublishRequest {
            listing: listing_value.clone(),
            kind: Some(KIND_LISTING),
            signer_session_id: options.signer_session_id.clone(),
            signer_authority: None,
            idempotency_key: options.idempotency_key.clone(),
        };
        self.client.publish_listing_via_radrootsd(&request).await
    }

    #[cfg(feature = "radrootsd-client")]
    pub async fn publish_draft_via_radrootsd(
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
            options.signer_session_id.clone(),
            None,
            options.idempotency_key.clone(),
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
fn sdk_publish_receipt_from_radrootsd_listing_response(
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
