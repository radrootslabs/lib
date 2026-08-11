//! Stable, bounded reason codes for status and health surfaces.

use core::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::StatusContractError;

pub const REASON_CODE_MAX_BYTES: usize = 64;
pub const REASON_CODES_MAX_ITEMS: usize = 32;

/// Shared reason codes whose meanings are service-neutral.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommonReasonCode {
    IdentityUnavailable,
    DatabaseSchemaMismatch,
    DatabaseReadOnly,
    DatabaseLowDisk,
    RequiredRelayUnavailable,
    SubscriberNotActive,
    SignerProviderUnavailable,
    OutboxInvariantFailed,
    PublicationBacklogExceeded,
    AdminListenerFailed,
    OperationsListenerFailed,
    ShutdownInProgress,
}

impl CommonReasonCode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::IdentityUnavailable => "identity_unavailable",
            Self::DatabaseSchemaMismatch => "database_schema_mismatch",
            Self::DatabaseReadOnly => "database_read_only",
            Self::DatabaseLowDisk => "database_low_disk",
            Self::RequiredRelayUnavailable => "required_relay_unavailable",
            Self::SubscriberNotActive => "subscriber_not_active",
            Self::SignerProviderUnavailable => "signer_provider_unavailable",
            Self::OutboxInvariantFailed => "outbox_invariant_failed",
            Self::PublicationBacklogExceeded => "publication_backlog_exceeded",
            Self::AdminListenerFailed => "admin_listener_failed",
            Self::OperationsListenerFailed => "operations_listener_failed",
            Self::ShutdownInProgress => "shutdown_in_progress",
        }
    }
}

/// A validated stable status reason code.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct ReasonCode(String);

impl ReasonCode {
    pub fn new(value: impl Into<String>) -> Result<Self, StatusContractError> {
        let value = value.into();
        if !valid_reason_code(&value) {
            return Err(StatusContractError::InvalidReasonCode);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<CommonReasonCode> for ReasonCode {
    fn from(value: CommonReasonCode) -> Self {
        Self(value.as_str().to_owned())
    }
}

impl fmt::Display for ReasonCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for ReasonCode {
    type Err = StatusContractError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl Serialize for ReasonCode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ReasonCode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

/// A unique, canonically sorted, bounded reason-code collection.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct ReasonCodes(Vec<ReasonCode>);

impl ReasonCodes {
    #[must_use]
    pub const fn empty() -> Self {
        Self(Vec::new())
    }

    pub fn new(values: impl IntoIterator<Item = ReasonCode>) -> Result<Self, StatusContractError> {
        let mut values: Vec<_> = values.into_iter().collect();
        values.sort_unstable();
        values.dedup();
        if values.len() > REASON_CODES_MAX_ITEMS {
            return Err(StatusContractError::TooManyReasonCodes {
                maximum: REASON_CODES_MAX_ITEMS,
            });
        }
        Ok(Self(values))
    }

    #[must_use]
    pub fn as_slice(&self) -> &[ReasonCode] {
        &self.0
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<'de> Deserialize<'de> for ReasonCodes {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = Vec::<ReasonCode>::deserialize(deserializer)?;
        if raw.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(serde::de::Error::custom(
                "reason codes must be unique and canonically sorted",
            ));
        }
        Self::new(raw).map_err(serde::de::Error::custom)
    }
}

fn valid_reason_code(value: &str) -> bool {
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    value.len() <= REASON_CODE_MAX_BYTES
        && first.is_ascii_lowercase()
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_codes_are_stable_valid_and_unique() {
        let codes = [
            CommonReasonCode::IdentityUnavailable,
            CommonReasonCode::DatabaseSchemaMismatch,
            CommonReasonCode::DatabaseReadOnly,
            CommonReasonCode::DatabaseLowDisk,
            CommonReasonCode::RequiredRelayUnavailable,
            CommonReasonCode::SubscriberNotActive,
            CommonReasonCode::SignerProviderUnavailable,
            CommonReasonCode::OutboxInvariantFailed,
            CommonReasonCode::PublicationBacklogExceeded,
            CommonReasonCode::AdminListenerFailed,
            CommonReasonCode::OperationsListenerFailed,
            CommonReasonCode::ShutdownInProgress,
        ];
        let reasons = ReasonCodes::new(codes.map(ReasonCode::from)).expect("common reasons");
        assert_eq!(reasons.as_slice().len(), codes.len());
        assert!(reasons.as_slice().windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn reason_code_validation_matches_frozen_contract() {
        for valid in ["a", "database_low_disk", "myc_reason_01"] {
            assert_eq!(ReasonCode::new(valid).unwrap().as_str(), valid);
        }
        assert!(ReasonCode::new("a".repeat(REASON_CODE_MAX_BYTES)).is_ok());

        for invalid in ["", "Upper", "1reason", "a-b", "a.b", "a b", "café"] {
            assert_eq!(
                ReasonCode::new(invalid),
                Err(StatusContractError::InvalidReasonCode)
            );
        }
        assert!(ReasonCode::new("a".repeat(REASON_CODE_MAX_BYTES + 1)).is_err());
    }

    #[test]
    fn collections_sort_deduplicate_bound_and_serialize_canonically() {
        let reasons = ReasonCodes::new([
            ReasonCode::new("z_reason").unwrap(),
            ReasonCode::new("a_reason").unwrap(),
            ReasonCode::new("z_reason").unwrap(),
        ])
        .expect("bounded reasons");
        assert_eq!(
            serde_json::to_string(&reasons).unwrap(),
            r#"["a_reason","z_reason"]"#
        );

        let over = (0..=REASON_CODES_MAX_ITEMS)
            .map(|index| ReasonCode::new(format!("reason_{index:02}")).unwrap());
        assert_eq!(
            ReasonCodes::new(over),
            Err(StatusContractError::TooManyReasonCodes {
                maximum: REASON_CODES_MAX_ITEMS
            })
        );
        assert!(serde_json::from_str::<ReasonCodes>(r#"["z_reason","a_reason"]"#).is_err());
        assert!(serde_json::from_str::<ReasonCodes>(r#"["a_reason","a_reason"]"#).is_err());
    }
}
