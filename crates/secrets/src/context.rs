//! Validated semantic authority for encrypted envelopes.

use crate::error::{ContextField, ContextValueError, Error};
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
use sha2::{Digest, Sha256};

/// Version of the canonical authenticated context encoding.
pub const ENVELOPE_CONTEXT_VERSION: u16 = 1;
/// Domain separator included in every canonical context encoding.
pub const ENVELOPE_CONTEXT_DOMAIN: &[u8] = b"radroots.envelope_context.v1";
/// Maximum UTF-8 length of a purpose identifier.
pub const ENVELOPE_PURPOSE_MAX_BYTES: usize = 128;
/// Maximum UTF-8 length of a subject type discriminator.
pub const ENVELOPE_SUBJECT_TYPE_MAX_BYTES: usize = 64;
/// Maximum length of a subject's canonical bytes.
pub const ENVELOPE_SUBJECT_VALUE_MAX_BYTES: usize = 128;
/// Maximum UTF-8 length of a payload schema identifier.
pub const PAYLOAD_SCHEMA_MAX_BYTES: usize = 128;

/// Validated, namespaced use-case identifier for protected plaintext.
#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EnvelopePurpose(String);

impl EnvelopePurpose {
    /// Parses a canonical lower-case namespaced purpose.
    pub fn parse(value: impl Into<String>) -> Result<Self, Error> {
        let value = value.into();
        validate_namespaced(
            value.as_str(),
            ENVELOPE_PURPOSE_MAX_BYTES,
            ContextField::Purpose,
        )?;
        Ok(Self(value))
    }

    /// Returns the validated identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for EnvelopePurpose {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("EnvelopePurpose(<validated>)")
    }
}

impl fmt::Display for EnvelopePurpose {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("<validated envelope purpose>")
    }
}

/// Validated, typed identity of the protected object.
#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EnvelopeSubject {
    subject_type: String,
    value: String,
}

impl EnvelopeSubject {
    /// Parses a subject type and its canonical, non-secret identity bytes.
    pub fn parse(subject_type: impl Into<String>, value: impl Into<String>) -> Result<Self, Error> {
        let subject_type = subject_type.into();
        let value = value.into();
        validate_label(
            subject_type.as_str(),
            ENVELOPE_SUBJECT_TYPE_MAX_BYTES,
            ContextField::SubjectType,
        )?;
        validate_subject_value(value.as_str())?;
        Ok(Self {
            subject_type,
            value,
        })
    }

    /// Returns the validated type discriminator.
    #[must_use]
    pub fn subject_type(&self) -> &str {
        self.subject_type.as_str()
    }

    /// Returns the validated canonical subject value.
    #[must_use]
    pub fn value(&self) -> &str {
        self.value.as_str()
    }
}

impl fmt::Debug for EnvelopeSubject {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EnvelopeSubject")
            .field("subject_type", &self.subject_type)
            .field("value", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for EnvelopeSubject {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:<redacted>", self.subject_type)
    }
}

/// Validated, version-bearing schema identifier for protected plaintext.
#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PayloadSchemaId(String);

impl PayloadSchemaId {
    /// Parses a canonical lower-case namespaced payload schema identifier.
    pub fn parse(value: impl Into<String>) -> Result<Self, Error> {
        let value = value.into();
        validate_namespaced(
            value.as_str(),
            PAYLOAD_SCHEMA_MAX_BYTES,
            ContextField::PayloadSchema,
        )?;
        Ok(Self(value))
    }

    /// Returns the validated identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for PayloadSchemaId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PayloadSchemaId(<validated>)")
    }
}

impl fmt::Display for PayloadSchemaId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("<validated payload schema>")
    }
}

/// Independently validated semantic authority authenticated by an envelope.
#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EnvelopeContext {
    purpose: EnvelopePurpose,
    subject: EnvelopeSubject,
    payload_schema: PayloadSchemaId,
}

impl EnvelopeContext {
    /// Constructs a context only from independently validated parts.
    #[must_use]
    pub const fn new(
        purpose: EnvelopePurpose,
        subject: EnvelopeSubject,
        payload_schema: PayloadSchemaId,
    ) -> Self {
        Self {
            purpose,
            subject,
            payload_schema,
        }
    }

    /// Returns the authenticated use-case identifier.
    #[must_use]
    pub const fn purpose(&self) -> &EnvelopePurpose {
        &self.purpose
    }

    /// Returns the authenticated typed subject.
    #[must_use]
    pub const fn subject(&self) -> &EnvelopeSubject {
        &self.subject
    }

    /// Returns the authenticated payload schema identifier.
    #[must_use]
    pub const fn payload_schema(&self) -> &PayloadSchemaId {
        &self.payload_schema
    }

