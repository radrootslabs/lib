//! Typed secret identifiers and references.

use crate::error::{Error, SecretIdError};
use alloc::string::{String, ToString};
use core::fmt;
use core::num::NonZeroU32;
use core::str::FromStr;

/// Maximum encoded length of a portable secret identifier.
pub const SECRET_ID_MAX_BYTES: usize = 128;

/// A validated, backend-independent secret identifier.
///
/// Identifier contents are available only through [`Self::as_str`]. Ordinary
/// display and debug formatting are intentionally redacted.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SecretId(String);

impl SecretId {
    /// Parses an identifier from the portable ASCII alphabet.
    ///
    /// The first character must be alphanumeric. Remaining characters may
    /// additionally use `.`, `_`, `-`, and `:` separators.
    pub fn parse(value: impl AsRef<str>) -> Result<Self, Error> {
        let value = value.as_ref();
        if value.is_empty() {
            return Err(Error::InvalidSecretId(SecretIdError::Empty));
        }
        if value.len() > SECRET_ID_MAX_BYTES {
            return Err(Error::InvalidSecretId(SecretIdError::TooLong {
                actual_bytes: value.len(),
                max_bytes: SECRET_ID_MAX_BYTES,
            }));
        }
        for (byte_offset, character) in value.char_indices() {
            let valid = character.is_ascii_alphanumeric()
                || (byte_offset > 0 && matches!(character, '.' | '_' | '-' | ':'));
            if !valid {
                return Err(Error::InvalidSecretId(SecretIdError::InvalidCharacter {
                    byte_offset,
                }));
            }
        }
        Ok(Self(value.to_string()))
    }

    /// Returns the validated identifier for explicit backend use.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for SecretId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretId(<redacted>)")
    }
}

impl fmt::Display for SecretId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("<redacted secret id>")
    }
}

impl FromStr for SecretId {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for SecretId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for SecretId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = <String as serde::Deserialize>::deserialize(deserializer)?;
        Self::parse(value).map_err(serde::de::Error::custom)
    }
}

/// A provider-owned key revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KeyVersion(NonZeroU32);

impl KeyVersion {
    /// Creates a non-zero key version.
    pub const fn new(value: u32) -> Result<Self, Error> {
        match NonZeroU32::new(value) {
            Some(value) => Ok(Self(value)),
            None => Err(Error::InvalidKeyVersion),
        }
    }

    /// Returns the numeric version.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0.get()
    }
}

/// The explicit adapter family that owns a secret reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum BackendKind {
    /// Deterministic in-process storage selected by the host.
    Memory,
    /// Explicit file-backed storage selected by the host.
    File,
    /// Operating-system keyring storage selected by the host.
    Keyring,
    /// A host-provided implementation outside the built-in adapters.
    External,
}

/// A single-owner capability handle for a secret held by a provider.
///
/// Cloning and ordinary serialization are intentionally unavailable. Debug
/// output never reveals the identifier.
///
/// ```compile_fail
/// use radroots_secrets::{SecretId, SecretRef};
/// use radroots_secrets::id::{BackendKind, KeyVersion};
///
/// let reference = SecretRef::new(
///     SecretId::parse("account-signing-key")?,
///     BackendKind::Memory,
///     KeyVersion::new(1)?,
/// );
/// let _duplicate = reference.clone();
/// # Ok::<(), radroots_secrets::Error>(())
/// ```
///
/// ```compile_fail
/// use radroots_secrets::{SecretId, SecretRef};
/// use radroots_secrets::id::{BackendKind, KeyVersion};
///
/// let reference = SecretRef::new(
///     SecretId::parse("account-signing-key")?,
///     BackendKind::Memory,
///     KeyVersion::new(1)?,
/// );
/// let _json = serde_json::to_string(&reference)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct SecretRef {
    id: SecretId,
    backend: BackendKind,
    key_version: KeyVersion,
}

impl SecretRef {
    /// Creates a capability reference from validated metadata.
    #[must_use]
    pub const fn new(id: SecretId, backend: BackendKind, key_version: KeyVersion) -> Self {
        Self {
            id,
            backend,
            key_version,
        }
    }

    /// Returns the validated provider-local identifier.
    #[must_use]
    pub const fn id(&self) -> &SecretId {
        &self.id
    }

    /// Returns the adapter family that owns the secret.
    #[must_use]
    pub const fn backend(&self) -> BackendKind {
        self.backend
    }

    /// Returns the expected provider key version.
    #[must_use]
    pub const fn key_version(&self) -> KeyVersion {
        self.key_version
    }
}

impl fmt::Debug for SecretRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SecretRef")
            .field("id", &"<redacted>")
            .field("backend", &self.backend)
            .field("key_version", &self.key_version)
            .finish()
    }
}
