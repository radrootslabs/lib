use crate::RadrootsSdkError;
use core::fmt;
use radroots_event::ids::{RadrootsEventId, RadrootsPublicKey};
use serde::ser::SerializeStruct;

pub const SDK_IDEMPOTENCY_KEY_MAX_LEN: usize = 256;

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct SdkIdempotencyKey(String);

impl SdkIdempotencyKey {
    pub fn new(value: impl AsRef<str>) -> Result<Self, RadrootsSdkError> {
        let value = value.as_ref();
        if value.is_empty() {
            return Err(invalid_request("idempotency key must not be empty"));
        }
        if value.trim() != value {
            return Err(invalid_request(
                "idempotency key must not include boundary whitespace",
            ));
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
        if !is_uuid_v7(value) {
            return Err(invalid_request("idempotency key must be a UUIDv7"));
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn into_string(self) -> String {
        self.0
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

impl serde::Serialize for SdkIdempotencyKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("SdkIdempotencyKey", 2)?;
        state.serialize_field("value", "<redacted>")?;
        state.serialize_field("len", &self.0.len())?;
        state.end()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct SdkTradeIdempotencyRecord {
    pub idempotency_key: SdkIdempotencyKey,
    pub operation_kind: String,
    pub actor_pubkey: RadrootsPublicKey,
    pub digest: String,
    pub canonical_payload_hash: String,
    pub expected_event_id: RadrootsEventId,
    pub outbox_operation_id: i64,
}

impl SdkTradeIdempotencyRecord {
    pub fn matches_payload(&self, canonical_payload_hash: &str) -> bool {
        self.canonical_payload_hash == canonical_payload_hash
    }

    pub fn conflict_error(&self, new_digest: impl Into<String>) -> RadrootsSdkError {
        RadrootsSdkError::IdempotencyConflict {
            operation_kind: self.operation_kind.clone(),
            expected_pubkey_prefix: self.actor_pubkey.as_str().chars().take(12).collect(),
            existing_digest_prefix: self.digest.chars().take(12).collect(),
            new_digest_prefix: new_digest.into().chars().take(12).collect(),
        }
    }
}

fn invalid_request(message: impl Into<String>) -> RadrootsSdkError {
    RadrootsSdkError::InvalidRequest {
        message: message.into(),
    }
}

fn is_uuid_v7(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 36
        && bytes[8] == b'-'
        && bytes[13] == b'-'
        && bytes[18] == b'-'
        && bytes[23] == b'-'
        && bytes[14] == b'7'
        && matches!(bytes[19], b'8' | b'9' | b'a' | b'b')
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 8 | 13 | 18 | 23) || byte.is_ascii_hexdigit())
}

#[cfg(test)]
#[path = "../tests/unit/idempotency_tests.rs"]
mod tests;
