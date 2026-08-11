//! Canonical v1 service backup manifest model.

use core::{fmt, num::NonZeroU32};
use std::error::Error;

use radroots_runtime_paths::{InstanceId, ServiceId};
use radroots_storage::event::SourceGeneration;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::ServiceDatabaseMetadata;

/// Exact v1 backup manifest schema identifier.
pub const BACKUP_MANIFEST_SCHEMA: &str = "radroots.service-backup";
/// Exact v1 backup manifest schema version.
pub const BACKUP_MANIFEST_SCHEMA_VERSION: u32 = 1;
/// Maximum accepted or emitted canonical manifest size.
pub const BACKUP_MANIFEST_CANONICAL_MAX_BYTES: usize = 1_024;
/// Sole member name admitted by the v1 manifest.
pub const BACKUP_STATE_MEMBER_NAME: &str = "state.sqlite";

const INTEGRITY_OK: &str = "ok";

/// Injected positive backup creation time in Unix milliseconds.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct BackupCreatedAtUnixMs(u64);

impl BackupCreatedAtUnixMs {
    /// Validates a timestamp representable by SQLite's signed integer range.
    pub const fn new(value: u64) -> Result<Self, BackupManifestContractError> {
        if value == 0 || value > i64::MAX as u64 {
            return Err(BackupManifestContractError::InvalidCreationTime);
        }
        Ok(Self(value))
    }

    /// Returns milliseconds since the Unix epoch.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Exact SHA-256 of one backup member.
#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub struct BackupMemberSha256([u8; 32]);

impl BackupMemberSha256 {
    /// Constructs a digest from independently computed bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns the exact digest bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for BackupMemberSha256 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("BackupMemberSha256([redacted])")
    }
}

/// SHA-256 of the exact canonical manifest bytes.
#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub struct BackupManifestSha256([u8; 32]);

impl BackupManifestSha256 {
    /// Constructs a digest pinned by independently protected provenance.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns the exact digest bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for BackupManifestSha256 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("BackupManifestSha256([redacted])")
    }
}

/// Exact successful integrity projection frozen by the v1 manifest.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct BackupManifestIntegrity {
    _private: (),
}

impl BackupManifestIntegrity {
    /// Returns the exact SQLite integrity result.
    #[must_use]
    pub const fn sqlite(self) -> &'static str {
        INTEGRITY_OK
    }

    /// Returns the exact foreign-key integrity result.
    #[must_use]
    pub const fn foreign_keys(self) -> &'static str {
        INTEGRITY_OK
    }
}

impl fmt::Debug for BackupManifestIntegrity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("BackupManifestIntegrity")
    }
}

/// Sole state database member carried by a v1 manifest.
#[derive(Clone, PartialEq, Eq)]
pub struct ServiceBackupMember {
    byte_length: u64,
    sha256: BackupMemberSha256,
}

impl ServiceBackupMember {
    /// Returns the fixed v1 member name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        BACKUP_STATE_MEMBER_NAME
    }

    /// Returns the exact nonzero captured byte length.
    #[must_use]
    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }

    /// Returns the exact captured member digest.
    #[must_use]
    pub const fn sha256(&self) -> BackupMemberSha256 {
        self.sha256
    }
}

impl fmt::Debug for ServiceBackupMember {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ServiceBackupMember")
            .field("name", &BACKUP_STATE_MEMBER_NAME)
            .field("byte_length", &self.byte_length)
            .field("sha256", &"[redacted]")
            .finish()
    }
}

/// Structurally valid canonical v1 backup manifest.
#[derive(Clone, PartialEq, Eq)]
pub struct ServiceBackupManifest {
    service: ServiceId,
    instance: InstanceId,
    source_generation: SourceGeneration,
    state_schema_version: NonZeroU32,
    created_at_unix_ms: BackupCreatedAtUnixMs,
    member: ServiceBackupMember,
    canonical_bytes: Box<[u8]>,
    digest: BackupManifestSha256,
}

