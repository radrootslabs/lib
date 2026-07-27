use crate::{
    RADROOTS_TRANSPORT_IDENTIFIER_MAX_BYTES, RADROOTS_TRANSPORT_OPAQUE_PAYLOAD_MAX_BYTES,
    RADROOTS_TRANSPORT_RETICULUM_PAYLOAD_MAX_BYTES, RADROOTS_TRANSPORT_SIGNED_EVENT_JSON_MAX_BYTES,
    RadrootsTransportError, limits::ensure_resource_limit,
};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsTransportPayload {
    body: RadrootsTransportPayloadBody,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
enum RadrootsTransportPayloadBody {
    SignedEventJson {
        event_id: String,
        raw_json: String,
        digest: String,
    },
    MeshFrameCbor {
        message_id: String,
        bytes: Vec<u8>,
        digest: String,
    },
    OpaqueBytes {
        label: String,
        bytes: Vec<u8>,
        digest: String,
    },
}

impl RadrootsTransportPayload {
    pub fn unchecked_signed_event_json(
        event_id: impl AsRef<str>,
        raw_json: impl AsRef<str>,
    ) -> Result<Self, RadrootsTransportError> {
        Self::signed_event_json(event_id.as_ref(), raw_json.as_ref())
    }

    fn signed_event_json(event_id: &str, raw_json: &str) -> Result<Self, RadrootsTransportError> {
        let event_id = validate_hex_id(event_id)?;
        let raw_json = validate_raw_json(raw_json)?;
        let digest = sha256_hex(raw_json.as_bytes());
        Ok(Self {
            body: RadrootsTransportPayloadBody::SignedEventJson {
                event_id,
                raw_json,
                digest,
            },
        })
    }

    pub fn unchecked_signed_event_json_with_digest(
        event_id: impl AsRef<str>,
        raw_json: impl AsRef<str>,
        digest: impl AsRef<str>,
    ) -> Result<Self, RadrootsTransportError> {
        Self::signed_event_json_with_digest(event_id.as_ref(), raw_json.as_ref(), digest.as_ref())
    }

    fn signed_event_json_with_digest(
        event_id: &str,
        raw_json: &str,
        digest: &str,
    ) -> Result<Self, RadrootsTransportError> {
        let payload = Self::signed_event_json(event_id, raw_json)?;
        validate_supplied_digest(payload.digest(), digest)?;
        Ok(payload)
    }

    pub fn mesh_frame_cbor(
        message_id: impl AsRef<str>,
        bytes: impl AsRef<[u8]>,
    ) -> Result<Self, RadrootsTransportError> {
        Self::validated_mesh_frame_cbor(message_id.as_ref(), bytes.as_ref())
    }

    fn validated_mesh_frame_cbor(
        message_id: &str,
        bytes: &[u8],
    ) -> Result<Self, RadrootsTransportError> {
        let message_id = validate_token_id(message_id)?;
        let bytes = validate_bytes(
            "mesh_frame_cbor_bytes",
            bytes,
            RADROOTS_TRANSPORT_RETICULUM_PAYLOAD_MAX_BYTES,
        )?;
        let digest = sha256_hex(bytes.as_slice());
        Ok(Self {
            body: RadrootsTransportPayloadBody::MeshFrameCbor {
                message_id,
                bytes,
                digest,
            },
        })
    }

    pub fn mesh_frame_cbor_with_digest(
        message_id: impl AsRef<str>,
        bytes: impl AsRef<[u8]>,
        digest: impl AsRef<str>,
    ) -> Result<Self, RadrootsTransportError> {
        Self::validated_mesh_frame_cbor_with_digest(
            message_id.as_ref(),
            bytes.as_ref(),
            digest.as_ref(),
        )
    }

    fn validated_mesh_frame_cbor_with_digest(
        message_id: &str,
        bytes: &[u8],
        digest: &str,
    ) -> Result<Self, RadrootsTransportError> {
        let payload = Self::validated_mesh_frame_cbor(message_id, bytes)?;
        validate_supplied_digest(payload.digest(), digest)?;
        Ok(payload)
    }

    pub fn opaque_bytes(
        label: impl AsRef<str>,
        bytes: impl AsRef<[u8]>,
    ) -> Result<Self, RadrootsTransportError> {
        Self::validated_opaque_bytes(label.as_ref(), bytes.as_ref())
    }

    fn validated_opaque_bytes(label: &str, bytes: &[u8]) -> Result<Self, RadrootsTransportError> {
        let label = validate_label(label)?;
        let bytes = validate_bytes(
            "opaque_payload_bytes",
            bytes,
            RADROOTS_TRANSPORT_OPAQUE_PAYLOAD_MAX_BYTES,
        )?;
        let digest = sha256_hex(bytes.as_slice());
        Ok(Self {
            body: RadrootsTransportPayloadBody::OpaqueBytes {
                label,
                bytes,
                digest,
            },
        })
    }

    pub fn opaque_bytes_with_digest(
        label: impl AsRef<str>,
        bytes: impl AsRef<[u8]>,
        digest: impl AsRef<str>,
    ) -> Result<Self, RadrootsTransportError> {
        Self::validated_opaque_bytes_with_digest(label.as_ref(), bytes.as_ref(), digest.as_ref())
    }

    fn validated_opaque_bytes_with_digest(
        label: &str,
        bytes: &[u8],
        digest: &str,
    ) -> Result<Self, RadrootsTransportError> {
        let payload = Self::validated_opaque_bytes(label, bytes)?;
        validate_supplied_digest(payload.digest(), digest)?;
        Ok(payload)
    }

    pub fn digest(&self) -> &str {
        match &self.body {
            RadrootsTransportPayloadBody::SignedEventJson { digest, .. }
            | RadrootsTransportPayloadBody::MeshFrameCbor { digest, .. }
            | RadrootsTransportPayloadBody::OpaqueBytes { digest, .. } => digest.as_str(),
        }
    }

    pub fn payload_kind(&self) -> &'static str {
        match &self.body {
            RadrootsTransportPayloadBody::SignedEventJson { .. } => "signed_event_json",
            RadrootsTransportPayloadBody::MeshFrameCbor { .. } => "mesh_frame_cbor",
            RadrootsTransportPayloadBody::OpaqueBytes { .. } => "opaque_bytes",
        }
    }

    pub fn signed_event_json_parts(&self) -> Option<(&str, &str)> {
        match &self.body {
            RadrootsTransportPayloadBody::SignedEventJson {
                event_id, raw_json, ..
            } => Some((event_id, raw_json)),
            RadrootsTransportPayloadBody::MeshFrameCbor { .. }
            | RadrootsTransportPayloadBody::OpaqueBytes { .. } => None,
        }
    }

    pub fn mesh_frame_cbor_parts(&self) -> Option<(&str, &[u8])> {
        match &self.body {
            RadrootsTransportPayloadBody::MeshFrameCbor {
                message_id, bytes, ..
            } => Some((message_id, bytes)),
            RadrootsTransportPayloadBody::SignedEventJson { .. }
            | RadrootsTransportPayloadBody::OpaqueBytes { .. } => None,
        }
    }

    pub fn opaque_bytes_parts(&self) -> Option<(&str, &[u8])> {
        match &self.body {
            RadrootsTransportPayloadBody::OpaqueBytes { label, bytes, .. } => Some((label, bytes)),
            RadrootsTransportPayloadBody::SignedEventJson { .. }
            | RadrootsTransportPayloadBody::MeshFrameCbor { .. } => None,
        }
    }

    pub fn validate(&self) -> Result<(), RadrootsTransportError> {
        let canonical = match &self.body {
            RadrootsTransportPayloadBody::SignedEventJson {
                event_id,
                raw_json,
                digest,
            } => Self::signed_event_json_with_digest(event_id, raw_json, digest)?,
            RadrootsTransportPayloadBody::MeshFrameCbor {
                message_id,
                bytes,
                digest,
            } => Self::validated_mesh_frame_cbor_with_digest(message_id, bytes, digest)?,
            RadrootsTransportPayloadBody::OpaqueBytes {
                label,
                bytes,
                digest,
            } => Self::validated_opaque_bytes_with_digest(label, bytes, digest)?,
        };
        if &canonical != self {
            return Err(RadrootsTransportError::InvalidPayloadLabel);
        }
        Ok(())
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for RadrootsTransportPayload {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.body.serialize(serializer)
    }
}

#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
enum RadrootsTransportPayloadWire {
    SignedEventJson {
        #[serde(deserialize_with = "deserialize_payload_event_id")]
        event_id: String,
        #[serde(deserialize_with = "deserialize_signed_event_json")]
        raw_json: String,
        #[serde(deserialize_with = "deserialize_payload_digest")]
        digest: String,
    },
    MeshFrameCbor {
        #[serde(deserialize_with = "deserialize_payload_identifier")]
        message_id: String,
        #[serde(deserialize_with = "deserialize_reticulum_payload_bytes")]
        bytes: Vec<u8>,
        #[serde(deserialize_with = "deserialize_payload_digest")]
        digest: String,
    },
    OpaqueBytes {
        #[serde(deserialize_with = "deserialize_payload_identifier")]
        label: String,
        #[serde(deserialize_with = "deserialize_opaque_payload_bytes")]
        bytes: Vec<u8>,
        #[serde(deserialize_with = "deserialize_payload_digest")]
        digest: String,
    },
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for RadrootsTransportPayload {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = RadrootsTransportPayloadWire::deserialize(deserializer)?;
        match wire {
            RadrootsTransportPayloadWire::SignedEventJson {
                event_id,
                raw_json,
                digest,
            } => Self::signed_event_json_with_digest(&event_id, &raw_json, &digest),
            RadrootsTransportPayloadWire::MeshFrameCbor {
                message_id,
                bytes,
                digest,
            } => Self::validated_mesh_frame_cbor_with_digest(&message_id, &bytes, &digest),
            RadrootsTransportPayloadWire::OpaqueBytes {
                label,
                bytes,
                digest,
            } => Self::validated_opaque_bytes_with_digest(&label, &bytes, &digest),
        }
        .map_err(serde::de::Error::custom)
    }
}

#[cfg(feature = "serde")]
fn deserialize_payload_event_id<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    crate::serde_bounds::deserialize_string(deserializer, "payload_id", 64)
}

