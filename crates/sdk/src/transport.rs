use crate::RadrootsSdkError;
use radroots_transport::{
    RadrootsTransportDeliveryReceipt, RadrootsTransportKind, RadrootsTransportTarget,
    RadrootsTransportTargetFingerprint, RadrootsTransportTargetReceipt, RadrootsTransportTargetSet,
};
use radroots_transport_nostr::{RadrootsRelayUrl, RadrootsRelayUrlPolicy};
use serde::ser::SerializeStruct;
use std::collections::BTreeSet;

pub use radroots_transport::{
    RadrootsTransportDeliveryReceipt as TransportDeliveryReceipt,
    RadrootsTransportDeliveryTargetStatus as TransportDeliveryTargetStatus,
    RadrootsTransportKind as TransportKind, RadrootsTransportOutcome as TransportOutcome,
    RadrootsTransportTargetReceipt as TransportTargetReceipt,
};

pub const SDK_TRANSPORT_TARGET_MAX_COUNT: usize = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PublishMode {
    DryRun,
    EnqueueOnly,
    EnqueueAndPublish,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SatisfactionPolicy {
    NoWait,
    AtLeastOneTarget,
    AllTargets,
    AtLeast { required: u16 },
}

impl SatisfactionPolicy {
    pub fn at_least(required: u16) -> Result<Self, RadrootsSdkError> {
        if required == 0 {
            return Err(RadrootsSdkError::InvalidRequest {
                message: "satisfaction policy must require at least one target".to_owned(),
            });
        }
        Ok(Self::AtLeast { required })
    }
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
    UseConfiguredProfile,
    Explicit(TargetSet),
    UseTransportProfile,
}

impl TargetPolicy {
    pub fn configured_profile() -> Self {
        Self::UseConfiguredProfile
    }

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

    pub fn use_transport_profile() -> Self {
        Self::UseTransportProfile
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
            Self::UseConfiguredProfile => {
                let mut state = serializer.serialize_struct("TargetPolicy", 1)?;
                state.serialize_field("kind", "use_configured_profile")?;
                state.end()
            }
            Self::Explicit(targets) => {
                let mut state = serializer.serialize_struct("TargetPolicy", 3)?;
                state.serialize_field("kind", "explicit")?;
                state.serialize_field("targets", targets.targets())?;
                state.serialize_field("canonical_targets", targets.canonical_targets())?;
                state.end()
            }
            Self::UseTransportProfile => {
                let mut state = serializer.serialize_struct("TargetPolicy", 1)?;
                state.serialize_field("kind", "use_transport_profile")?;
                state.end()
            }
        }
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
        let mut seen = BTreeSet::new();
        for relay in relays {
            let normalized = normalized_nostr_relay_url(relay.as_ref(), policy)?;
            if seen.insert(normalized.clone()) {
                targets.push(RadrootsTransportTarget::new(
                    RadrootsTransportKind::Nostr,
                    normalized,
                )?);
            }
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

    pub(crate) fn from_normalized_nostr_relays(
        relays: Vec<String>,
    ) -> Result<Self, RadrootsSdkError> {
        let mut targets = Vec::new();
        let mut seen = BTreeSet::new();
        for relay in relays {
            if seen.insert(relay.clone()) {
                targets.push(RadrootsTransportTarget::new(
                    RadrootsTransportKind::Nostr,
                    relay,
                )?);
            }
        }
        Self::from_transport_targets(targets)
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
        let canonical_targets = targets
            .iter()
            .map(|target| {
                RadrootsTransportTargetFingerprint::from_target(&target.kind, &target.uri)
                    .to_string()
            })
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
    behavior: ReticulumPreviewBehavior,
}

impl ReticulumPreviewProfile {
    pub fn preview_unavailable() -> Self {
        Self {
            endpoint_uri: "reticulum:preview".to_owned(),
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

    pub fn behavior(&self) -> ReticulumPreviewBehavior {
        self.behavior
    }

    pub fn target_set(&self) -> Result<TargetSet, RadrootsSdkError> {
        TargetSet::transport_targets(vec![RadrootsTransportTarget::new(
            RadrootsTransportKind::Reticulum,
            self.endpoint_uri.as_str(),
        )?])
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

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct ProxyProfile {
    endpoint_url: String,
}

impl ProxyProfile {
    pub fn new(endpoint_url: impl Into<String>) -> Self {
        Self {
            endpoint_url: endpoint_url.into(),
        }
    }

    pub fn endpoint_url(&self) -> &str {
        self.endpoint_url.as_str()
    }

    pub(crate) fn target_set(&self) -> Result<TargetSet, RadrootsSdkError> {
        TargetSet::transport_targets(vec![RadrootsTransportTarget::new(
            RadrootsTransportKind::custom("radrootsd_proxy")?,
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

    pub(crate) fn configured_nostr_relay_urls(&self) -> Vec<String> {
        match self {
            Self::Nostr { profile } => profile.relay_urls(),
            Self::Hybrid { profile } => profile.nostr().relay_urls(),
            Self::LocalOnly | Self::ReticulumPreview { .. } | Self::Proxy { .. } => Vec::new(),
        }
    }
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
    pub fn satisfied_target_count(&self) -> usize {
        self.target_receipts
            .iter()
            .filter(|receipt| receipt.status.counts_as_satisfied())
            .count()
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
