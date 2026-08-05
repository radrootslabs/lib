//! Versioned context-bound encrypted-envelope contracts.

use crate::context::{
    ENVELOPE_CONTEXT_DOMAIN, ENVELOPE_CONTEXT_VERSION, ENVELOPE_PURPOSE_MAX_BYTES,
    ENVELOPE_SUBJECT_TYPE_MAX_BYTES, ENVELOPE_SUBJECT_VALUE_MAX_BYTES, EnvelopeContext,
    EnvelopePurpose, EnvelopeSubject, PAYLOAD_SCHEMA_MAX_BYTES, PayloadSchemaId,
};
use crate::error::Error;
use crate::id::{BackendKind, KeyVersion};
use crate::wrapping::{KeyWrapping, SecretMaterial, UnwrapRequest, WrapRequest, WrappedSecret};
use crate::{SecretId, SecretRef};
use alloc::string::String;
use alloc::vec::Vec;
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use core::fmt;

const MAGIC: [u8; 4] = *b"RRS1";
const DATA_KEY_BYTES: usize = 32;
const AEAD_TAG_BYTES: usize = 16;
const NONCE_BYTES: usize = 24;

/// Legacy structurally authenticated envelope format.
pub const LEGACY_ENVELOPE_VERSION: u16 = 1;
/// Current context-bound authenticated envelope format.
pub const ENVELOPE_VERSION: u16 = 2;
/// Maximum encoded envelope size accepted from storage.
pub const ENVELOPE_MAX_BYTES: usize = 256 * 1024;

/// Authenticated-encryption algorithm used by an envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Cipher {
    /// XChaCha20-Poly1305 with a 192-bit nonce.
    XChaCha20Poly1305,
}

impl Cipher {
    const fn code(self) -> u8 {
        match self {
            Self::XChaCha20Poly1305 => 1,
        }
    }

    const fn from_code(code: u8) -> Result<Self, Error> {
        match code {
            1 => Ok(Self::XChaCha20Poly1305),
            cipher => Err(Error::UnsupportedCipher { cipher }),
        }
    }
}

/// How the data-encryption key is protected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum KeySource {
    /// The host-selected [`KeyWrapping`] provider protects the data key.
    ProviderWrapped,
}

impl KeySource {
    const fn code(self) -> u8 {
        match self {
            Self::ProviderWrapped => 1,
        }
    }

    const fn from_code(code: u8) -> Result<Self, Error> {
        match code {
            1 => Ok(Self::ProviderWrapped),
            key_source => Err(Error::UnsupportedKeySource { key_source }),
        }
    }
}

/// Explicit 192-bit nonce supplied by the host.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Nonce([u8; NONCE_BYTES]);

impl Nonce {
    /// Creates a nonce from exact caller-supplied bytes.
    #[must_use]
    pub const fn new(bytes: [u8; NONCE_BYTES]) -> Self {
        Self(bytes)
    }

    /// Returns the nonce bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; NONCE_BYTES] {
        &self.0
    }
}

impl fmt::Debug for Nonce {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Nonce(<redacted>)")
    }
}

/// Caller-supplied cryptographic material for one sealing operation.
pub struct SealMaterial {
    data_key: SecretMaterial,
    nonce: Nonce,
}

impl SealMaterial {
    /// Couples an explicitly generated data key and nonce.
    #[must_use]
    pub const fn new(data_key: SecretMaterial, nonce: Nonce) -> Self {
        Self { data_key, nonce }
    }
}

impl fmt::Debug for SealMaterial {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SealMaterial(<redacted>)")
    }
}

/// Complete input for one context-bound envelope sealing operation.
pub struct SealRequest<'a> {
    reference: SecretRef,
    context: EnvelopeContext,
    plaintext: &'a SecretMaterial,
    material: SealMaterial,
}

impl<'a> SealRequest<'a> {
    /// Creates a request without generating entropy or selecting a provider.
    #[must_use]
    pub const fn new(
        reference: SecretRef,
        context: EnvelopeContext,
        plaintext: &'a SecretMaterial,
        material: SealMaterial,
    ) -> Self {
        Self {
            reference,
            context,
            plaintext,
            material,
        }
    }
}

impl fmt::Debug for SealRequest<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SealRequest(<redacted>)")
    }
}