#[cfg(feature = "serde")]
fn deserialize_payload_identifier<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    crate::serde_bounds::deserialize_string(
        deserializer,
        "payload_id",
        RADROOTS_TRANSPORT_IDENTIFIER_MAX_BYTES,
    )
}

#[cfg(feature = "serde")]
fn deserialize_signed_event_json<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    crate::serde_bounds::deserialize_string(
        deserializer,
        "signed_event_json_bytes",
        RADROOTS_TRANSPORT_SIGNED_EVENT_JSON_MAX_BYTES,
    )
}

#[cfg(feature = "serde")]
fn deserialize_payload_digest<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    crate::serde_bounds::deserialize_string(deserializer, "digest", 64)
}

#[cfg(feature = "serde")]
fn deserialize_reticulum_payload_bytes<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    crate::serde_bounds::deserialize_vec(
        deserializer,
        "mesh_frame_cbor_bytes",
        RADROOTS_TRANSPORT_RETICULUM_PAYLOAD_MAX_BYTES,
    )
}

#[cfg(feature = "serde")]
fn deserialize_opaque_payload_bytes<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    crate::serde_bounds::deserialize_vec(
        deserializer,
        "opaque_payload_bytes",
        RADROOTS_TRANSPORT_OPAQUE_PAYLOAD_MAX_BYTES,
    )
}

