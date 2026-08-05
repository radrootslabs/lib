//! Protected private-artifact metadata contracts.
//!
//! These contracts intentionally contain neither plaintext nor ciphertext.
//! Encryption, key access, and backend schema remain implementation details.

use core::fmt;
use radroots_transport::BoxFuture;
use sha2::{Digest, Sha256};

use crate::Error;

pub const ARTIFACT_KIND_MAX_BYTES: usize = 96;
pub const ARTIFACT_SCHEMA_MAX_BYTES: usize = 128;
pub const SECRET_PROVIDER_MAX_BYTES: usize = 64;
pub const SECRET_REFERENCE_MAX_BYTES: usize = 512;
pub const EXPIRED_ARTIFACT_QUERY_LIMIT_MAX: u16 = 256;
pub const PRIVATE_ARTIFACT_ENVELOPE_PURPOSE_PREFIX: &str = "radroots.private_artifact.";
pub const PRIVATE_ARTIFACT_ENVELOPE_SUBJECT_TYPE: &str = "private_artifact";
const ENVELOPE_CONTEXT_DOMAIN: &[u8] = b"radroots.envelope_context.v1";
const ENVELOPE_CONTEXT_VERSION: u16 = 1;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
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

impl fmt::Debug for PrivateArtifactId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PrivateArtifactId(<redacted>)")
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ArtifactKind(String);

impl ArtifactKind {
    pub fn parse(value: impl Into<String>) -> Result<Self, Error> {
        let value = value.into();
        if !valid_namespaced(value.as_str(), ARTIFACT_KIND_MAX_BYTES, 2) {
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
        if !valid_schema(value.as_str()) {
            return Err(Error::InvalidPrivateArtifactSchema);
        }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Opaque envelope context derived only from immutable artifact metadata.
#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PrivateArtifactEnvelopeContext {
    purpose: String,
    subject_type: &'static str,
    subject: String,
    payload_schema: String,
}

impl PrivateArtifactEnvelopeContext {
    fn derive(
        artifact_id: PrivateArtifactId,
        kind: &ArtifactKind,
        schema_id: &ArtifactSchemaId,
    ) -> Self {
        Self {
            purpose: format!(
                "{PRIVATE_ARTIFACT_ENVELOPE_PURPOSE_PREFIX}{}",
                kind.as_str()
            ),
            subject_type: PRIVATE_ARTIFACT_ENVELOPE_SUBJECT_TYPE,
            subject: hex_artifact_id(artifact_id),
            payload_schema: schema_id.as_str().to_owned(),
        }
    }

    pub fn purpose(&self) -> &str {
        self.purpose.as_str()
    }
    pub const fn subject_type(&self) -> &'static str {
        self.subject_type
    }
    pub fn subject(&self) -> &str {
        self.subject.as_str()
    }
    pub fn payload_schema(&self) -> &str {
        self.payload_schema.as_str()
    }
    pub fn fingerprint(&self) -> [u8; 32] {
        Sha256::digest(self.canonical_bytes()).into()
    }

    fn canonical_bytes(&self) -> Vec<u8> {
        let mut encoded = Vec::new();
        encoded.extend_from_slice(&ENVELOPE_CONTEXT_VERSION.to_be_bytes());
        encoded.extend_from_slice(ENVELOPE_CONTEXT_DOMAIN);
        for value in [
            self.purpose.as_bytes(),
            self.subject_type.as_bytes(),
            self.subject.as_bytes(),
            self.payload_schema.as_bytes(),
        ] {
            let length = u16::try_from(value.len())
                .expect("validated private-artifact envelope context fits u16");
            encoded.extend_from_slice(&length.to_be_bytes());
            encoded.extend_from_slice(value);
        }
        encoded
    }
}

impl fmt::Debug for PrivateArtifactEnvelopeContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PrivateArtifactEnvelopeContext")
            .field("purpose", &"<derived>")
            .field("subject_type", &self.subject_type)
            .field("subject", &"<redacted>")
            .field("payload_schema", &"<derived>")
            .finish()
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

/// Host-generated idempotency identity for one reseal commit.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PrivateArtifactResealId([u8; 16]);

impl PrivateArtifactResealId {
    pub const fn new(bytes: [u8; 16]) -> Result<Self, Error> {
        if bytes_are_zero(&bytes) {
            return Err(Error::InvalidPrivateArtifactResealId);
        }
        Ok(Self(bytes))
    }
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

impl fmt::Debug for PrivateArtifactResealId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PrivateArtifactResealId(<redacted>)")
    }
}

/// Backend-neutral metadata fence for one atomic envelope reseal.
#[derive(Clone, Eq, PartialEq)]
pub struct PrivateArtifactResealRequest {
    reseal_id: PrivateArtifactResealId,
    artifact_id: PrivateArtifactId,
    expected_revision: PrivateArtifactRevision,
    expected_commitment: ArtifactCommitment,
    next_commitment: ArtifactCommitment,
    next_protected_size_bytes: u64,
    next_secret_reference: DurableSecretReference,
    committed_at_unix_ms: u64,
}