impl ServiceBackupManifest {
    /// Parses only exact compact canonical v1 bytes.
    ///
    /// Structural parsing does not verify a member file. Step 065's verifier
    /// owns expected-intent, length, digest, SQLite identity, and integrity
    /// qualification.
    pub fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, BackupManifestContractError> {
        if bytes.is_empty() {
            return Err(BackupManifestContractError::MalformedEncoding);
        }
        if bytes.len() > BACKUP_MANIFEST_CANONICAL_MAX_BYTES {
            return Err(BackupManifestContractError::ManifestTooLarge);
        }
        let wire: WireManifest = serde_json::from_slice(bytes)
            .map_err(|_| BackupManifestContractError::MalformedEncoding)?;
        let manifest = Self::from_wire(wire)?;
        if manifest.canonical_bytes.as_ref() != bytes {
            return Err(BackupManifestContractError::NonCanonicalEncoding);
        }
        Ok(manifest)
    }

    /// Returns the exact compact canonical UTF-8 JSON bytes.
    #[must_use]
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Returns SHA-256 over the exact canonical bytes.
    #[must_use]
    pub const fn digest(&self) -> BackupManifestSha256 {
        self.digest
    }

    #[must_use]
    pub const fn schema(&self) -> &'static str {
        BACKUP_MANIFEST_SCHEMA
    }

    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        BACKUP_MANIFEST_SCHEMA_VERSION
    }

    #[must_use]
    pub fn service(&self) -> &ServiceId {
        &self.service
    }

    #[must_use]
    pub fn instance(&self) -> &InstanceId {
        &self.instance
    }

    #[must_use]
    pub const fn source_generation(&self) -> SourceGeneration {
        self.source_generation
    }

    #[must_use]
    pub const fn state_schema_version(&self) -> NonZeroU32 {
        self.state_schema_version
    }

    #[must_use]
    pub const fn created_at_unix_ms(&self) -> BackupCreatedAtUnixMs {
        self.created_at_unix_ms
    }

    #[must_use]
    pub fn members(&self) -> &[ServiceBackupMember] {
        core::slice::from_ref(&self.member)
    }

    #[must_use]
    pub const fn integrity(&self) -> BackupManifestIntegrity {
        BackupManifestIntegrity { _private: () }
    }

    #[must_use]
    pub const fn protected_material_included(&self) -> bool {
        false
    }

    #[allow(dead_code)]
    pub(crate) fn from_capture(
        metadata: &ServiceDatabaseMetadata,
        created_at_unix_ms: BackupCreatedAtUnixMs,
        state_byte_length: u64,
        state_sha256: BackupMemberSha256,
    ) -> Result<Self, BackupManifestContractError> {
        Self::build(
            metadata.service().clone(),
            metadata.instance().clone(),
            metadata.source_generation(),
            metadata.state_schema_version(),
            created_at_unix_ms,
            state_byte_length,
            state_sha256,
        )
    }

    fn from_wire(wire: WireManifest) -> Result<Self, BackupManifestContractError> {
        if wire.schema != BACKUP_MANIFEST_SCHEMA {
            return Err(BackupManifestContractError::InvalidSchema);
        }
        if wire.schema_version != BACKUP_MANIFEST_SCHEMA_VERSION {
            return Err(BackupManifestContractError::UnsupportedVersion);
        }
        let service = ServiceId::new(wire.service)
            .map_err(|_| BackupManifestContractError::InvalidServiceIdentity)?;
        let instance = InstanceId::new(wire.instance)
            .map_err(|_| BackupManifestContractError::InvalidInstanceIdentity)?;
        let source_generation = decode_hex_32(&wire.source_generation)
            .and_then(|value| SourceGeneration::new(value).ok())
            .ok_or(BackupManifestContractError::InvalidSourceGeneration)?;
        let state_schema_version = NonZeroU32::new(wire.state_schema_version)
            .ok_or(BackupManifestContractError::InvalidStateSchemaVersion)?;
        let created_at_unix_ms = BackupCreatedAtUnixMs::new(wire.created_at_unix_ms)?;
        let [member] = wire.members.as_slice() else {
            return Err(BackupManifestContractError::InvalidMemberInventory);
        };
        if member.name != BACKUP_STATE_MEMBER_NAME {
            return Err(BackupManifestContractError::InvalidMemberName);
        }
        if member.byte_length == 0 {
            return Err(BackupManifestContractError::InvalidMemberLength);
        }
        let member_sha256 = BackupMemberSha256(
            decode_hex_32(&member.sha256)
                .ok_or(BackupManifestContractError::InvalidMemberDigest)?,
        );
        if wire.integrity.sqlite != INTEGRITY_OK || wire.integrity.foreign_keys != INTEGRITY_OK {
            return Err(BackupManifestContractError::InvalidIntegrity);
        }
        if wire.protected_material_included {
            return Err(BackupManifestContractError::ProtectedMaterialIncluded);
        }
        Self::build(
            service,
            instance,
            source_generation,
            state_schema_version,
            created_at_unix_ms,
            member.byte_length,
            member_sha256,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn build(
        service: ServiceId,
        instance: InstanceId,
        source_generation: SourceGeneration,
        state_schema_version: NonZeroU32,
        created_at_unix_ms: BackupCreatedAtUnixMs,
        state_byte_length: u64,
        state_sha256: BackupMemberSha256,
    ) -> Result<Self, BackupManifestContractError> {
        if state_byte_length == 0 {
            return Err(BackupManifestContractError::InvalidMemberLength);
        }
        let source_generation_hex = encode_hex(source_generation.as_bytes());
        let state_sha256_hex = encode_hex(state_sha256.as_bytes());
        let canonical = CanonicalManifest {
            schema: BACKUP_MANIFEST_SCHEMA,
            schema_version: BACKUP_MANIFEST_SCHEMA_VERSION,
            service: service.as_str(),
            instance: instance.as_str(),
            source_generation: &source_generation_hex,
            state_schema_version: state_schema_version.get(),
            created_at_unix_ms: created_at_unix_ms.get(),
            members: [CanonicalMember {
                name: BACKUP_STATE_MEMBER_NAME,
                byte_length: state_byte_length,
                sha256: &state_sha256_hex,
            }],
            integrity: CanonicalIntegrity {
                sqlite: INTEGRITY_OK,
                foreign_keys: INTEGRITY_OK,
            },
            protected_material_included: false,
        };
        let canonical_bytes = serde_json::to_vec(&canonical)
            .map_err(|_| BackupManifestContractError::EncodingFailure)?;
        if canonical_bytes.len() > BACKUP_MANIFEST_CANONICAL_MAX_BYTES {
            return Err(BackupManifestContractError::ManifestTooLarge);
        }
        let digest = BackupManifestSha256(Sha256::digest(&canonical_bytes).into());
        Ok(Self {
            service,
            instance,
            source_generation,
            state_schema_version,
            created_at_unix_ms,
            member: ServiceBackupMember {
                byte_length: state_byte_length,
                sha256: state_sha256,
            },
            canonical_bytes: canonical_bytes.into_boxed_slice(),
            digest,
        })
    }
}

impl fmt::Debug for ServiceBackupManifest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ServiceBackupManifest")
            .field("schema_version", &BACKUP_MANIFEST_SCHEMA_VERSION)
            .field("service", &"[redacted]")
            .field("instance", &"[redacted]")
            .field("source_generation", &"[redacted]")
            .field("state_schema_version", &self.state_schema_version)
            .field("created_at_unix_ms", &self.created_at_unix_ms)
            .field("member", &self.member)
            .field("digest", &"[redacted]")
            .field("protected_material_included", &false)
            .finish()
    }
}

