use crate::RadrootsSdkError;
use core::fmt;
use serde::ser::SerializeStruct;
use sha2::{Digest, Sha256};

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

#[derive(serde::Serialize)]
struct SdkIdempotencyDerivationInput<'a> {
    operation_kind: &'static str,
    expected_event_id: &'a str,
    expected_pubkey: &'a str,
    target_relays: &'a [String],
}

fn invalid_request(message: impl Into<String>) -> RadrootsSdkError {
    RadrootsSdkError::InvalidRequest {
        message: message.into(),
    }
}
