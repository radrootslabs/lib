use crate::RadrootsSdkError;
use radroots_relay_transport::{RadrootsRelayUrl, RadrootsRelayUrlPolicy};
use std::collections::BTreeSet;

pub const SDK_RELAY_TARGET_MAX_COUNT: usize = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SdkRelayUrlPolicy {
    Public,
    Localhost,
}

impl SdkRelayUrlPolicy {
    fn relay_transport_policy(self) -> RadrootsRelayUrlPolicy {
        match self {
            Self::Public => RadrootsRelayUrlPolicy::Public,
            Self::Localhost => RadrootsRelayUrlPolicy::LocalDev,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SdkRelayTargetPolicy {
    Explicit(SdkRelayTargetSet),
    UseConfiguredRelays,
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
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SdkRelayTargetSet {
    relays: Vec<String>,
}

impl SdkRelayTargetSet {
    pub fn new<I, S>(relays: I, policy: SdkRelayUrlPolicy) -> Result<Self, RadrootsSdkError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut normalized = BTreeSet::new();
        for relay in relays {
            normalized.insert(normalized_relay_url(relay.as_ref(), policy)?);
        }
        Self::from_normalized_set(normalized)
    }

    pub fn relays(&self) -> &[String] {
        self.relays.as_slice()
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
        let normalized = relays.into_iter().collect::<BTreeSet<_>>();
        Self::from_normalized_set(normalized)
    }

    fn from_normalized_set(normalized: BTreeSet<String>) -> Result<Self, RadrootsSdkError> {
        if normalized.is_empty() {
            return Err(RadrootsSdkError::empty_target_relays(
                "sdk relay target set",
            ));
        }
        if normalized.len() > SDK_RELAY_TARGET_MAX_COUNT {
            return Err(RadrootsSdkError::relay_target_limit_exceeded(
                SDK_RELAY_TARGET_MAX_COUNT,
                normalized.len(),
            ));
        }
        Ok(Self {
            relays: normalized.into_iter().collect(),
        })
    }
}

fn normalized_relay_url(
    value: &str,
    policy: SdkRelayUrlPolicy,
) -> Result<String, RadrootsSdkError> {
    let relay = RadrootsRelayUrl::parse(value, policy.relay_transport_policy())?;
    let normalized = relay.into_string();
    if normalized.starts_with("ws://") && !is_local_ws_relay(normalized.as_str()) {
        return Err(RadrootsSdkError::invalid_relay_url(
            normalized,
            "ws relay targets are limited to localhost, 127.0.0.1, or [::1]",
        ));
    }
    Ok(normalized)
}

fn is_local_ws_relay(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("ws://") else {
        return false;
    };
    let authority = rest
        .split_once('/')
        .map(|(authority, _)| authority)
        .unwrap_or(rest);
    let host = relay_authority_host(authority);
    matches!(host.as_deref(), Some("localhost" | "127.0.0.1" | "[::1]"))
}

fn relay_authority_host(authority: &str) -> Option<String> {
    if let Some(after_open) = authority.strip_prefix('[') {
        let close_index = after_open.find(']')?;
        return Some(format!("[{}]", &after_open[..close_index]));
    }
    Some(
        authority
            .split_once(':')
            .map(|(host, _)| host)
            .unwrap_or(authority)
            .to_owned(),
    )
}
