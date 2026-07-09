use crate::RadrootsSdkError;
use radroots_transport::{
    RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI, RADROOTS_RETICULUM_UNAVAILABLE_MESSAGE,
    RadrootsTransportDeliveryReceipt, RadrootsTransportImplementationState, RadrootsTransportKind,
    RadrootsTransportMeshScopeId, RadrootsTransportSatisfactionClass,
    RadrootsTransportSatisfactionPolicy, RadrootsTransportStatus, RadrootsTransportTarget,
    RadrootsTransportTargetFingerprint, RadrootsTransportTargetReceipt, RadrootsTransportTargetSet,
};
use radroots_transport_nostr::{RadrootsRelayUrl, RadrootsRelayUrlPolicy};
use serde::ser::{SerializeStruct, Serializer};
use std::collections::BTreeSet;

pub use radroots_transport::{
    RadrootsTransportDeliveryReceipt as TransportDeliveryReceipt,
    RadrootsTransportDeliveryTargetStatus as TransportDeliveryTargetStatus,
    RadrootsTransportKind as TransportKind, RadrootsTransportOutcome as TransportOutcome,
    RadrootsTransportSatisfactionClass as TransportSatisfactionClass,
    RadrootsTransportTargetReceipt as TransportTargetReceipt,
};

pub const SDK_TRANSPORT_TARGET_MAX_COUNT: usize = 20;
pub const RETICULUM_AGENT_ENDPOINT_PREFIX: &str = "reticulum-agent:";

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PublishMode {
    DryRun,
    EnqueueOnly,
    EnqueueAndPublish,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SatisfactionPolicy {
    NoWait,
    AnyAccepted,
    AllAccepted,
    QuorumAccepted {
        threshold: u16,
    },
    AnyDelivered,
    AllDelivered,
    QuorumDelivered {
        threshold: u16,
    },
    RequiredAcceptedTargets {
        target_fingerprints: Vec<RadrootsTransportTargetFingerprint>,
    },
    RequiredDeliveredTargets {
        target_fingerprints: Vec<RadrootsTransportTargetFingerprint>,
    },
}

impl SatisfactionPolicy {
    pub fn quorum_accepted(threshold: u16) -> Result<Self, RadrootsSdkError> {
        validate_satisfaction_threshold(threshold)?;
        Ok(Self::QuorumAccepted { threshold })
    }

    pub fn quorum_delivered(threshold: u16) -> Result<Self, RadrootsSdkError> {
        validate_satisfaction_threshold(threshold)?;
        Ok(Self::QuorumDelivered { threshold })
    }

    pub fn required_accepted_targets<I, S>(targets: I) -> Result<Self, RadrootsSdkError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Ok(Self::RequiredAcceptedTargets {
            target_fingerprints: required_target_fingerprints(targets)?,
        })
    }

    pub fn required_delivered_targets<I, S>(targets: I) -> Result<Self, RadrootsSdkError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Ok(Self::RequiredDeliveredTargets {
            target_fingerprints: required_target_fingerprints(targets)?,
        })
    }

    pub fn is_no_wait(&self) -> bool {
        matches!(self, Self::NoWait)
    }

    pub(crate) fn transport_satisfaction_policy(
        &self,
    ) -> Result<RadrootsTransportSatisfactionPolicy, RadrootsSdkError> {
        Ok(match self {
            Self::NoWait => RadrootsTransportSatisfactionPolicy::no_wait(),
            Self::AnyAccepted => RadrootsTransportSatisfactionPolicy::any_accepted(),
            Self::AllAccepted => RadrootsTransportSatisfactionPolicy::all_accepted(),
            Self::QuorumAccepted { threshold } => {
                RadrootsTransportSatisfactionPolicy::quorum_accepted(*threshold)
            }
            Self::AnyDelivered => RadrootsTransportSatisfactionPolicy::any_delivered(),
            Self::AllDelivered => RadrootsTransportSatisfactionPolicy::all_delivered(),
            Self::QuorumDelivered { threshold } => {
                RadrootsTransportSatisfactionPolicy::quorum_delivered(*threshold)
            }
            Self::RequiredAcceptedTargets {
                target_fingerprints,
            } => RadrootsTransportSatisfactionPolicy::required_targets(
                RadrootsTransportSatisfactionClass::Accepted,
                target_fingerprints.clone(),
            )?,
            Self::RequiredDeliveredTargets {
                target_fingerprints,
            } => RadrootsTransportSatisfactionPolicy::required_targets(
                RadrootsTransportSatisfactionClass::Delivered,
                target_fingerprints.clone(),
            )?,
        })
    }
}

