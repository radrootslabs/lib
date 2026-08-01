//! Protected private-artifact metadata contracts.
//!
//! These contracts intentionally contain neither plaintext nor ciphertext.
//! Encryption, key access, and backend schema remain implementation details.

use core::fmt;
use radroots_transport::BoxFuture;

use crate::Error;

pub const ARTIFACT_KIND_MAX_BYTES: usize = 128;
pub const ARTIFACT_SCHEMA_MAX_BYTES: usize = 128;
pub const SECRET_PROVIDER_MAX_BYTES: usize = 64;
pub const SECRET_REFERENCE_MAX_BYTES: usize = 512;
pub const EXPIRED_ARTIFACT_QUERY_LIMIT_MAX: u16 = 256;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PrivateArtifactId([u8; 16]);

impl PrivateArtifactId {
    pub const fn new(bytes: [u8; 16]) -> Result<Self, Error> {
        if bytes_are_zero(&bytes) {
            return Err(Error::InvalidPrivateArtifactId);
        }
        Ok(Self(bytes))
    }
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ArtifactKind(String);

impl ArtifactKind {
    pub fn parse(value: impl Into<String>) -> Result<Self, Error> {
        let value = value.into();
        if !valid_label(value.as_str(), ARTIFACT_KIND_MAX_BYTES) {
            return Err(Error::InvalidPrivateArtifactKind);
        }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ArtifactSchemaId(String);

impl ArtifactSchemaId {
    pub fn parse(value: impl Into<String>) -> Result<Self, Error> {
        let value = value.into();
        if !valid_label(value.as_str(), ARTIFACT_SCHEMA_MAX_BYTES) {
            return Err(Error::InvalidPrivateArtifactSchema);
        }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// SHA-256 commitment to the exact protected representation.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ArtifactCommitment([u8; 32]);

impl ArtifactCommitment {
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Persistable provider reference metadata, never a secret capability itself.
#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DurableSecretReference {
    provider: String,
    opaque_reference: String,
    key_version: u32,
}

impl DurableSecretReference {
    pub fn new(
        provider: impl Into<String>,
        opaque_reference: impl Into<String>,
        key_version: u32,
    ) -> Result<Self, Error> {
        let provider = provider.into();
        let opaque_reference = opaque_reference.into();
        if !valid_label(provider.as_str(), SECRET_PROVIDER_MAX_BYTES)
            || opaque_reference.is_empty()
            || opaque_reference.len() > SECRET_REFERENCE_MAX_BYTES
            || opaque_reference != opaque_reference.trim()
            || opaque_reference.chars().any(char::is_control)
            || key_version == 0
        {
            return Err(Error::InvalidPrivateArtifactSecretReference);
        }
        Ok(Self {
            provider,
            opaque_reference,
            key_version,
        })
    }
    pub fn provider(&self) -> &str {
        self.provider.as_str()
    }
    pub fn opaque_reference(&self) -> &str {
        self.opaque_reference.as_str()
    }
    pub const fn key_version(&self) -> u32 {
        self.key_version
    }
}

impl fmt::Debug for DurableSecretReference {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DurableSecretReference")
            .field("provider", &self.provider)
            .field("opaque_reference", &"[REDACTED]")
            .field("key_version", &self.key_version)
            .finish()
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for DurableSecretReference {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("DurableSecretReference", 3)?;
        state.serialize_field("provider", &self.provider)?;
        state.serialize_field("opaque_reference", &self.opaque_reference)?;
        state.serialize_field("key_version", &self.key_version)?;
        state.end()
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for DurableSecretReference {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            provider: String,
            opaque_reference: String,
            key_version: u32,
        }
        let wire = Wire::deserialize(deserializer)?;
        Self::new(wire.provider, wire.opaque_reference, wire.key_version)
            .map_err(serde::de::Error::custom)
    }
}

/// Minimum retention and optional automatic expiry policy.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetentionPolicy {
    delete_not_before_unix_ms: Option<u64>,
    expires_at_unix_ms: Option<u64>,
}

impl RetentionPolicy {
    pub const fn indefinite() -> Self {
        Self {
            delete_not_before_unix_ms: None,
            expires_at_unix_ms: None,
        }
    }
    pub const fn new(
        delete_not_before_unix_ms: Option<u64>,
        expires_at_unix_ms: Option<u64>,
    ) -> Result<Self, Error> {
        if matches!(delete_not_before_unix_ms, Some(0)) || matches!(expires_at_unix_ms, Some(0)) {
            return Err(Error::InvalidPrivateArtifactRetention);
        }
        Ok(Self {
            delete_not_before_unix_ms,
            expires_at_unix_ms,
        })
    }
    pub const fn delete_not_before_unix_ms(self) -> Option<u64> {
        self.delete_not_before_unix_ms
    }
    pub const fn expires_at_unix_ms(self) -> Option<u64> {
        self.expires_at_unix_ms
    }
    pub const fn is_expired_at(self, unix_ms: u64) -> bool {
        matches!(self.expires_at_unix_ms, Some(expires) if unix_ms >= expires)
    }
    pub const fn permits_deletion_at(self, unix_ms: u64) -> bool {
        !matches!(self.delete_not_before_unix_ms, Some(not_before) if unix_ms < not_before)
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PrivateArtifactRevision(u64);

impl PrivateArtifactRevision {
    pub const INITIAL: Self = Self(1);
    pub const fn new(value: u64) -> Result<Self, Error> {
        if value == 0 {
            Err(Error::InvalidPrivateArtifactRevision)
        } else {
            Ok(Self(value))
        }
    }
    pub const fn get(self) -> u64 {
        self.0
    }
    fn next(self) -> Result<Self, Error> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or(Error::CorruptPrivateArtifactMetadata)
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrivateArtifactStage {
    Active,
    Expired,
    Tombstoned,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeletionReason {
    UserRequested,
    RetentionExpired,
    KeyRevoked,
    IntegrityFailure,
    OperatorRequested,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactTombstone {
    deleted_at_unix_ms: u64,
    reason: DeletionReason,
    commitment: ArtifactCommitment,
}

impl ArtifactTombstone {
    pub const fn deleted_at_unix_ms(self) -> u64 {
        self.deleted_at_unix_ms
    }
    pub const fn reason(self) -> DeletionReason {
        self.reason
    }
    pub const fn commitment(self) -> ArtifactCommitment {
        self.commitment
    }
}

/// Metadata for one protected artifact; no protected bytes are present.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrivateArtifactMetadata {
    artifact_id: PrivateArtifactId,
    kind: ArtifactKind,
    schema_id: ArtifactSchemaId,
    commitment: ArtifactCommitment,
    protected_size_bytes: u64,
    secret_reference: DurableSecretReference,
    retention: RetentionPolicy,
    revision: PrivateArtifactRevision,
    stage: PrivateArtifactStage,
    created_at_unix_ms: u64,
    updated_at_unix_ms: u64,
    tombstone: Option<ArtifactTombstone>,
}

impl PrivateArtifactMetadata {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        artifact_id: PrivateArtifactId,
        kind: ArtifactKind,
        schema_id: ArtifactSchemaId,
        commitment: ArtifactCommitment,
        protected_size_bytes: u64,
        secret_reference: DurableSecretReference,
        retention: RetentionPolicy,
        created_at_unix_ms: u64,
    ) -> Result<Self, Error> {
        if protected_size_bytes == 0
            || created_at_unix_ms == 0
            || matches!(retention.delete_not_before_unix_ms(), Some(value) if value < created_at_unix_ms)
            || matches!(retention.expires_at_unix_ms(), Some(value) if value < created_at_unix_ms)
        {
            return Err(Error::InvalidPrivateArtifactMetadata);
        }
        Ok(Self {
            artifact_id,
            kind,
            schema_id,
            commitment,
            protected_size_bytes,
            secret_reference,
            retention,
            revision: PrivateArtifactRevision::INITIAL,
            stage: PrivateArtifactStage::Active,
            created_at_unix_ms,
            updated_at_unix_ms: created_at_unix_ms,
            tombstone: None,
        })
    }
    pub const fn artifact_id(&self) -> PrivateArtifactId {
        self.artifact_id
    }
    pub const fn kind(&self) -> &ArtifactKind {
        &self.kind
    }
    pub const fn schema_id(&self) -> &ArtifactSchemaId {
        &self.schema_id
    }
    pub const fn commitment(&self) -> ArtifactCommitment {
        self.commitment
    }
    pub const fn protected_size_bytes(&self) -> u64 {
        self.protected_size_bytes
    }
    pub const fn secret_reference(&self) -> &DurableSecretReference {
        &self.secret_reference
    }
    pub const fn retention(&self) -> RetentionPolicy {
        self.retention
    }
    pub const fn revision(&self) -> PrivateArtifactRevision {
        self.revision
    }
    pub const fn stage(&self) -> PrivateArtifactStage {
        self.stage
    }
    pub const fn created_at_unix_ms(&self) -> u64 {
        self.created_at_unix_ms
    }
    pub const fn updated_at_unix_ms(&self) -> u64 {
        self.updated_at_unix_ms
    }
    pub const fn tombstone_record(&self) -> Option<ArtifactTombstone> {
        self.tombstone
    }

    pub fn mark_expired(
        &self,
        expected_revision: PrivateArtifactRevision,
        at_unix_ms: u64,
    ) -> Result<Self, Error> {
        self.validate_transition(expected_revision, at_unix_ms)?;
        if self.stage != PrivateArtifactStage::Active || !self.retention.is_expired_at(at_unix_ms) {
            return Err(Error::PrivateArtifactNotExpired);
        }
        let mut next = self.clone();
        next.revision = self.revision.next()?;
        next.stage = PrivateArtifactStage::Expired;
        next.updated_at_unix_ms = at_unix_ms;
        Ok(next)
    }

    pub fn tombstone(
        &self,
        expected_revision: PrivateArtifactRevision,
        at_unix_ms: u64,
        reason: DeletionReason,
    ) -> Result<Self, Error> {
        self.validate_transition(expected_revision, at_unix_ms)?;
        if self.stage == PrivateArtifactStage::Tombstoned {
            return Err(Error::PrivateArtifactTombstoned);
        }
        if !self.retention.permits_deletion_at(at_unix_ms) {
            return Err(Error::PrivateArtifactRetentionActive);
        }
        if reason == DeletionReason::RetentionExpired && !self.retention.is_expired_at(at_unix_ms) {
            return Err(Error::PrivateArtifactNotExpired);
        }
        let mut next = self.clone();
        next.revision = self.revision.next()?;
        next.stage = PrivateArtifactStage::Tombstoned;
        next.updated_at_unix_ms = at_unix_ms;
        next.tombstone = Some(ArtifactTombstone {
            deleted_at_unix_ms: at_unix_ms,
            reason,
            commitment: self.commitment,
        });
        Ok(next)
    }

    fn validate_transition(
        &self,
        expected_revision: PrivateArtifactRevision,
        at_unix_ms: u64,
    ) -> Result<(), Error> {
        if expected_revision != self.revision {
            return Err(Error::PrivateArtifactRevisionConflict);
        }
        if at_unix_ms < self.updated_at_unix_ms {
            return Err(Error::InvalidPrivateArtifactTimestamp);
        }
        Ok(())
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrivateArtifactStatus {
    pub active: u64,
    pub expired: u64,
    pub tombstoned: u64,
}

impl PrivateArtifactStatus {
    pub fn total(self) -> Option<u64> {
        self.active
            .checked_add(self.expired)?
            .checked_add(self.tombstoned)
    }
}

/// Backend-neutral metadata-only private-artifact SPI.
pub trait PrivateArtifactStore: Send + Sync {
    fn put_metadata(
        &self,
        metadata: PrivateArtifactMetadata,
    ) -> BoxFuture<'_, Result<PrivateArtifactMetadata, Error>>;
    fn metadata(
        &self,
        artifact_id: PrivateArtifactId,
    ) -> BoxFuture<'_, Result<Option<PrivateArtifactMetadata>, Error>>;
    fn mark_expired(
        &self,
        artifact_id: PrivateArtifactId,
        expected_revision: PrivateArtifactRevision,
        at_unix_ms: u64,
    ) -> BoxFuture<'_, Result<PrivateArtifactMetadata, Error>>;
    fn tombstone(
        &self,
        artifact_id: PrivateArtifactId,
        expected_revision: PrivateArtifactRevision,
        at_unix_ms: u64,
        reason: DeletionReason,
    ) -> BoxFuture<'_, Result<PrivateArtifactMetadata, Error>>;
    fn expired(
        &self,
        at_unix_ms: u64,
        limit: u16,
    ) -> BoxFuture<'_, Result<Vec<PrivateArtifactMetadata>, Error>>;
    fn status(&self) -> BoxFuture<'_, Result<PrivateArtifactStatus, Error>>;
}

fn valid_label(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value == value.trim()
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-' | b'.')
        })
}

const fn bytes_are_zero(bytes: &[u8; 16]) -> bool {
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != 0 {
            return false;
        }
        index += 1;
    }
    true
}