fn validate_hex_id(raw: &str) -> Result<String, RadrootsTransportError> {
    if raw.len() != 64 || !raw.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(RadrootsTransportError::InvalidPayloadId);
    }
    let lowered = raw.to_ascii_lowercase();
    if raw != lowered {
        return Err(RadrootsTransportError::InvalidPayloadId);
    }
    Ok(lowered)
}

fn validate_token_id(raw: &str) -> Result<String, RadrootsTransportError> {
    if raw.is_empty() {
        return Err(RadrootsTransportError::EmptyPayloadId);
    }
    ensure_resource_limit(
        "payload_id",
        raw.len(),
        RADROOTS_TRANSPORT_IDENTIFIER_MAX_BYTES,
    )?;
    if raw != raw.trim()
        || raw
            .chars()
            .any(|ch| !(ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.')))
    {
        return Err(RadrootsTransportError::InvalidPayloadId);
    }
    Ok(raw.to_string())
}

fn validate_label(raw: &str) -> Result<String, RadrootsTransportError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(RadrootsTransportError::EmptyPayloadLabel);
    }
    ensure_resource_limit(
        "payload_label",
        raw.len(),
        RADROOTS_TRANSPORT_IDENTIFIER_MAX_BYTES,
    )?;
    if raw != trimmed || trimmed.chars().any(char::is_control) {
        return Err(RadrootsTransportError::InvalidPayloadLabel);
    }
    Ok(trimmed.to_string())
}

fn validate_raw_json(raw: &str) -> Result<String, RadrootsTransportError> {
    if raw.is_empty() {
        return Err(RadrootsTransportError::EmptyPayloadBytes);
    }
    ensure_resource_limit(
        "signed_event_json_bytes",
        raw.len(),
        RADROOTS_TRANSPORT_SIGNED_EVENT_JSON_MAX_BYTES,
    )?;
    if raw != raw.trim()
        || raw.chars().any(char::is_control)
        || !raw.starts_with('{')
        || !raw.ends_with('}')
    {
        return Err(RadrootsTransportError::InvalidPayloadBytes);
    }
    Ok(raw.to_string())
}

fn validate_bytes(
    field: &'static str,
    raw: &[u8],
    max: usize,
) -> Result<Vec<u8>, RadrootsTransportError> {
    if raw.is_empty() {
        return Err(RadrootsTransportError::EmptyPayloadBytes);
    }
    ensure_resource_limit(field, raw.len(), max)?;
    Ok(raw.to_vec())
}

fn validate_supplied_digest(expected: &str, supplied: &str) -> Result<(), RadrootsTransportError> {
    validate_digest(supplied)?;
    if supplied != expected {
        return Err(RadrootsTransportError::PayloadDigestMismatch);
    }
    Ok(())
}

fn validate_digest(raw: &str) -> Result<(), RadrootsTransportError> {
    if raw.len() != 64
        || !raw
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(RadrootsTransportError::InvalidPayloadDigest);
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex_encode(Sha256::digest(bytes).as_slice())
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}