    /// Encodes the deterministic context wire representation used by envelope v2.
    #[must_use]
    pub fn to_canonical_bytes(&self) -> Vec<u8> {
        let purpose = self.purpose.as_str().as_bytes();
        let subject_type = self.subject.subject_type().as_bytes();
        let subject_value = self.subject.value().as_bytes();
        let payload_schema = self.payload_schema.as_str().as_bytes();
        let mut encoded = Vec::with_capacity(
            2 + ENVELOPE_CONTEXT_DOMAIN.len()
                + 2
                + purpose.len()
                + 2
                + subject_type.len()
                + 2
                + subject_value.len()
                + 2
                + payload_schema.len(),
        );
        encoded.extend_from_slice(&ENVELOPE_CONTEXT_VERSION.to_be_bytes());
        encoded.extend_from_slice(ENVELOPE_CONTEXT_DOMAIN);
        push_bounded(&mut encoded, purpose);
        push_bounded(&mut encoded, subject_type);
        push_bounded(&mut encoded, subject_value);
        push_bounded(&mut encoded, payload_schema);
        encoded
    }

    /// Returns the SHA-256 identity used to bind provider wrapping requests.
    #[must_use]
    pub fn authentication_digest(&self) -> [u8; 32] {
        Sha256::digest(self.to_canonical_bytes()).into()
    }
}

impl fmt::Debug for EnvelopeContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EnvelopeContext")
            .field("purpose", &self.purpose)
            .field("subject", &self.subject)
            .field("payload_schema", &self.payload_schema)
            .finish()
    }
}

fn push_bounded(encoded: &mut Vec<u8>, value: &[u8]) {
    let length = u16::try_from(value.len()).unwrap_or_else(|_| {
        unreachable!("validated envelope context fields are bounded below u16::MAX")
    });
    encoded.extend_from_slice(&length.to_be_bytes());
    encoded.extend_from_slice(value);
}

fn validate_namespaced(value: &str, max: usize, field: ContextField) -> Result<(), Error> {
    validate_length(value, max, field)?;
    if !value.contains('.') || !value.split('.').all(valid_segment) {
        return Err(invalid(field, ContextValueError::NonCanonical));
    }
    Ok(())
}

fn validate_label(value: &str, max: usize, field: ContextField) -> Result<(), Error> {
    validate_length(value, max, field)?;
    if !valid_segment(value) {
        return Err(invalid(field, ContextValueError::NonCanonical));
    }
    Ok(())
}

fn validate_subject_value(value: &str) -> Result<(), Error> {
    validate_length(
        value,
        ENVELOPE_SUBJECT_VALUE_MAX_BYTES,
        ContextField::SubjectValue,
    )?;
    if !value.bytes().all(|byte| {
        byte.is_ascii_lowercase()
            || byte.is_ascii_digit()
            || matches!(byte, b'_' | b'-' | b'.' | b':' | b'/')
    }) {
        return Err(invalid(
            ContextField::SubjectValue,
            ContextValueError::NonCanonical,
        ));
    }
    Ok(())
}

fn validate_length(value: &str, max: usize, field: ContextField) -> Result<(), Error> {
    if value.is_empty() {
        return Err(invalid(field, ContextValueError::Empty));
    }
    if value.len() > max {
        return Err(invalid(
            field,
            ContextValueError::TooLong {
                actual_bytes: value.len(),
                max_bytes: max,
            },
        ));
    }
    if !value.is_ascii() || value != value.trim() || value.chars().any(char::is_control) {
        return Err(invalid(field, ContextValueError::NonCanonical));
    }
    Ok(())
}

fn valid_segment(segment: &str) -> bool {
    let mut bytes = segment.bytes();
    matches!(bytes.next(), Some(first) if first.is_ascii_lowercase())
        && bytes.all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
}

const fn invalid(field: ContextField, reason: ContextValueError) -> Error {
    Error::InvalidContextValue { field, reason }
}

#[cfg(feature = "serde")]
mod serde_impl {
    use super::{EnvelopeContext, EnvelopePurpose, EnvelopeSubject, PayloadSchemaId};
    use alloc::string::String;

    #[derive(serde::Serialize, serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct WireContext {
        purpose: String,
        subject_type: String,
        subject: String,
        payload_schema: String,
    }

    impl serde::Serialize for EnvelopeContext {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            WireContext {
                purpose: String::from(self.purpose().as_str()),
                subject_type: String::from(self.subject().subject_type()),
                subject: String::from(self.subject().value()),
                payload_schema: String::from(self.payload_schema().as_str()),
            }
            .serialize(serializer)
        }
    }

    impl<'de> serde::Deserialize<'de> for EnvelopeContext {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let wire = WireContext::deserialize(deserializer)?;
            Ok(Self::new(
                EnvelopePurpose::parse(wire.purpose).map_err(serde::de::Error::custom)?,
                EnvelopeSubject::parse(wire.subject_type, wire.subject)
                    .map_err(serde::de::Error::custom)?,
                PayloadSchemaId::parse(wire.payload_schema).map_err(serde::de::Error::custom)?,
            ))
        }
    }
}