/// Stable, source-free backup manifest validation failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackupManifestContractError {
    ManifestTooLarge,
    MalformedEncoding,
    NonCanonicalEncoding,
    EncodingFailure,
    InvalidSchema,
    UnsupportedVersion,
    InvalidServiceIdentity,
    InvalidInstanceIdentity,
    InvalidSourceGeneration,
    InvalidStateSchemaVersion,
    InvalidCreationTime,
    InvalidMemberInventory,
    InvalidMemberName,
    InvalidMemberLength,
    InvalidMemberDigest,
    InvalidIntegrity,
    ProtectedMaterialIncluded,
}

impl fmt::Display for BackupManifestContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ManifestTooLarge => "backup manifest exceeds its byte limit",
            Self::MalformedEncoding => "backup manifest encoding is malformed",
            Self::NonCanonicalEncoding => "backup manifest encoding is not canonical",
            Self::EncodingFailure => "backup manifest could not be encoded",
            Self::InvalidSchema => "backup manifest schema is invalid",
            Self::UnsupportedVersion => "backup manifest version is unsupported",
            Self::InvalidServiceIdentity => "backup manifest service identity is invalid",
            Self::InvalidInstanceIdentity => "backup manifest instance identity is invalid",
            Self::InvalidSourceGeneration => "backup manifest source generation is invalid",
            Self::InvalidStateSchemaVersion => "backup manifest state schema version is invalid",
            Self::InvalidCreationTime => "backup manifest creation time is invalid",
            Self::InvalidMemberInventory => "backup manifest member inventory is invalid",
            Self::InvalidMemberName => "backup manifest member name is invalid",
            Self::InvalidMemberLength => "backup manifest member length is invalid",
            Self::InvalidMemberDigest => "backup manifest member digest is invalid",
            Self::InvalidIntegrity => "backup manifest integrity result is invalid",
            Self::ProtectedMaterialIncluded => "backup manifest includes protected material",
        })
    }
}