/// A versioned authenticated envelope with provider-wrapped key material.
pub struct EncryptedEnvelope {
    version: u16,
    cipher: Cipher,
    key_source: KeySource,
    reference: SecretRef,
    context: Option<EnvelopeContext>,
    nonce: Nonce,
    wrapped_key: WrappedSecret,
    ciphertext: Vec<u8>,
}

impl EncryptedEnvelope {
    /// Seals plaintext as v2 using explicit context, key, and nonce material.
    pub async fn seal(wrapping: &dyn KeyWrapping, request: SealRequest<'_>) -> Result<Self, Error> {
        let SealRequest {
            reference,
            context,
            plaintext,
            material,
        } = request;
        validate_data_key(&material.data_key)?;
        let wrapped_key = wrapping
            .wrap(WrapRequest::new(&reference, &context, &material.data_key))
            .await?;
        let ciphertext_len =
            plaintext
                .len()
                .checked_add(AEAD_TAG_BYTES)
                .ok_or(Error::EnvelopeTooLarge {
                    actual_bytes: usize::MAX,
                    max_bytes: ENVELOPE_MAX_BYTES,
                })?;
        let aad = encode_header(
            ENVELOPE_VERSION,
            Cipher::XChaCha20Poly1305,
            KeySource::ProviderWrapped,
            &reference,
            Some(&context),
            material.nonce,
            &wrapped_key,
            ciphertext_len,
        )?;
        let ciphertext = material.data_key.expose_secret(|data_key| {
            plaintext.expose_secret(|plaintext| {
                let cipher = XChaCha20Poly1305::new(Key::from_slice(data_key));
                cipher
                    .encrypt(
                        XNonce::from_slice(material.nonce.as_bytes()),
                        Payload {
                            msg: plaintext,
                            aad: aad.as_slice(),
                        },
                    )
                    .map_err(|_| Error::EncryptFailed)
            })
        })?;
        let envelope = Self {
            version: ENVELOPE_VERSION,
            cipher: Cipher::XChaCha20Poly1305,
            key_source: KeySource::ProviderWrapped,
            reference,
            context: Some(context),
            nonce: material.nonce,
            wrapped_key,
            ciphertext,
        };
        envelope.validate()?;
        Ok(envelope)
    }

    /// Authenticates v2 using independently expected context before releasing plaintext.
    pub async fn open(
        &self,
        wrapping: &dyn KeyWrapping,
        expected_context: &EnvelopeContext,
    ) -> Result<SecretMaterial, Error> {
        self.validate()?;
        if self.version == LEGACY_ENVELOPE_VERSION {
            return Err(Error::LegacyEnvelopeDenied);
        }
        let stored_context = self.context.as_ref().ok_or(Error::EnvelopeMalformed)?;
        if stored_context != expected_context {
            return Err(Error::ContextMismatch);
        }
        let data_key = wrapping
            .unwrap(UnwrapRequest::new(
                &self.reference,
                expected_context,
                &self.wrapped_key,
            ))
            .await?;
        validate_data_key(&data_key)?;
        let aad = self.encoded_header()?;
        let plaintext = data_key.expose_secret(|data_key| {
            let cipher = XChaCha20Poly1305::new(Key::from_slice(data_key));
            cipher
                .decrypt(
                    XNonce::from_slice(self.nonce.as_bytes()),
                    Payload {
                        msg: self.ciphertext.as_slice(),
                        aad: aad.as_slice(),
                    },
                )
                .map_err(|_| Error::DecryptFailed)
        })?;
        SecretMaterial::from_owned(plaintext)
    }

    /// Returns the authenticated provider reference.
    #[must_use]
    pub const fn reference(&self) -> &SecretRef {
        &self.reference
    }

    /// Returns the format version.
    #[must_use]
    pub const fn version(&self) -> u16 {
        self.version
    }

    /// Returns the authenticated v2 context, or `None` for a legacy v1 envelope.
    #[must_use]
    pub const fn context(&self) -> Option<&EnvelopeContext> {
        self.context.as_ref()
    }

    /// Returns the authenticated cipher identifier.
    #[must_use]
    pub const fn cipher(&self) -> Cipher {
        self.cipher
    }

    /// Returns the authenticated key-source identifier.
    #[must_use]
    pub const fn key_source(&self) -> KeySource {
        self.key_source
    }

