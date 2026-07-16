use crate::RadrootsSdkError;
use radroots_transport::{
    RADROOTS_RETICULUM_ENDPOINT_URI, RADROOTS_RETICULUM_UNAVAILABLE_MESSAGE,
    RadrootsTransportCapabilityAvailability, RadrootsTransportCapabilityMaturity,
    RadrootsTransportImplementationState, RadrootsTransportMeshScopeId,
    RadrootsTransportSatisfactionPolicy, RadrootsTransportStatus, RadrootsTransportTarget,
    RadrootsTransportTargetFingerprint, RadrootsTransportTargetSet,
};
use radroots_transport_nostr::{RadrootsRelayUrl, RadrootsRelayUrlPolicy};
use serde::ser::{SerializeStruct, Serializer};
use std::collections::BTreeSet;

pub use radroots_transport::{
    RadrootsTransportDeliveryReceipt, RadrootsTransportDeliveryTargetStatus, RadrootsTransportKind,
    RadrootsTransportOutcome, RadrootsTransportSatisfactionClass, RadrootsTransportTargetReceipt,
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

    pub fn local_reticulum() -> Self {
        Self(RadrootsTransportMeshScopeId::local_reticulum())
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
    pub fn nostr_relays<I, S>(
        relays: I,
        policy: NostrRelayUrlPolicy,
    ) -> Result<Self, RadrootsSdkError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut targets = Vec::new();
        for relay in relays {
            let normalized = normalized_nostr_relay_url(relay.as_ref(), policy)?;
            targets.push(RadrootsTransportTarget::nostr_relay(normalized)?);
        }
        Self::from_transport_targets(targets)
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
        for target in &targets {
            if target.kind == RadrootsTransportKind::Reticulum
                && target.uri.as_str() != RADROOTS_RETICULUM_ENDPOINT_URI
            {
                return Err(RadrootsSdkError::InvalidRequest {
                    message: format!(
                        "Reticulum endpoint must be {RADROOTS_RETICULUM_ENDPOINT_URI}"
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
pub struct ReticulumProfile {
    endpoint_uri: String,
    scope: MeshScopeId,
    agent_endpoint: Option<ReticulumAgentEndpoint>,
    behavior: ReticulumBehavior,
}

impl ReticulumProfile {
    pub fn deferred_until_implemented() -> Self {
        Self {
            endpoint_uri: RADROOTS_RETICULUM_ENDPOINT_URI.to_owned(),
            scope: MeshScopeId::local_reticulum(),
            agent_endpoint: None,
            behavior: ReticulumBehavior::RejectDeliveryAttempts,
        }
    }

    pub fn with_behavior(mut self, behavior: ReticulumBehavior) -> Self {
        self.behavior = behavior;
        self
    }

    pub fn endpoint_uri(&self) -> &str {
        self.endpoint_uri.as_str()
    }

    pub fn scope(&self) -> &MeshScopeId {
        &self.scope
    }

    pub fn agent_endpoint(&self) -> Option<&ReticulumAgentEndpoint> {
        self.agent_endpoint.as_ref()
    }

    pub fn with_agent_endpoint(mut self, agent_endpoint: ReticulumAgentEndpoint) -> Self {
        self.agent_endpoint = Some(agent_endpoint);
        self
    }

    pub fn with_scope(mut self, scope: MeshScopeId) -> Self {
        self.scope = scope;
        self
    }

    pub fn behavior(&self) -> ReticulumBehavior {
        self.behavior
    }

    pub fn target_set(&self) -> Result<TargetSet, RadrootsSdkError> {
        if self.endpoint_uri.as_str() != radroots_transport::RADROOTS_RETICULUM_ENDPOINT_URI {
            return Err(radroots_transport::RadrootsTransportError::InvalidTargetUri.into());
        }
        TargetSet::transport_targets(vec![RadrootsTransportTarget::reticulum_with_metadata(
            self.endpoint_uri.as_str(),
            Some(self.scope.transport_scope()),
            None,
        )?])
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(transparent)]
pub struct ReticulumAgentEndpoint(String);

impl ReticulumAgentEndpoint {
    pub fn parse(raw: impl AsRef<str>) -> Result<Self, RadrootsSdkError> {
        let uri = raw.as_ref();
        let Some(suffix) = uri.strip_prefix(RETICULUM_AGENT_ENDPOINT_PREFIX) else {
            return Err(RadrootsSdkError::InvalidRequest {
                message: "Reticulum agent endpoint is invalid".to_owned(),
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
                message: "Reticulum agent endpoint is invalid".to_owned(),
            });
        }
        Ok(Self(uri.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl Default for ReticulumProfile {
    fn default() -> Self {
        Self::deferred_until_implemented()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ReticulumBehavior {
    RejectDeliveryAttempts,
    DeferDeliveryPlans,
}

impl ReticulumBehavior {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RejectDeliveryAttempts => "reject_delivery_attempts",
            Self::DeferDeliveryPlans => "defer_delivery_plans",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct MultiTargetProfile {
    nostr: NostrProfile,
    reticulum: ReticulumProfile,
}

impl MultiTargetProfile {
    pub fn new(nostr: NostrProfile, reticulum: ReticulumProfile) -> Self {
        Self { nostr, reticulum }
    }

    pub fn nostr(&self) -> &NostrProfile {
        &self.nostr
    }

    pub fn reticulum(&self) -> &ReticulumProfile {
        &self.reticulum
    }
}

#[derive(Clone, Default, PartialEq, Eq)]
pub enum RadrootsdExecutionAuth {
    #[default]
    None,
    BearerToken(String),
}

impl core::fmt::Debug for RadrootsdExecutionAuth {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::None => f.write_str("None"),
            Self::BearerToken(_) => f.write_str("BearerToken(<redacted>)"),
        }
    }
}

impl serde::Serialize for RadrootsdExecutionAuth {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("RadrootsdExecutionAuth", 1)?;
        match self {
            Self::None => state.serialize_field("kind", "none")?,
            Self::BearerToken(_) => state.serialize_field("kind", "bearer_token")?,
        }
        state.end()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct RadrootsdExecutionProfile {
    endpoint_url: String,
    auth: RadrootsdExecutionAuth,
}

impl RadrootsdExecutionProfile {
    pub fn new(endpoint_url: impl Into<String>) -> Self {
        Self {
            endpoint_url: endpoint_url.into(),
            auth: RadrootsdExecutionAuth::None,
        }
    }

    pub fn with_bearer_token(mut self, token: impl Into<String>) -> Self {
        self.auth = RadrootsdExecutionAuth::BearerToken(token.into());
        self
    }

    pub fn endpoint_url(&self) -> &str {
        self.endpoint_url.as_str()
    }

    pub fn auth(&self) -> &RadrootsdExecutionAuth {
        &self.auth
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
#[non_exhaustive]
pub enum TransportProfile {
    #[default]
    LocalOnly,
    Nostr {
        profile: NostrProfile,
    },
    Reticulum {
        profile: ReticulumProfile,
    },
    MultiTarget {
        profile: MultiTargetProfile,
    },
}

impl TransportProfile {
    pub fn local_only() -> Self {
        Self::LocalOnly
    }

    pub fn nostr(profile: NostrProfile) -> Self {
        Self::Nostr { profile }
    }

    pub fn reticulum(profile: ReticulumProfile) -> Self {
        Self::Reticulum { profile }
    }

    pub fn multi_target(profile: MultiTargetProfile) -> Self {
        Self::MultiTarget { profile }
    }

    pub(crate) fn transport_profile_id(&self) -> &'static str {
        match self {
            Self::LocalOnly => "local_only",
            Self::Nostr { .. } => "nostr",
            Self::Reticulum { .. } => "reticulum",
            Self::MultiTarget { .. } => "multi_target",
        }
    }

    pub(crate) fn target_set(&self) -> Result<Option<TargetSet>, RadrootsSdkError> {
        match self {
            Self::LocalOnly => Ok(None),
            Self::Nostr { profile } => Ok(Some(profile.target_set().clone())),
            Self::Reticulum { profile } => Ok(Some(profile.target_set()?)),
            Self::MultiTarget { profile } => {
                let mut targets = profile.nostr().target_set().targets().to_vec();
                targets.extend(profile.reticulum().target_set()?.into_targets());
                Ok(Some(TargetSet::transport_targets(targets)?))
            }
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
            Self::Reticulum { profile } => {
                vec![reticulum_transport_status(
                    self.transport_profile_id(),
                    profile.endpoint_uri(),
                )]
            }
            Self::MultiTarget { profile } => vec![
                nostr_transport_status(self.transport_profile_id()),
                reticulum_transport_status(
                    self.transport_profile_id(),
                    profile.reticulum().endpoint_uri(),
                ),
            ],
        }
    }

    pub(crate) fn configured_nostr_relay_urls(&self) -> Vec<String> {
        match self {
            Self::Nostr { profile } => profile.relay_urls(),
            Self::MultiTarget { profile } => profile.nostr().relay_urls(),
            Self::LocalOnly | Self::Reticulum { .. } => Vec::new(),
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

fn reticulum_transport_status(profile_id: &str, endpoint_uri: &str) -> RadrootsTransportStatus {
    RadrootsTransportStatus::new(
        RadrootsTransportKind::Reticulum,
        true,
        RadrootsTransportImplementationState::Real,
        false,
        RADROOTS_RETICULUM_UNAVAILABLE_MESSAGE,
    )
    .with_profile_id(profile_id)
    .with_endpoint_uri(endpoint_uri)
    .with_maturity(RadrootsTransportCapabilityMaturity::Preview)
    .with_availability(RadrootsTransportCapabilityAvailability::Unavailable)
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
    pub fn satisfied_target_count(
        &self,
        satisfaction_class: RadrootsTransportSatisfactionClass,
    ) -> usize {
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