impl Error for BackupManifestContractError {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireManifest {
    schema: String,
    schema_version: u32,
    service: String,
    instance: String,
    source_generation: String,
    state_schema_version: u32,
    created_at_unix_ms: u64,
    members: Vec<WireMember>,
    integrity: WireIntegrity,
    protected_material_included: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireMember {
    name: String,
    byte_length: u64,
    sha256: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireIntegrity {
    sqlite: String,
    foreign_keys: String,
}

#[derive(Serialize)]
struct CanonicalManifest<'a> {
    schema: &'a str,
    schema_version: u32,
    service: &'a str,
    instance: &'a str,
    source_generation: &'a str,
    state_schema_version: u32,
    created_at_unix_ms: u64,
    members: [CanonicalMember<'a>; 1],
    integrity: CanonicalIntegrity<'a>,
    protected_material_included: bool,
}

#[derive(Serialize)]
struct CanonicalMember<'a> {
    name: &'a str,
    byte_length: u64,
    sha256: &'a str,
}

#[derive(Serialize)]
struct CanonicalIntegrity<'a> {
    sqlite: &'a str,
    foreign_keys: &'a str,
}

fn encode_hex(bytes: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(64);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn decode_hex_32(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 {
        return None;
    }
    let mut decoded = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let high = decode_lower_hex(pair[0])?;
        let low = decode_lower_hex(pair[1])?;
        decoded[index] = (high << 4) | low;
    }
    Some(decoded)
}

const fn decode_lower_hex(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ServiceSqliteApplicationId, ServiceSqlitePaths};
    use radroots_runtime_paths::{
        RadrootsHostEnvironment, RadrootsPathProfile, RadrootsPathResolver, RadrootsPlatform,
        RuntimeContext, RuntimeContextBootstrap, RuntimeContextSource,
    };
    use std::path::PathBuf;

    const GENERATION: &str = "0101010101010101010101010101010101010101010101010101010101010101";
    const MEMBER_SHA: &str = "abababababababababababababababababababababababababababababababab";
    const CANONICAL: &str = concat!(
        "{\"schema\":\"radroots.service-backup\",\"schema_version\":1,",
        "\"service\":\"myc\",\"instance\":\"primary\",",
        "\"source_generation\":\"0101010101010101010101010101010101010101010101010101010101010101\",",
        "\"state_schema_version\":1,\"created_at_unix_ms\":1700000000000,",
        "\"members\":[{\"name\":\"state.sqlite\",\"byte_length\":12345,",
        "\"sha256\":\"abababababababababababababababababababababababababababababababab\"}],",
        "\"integrity\":{\"sqlite\":\"ok\",\"foreign_keys\":\"ok\"},",
        "\"protected_material_included\":false}"
    );

    fn parsed() -> ServiceBackupManifest {
        ServiceBackupManifest::from_canonical_bytes(CANONICAL.as_bytes())
            .expect("canonical backup manifest")
    }

    fn replace_once(old: &str, new: &str) -> String {
        CANONICAL.replacen(old, new, 1)
    }

    fn paths() -> ServiceSqlitePaths {
        let context = RuntimeContext::resolve(
            &RadrootsPathResolver::new(RadrootsPlatform::Linux, RadrootsHostEnvironment::default()),
            RuntimeContextBootstrap::new(
                RadrootsPathProfile::RepoLocal,
                Some(PathBuf::from("/isolated/backup-manifest")),
                RuntimeContextSource::BootstrapCli,
                RuntimeContextSource::BootstrapCli,
            )
            .expect("bootstrap"),
            ServiceId::new("myc").expect("service"),
            InstanceId::new("primary").expect("instance"),
        )
        .expect("runtime context");
        ServiceSqlitePaths::from_runtime_context(&context).expect("SQLite paths")
    }