    /// Encodes the validated envelope into its deterministic binary form.
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        self.validate()?;
        let mut encoded = self.encoded_header()?;
        encoded.extend_from_slice(self.ciphertext.as_slice());
        if encoded.len() > ENVELOPE_MAX_BYTES {
            return Err(Error::EnvelopeTooLarge {
                actual_bytes: encoded.len(),
                max_bytes: ENVELOPE_MAX_BYTES,
            });
        }
        Ok(encoded)
    }

    /// Decodes and validates v1 or v2 without accessing a provider.
    pub fn decode(encoded: &[u8]) -> Result<Self, Error> {
        if encoded.len() > ENVELOPE_MAX_BYTES {
            return Err(Error::EnvelopeTooLarge {
                actual_bytes: encoded.len(),
                max_bytes: ENVELOPE_MAX_BYTES,
            });
        }
        let mut decoder = Decoder::new(encoded);
        if decoder.take_array::<4>()? != MAGIC {
            return Err(Error::EnvelopeMalformed);
        }
        let version = decoder.u16()?;
        if !matches!(version, LEGACY_ENVELOPE_VERSION | ENVELOPE_VERSION) {
            return Err(Error::UnsupportedEnvelopeVersion { version });
        }
        let cipher = Cipher::from_code(decoder.u8()?)?;
        let key_source = KeySource::from_code(decoder.u8()?)?;
        let backend = BackendKind::from_code(decoder.u8()?)?;
        let key_version = KeyVersion::new(decoder.u32()?)?;
        let id = decoder.bounded_string(u16::MAX.into())?;
        let reference = SecretRef::new(SecretId::parse(id)?, backend, key_version);
        let context = if version == ENVELOPE_VERSION {
            Some(decode_context(&mut decoder)?)
        } else {
            None
        };
        let nonce = Nonce::new(decoder.take_array::<NONCE_BYTES>()?);
        let wrapped_len = decoder.u32_usize()?;
        let wrapped_key = WrappedSecret::from_bytes(decoder.take(wrapped_len)?.to_vec())?;
        let ciphertext_len = decoder.u32_usize()?;
        let ciphertext = decoder.take(ciphertext_len)?.to_vec();
        if !decoder.is_empty() {
            return Err(Error::EnvelopeMalformed);
        }
        let envelope = Self {
            version,
            cipher,
            key_source,
            reference,
            context,
            nonce,
            wrapped_key,
            ciphertext,
        };
        envelope.validate()?;
        Ok(envelope)
    }

    fn encoded_header(&self) -> Result<Vec<u8>, Error> {
        encode_header(
            self.version,
            self.cipher,
            self.key_source,
            &self.reference,
            self.context.as_ref(),
            self.nonce,
            &self.wrapped_key,
            self.ciphertext.len(),
        )
    }

    fn validate(&self) -> Result<(), Error> {
        match (self.version, self.context.is_some()) {
            (LEGACY_ENVELOPE_VERSION, false) | (ENVELOPE_VERSION, true) => {}
            (LEGACY_ENVELOPE_VERSION | ENVELOPE_VERSION, _) => {
                return Err(Error::EnvelopeMalformed);
            }
            (version, _) => return Err(Error::UnsupportedEnvelopeVersion { version }),
        }
        if self.ciphertext.len() < AEAD_TAG_BYTES {
            return Err(Error::EnvelopeMalformed);
        }
        let total = self.encoded_header()?.len() + self.ciphertext.len();
        if total > ENVELOPE_MAX_BYTES {
            return Err(Error::EnvelopeTooLarge {
                actual_bytes: total,
                max_bytes: ENVELOPE_MAX_BYTES,
            });
        }
        Ok(())
    }
}