impl fmt::Debug for PrivateArtifactResealRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PrivateArtifactResealRequest")
            .field("reseal_id", &self.reseal_id)
            .field("artifact_id", &self.artifact_id)
            .field("expected_revision", &self.expected_revision)
            .field("expected_commitment", &"<commitment>")
            .field("next_commitment", &"<commitment>")
            .field("next_protected_size_bytes", &self.next_protected_size_bytes)
            .field("next_secret_reference", &self.next_secret_reference)
            .field("committed_at_unix_ms", &self.committed_at_unix_ms)
            .finish()
    }
}

impl PrivateArtifactResealRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        reseal_id: PrivateArtifactResealId,
        artifact_id: PrivateArtifactId,
        expected_revision: PrivateArtifactRevision,
        expected_commitment: ArtifactCommitment,
        next_commitment: ArtifactCommitment,
        next_protected_size_bytes: u64,
        next_secret_reference: DurableSecretReference,
        committed_at_unix_ms: u64,
    ) -> Result<Self, Error> {
        if expected_commitment == next_commitment
            || next_protected_size_bytes == 0
            || committed_at_unix_ms == 0
        {
            return Err(Error::InvalidPrivateArtifactResealRequest);
        }
        Ok(Self {
            reseal_id,
            artifact_id,
            expected_revision,
            expected_commitment,
            next_commitment,
            next_protected_size_bytes,
            next_secret_reference,
            committed_at_unix_ms,
        })
    }

    pub const fn reseal_id(&self) -> PrivateArtifactResealId {
        self.reseal_id
    }
    pub const fn artifact_id(&self) -> PrivateArtifactId {
        self.artifact_id
    }
    pub const fn expected_revision(&self) -> PrivateArtifactRevision {
        self.expected_revision
    }
    pub const fn expected_commitment(&self) -> ArtifactCommitment {
        self.expected_commitment
    }
    pub const fn next_commitment(&self) -> ArtifactCommitment {
        self.next_commitment
    }
    pub const fn next_protected_size_bytes(&self) -> u64 {
        self.next_protected_size_bytes
    }
    pub const fn next_secret_reference(&self) -> &DurableSecretReference {
        &self.next_secret_reference
    }
    pub const fn committed_at_unix_ms(&self) -> u64 {
        self.committed_at_unix_ms
    }
    pub fn fingerprint(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(self.reseal_id.as_bytes());
        hasher.update(self.artifact_id.as_bytes());
        hasher.update(self.expected_revision.get().to_be_bytes());
        hasher.update(self.expected_commitment.as_bytes());
        hasher.update(self.next_commitment.as_bytes());
        hasher.update(self.next_protected_size_bytes.to_be_bytes());
        hash_string(&mut hasher, self.next_secret_reference.provider());
        hash_string(&mut hasher, self.next_secret_reference.opaque_reference());
        hasher.update(self.next_secret_reference.key_version().to_be_bytes());
        hasher.update(self.committed_at_unix_ms.to_be_bytes());
        hasher.finalize().into()
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrivateArtifactResealDisposition {
    Committed,
    Replayed,
}

/// Durable receipt used to distinguish exact replay from conflicting reuse.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct PrivateArtifactResealReceipt {
    reseal_id: PrivateArtifactResealId,
    artifact_id: PrivateArtifactId,
    committed_revision: PrivateArtifactRevision,
    request_fingerprint: [u8; 32],
    disposition: PrivateArtifactResealDisposition,
}

impl PrivateArtifactResealReceipt {
    pub fn committed(
        request: &PrivateArtifactResealRequest,
        committed_revision: PrivateArtifactRevision,
    ) -> Self {
        Self {
            reseal_id: request.reseal_id,
            artifact_id: request.artifact_id,
            committed_revision,
            request_fingerprint: request.fingerprint(),
            disposition: PrivateArtifactResealDisposition::Committed,
        }
    }

    pub fn replay(&self, request: &PrivateArtifactResealRequest) -> Result<Self, Error> {
        if self.reseal_id != request.reseal_id
            || self.artifact_id != request.artifact_id
            || self.request_fingerprint != request.fingerprint()
        {
            return Err(Error::PrivateArtifactResealConflict);
        }
        Ok(Self {
            disposition: PrivateArtifactResealDisposition::Replayed,
            ..*self
        })
    }

    pub const fn reseal_id(self) -> PrivateArtifactResealId {
        self.reseal_id
    }
    pub const fn artifact_id(self) -> PrivateArtifactId {
        self.artifact_id
    }
    pub const fn committed_revision(self) -> PrivateArtifactRevision {
        self.committed_revision
    }
    pub const fn disposition(self) -> PrivateArtifactResealDisposition {
        self.disposition
    }

    pub const fn request_fingerprint(self) -> [u8; 32] {
        self.request_fingerprint
    }
}

impl fmt::Debug for PrivateArtifactResealReceipt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PrivateArtifactResealReceipt")
            .field("reseal_id", &self.reseal_id)
            .field("artifact_id", &self.artifact_id)
            .field("committed_revision", &self.committed_revision)
            .field("request_fingerprint", &"<commitment>")
            .field("disposition", &self.disposition)
            .finish()
    }
}