    #[test]
    fn canonical_bytes_and_digest_are_frozen() {
        let manifest = parsed();
        assert_eq!(manifest.canonical_bytes(), CANONICAL.as_bytes());
        assert_eq!(manifest.schema(), BACKUP_MANIFEST_SCHEMA);
        assert_eq!(manifest.schema_version(), BACKUP_MANIFEST_SCHEMA_VERSION);
        assert_eq!(manifest.service().as_str(), "myc");
        assert_eq!(manifest.instance().as_str(), "primary");
        assert_eq!(
            encode_hex(manifest.source_generation().as_bytes()),
            GENERATION
        );
        assert_eq!(manifest.state_schema_version().get(), 1);
        assert_eq!(manifest.created_at_unix_ms().get(), 1_700_000_000_000);
        assert_eq!(manifest.members().len(), 1);
        assert_eq!(manifest.members()[0].name(), BACKUP_STATE_MEMBER_NAME);
        assert_eq!(manifest.members()[0].byte_length(), 12_345);
        assert_eq!(
            encode_hex(manifest.members()[0].sha256().as_bytes()),
            MEMBER_SHA
        );
        assert_eq!(manifest.integrity().sqlite(), INTEGRITY_OK);
        assert_eq!(manifest.integrity().foreign_keys(), INTEGRITY_OK);
        assert!(!manifest.protected_material_included());
        assert_eq!(
            encode_hex(manifest.digest().as_bytes()),
            "1e9212b4a8e8db0d96134fc9fc10392b63de1ebea2e84cc201615bbb75cd7fd8"
        );
    }

    #[test]
    fn capture_constructor_reuses_exact_database_identity() {
        let paths = paths();
        let metadata = ServiceDatabaseMetadata::new(
            &paths,
            SourceGeneration::new([1; 32]).expect("generation"),
            NonZeroU32::new(1).expect("schema"),
            1_600_000_000_000,
            ServiceSqliteApplicationId::new(7).expect("application ID"),
        )
        .expect("metadata");
        let manifest = ServiceBackupManifest::from_capture(
            &metadata,
            BackupCreatedAtUnixMs::new(1_700_000_000_000).expect("backup time"),
            12_345,
            BackupMemberSha256::from_bytes([0xab; 32]),
        )
        .expect("captured manifest model");
        assert_eq!(manifest.canonical_bytes(), CANONICAL.as_bytes());
    }

    #[test]
    fn noncanonical_and_ambiguous_encodings_fail_closed() {
        let reordered = CANONICAL.replacen(
            "\"schema\":\"radroots.service-backup\",\"schema_version\":1",
            "\"schema_version\":1,\"schema\":\"radroots.service-backup\"",
            1,
        );
        for bytes in [format!(" {CANONICAL}"), format!("{CANONICAL}\n"), reordered] {
            assert_eq!(
                ServiceBackupManifest::from_canonical_bytes(bytes.as_bytes()),
                Err(BackupManifestContractError::NonCanonicalEncoding)
            );
        }
        for bytes in [
            format!("\u{feff}{CANONICAL}"),
            replace_once(
                "\"schema_version\":1",
                "\"schema_version\":1,\"schema_version\":1",
            ),
            replace_once(
                "\"schema_version\":1",
                "\"schema_version\":1,\"unknown\":false",
            ),
            replace_once("\"service\":\"myc\"", "\"service\":null"),
        ] {
            assert_eq!(
                ServiceBackupManifest::from_canonical_bytes(bytes.as_bytes()),
                Err(BackupManifestContractError::MalformedEncoding)
            );
        }
        assert_eq!(
            ServiceBackupManifest::from_canonical_bytes(&vec![b'x'; 1_025]),
            Err(BackupManifestContractError::ManifestTooLarge)
        );
    }

