use crate::RadrootsSdkError;
use core::fmt;
use radroots_relay_transport::{RadrootsRelayUrl, RadrootsRelayUrlPolicy};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const SDK_RELAY_TARGET_MAX_COUNT: usize = 20;
pub const SDK_IDEMPOTENCY_KEY_MAX_LEN: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SdkRelayTargetPolicy {
    Public,
    Localhost,
}

impl SdkRelayTargetPolicy {
    fn relay_transport_policy(self) -> RadrootsRelayUrlPolicy {
        match self {
            Self::Public => RadrootsRelayUrlPolicy::Public,
            Self::Localhost => RadrootsRelayUrlPolicy::LocalDev,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SdkRelayTargetSet {
    relays: Vec<String>,
}

impl SdkRelayTargetSet {
    pub fn new<I, S>(relays: I, policy: SdkRelayTargetPolicy) -> Result<Self, RadrootsSdkError>
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
        policy: SdkRelayTargetPolicy,
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
            return Err(invalid_request("relay target set must not be empty"));
        }
        if normalized.len() > SDK_RELAY_TARGET_MAX_COUNT {
            return Err(invalid_request(format!(
                "relay target set must contain at most {SDK_RELAY_TARGET_MAX_COUNT} relays"
            )));
        }
        Ok(Self {
            relays: normalized.into_iter().collect(),
        })
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct SdkIdempotencyKey(String);

impl SdkIdempotencyKey {
    pub fn new(value: impl AsRef<str>) -> Result<Self, RadrootsSdkError> {
        let value = value.as_ref().trim();
        if value.is_empty() {
            return Err(invalid_request("idempotency key must not be empty"));
        }
        if value.len() > SDK_IDEMPOTENCY_KEY_MAX_LEN {
            return Err(invalid_request(format!(
                "idempotency key must be at most {SDK_IDEMPOTENCY_KEY_MAX_LEN} bytes"
            )));
        }
        if value.chars().any(char::is_control) {
            return Err(invalid_request(
                "idempotency key must not contain control characters",
            ));
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn into_string(self) -> String {
        self.0
    }

    pub(crate) fn derive(
        operation_kind: &'static str,
        expected_event_id: &str,
        expected_pubkey: &str,
        target_relays: &[String],
    ) -> Result<Self, RadrootsSdkError> {
        let input = SdkIdempotencyDerivationInput {
            operation_kind,
            expected_event_id,
            expected_pubkey,
            target_relays,
        };
        let bytes =
            serde_json::to_vec(&input).map_err(|error| RadrootsSdkError::InvalidRequest {
                message: format!("idempotency derivation failed: {error}"),
            })?;
        let digest = hex::encode(Sha256::digest(bytes));
        Self::new(format!("{operation_kind}:{digest}"))
    }
}

impl fmt::Debug for SdkIdempotencyKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SdkIdempotencyKey")
            .field("value", &"<redacted>")
            .field("len", &self.0.len())
            .finish()
    }
}

#[derive(serde::Serialize)]
struct SdkIdempotencyDerivationInput<'a> {
    operation_kind: &'static str,
    expected_event_id: &'a str,
    expected_pubkey: &'a str,
    target_relays: &'a [String],
}

fn normalized_relay_url(
    value: &str,
    policy: SdkRelayTargetPolicy,
) -> Result<String, RadrootsSdkError> {
    let relay = RadrootsRelayUrl::parse(value, policy.relay_transport_policy())
        .map_err(|error| invalid_request(format!("invalid relay target: {error}")))?;
    let normalized = relay.into_string();
    if normalized.starts_with("ws://") && !is_local_ws_relay(normalized.as_str()) {
        return Err(invalid_request(
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

fn invalid_request(message: impl Into<String>) -> RadrootsSdkError {
    RadrootsSdkError::InvalidRequest {
        message: message.into(),
    }
}