/// Bounded migration inventory without artifact or user identity.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PrivateArtifactEnvelopeMigrationStatus {
    pub v1_pending: u64,
    pub v2_current: u64,
    pub corrupt: u64,
    pub blocked_provider: u64,
    pub conflicted: u64,
}

impl PrivateArtifactEnvelopeMigrationStatus {
    pub fn total(self) -> Option<u64> {
        self.v1_pending
            .checked_add(self.v2_current)?
            .checked_add(self.corrupt)?
            .checked_add(self.blocked_provider)?
            .checked_add(self.conflicted)
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

    /// Reconstructs and validates metadata at a durable backend boundary.
    #[allow(clippy::too_many_arguments)]
    pub fn from_durable_parts(
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
        tombstone: Option<(u64, DeletionReason, ArtifactCommitment)>,
    ) -> Result<Self, Error> {
        let initial = Self::new(
            artifact_id,
            kind,
            schema_id,
            commitment,
            protected_size_bytes,
            secret_reference,
            retention,
            created_at_unix_ms,
        )?;
        if updated_at_unix_ms < created_at_unix_ms {
            return Err(Error::CorruptPrivateArtifactMetadata);
        }
        let tombstone =
            tombstone.map(
                |(deleted_at_unix_ms, reason, tombstone_commitment)| ArtifactTombstone {
                    deleted_at_unix_ms,
                    reason,
                    commitment: tombstone_commitment,
                },
            );
        let valid = match (stage, revision.get(), tombstone) {
            (PrivateArtifactStage::Active, revision, None) => {
                (revision == 1 && updated_at_unix_ms == created_at_unix_ms)
                    || (revision > 1 && updated_at_unix_ms > created_at_unix_ms)
            }
            (PrivateArtifactStage::Expired, revision, None) => {
                revision >= 2 && retention.is_expired_at(updated_at_unix_ms)
            }
            (PrivateArtifactStage::Tombstoned, revision, Some(tombstone)) => {
                revision >= 2
                    && tombstone.deleted_at_unix_ms == updated_at_unix_ms
                    && tombstone.commitment == commitment
                    && retention.permits_deletion_at(updated_at_unix_ms)
                    && (tombstone.reason != DeletionReason::RetentionExpired
                        || retention.is_expired_at(updated_at_unix_ms))
            }
            _ => false,
        };
        if !valid {
            return Err(Error::CorruptPrivateArtifactMetadata);
        }
        Ok(Self {
            revision,
            stage,
            updated_at_unix_ms,
            tombstone,
            ..initial
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

    /// Derives the only valid envelope context for this artifact.
    pub fn envelope_context(&self) -> PrivateArtifactEnvelopeContext {
        PrivateArtifactEnvelopeContext::derive(self.artifact_id, &self.kind, &self.schema_id)
    }

    /// Applies the metadata half of a fenced envelope reseal.
    pub fn resealed(&self, request: &PrivateArtifactResealRequest) -> Result<Self, Error> {
        if self.stage != PrivateArtifactStage::Active
            || request.artifact_id != self.artifact_id
            || request.expected_revision != self.revision
            || request.expected_commitment != self.commitment
        {
            return Err(Error::PrivateArtifactResealConflict);
        }
        if request.committed_at_unix_ms <= self.updated_at_unix_ms {
            return Err(Error::InvalidPrivateArtifactTimestamp);
        }
        let mut next = self.clone();
        next.commitment = request.next_commitment;
        next.protected_size_bytes = request.next_protected_size_bytes;
        next.secret_reference = request.next_secret_reference.clone();
        next.revision = self.revision.next()?;
        next.updated_at_unix_ms = request.committed_at_unix_ms;
        Ok(next)
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
    fn reseal_metadata(
        &self,
        request: PrivateArtifactResealRequest,
    ) -> BoxFuture<'_, Result<PrivateArtifactResealReceipt, Error>>;
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

fn valid_schema(value: &str) -> bool {
    if !valid_namespaced(value, ARTIFACT_SCHEMA_MAX_BYTES, 3) {
        return false;
    }
    value.rsplit('.').next().is_some_and(|last| {
        last.strip_prefix('v').is_some_and(|version| {
            !version.is_empty() && version.bytes().all(|byte| byte.is_ascii_digit())
        })
    })
}

fn valid_namespaced(value: &str, max: usize, minimum_segments: usize) -> bool {
    valid_label(value, max)
        && value.split('.').count() >= minimum_segments
        && value.split('.').all(|segment| {
            let mut bytes = segment.bytes();
            bytes.next().is_some_and(|byte| byte.is_ascii_lowercase())
                && bytes.all(|byte| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || matches!(byte, b'_' | b'-')
                })
        })
}

fn hex_artifact_id(artifact_id: PrivateArtifactId) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(32);
    for byte in artifact_id.as_bytes() {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn hash_string(hasher: &mut Sha256, value: &str) {
    let length = u32::try_from(value.len()).expect("validated private-artifact field fits u32");
    hasher.update(length.to_be_bytes());
    hasher.update(value.as_bytes());
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