fn validate_satisfaction_threshold(threshold: u16) -> Result<(), RadrootsSdkError> {
    if threshold == 0 {
        return Err(RadrootsSdkError::InvalidRequest {
            message: "satisfaction policy threshold must require at least one target".to_owned(),
        });
    }
    Ok(())
}

fn required_target_fingerprints<I, S>(
    targets: I,
) -> Result<Vec<RadrootsTransportTargetFingerprint>, RadrootsSdkError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let fingerprints = targets
        .into_iter()
        .map(|target| RadrootsTransportTargetFingerprint::parse(target.as_ref()))
        .collect::<Result<Vec<_>, _>>()?;
    RadrootsTransportSatisfactionPolicy::required_targets(
        RadrootsTransportSatisfactionClass::Accepted,
        fingerprints.clone(),
    )?;
    Ok(fingerprints)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum NostrRelayUrlPolicy {
    Public,
    Localhost,
}

impl NostrRelayUrlPolicy {
    pub(crate) fn nostr_transport_policy(self) -> RadrootsRelayUrlPolicy {
        match self {
            Self::Public => RadrootsRelayUrlPolicy::Public,
            Self::Localhost => RadrootsRelayUrlPolicy::Localhost,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum TargetPolicy {
    Explicit(TargetSet),
    DefaultProfile,
    LocalOnly,
    MeshScope(MeshScopeId),
}

impl TargetPolicy {
    pub fn explicit(targets: TargetSet) -> Self {
        Self::Explicit(targets)
    }

    pub fn try_nostr_relays<I, S>(
        relays: I,
        url_policy: NostrRelayUrlPolicy,
    ) -> Result<Self, RadrootsSdkError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Ok(Self::Explicit(TargetSet::nostr_relays(relays, url_policy)?))
    }

    pub fn default_profile() -> Self {
        Self::DefaultProfile
    }

    pub fn local_only() -> Self {
        Self::LocalOnly
    }

    pub fn mesh_scope(scope: MeshScopeId) -> Self {
        Self::MeshScope(scope)
    }

    #[cfg(any(feature = "signer-adapters", test))]
    pub(crate) fn workflow_target_policy(self) -> Self {
        self
    }
}

impl serde::Serialize for TargetPolicy {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Explicit(targets) => {
                let mut state = serializer.serialize_struct("TargetPolicy", 3)?;
                state.serialize_field("kind", "explicit")?;
                state.serialize_field("targets", targets.targets())?;
                state.serialize_field("canonical_targets", targets.canonical_targets())?;
                state.end()
            }
            Self::DefaultProfile => {
                let mut state = serializer.serialize_struct("TargetPolicy", 1)?;
                state.serialize_field("kind", "default_profile")?;
                state.end()
            }
            Self::LocalOnly => {
                let mut state = serializer.serialize_struct("TargetPolicy", 1)?;
                state.serialize_field("kind", "local_only")?;
                state.end()
            }
            Self::MeshScope(scope) => {
                let mut state = serializer.serialize_struct("TargetPolicy", 2)?;
                state.serialize_field("kind", "mesh_scope")?;
                state.serialize_field("scope", scope)?;
                state.end()
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(transparent)]
pub struct MeshScopeId(RadrootsTransportMeshScopeId);

impl MeshScopeId {
    pub fn parse(raw: impl AsRef<str>) -> Result<Self, RadrootsSdkError> {
        Ok(Self(RadrootsTransportMeshScopeId::parse(raw)?))
    }

    pub fn local_preview() -> Self {
        Self(RadrootsTransportMeshScopeId::local_preview())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub(crate) fn transport_scope(&self) -> RadrootsTransportMeshScopeId {
        self.0.clone()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetSet {
    targets: Vec<RadrootsTransportTarget>,
    canonical_targets: Vec<String>,
}

impl TargetSet {
    pub fn new<I, S>(relays: I, policy: NostrRelayUrlPolicy) -> Result<Self, RadrootsSdkError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut targets = Vec::new();
        for relay in relays {
            let normalized = normalized_nostr_relay_url(relay.as_ref(), policy)?;
            targets.push(RadrootsTransportTarget::new(
                RadrootsTransportKind::Nostr,
                normalized,
            )?);
        }
        Self::from_transport_targets(targets)
    }

    pub fn nostr_relays<I, S>(
        relays: I,
        policy: NostrRelayUrlPolicy,
    ) -> Result<Self, RadrootsSdkError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self::new(relays, policy)
    }

    pub fn transport_targets(
        targets: Vec<RadrootsTransportTarget>,
    ) -> Result<Self, RadrootsSdkError> {
        Self::from_transport_targets(targets)
    }

    pub fn targets(&self) -> &[RadrootsTransportTarget] {
        self.targets.as_slice()
    }

    pub fn canonical_targets(&self) -> &[String] {
        self.canonical_targets.as_slice()
    }

    pub fn transport_target_set(&self) -> Result<RadrootsTransportTargetSet, RadrootsSdkError> {
        Ok(RadrootsTransportTargetSet::new(self.targets.clone())?)
    }

    pub fn nostr_relay_urls(&self) -> Vec<String> {
        self.targets
            .iter()
            .filter(|target| target.kind == RadrootsTransportKind::Nostr)
            .map(|target| target.uri.as_str().to_owned())
            .collect()
    }

    pub fn into_targets(self) -> Vec<RadrootsTransportTarget> {
        self.targets
    }

    pub fn len(&self) -> usize {
        self.targets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    fn from_transport_targets(
        targets: Vec<RadrootsTransportTarget>,
    ) -> Result<Self, RadrootsSdkError> {
        if targets.is_empty() {
            return Err(RadrootsSdkError::empty_transport_targets(
                "sdk transport target set",
            ));
        }
        if targets.len() > SDK_TRANSPORT_TARGET_MAX_COUNT {
            return Err(RadrootsSdkError::transport_target_limit_exceeded(
                SDK_TRANSPORT_TARGET_MAX_COUNT,
                targets.len(),
            ));
        }
        if targets
            .iter()
            .any(|target| target.kind == RadrootsTransportKind::Proxy)
            && (targets.len() != 1 || targets[0].kind != RadrootsTransportKind::Proxy)
        {
            return Err(RadrootsSdkError::InvalidRequest {
                message: "proxy transport targets must be the only target in a target set"
                    .to_owned(),
            });
        }
        for target in &targets {
            if target.kind == RadrootsTransportKind::Reticulum
                && target.uri.as_str() != RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI
            {
                return Err(RadrootsSdkError::InvalidRequest {
                    message: format!(
                        "Reticulum preview endpoint must be {RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI}"
                    ),
                });
            }
        }
        RadrootsTransportTargetSet::new(targets.clone())?;
        let canonical_targets = targets
            .iter()
            .map(|target| target.fingerprint.to_string())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        Ok(Self {
            targets,
            canonical_targets,
        })
    }
}

impl serde::Serialize for TargetSet {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("TargetSet", 2)?;
        state.serialize_field("targets", self.targets())?;
        state.serialize_field("canonical_targets", self.canonical_targets())?;
        state.end()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct NostrProfile {
    target_set: TargetSet,
}

impl NostrProfile {
    pub fn new<I, S>(relays: I, policy: NostrRelayUrlPolicy) -> Result<Self, RadrootsSdkError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Ok(Self {
            target_set: TargetSet::nostr_relays(relays, policy)?,
        })
    }

    pub fn target_set(&self) -> &TargetSet {
        &self.target_set
    }

    pub fn relay_urls(&self) -> Vec<String> {
        self.target_set.nostr_relay_urls()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct ReticulumPreviewProfile {
    endpoint_uri: String,
    scope: MeshScopeId,
    agent_endpoint: Option<ReticulumPreviewAgentEndpoint>,
    behavior: ReticulumPreviewBehavior,
}

impl ReticulumPreviewProfile {
    pub fn preview_unavailable() -> Self {
        Self {
            endpoint_uri: RADROOTS_RETICULUM_PREVIEW_ENDPOINT_URI.to_owned(),
            scope: MeshScopeId::local_preview(),
            agent_endpoint: None,
            behavior: ReticulumPreviewBehavior::RejectDeliveryAttempts,
        }
    }

    pub fn with_behavior(mut self, behavior: ReticulumPreviewBehavior) -> Self {
        self.behavior = behavior;
        self
    }

    pub fn endpoint_uri(&self) -> &str {
        self.endpoint_uri.as_str()
    }

    pub fn scope(&self) -> &MeshScopeId {
        &self.scope
    }

    pub fn agent_endpoint(&self) -> Option<&ReticulumPreviewAgentEndpoint> {
        self.agent_endpoint.as_ref()
    }

    pub fn with_agent_endpoint(mut self, agent_endpoint: ReticulumPreviewAgentEndpoint) -> Self {
        self.agent_endpoint = Some(agent_endpoint);
        self
    }

    pub fn with_scope(mut self, scope: MeshScopeId) -> Self {
        self.scope = scope;
        self
    }

    pub fn behavior(&self) -> ReticulumPreviewBehavior {
        self.behavior
    }

    pub fn target_set(&self) -> Result<TargetSet, RadrootsSdkError> {
        TargetSet::transport_targets(vec![RadrootsTransportTarget::new_with_metadata(
            RadrootsTransportKind::Reticulum,
            self.endpoint_uri.as_str(),
            Some(self.scope.transport_scope()),
            None,
        )?])
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(transparent)]
pub struct ReticulumPreviewAgentEndpoint(String);

impl ReticulumPreviewAgentEndpoint {
    pub fn parse(raw: impl AsRef<str>) -> Result<Self, RadrootsSdkError> {
        let uri = raw.as_ref();
        let Some(suffix) = uri.strip_prefix(RETICULUM_AGENT_ENDPOINT_PREFIX) else {
            return Err(RadrootsSdkError::InvalidRequest {
                message: "Reticulum preview agent endpoint is invalid".to_owned(),
            });
        };
        if uri.is_empty()
            || uri != uri.trim()
            || suffix.is_empty()
            || uri
                .chars()
                .any(|ch| ch.is_ascii_control() || ch.is_ascii_whitespace())
        {
            return Err(RadrootsSdkError::InvalidRequest {
                message: "Reticulum preview agent endpoint is invalid".to_owned(),
            });
        }
        Ok(Self(uri.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl Default for ReticulumPreviewProfile {
    fn default() -> Self {
        Self::preview_unavailable()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ReticulumPreviewBehavior {
    RejectDeliveryAttempts,
    DeferDeliveryPlans,
}

impl ReticulumPreviewBehavior {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RejectDeliveryAttempts => "reject_delivery_attempts",
            Self::DeferDeliveryPlans => "defer_delivery_plans",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct HybridProfile {
    nostr: NostrProfile,
    reticulum_preview: ReticulumPreviewProfile,
}

impl HybridProfile {
    pub fn new(nostr: NostrProfile, reticulum_preview: ReticulumPreviewProfile) -> Self {
        Self {
            nostr,
            reticulum_preview,
        }
    }

    pub fn nostr(&self) -> &NostrProfile {
        &self.nostr
    }

    pub fn reticulum_preview(&self) -> &ReticulumPreviewProfile {
        &self.reticulum_preview
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ProxyAuth {
    None,
    BearerToken(String),
}

impl Default for ProxyAuth {
    fn default() -> Self {
        Self::None
    }
}

impl core::fmt::Debug for ProxyAuth {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::None => f.write_str("None"),
            Self::BearerToken(_) => f.write_str("BearerToken(<redacted>)"),
        }
    }
}

impl serde::Serialize for ProxyAuth {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("ProxyAuth", 1)?;
        match self {
            Self::None => state.serialize_field("kind", "none")?,
            Self::BearerToken(_) => state.serialize_field("kind", "bearer_token")?,
        }
        state.end()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct ProxyProfile {
    endpoint_url: String,
    auth: ProxyAuth,
}

impl ProxyProfile {
    pub fn new(endpoint_url: impl Into<String>) -> Self {
        Self {
            endpoint_url: endpoint_url.into(),
            auth: ProxyAuth::None,
        }
    }

    pub fn with_bearer_token(mut self, token: impl Into<String>) -> Self {
        self.auth = ProxyAuth::BearerToken(token.into());
        self
    }

    pub fn endpoint_url(&self) -> &str {
        self.endpoint_url.as_str()
    }

    pub fn auth(&self) -> &ProxyAuth {
        &self.auth
    }

    pub(crate) fn target_set(&self) -> Result<TargetSet, RadrootsSdkError> {
        TargetSet::transport_targets(vec![RadrootsTransportTarget::new(
            RadrootsTransportKind::Proxy,
            self.endpoint_url.as_str(),
        )?])
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
#[non_exhaustive]
pub enum TransportProfile {
    LocalOnly,
    Nostr { profile: NostrProfile },
    ReticulumPreview { profile: ReticulumPreviewProfile },
    Hybrid { profile: HybridProfile },
    Proxy { profile: ProxyProfile },
}

impl Default for TransportProfile {
    fn default() -> Self {
        Self::LocalOnly
    }
}

impl TransportProfile {
    pub fn local_only() -> Self {
        Self::LocalOnly
    }

    pub fn nostr(profile: NostrProfile) -> Self {
        Self::Nostr { profile }
    }

    pub fn reticulum_preview(profile: ReticulumPreviewProfile) -> Self {
        Self::ReticulumPreview { profile }
    }

    pub fn hybrid(profile: HybridProfile) -> Self {
        Self::Hybrid { profile }
    }

    pub fn proxy(profile: ProxyProfile) -> Self {
        Self::Proxy { profile }
    }

    pub fn supports_delegated_target_resolution(&self) -> bool {
        matches!(self, Self::Proxy { .. })
    }

    pub(crate) fn transport_profile_id(&self) -> &'static str {
        match self {
            Self::LocalOnly => "local_only",
            Self::Nostr { .. } => "nostr",
            Self::ReticulumPreview { .. } => "reticulum_preview",
            Self::Hybrid { .. } => "hybrid",
            Self::Proxy { .. } => "proxy",
        }
    }

    pub(crate) fn target_set(&self) -> Result<Option<TargetSet>, RadrootsSdkError> {
        match self {
            Self::LocalOnly => Ok(None),
            Self::Nostr { profile } => Ok(Some(profile.target_set().clone())),
            Self::ReticulumPreview { profile } => Ok(Some(profile.target_set()?)),
            Self::Hybrid { profile } => {
                let mut targets = profile.nostr().target_set().targets().to_vec();
                targets.extend(profile.reticulum_preview().target_set()?.into_targets());
                Ok(Some(TargetSet::transport_targets(targets)?))
            }
            Self::Proxy { profile } => Ok(Some(profile.target_set()?)),
        }
    }

    pub(crate) fn configured_transport_targets(
        &self,
    ) -> Result<Vec<RadrootsTransportTarget>, RadrootsSdkError> {
        Ok(self
            .target_set()?
            .map(TargetSet::into_targets)
            .unwrap_or_default())
    }

    pub(crate) fn transport_statuses(&self) -> Vec<RadrootsTransportStatus> {
        match self {
            Self::LocalOnly => vec![
                RadrootsTransportStatus::new(
                    RadrootsTransportKind::Local,
                    true,
                    RadrootsTransportImplementationState::Real,
                    false,
                    "local persistence only",
                )
                .with_profile_id(self.transport_profile_id()),
            ],
            Self::Nostr { .. } => vec![nostr_transport_status(self.transport_profile_id())],
            Self::ReticulumPreview { profile } => {
                vec![reticulum_preview_transport_status(
                    self.transport_profile_id(),
                    profile.endpoint_uri(),
                )]
            }
            Self::Hybrid { profile } => vec![
                nostr_transport_status(self.transport_profile_id()),
                reticulum_preview_transport_status(
                    self.transport_profile_id(),
                    profile.reticulum_preview().endpoint_uri(),
                ),
            ],
            Self::Proxy { profile } => {
                let auth_configured = matches!(profile.auth(), ProxyAuth::BearerToken(_));
                vec![
                    RadrootsTransportStatus::new(
                        RadrootsTransportKind::Proxy,
                        auth_configured,
                        RadrootsTransportImplementationState::Real,
                        auth_configured,
                        if auth_configured {
                            "ready"
                        } else {
                            "proxy transport requires bearer token"
                        },
                    )
                    .with_profile_id(self.transport_profile_id())
                    .with_endpoint_uri(profile.endpoint_url()),
                ]
            }
        }
    }

    pub(crate) fn configured_nostr_relay_urls(&self) -> Vec<String> {
        match self {
            Self::Nostr { profile } => profile.relay_urls(),
            Self::Hybrid { profile } => profile.nostr().relay_urls(),
            Self::LocalOnly | Self::ReticulumPreview { .. } | Self::Proxy { .. } => Vec::new(),
        }
    }
}

fn nostr_transport_status(profile_id: &str) -> RadrootsTransportStatus {
    RadrootsTransportStatus::new(
        RadrootsTransportKind::Nostr,
        true,
        RadrootsTransportImplementationState::Real,
        true,
        "ready",
    )
    .with_profile_id(profile_id)
}

fn reticulum_preview_transport_status(
    profile_id: &str,
    endpoint_uri: &str,
) -> RadrootsTransportStatus {
    RadrootsTransportStatus::new(
        RadrootsTransportKind::Reticulum,
        true,
        RadrootsTransportImplementationState::PreviewUnavailable,
        false,
        RADROOTS_RETICULUM_UNAVAILABLE_MESSAGE,
    )
    .with_profile_id(profile_id)
    .with_endpoint_uri(endpoint_uri)
}

impl From<RadrootsTransportDeliveryReceipt> for TransportReceipt {
    fn from(value: RadrootsTransportDeliveryReceipt) -> Self {
        Self {
            request_id: value.request_id,
            target_receipts: value.target_receipts,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct TransportReceipt {
    pub request_id: String,
    pub target_receipts: Vec<RadrootsTransportTargetReceipt>,
}

impl TransportReceipt {
    pub fn satisfied_target_count(&self, satisfaction_class: TransportSatisfactionClass) -> usize {
        self.target_receipts
            .iter()
            .filter(|receipt| receipt.status.counts_as_satisfied(satisfaction_class))
            .count()
    }

    pub fn is_satisfied_by(&self, policy: &SatisfactionPolicy) -> Result<bool, RadrootsSdkError> {
        Ok(RadrootsTransportDeliveryReceipt {
            request_id: self.request_id.clone(),
            target_receipts: self.target_receipts.clone(),
        }
        .is_satisfied_by(&policy.transport_satisfaction_policy()?)?)
    }
}

fn normalized_nostr_relay_url(
    value: &str,
    policy: NostrRelayUrlPolicy,
) -> Result<String, RadrootsSdkError> {
    let relay = RadrootsRelayUrl::parse(value, policy.nostr_transport_policy())?;
    Ok(relay.into_string())
}

#[cfg(test)]
#[path = "../tests/unit/transport_tests.rs"]
mod tests;