impl fmt::Debug for EncryptedEnvelope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EncryptedEnvelope")
            .field("version", &self.version)
            .field("cipher", &self.cipher)
            .field("key_source", &self.key_source)
            .field("reference", &self.reference)
            .field("context", &self.context)
            .field("nonce", &"<redacted>")
            .field("wrapped_key", &"<redacted>")
            .field("ciphertext", &"<redacted>")
            .finish()
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for EncryptedEnvelope {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let encoded = self.encode().map_err(serde::ser::Error::custom)?;
        serde::Serialize::serialize(&encoded, serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for EncryptedEnvelope {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let encoded = <Vec<u8> as serde::Deserialize>::deserialize(deserializer)?;
        Self::decode(encoded.as_slice()).map_err(serde::de::Error::custom)
    }
}

fn decode_context(decoder: &mut Decoder<'_>) -> Result<EnvelopeContext, Error> {
    let version = decoder.u16()?;
    if version != ENVELOPE_CONTEXT_VERSION {
        return Err(Error::UnsupportedContextVersion { version });
    }
    if decoder.take(ENVELOPE_CONTEXT_DOMAIN.len())? != ENVELOPE_CONTEXT_DOMAIN {
        return Err(Error::EnvelopeMalformed);
    }
    let purpose = EnvelopePurpose::parse(decoder.bounded_string(ENVELOPE_PURPOSE_MAX_BYTES)?)?;
    let subject_type = decoder.bounded_string(ENVELOPE_SUBJECT_TYPE_MAX_BYTES)?;
    let subject_value = decoder.bounded_string(ENVELOPE_SUBJECT_VALUE_MAX_BYTES)?;
    let subject = EnvelopeSubject::parse(subject_type, subject_value)?;
    let payload_schema = PayloadSchemaId::parse(decoder.bounded_string(PAYLOAD_SCHEMA_MAX_BYTES)?)?;
    Ok(EnvelopeContext::new(purpose, subject, payload_schema))
}

fn validate_data_key(data_key: &SecretMaterial) -> Result<(), Error> {
    if data_key.len() != DATA_KEY_BYTES {
        return Err(Error::InvalidDataKeyLength {
            actual_bytes: data_key.len(),
        });
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn encode_header(
    version: u16,
    cipher: Cipher,
    key_source: KeySource,
    reference: &SecretRef,
    context: Option<&EnvelopeContext>,
    nonce: Nonce,
    wrapped_key: &WrappedSecret,
    ciphertext_len: usize,
) -> Result<Vec<u8>, Error> {
    let id = reference.id().as_str().as_bytes();
    let id_len = u16::try_from(id.len()).map_err(|_| Error::EnvelopeMalformed)?;
    let wrapped_len =
        u32::try_from(wrapped_key.as_bytes().len()).map_err(|_| Error::EnvelopeMalformed)?;
    let ciphertext_len = u32::try_from(ciphertext_len).map_err(|_| Error::EnvelopeTooLarge {
        actual_bytes: ciphertext_len,
        max_bytes: ENVELOPE_MAX_BYTES,
    })?;
    let context_len = context.map_or(0, |value| value.to_canonical_bytes().len());
    let capacity = 4
        + 2
        + 1
        + 1
        + 1
        + 4
        + 2
        + id.len()
        + context_len
        + NONCE_BYTES
        + 4
        + wrapped_key.as_bytes().len()
        + 4;
    let mut encoded = Vec::with_capacity(capacity);
    encoded.extend_from_slice(&MAGIC);
    encoded.extend_from_slice(&version.to_be_bytes());
    encoded.push(cipher.code());
    encoded.push(key_source.code());
    encoded.push(reference.backend().code());
    encoded.extend_from_slice(&reference.key_version().get().to_be_bytes());
    encoded.extend_from_slice(&id_len.to_be_bytes());
    encoded.extend_from_slice(id);
    match (version, context) {
        (LEGACY_ENVELOPE_VERSION, None) => {}
        (ENVELOPE_VERSION, Some(context)) => {
            encoded.extend_from_slice(&context.to_canonical_bytes());
        }
        (LEGACY_ENVELOPE_VERSION | ENVELOPE_VERSION, _) => {
            return Err(Error::EnvelopeMalformed);
        }
        (version, _) => return Err(Error::UnsupportedEnvelopeVersion { version }),
    }
    encoded.extend_from_slice(nonce.as_bytes());
    encoded.extend_from_slice(&wrapped_len.to_be_bytes());
    encoded.extend_from_slice(wrapped_key.as_bytes());
    encoded.extend_from_slice(&ciphertext_len.to_be_bytes());
    Ok(encoded)
}

struct Decoder<'a> {
    remaining: &'a [u8],
}

impl<'a> Decoder<'a> {
    const fn new(encoded: &'a [u8]) -> Self {
        Self { remaining: encoded }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], Error> {
        if length > self.remaining.len() {
            return Err(Error::EnvelopeMalformed);
        }
        let (value, remaining) = self.remaining.split_at(length);
        self.remaining = remaining;
        Ok(value)
    }

    fn take_array<const N: usize>(&mut self) -> Result<[u8; N], Error> {
        self.take(N)?
            .try_into()
            .map_err(|_| Error::EnvelopeMalformed)
    }

    fn u8(&mut self) -> Result<u8, Error> {
        Ok(self.take_array::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, Error> {
        Ok(u16::from_be_bytes(self.take_array()?))
    }

    fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_be_bytes(self.take_array()?))
    }

    fn u32_usize(&mut self) -> Result<usize, Error> {
        usize::try_from(self.u32()?).map_err(|_| Error::EnvelopeMalformed)
    }

    fn bounded_string(&mut self, max: usize) -> Result<String, Error> {
        let length = usize::from(self.u16()?);
        if length > max {
            return Err(Error::EnvelopeMalformed);
        }
        let value =
            core::str::from_utf8(self.take(length)?).map_err(|_| Error::EnvelopeMalformed)?;
        Ok(String::from(value))
    }

    const fn is_empty(&self) -> bool {
        self.remaining.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn context() -> EnvelopeContext {
        EnvelopeContext::new(
            EnvelopePurpose::parse("radroots.private_artifact").expect("purpose"),
            EnvelopeSubject::parse("private_artifact", "01010101010101010101010101010101")
                .expect("subject"),
            PayloadSchemaId::parse("trade.private_terms.v1").expect("schema"),
        )
    }

    fn envelope() -> EncryptedEnvelope {
        EncryptedEnvelope {
            version: ENVELOPE_VERSION,
            cipher: Cipher::XChaCha20Poly1305,
            key_source: KeySource::ProviderWrapped,
            reference: SecretRef::new(
                SecretId::parse("coverage-key").expect("id"),
                BackendKind::Memory,
                KeyVersion::new(1).expect("version"),
            ),
            context: Some(context()),
            nonce: Nonce::new([7; NONCE_BYTES]),
            wrapped_key: WrappedSecret::from_bytes(vec![8; 32]).expect("wrapped"),
            ciphertext: vec![9; AEAD_TAG_BYTES],
        }
    }

    #[test]
    fn decode_and_validation_reject_every_bounded_wire_failure() {
        let encoded = envelope().encode().expect("encoded");
        assert_eq!(
            EncryptedEnvelope::decode(&encoded)
                .expect("decode")
                .version(),
            ENVELOPE_VERSION
        );
        assert!(matches!(
            EncryptedEnvelope::decode(&vec![0; ENVELOPE_MAX_BYTES + 1]),
            Err(Error::EnvelopeTooLarge { .. })
        ));
        assert_eq!(
            EncryptedEnvelope::decode(&[]).err(),
            Some(Error::EnvelopeMalformed)
        );

        let mut malformed = encoded.clone();
        malformed[0] = b'X';
        assert_eq!(
            EncryptedEnvelope::decode(&malformed).err(),
            Some(Error::EnvelopeMalformed)
        );
        for (offset, expected) in [
            (5, Error::UnsupportedEnvelopeVersion { version: 3 }),
            (6, Error::UnsupportedCipher { cipher: 9 }),
            (7, Error::UnsupportedKeySource { key_source: 9 }),
            (8, Error::UnsupportedBackend { backend: 9 }),
        ] {
            let mut unsupported = encoded.clone();
            unsupported[offset] = if offset == 5 { 3 } else { 9 };
            assert_eq!(
                EncryptedEnvelope::decode(&unsupported).err(),
                Some(expected)
            );
        }
        let context_version_offset = 4 + 2 + 1 + 1 + 1 + 4 + 2 + "coverage-key".len();
        let mut unsupported = encoded.clone();
        unsupported[context_version_offset + 1] = 2;
        assert_eq!(
            EncryptedEnvelope::decode(&unsupported).err(),
            Some(Error::UnsupportedContextVersion { version: 2 })
        );
        let mut trailing = encoded;
        trailing.push(0);
        assert_eq!(
            EncryptedEnvelope::decode(&trailing).err(),
            Some(Error::EnvelopeMalformed)
        );

        let mut invalid = envelope();
        invalid.version = 3;
        assert_eq!(
            invalid.encode().err(),
            Some(Error::UnsupportedEnvelopeVersion { version: 3 })
        );
        let mut invalid = envelope();
        invalid.context = None;
        assert_eq!(invalid.encode().err(), Some(Error::EnvelopeMalformed));
        let mut invalid = envelope();
        invalid.ciphertext.clear();
        assert_eq!(invalid.encode().err(), Some(Error::EnvelopeMalformed));
        let mut invalid = envelope();
        invalid.ciphertext = vec![0; ENVELOPE_MAX_BYTES];
        assert!(matches!(
            invalid.encode(),
            Err(Error::EnvelopeTooLarge { .. })
        ));
    }
}