    #[test]
    fn schema_identity_generation_version_and_time_are_exact() {
        for (old, new, expected) in [
            (
                "radroots.service-backup",
                "radroots.other-backup",
                BackupManifestContractError::InvalidSchema,
            ),
            (
                "\"schema_version\":1",
                "\"schema_version\":2",
                BackupManifestContractError::UnsupportedVersion,
            ),
            (
                "\"service\":\"myc\"",
                "\"service\":\"../myc\"",
                BackupManifestContractError::InvalidServiceIdentity,
            ),
            (
                "\"instance\":\"primary\"",
                "\"instance\":\"PRIMARY\"",
                BackupManifestContractError::InvalidInstanceIdentity,
            ),
            (
                "\"state_schema_version\":1",
                "\"state_schema_version\":0",
                BackupManifestContractError::InvalidStateSchemaVersion,
            ),
            (
                "\"created_at_unix_ms\":1700000000000",
                "\"created_at_unix_ms\":0",
                BackupManifestContractError::InvalidCreationTime,
            ),
        ] {
            assert_eq!(
                ServiceBackupManifest::from_canonical_bytes(replace_once(old, new).as_bytes()),
                Err(expected)
            );
        }
        for generation in [
            "0".repeat(64),
            "AB".repeat(32),
            "01".repeat(31),
            format!("{}g", &GENERATION[..63]),
        ] {
            assert_eq!(
                ServiceBackupManifest::from_canonical_bytes(
                    replace_once(GENERATION, &generation).as_bytes()
                ),
                Err(BackupManifestContractError::InvalidSourceGeneration)
            );
        }
        assert_eq!(
            BackupCreatedAtUnixMs::new(i64::MAX as u64).map(BackupCreatedAtUnixMs::get),
            Ok(i64::MAX as u64)
        );
        assert_eq!(
            BackupCreatedAtUnixMs::new(i64::MAX as u64 + 1),
            Err(BackupManifestContractError::InvalidCreationTime)
        );
    }

    #[test]
    fn member_inventory_length_and_digest_are_exact() {
        let singleton = format!(
            "[{{\"name\":\"state.sqlite\",\"byte_length\":12345,\"sha256\":\"{MEMBER_SHA}\"}}]"
        );
        for (members, expected) in [
            (
                "[]".to_owned(),
                BackupManifestContractError::InvalidMemberInventory,
            ),
            (
                format!(
                    "[{{\"name\":\"state.sqlite\",\"byte_length\":12345,\"sha256\":\"{MEMBER_SHA}\"}},{{\"name\":\"state.sqlite\",\"byte_length\":12345,\"sha256\":\"{MEMBER_SHA}\"}}]"
                ),
                BackupManifestContractError::InvalidMemberInventory,
            ),
        ] {
            assert_eq!(
                ServiceBackupManifest::from_canonical_bytes(
                    replace_once(&singleton, &members).as_bytes()
                ),
                Err(expected)
            );
        }
        for name in ["", "state.db", "../state.sqlite"] {
            assert_eq!(
                ServiceBackupManifest::from_canonical_bytes(
                    replace_once("state.sqlite", name).as_bytes()
                ),
                Err(BackupManifestContractError::InvalidMemberName)
            );
        }
        assert_eq!(
            ServiceBackupManifest::from_canonical_bytes(
                replace_once("\"byte_length\":12345", "\"byte_length\":0").as_bytes()
            ),
            Err(BackupManifestContractError::InvalidMemberLength)
        );
        assert!(
            ServiceBackupManifest::from_canonical_bytes(
                replace_once(
                    "\"byte_length\":12345",
                    "\"byte_length\":18446744073709551615"
                )
                .as_bytes()
            )
            .is_ok()
        );
        for digest in [
            MEMBER_SHA.to_uppercase(),
            "ab".repeat(31),
            format!("{}g", &MEMBER_SHA[..63]),
        ] {
            assert_eq!(
                ServiceBackupManifest::from_canonical_bytes(
                    replace_once(MEMBER_SHA, &digest).as_bytes()
                ),
                Err(BackupManifestContractError::InvalidMemberDigest)
            );
        }
    }

    #[test]
    fn integrity_and_protected_material_fail_closed() {
        for invalid in [
            replace_once("\"sqlite\":\"ok\"", "\"sqlite\":\"failed\""),
            replace_once("\"foreign_keys\":\"ok\"", "\"foreign_keys\":\"failed\""),
        ] {
            assert_eq!(
                ServiceBackupManifest::from_canonical_bytes(invalid.as_bytes()),
                Err(BackupManifestContractError::InvalidIntegrity)
            );
        }
        assert_eq!(
            ServiceBackupManifest::from_canonical_bytes(
                replace_once(
                    "\"protected_material_included\":false",
                    "\"protected_material_included\":true"
                )
                .as_bytes()
            ),
            Err(BackupManifestContractError::ProtectedMaterialIncluded)
        );
    }

    #[test]
    fn diagnostics_are_source_free_and_redacted() {
        let debug = format!("{:?}", parsed());
        for forbidden in ["myc", "primary", GENERATION, MEMBER_SHA] {
            assert!(!debug.contains(forbidden));
        }
        let error = BackupManifestContractError::InvalidMemberDigest;
        assert!(error.source().is_none());
        assert_eq!(
            error.to_string(),
            "backup manifest member digest is invalid"
        );
    }
}
