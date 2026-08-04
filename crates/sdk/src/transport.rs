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
#[cfg(feature = "nostr")]
use std::sync::{Arc, RwLock};

#[cfg(feature = "nostr")]
pub use radroots_transport_nostr::RelayUrlPolicy;

const PREVIEW_UNAVAILABLE_MESSAGE: &str = "preview transport is unavailable in this SDK release";

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
    policy: RelayUrlPolicy,
    state: Arc<RwLock<Option<NostrState>>>,
}

#[cfg(feature = "nostr")]
#[derive(Clone)]
struct NostrState {
    transport: Arc<radroots_transport_nostr::NostrTransport>,
    targets: TargetSet,
}

#[cfg(feature = "nostr")]
impl NostrSlot {
    /// Creates an inert slot with an explicit destination policy.
    #[must_use]
    pub fn new(policy: RelayUrlPolicy) -> Self {
        Self {
            policy,
            state: Arc::new(RwLock::new(None)),
        }
    }

    /// Validates and atomically installs the complete relay selection.
    pub fn configure<I, S>(&self, relays: I) -> crate::Result<()>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let config = radroots_transport_nostr::Config::new(self.policy, relays)
            .map_err(crate::Error::invalid_host_configuration)?;
        let targets = TargetSet::new(
            config
                .relays()
                .iter()
                .map(radroots_transport_nostr::RelayUrl::to_target)
                .collect::<Result<Vec<_>, _>>()
                .map_err(crate::Error::invalid_host_configuration)?,
        )
        .map_err(|_| crate::Error::invalid_host_configuration_without_source())?;
        let state = NostrState {
            transport: Arc::new(radroots_transport_nostr::NostrTransport::new(config)),
            targets,
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

    /// Returns the currently selected canonical targets.
    #[must_use]
    pub fn targets(&self) -> Option<TargetSet> {
        self.snapshot().map(|state| state.targets)
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
        Result<radroots_transport::DeliveryReceipt, radroots_transport::Error>,
    > {
        Box::pin(async move {
            let state = self
                .snapshot()
                .ok_or(radroots_transport::Error::UnsupportedOperation)?;
            radroots_transport::EventSink::deliver(state.transport.as_ref(), request).await
        })
    }
}

#[cfg(feature = "nostr")]
impl std::fmt::Debug for NostrSlot {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NostrSlot")
            .field("policy", &self.policy)
            .field("configured", &self.targets().is_some())
            .finish()
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
    use radroots_event::{SignedEvent, wire::v1::Nip01EventWire};
    use radroots_transport::{
        DeliveryRequest, Error, FetchRequest, TARGET_SET_MAX_ITEMS, Target,
        capability::{Availability, Maturity},
        policy::{SatisfactionClass, SatisfactionPolicy, TargetPolicy},
        sink::DeliveryPayload,
        source::FetchBounds,
        target::TargetFingerprint,
    };

    use super::*;

    fn target(index: usize) -> Target {
        Target::nostr_relay(format!("wss://relay-{index}.example")).expect("target")
    }

    fn signed_event() -> SignedEvent {
        let raw = r#"{"id":"56bfc78223bb2221bad82b539efdec1ade0f56d0eb0e1f592fd387df4b2ceee0","pubkey":"585591529da0bab31b3b1b1f986611cf5f435dca84f978c89ee8a40cca7103df","created_at":1700000001,"kind":0,"tags":[],"content":"{}","sig":"dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"}"#;
        let wire = Nip01EventWire::parse_json(raw).expect("wire event");
        SignedEvent::from_wire_verified_id(wire, raw).expect("signed event")
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
    fn nostr_slot_reconfiguration_is_validated_atomic_and_inert() {
        let slot = NostrSlot::new(RelayUrlPolicy::Local);
        assert!(slot.targets().is_none());
        assert!(slot.configure(["ws://127.0.0.1:7447"]).is_ok());
        let original = slot.targets().expect("configured targets");
        assert!(slot.configure(Vec::<String>::new()).is_err());
        assert_eq!(slot.targets(), Some(original));
        slot.clear();
        assert!(slot.targets().is_none());
        assert!(format!("{slot:?}").contains("configured: false"));
    }

    #[cfg(feature = "nostr")]
    #[test]
    fn poisoned_nostr_slot_fails_closed_for_every_host_operation() {
        let slot = NostrSlot::new(RelayUrlPolicy::Local);
        let state = Arc::clone(&slot.state);
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = state.write().expect("write lock");
            panic!("poison transport slot");
        }));

        slot.clear();
        assert!(slot.targets().is_none());
        assert!(slot.configure(["ws://127.0.0.1:7447"]).is_err());
    }

    #[cfg(feature = "nostr")]
    #[tokio::test]
    async fn empty_nostr_slot_reports_unavailable_and_rejects_operations() {
        use radroots_transport::{EventSink as _, EventSource as _};

        let slot = NostrSlot::new(RelayUrlPolicy::Local);
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
        assert_eq!(
            slot.deliver(deliver).await,
            Err(Error::UnsupportedOperation)
        );
    }
}
