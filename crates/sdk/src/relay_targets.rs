use crate::RadrootsSdkError;
use radroots_relay_transport::{RadrootsRelayUrl, RadrootsRelayUrlPolicy};
use serde::ser::SerializeStruct;
use std::collections::BTreeSet;

pub const SDK_RELAY_TARGET_MAX_COUNT: usize = 20;

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
pub enum AckPolicy {
    NoWait,
    AtLeastOneRelay,
    AllRelays,
    Quorum { required: u16 },
}

impl AckPolicy {
    pub fn quorum(required: u16) -> Result<Self, RadrootsSdkError> {
        if required == 0 {
            return Err(RadrootsSdkError::InvalidRequest {
                message: "ack policy quorum must require at least one relay".to_owned(),
            });
        }
        Ok(Self::Quorum { required })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RelayResolutionPolicy {
    ConfiguredRelays,
    Explicit(SdkRelayTargetSet),
}

impl RelayResolutionPolicy {
    pub fn configured_relays() -> Self {
        Self::ConfiguredRelays
    }

    pub fn explicit(targets: SdkRelayTargetSet) -> Self {
        Self::Explicit(targets)
    }

    pub fn try_explicit<I, S>(
        relays: I,
        url_policy: SdkRelayUrlPolicy,
    ) -> Result<Self, RadrootsSdkError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Ok(Self::Explicit(SdkRelayTargetSet::new(relays, url_policy)?))
    }

    pub(crate) fn workflow_target_policy(self) -> SdkRelayTargetPolicy {
        match self {
            Self::ConfiguredRelays => SdkRelayTargetPolicy::UseConfiguredRelays,
            Self::Explicit(targets) => SdkRelayTargetPolicy::Explicit(targets),
        }
    }
}

impl serde::Serialize for RelayResolutionPolicy {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::ConfiguredRelays => {
                let mut state = serializer.serialize_struct("RelayResolutionPolicy", 1)?;
                state.serialize_field("kind", "configured_relays")?;
                state.end()
            }
            Self::Explicit(targets) => {
                let mut state = serializer.serialize_struct("RelayResolutionPolicy", 3)?;
                state.serialize_field("kind", "explicit")?;
                state.serialize_field("relays", targets.relays())?;
                state.serialize_field("canonical_relays", targets.canonical_relays())?;
                state.end()
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SdkRelayUrlPolicy {
    Public,
    Localhost,
}

impl SdkRelayUrlPolicy {
    pub(crate) fn relay_transport_policy(self) -> RadrootsRelayUrlPolicy {
        match self {
            Self::Public => RadrootsRelayUrlPolicy::Public,
            Self::Localhost => RadrootsRelayUrlPolicy::Localhost,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SdkRelayTargetPolicy {
    Explicit(SdkRelayTargetSet),
    UseConfiguredRelays,
    UsePublishTransport,
}

impl SdkRelayTargetPolicy {
    pub fn explicit(targets: SdkRelayTargetSet) -> Self {
        Self::Explicit(targets)
    }

    pub fn try_explicit<I, S>(
        relays: I,
        url_policy: SdkRelayUrlPolicy,
    ) -> Result<Self, RadrootsSdkError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Ok(Self::Explicit(SdkRelayTargetSet::new(relays, url_policy)?))
    }

    pub fn use_publish_transport() -> Self {
        Self::UsePublishTransport
    }
}

impl serde::Serialize for SdkRelayTargetPolicy {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Explicit(targets) => {
                let mut state = serializer.serialize_struct("SdkRelayTargetPolicy", 3)?;
                state.serialize_field("kind", "explicit")?;
                state.serialize_field("relays", targets.relays())?;
                state.serialize_field("canonical_relays", targets.canonical_relays())?;
                state.end()
            }
            Self::UseConfiguredRelays => {
                let mut state = serializer.serialize_struct("SdkRelayTargetPolicy", 1)?;
                state.serialize_field("kind", "use_configured_relays")?;
                state.end()
            }
            Self::UsePublishTransport => {
                let mut state = serializer.serialize_struct("SdkRelayTargetPolicy", 1)?;
                state.serialize_field("kind", "use_publish_transport")?;
                state.end()
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SdkRelayTargetSet {
    relays: Vec<String>,
    canonical_relays: Vec<String>,
}

impl SdkRelayTargetSet {
    pub fn new<I, S>(relays: I, policy: SdkRelayUrlPolicy) -> Result<Self, RadrootsSdkError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut ordered_relays = Vec::new();
        let mut seen = BTreeSet::new();
        for relay in relays {
            let normalized = normalized_relay_url(relay.as_ref(), policy)?;
            if seen.insert(normalized.clone()) {
                ordered_relays.push(normalized);
            }
        }
        Self::from_normalized_ordered(ordered_relays)
    }

    pub fn relays(&self) -> &[String] {
        self.relays.as_slice()
    }

    pub fn canonical_relays(&self) -> &[String] {
        self.canonical_relays.as_slice()
    }

    pub fn into_vec(self) -> Vec<String> {
        self.relays
    }

    pub fn len(&self) -> usize {
        self.relays.len()
    }

    pub fn is_empty(&self) -> bool {
        self.relays.is_empty()
    }

    pub(crate) fn from_configured_relays<I, S>(
        relays: I,
        policy: SdkRelayUrlPolicy,
    ) -> Result<Vec<String>, RadrootsSdkError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let relays = relays.into_iter().collect::<Vec<_>>();
        if relays.is_empty() {
            return Ok(Vec::new());
        }
        Ok(Self::new(relays, policy)?.into_vec())
    }

    pub(crate) fn from_normalized_relays(relays: Vec<String>) -> Result<Self, RadrootsSdkError> {
        let mut ordered = Vec::new();
        let mut seen = BTreeSet::new();
        for relay in relays {
            if seen.insert(relay.clone()) {
                ordered.push(relay);
            }
        }
        Self::from_normalized_ordered(ordered)
    }

    fn from_normalized_ordered(relays: Vec<String>) -> Result<Self, RadrootsSdkError> {
        if relays.is_empty() {
            return Err(RadrootsSdkError::empty_target_relays(
                "sdk relay target set",
            ));
        }
        if relays.len() > SDK_RELAY_TARGET_MAX_COUNT {
            return Err(RadrootsSdkError::relay_target_limit_exceeded(
                SDK_RELAY_TARGET_MAX_COUNT,
                relays.len(),
            ));
        }
        let canonical_relays = relays.iter().cloned().collect::<BTreeSet<_>>();
        Ok(Self {
            relays,
            canonical_relays: canonical_relays.into_iter().collect(),
        })
    }
}

impl serde::Serialize for SdkRelayTargetSet {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("SdkRelayTargetSet", 2)?;
        state.serialize_field("relays", self.relays())?;
        state.serialize_field("canonical_relays", self.canonical_relays())?;
        state.end()
    }
}

fn normalized_relay_url(
    value: &str,
    policy: SdkRelayUrlPolicy,
) -> Result<String, RadrootsSdkError> {
    let relay = RadrootsRelayUrl::parse(value, policy.relay_transport_policy())?;
    Ok(relay.into_string())
}

#[cfg(test)]
#[path = "../tests/unit/relay_targets_tests.rs"]
mod tests;
