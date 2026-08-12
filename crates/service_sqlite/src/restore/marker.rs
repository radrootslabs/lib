//! Sealed v1 restore-recovery marker and durable descriptor-relative store.

#![allow(dead_code)] // Step 066 freezes private primitives consumed by Steps 067-069.

use core::{fmt, num::NonZeroU32};
use std::{error::Error, path::PathBuf};

#[cfg(test)]
use std::path::Path;

use radroots_runtime_paths::{InstanceId, ServiceId};
use radroots_storage::event::SourceGeneration;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    BackupManifestSha256, ServiceDatabaseIdentity, ServiceDatabaseMetadata,
    ServiceSqliteApplicationId, ServiceSqlitePaths,
};

#[cfg(any(target_os = "linux", target_os = "macos"))]
use crate::{ServiceSqliteError, ServiceSqliteErrorKind, WriterAuthority};

const RESTORE_MARKER_SCHEMA: &str = "radroots.service-sqlite.restore-marker";
const RESTORE_MARKER_SCHEMA_VERSION: u32 = 1;
const RESTORE_MARKER_MAX_BYTES: usize = 2_048;
const RESTORE_MARKER_CHECKSUM_DOMAIN: &[u8] = b"radroots.service_sqlite.restore_marker.v1\0";
pub(crate) const LIVE_FILE_NAME: &str = radroots_runtime_paths::SERVICE_STATE_DATABASE_FILE_NAME;
pub(crate) const STAGED_FILE_NAME: &str = "state.restore-staged.sqlite";
pub(crate) const BACKUP_FILE_NAME: &str = "state.restore-backup.sqlite";
pub(crate) const MARKER_FILE_NAME: &str = "state.restore-marker.v1";
pub(crate) const MARKER_NEXT_FILE_NAME: &str = "state.restore-marker.v1.next";

/// Exact retained identity and content expected for one restore artifact.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct RestoreArtifactExpectation {
    device: u64,
    inode: u64,
    byte_length: u64,
    sha256: [u8; 32],
}

impl RestoreArtifactExpectation {
    pub(crate) const fn new(
        device: u64,
        inode: u64,
        byte_length: u64,
        sha256: [u8; 32],
    ) -> Result<Self, RestoreMarkerContractError> {
        if byte_length == 0 || byte_length > i64::MAX as u64 {
            return Err(RestoreMarkerContractError::InvalidIdentity);
        }
        Ok(Self {
            device,
            inode,
            byte_length,
            sha256,
        })
    }

    pub(crate) const fn device(self) -> u64 {
        self.device
    }

    pub(crate) const fn inode(self) -> u64 {
        self.inode
    }

    pub(crate) const fn byte_length(self) -> u64 {
        self.byte_length
    }

    pub(crate) const fn sha256(self) -> [u8; 32] {
        self.sha256
    }
}

impl fmt::Debug for RestoreArtifactExpectation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RestoreArtifactExpectation([redacted])")
    }
}

/// Durable phases in the v1 restore-replacement protocol.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RestoreRecoveryPhase {
    Prepared,
    LiveRetained,
    ReplacementInstalled,
}

impl RestoreRecoveryPhase {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Prepared => "prepared",
            Self::LiveRetained => "live_retained",
            Self::ReplacementInstalled => "replacement_installed",
        }
    }

    fn parse(value: &str) -> Result<Self, RestoreMarkerContractError> {
        match value {
            "prepared" => Ok(Self::Prepared),
            "live_retained" => Ok(Self::LiveRetained),
            "replacement_installed" => Ok(Self::ReplacementInstalled),
            _ => Err(RestoreMarkerContractError::UnsupportedValue),
        }
    }

    const fn may_transition_to(self, next: Self) -> bool {
        self as u8 == next as u8
            || matches!(
                (self, next),
                (Self::Prepared, Self::LiveRetained)
                    | (Self::LiveRetained, Self::ReplacementInstalled)
            )
    }
}

/// Fixed recovery-artifact layout adjacent to canonical service state.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct RestoreRecoveryLayout {
    state_directory: PathBuf,
    live: PathBuf,
    staged: PathBuf,
    backup: PathBuf,
    marker: PathBuf,
    marker_next: PathBuf,
}

impl RestoreRecoveryLayout {
    pub(crate) fn for_paths(
        paths: &ServiceSqlitePaths,
    ) -> Result<Self, RestoreMarkerContractError> {
        let live = paths.state_database();
        let state_directory = live
            .parent()
            .filter(|parent| Some(*parent) == paths.state_lock().parent())
            .filter(|_| live.file_name().is_some_and(|name| name == LIVE_FILE_NAME))
            .filter(|parent| parent.is_absolute())
            .ok_or(RestoreMarkerContractError::InvalidLayout)?;
        Ok(Self {
            state_directory: state_directory.to_path_buf(),
            live: state_directory.join(LIVE_FILE_NAME),
            staged: state_directory.join(STAGED_FILE_NAME),
            backup: state_directory.join(BACKUP_FILE_NAME),
            marker: state_directory.join(MARKER_FILE_NAME),
            marker_next: state_directory.join(MARKER_NEXT_FILE_NAME),
        })
    }

    pub(crate) fn state_directory(&self) -> &PathBuf {
        &self.state_directory
    }

    pub(crate) fn staged(&self) -> &PathBuf {
        &self.staged
    }

    #[cfg(test)]
    fn file_names(&self) -> [&str; 5] {
        [
            LIVE_FILE_NAME,
            STAGED_FILE_NAME,
            BACKUP_FILE_NAME,
            MARKER_FILE_NAME,
            MARKER_NEXT_FILE_NAME,
        ]
    }
}

impl fmt::Debug for RestoreRecoveryLayout {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RestoreRecoveryLayout([redacted])")
    }
}

/// Immutable canonical v1 restore-recovery marker.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct RestoreRecoveryMarker {
    phase: RestoreRecoveryPhase,
    service: ServiceId,
    instance: InstanceId,
    source_generation: SourceGeneration,
    state_schema_version: NonZeroU32,
    application_id: ServiceSqliteApplicationId,
    source_manifest_sha256: BackupManifestSha256,
    live: RestoreArtifactExpectation,
    staged: RestoreArtifactExpectation,
    backup: RestoreArtifactExpectation,
    canonical_bytes: Box<[u8]>,
}

impl RestoreRecoveryMarker {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepared(
        metadata: &ServiceDatabaseMetadata,
        source_manifest_sha256: BackupManifestSha256,
        live: RestoreArtifactExpectation,
        staged: RestoreArtifactExpectation,
    ) -> Result<Self, RestoreMarkerContractError> {
        Self::build(
            RestoreRecoveryPhase::Prepared,
            metadata.service().clone(),
            metadata.instance().clone(),
            metadata.source_generation(),
            metadata.state_schema_version(),
            metadata.application_id(),
            source_manifest_sha256,
            live,
            staged,
            live,
        )
    }

    pub(crate) fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, RestoreMarkerContractError> {
        if bytes.is_empty() {
            return Err(RestoreMarkerContractError::MalformedEncoding);
        }
        if bytes.len() > RESTORE_MARKER_MAX_BYTES {
            return Err(RestoreMarkerContractError::MarkerTooLarge);
        }
        let wire: WireMarker = serde_json::from_slice(bytes)
            .map_err(|_| RestoreMarkerContractError::MalformedEncoding)?;
        if wire.schema != RESTORE_MARKER_SCHEMA {
            return Err(RestoreMarkerContractError::UnsupportedValue);
        }
        if wire.schema_version != RESTORE_MARKER_SCHEMA_VERSION {
            return Err(RestoreMarkerContractError::UnsupportedValue);
        }
        let marker = Self::build(
            RestoreRecoveryPhase::parse(&wire.phase)?,
            ServiceId::new(wire.service)
                .map_err(|_| RestoreMarkerContractError::InvalidIdentity)?,
            InstanceId::new(wire.instance)
                .map_err(|_| RestoreMarkerContractError::InvalidIdentity)?,
            SourceGeneration::new(decode_hex_32(&wire.source_generation)?)
                .map_err(|_| RestoreMarkerContractError::InvalidIdentity)?,
            NonZeroU32::new(wire.state_schema_version)
                .ok_or(RestoreMarkerContractError::InvalidIdentity)?,
            ServiceSqliteApplicationId::new(wire.application_id)
                .map_err(|_| RestoreMarkerContractError::InvalidIdentity)?,
            BackupManifestSha256::from_bytes(decode_hex_32(&wire.source_manifest_sha256)?),
            RestoreArtifactExpectation::try_from(wire.live)?,
            RestoreArtifactExpectation::try_from(wire.staged)?,
            RestoreArtifactExpectation::try_from(wire.backup)?,
        )?;
        let claimed = decode_hex_32(&wire.marker_sha256)?;
        let actual = marker_checksum(&marker.payload_bytes()?);
        if claimed != actual {
            return Err(RestoreMarkerContractError::ChecksumMismatch);
        }
        if marker.canonical_bytes.as_ref() != bytes {
            return Err(RestoreMarkerContractError::NonCanonicalEncoding);
        }
        Ok(marker)
    }

    pub(crate) fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub(crate) const fn phase(&self) -> RestoreRecoveryPhase {
        self.phase
    }

    pub(crate) const fn live(&self) -> RestoreArtifactExpectation {
        self.live
    }

    pub(crate) const fn staged(&self) -> RestoreArtifactExpectation {
        self.staged
    }

    pub(crate) const fn backup(&self) -> RestoreArtifactExpectation {
        self.backup
    }

    pub(crate) fn transitioned_to(
        &self,
        next: RestoreRecoveryPhase,
    ) -> Result<Self, RestoreMarkerContractError> {
        if !self.phase.may_transition_to(next) {
            return Err(RestoreMarkerContractError::IllegalTransition);
        }
        if self.phase == next {
            return Ok(self.clone());
        }
        Self::build(
            next,
            self.service.clone(),
            self.instance.clone(),
            self.source_generation,
            self.state_schema_version,
            self.application_id,
            self.source_manifest_sha256,
            self.live,
            self.staged,
            self.backup,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn build(
        phase: RestoreRecoveryPhase,
        service: ServiceId,
        instance: InstanceId,
        source_generation: SourceGeneration,
        state_schema_version: NonZeroU32,
        application_id: ServiceSqliteApplicationId,
        source_manifest_sha256: BackupManifestSha256,
        live: RestoreArtifactExpectation,
        staged: RestoreArtifactExpectation,
        backup: RestoreArtifactExpectation,
    ) -> Result<Self, RestoreMarkerContractError> {
        if live != backup || (live.device, live.inode) == (staged.device, staged.inode) {
            return Err(RestoreMarkerContractError::InvalidIdentity);
        }
        let mut marker = Self {
            phase,
            service,
            instance,
            source_generation,
            state_schema_version,
            application_id,
            source_manifest_sha256,
            live,
            staged,
            backup,
            canonical_bytes: Box::new([]),
        };
        let payload = marker.payload_bytes()?;
        let checksum = encode_hex(&marker_checksum(&payload));
        let canonical = marker.wire(&checksum);
        let canonical_bytes = serde_json::to_vec(&canonical)
            .map_err(|_| RestoreMarkerContractError::EncodingFailure)?;
        if canonical_bytes.len() > RESTORE_MARKER_MAX_BYTES {
            return Err(RestoreMarkerContractError::MarkerTooLarge);
        }
        marker.canonical_bytes = canonical_bytes.into_boxed_slice();
        Ok(marker)
    }

    fn payload_bytes(&self) -> Result<Vec<u8>, RestoreMarkerContractError> {
        serde_json::to_vec(&self.payload()).map_err(|_| RestoreMarkerContractError::EncodingFailure)
    }

    fn payload(&self) -> CanonicalPayload<'_> {
        CanonicalPayload {
            schema: RESTORE_MARKER_SCHEMA,
            schema_version: RESTORE_MARKER_SCHEMA_VERSION,
            phase: self.phase.as_str(),
            service: self.service.as_str(),
            instance: self.instance.as_str(),
            source_generation: encode_hex(self.source_generation.as_bytes()),
            state_schema_version: self.state_schema_version.get(),
            application_id: self.application_id.get(),
            source_manifest_sha256: encode_hex(self.source_manifest_sha256.as_bytes()),
            live: self.live.into(),
            staged: self.staged.into(),
            backup: self.backup.into(),
        }
    }

    fn wire<'a>(&'a self, marker_sha256: &'a str) -> CanonicalMarker<'a> {
        CanonicalMarker {
            payload: self.payload(),
            marker_sha256,
        }
    }

    fn matches_paths(&self, paths: &ServiceSqlitePaths) -> bool {
        self.service == *paths.service() && self.instance == *paths.instance()
    }

    pub(crate) fn matches_identity(&self, identity: &ServiceDatabaseIdentity) -> bool {
        self.service == *identity.service()
            && self.instance == *identity.instance()
            && self.source_generation == identity.source_generation()
            && self.application_id == identity.application_id()
            && self.state_schema_version <= identity.supported_state_schema_version()
    }
}

impl fmt::Debug for RestoreRecoveryMarker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RestoreRecoveryMarker")
            .field("schema_version", &RESTORE_MARKER_SCHEMA_VERSION)
            .field("phase", &self.phase)
            .finish_non_exhaustive()
    }
}

/// Source-free validation failure for private restore markers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RestoreMarkerContractError {
    MarkerTooLarge,
    MalformedEncoding,
    NonCanonicalEncoding,
    EncodingFailure,
    UnsupportedValue,
    ChecksumMismatch,
    InvalidIdentity,
    InvalidLayout,
    IllegalTransition,
}

impl fmt::Display for RestoreMarkerContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::MarkerTooLarge => "restore marker exceeds its byte limit",
            Self::MalformedEncoding => "restore marker encoding is malformed",
            Self::NonCanonicalEncoding => "restore marker encoding is not canonical",
            Self::EncodingFailure => "restore marker could not be encoded",
            Self::UnsupportedValue => "restore marker schema or value is unsupported",
            Self::ChecksumMismatch => "restore marker checksum does not match",
            Self::InvalidIdentity => "restore marker identity is invalid",
            Self::InvalidLayout => "restore recovery layout is invalid",
            Self::IllegalTransition => "restore marker transition is illegal",
        })
    }
}

impl Error for RestoreMarkerContractError {}

#[derive(Serialize)]
struct CanonicalPayload<'a> {
    schema: &'static str,
    schema_version: u32,
    phase: &'static str,
    service: &'a str,
    instance: &'a str,
    source_generation: String,
    state_schema_version: u32,
    application_id: u32,
    source_manifest_sha256: String,
    live: CanonicalArtifact,
    staged: CanonicalArtifact,
    backup: CanonicalArtifact,
}

#[derive(Serialize)]
struct CanonicalMarker<'a> {
    #[serde(flatten)]
    payload: CanonicalPayload<'a>,
    marker_sha256: &'a str,
}

#[derive(Clone, Serialize)]
struct CanonicalArtifact {
    device: u64,
    inode: u64,
    byte_length: u64,
    sha256: String,
}

impl From<RestoreArtifactExpectation> for CanonicalArtifact {
    fn from(value: RestoreArtifactExpectation) -> Self {
        Self {
            device: value.device,
            inode: value.inode,
            byte_length: value.byte_length,
            sha256: encode_hex(&value.sha256),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireMarker {
    schema: String,
    schema_version: u32,
    phase: String,
    service: String,
    instance: String,
    source_generation: String,
    state_schema_version: u32,
    application_id: u32,
    source_manifest_sha256: String,
    live: WireArtifact,
    staged: WireArtifact,
    backup: WireArtifact,
    marker_sha256: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireArtifact {
    device: u64,
    inode: u64,
    byte_length: u64,
    sha256: String,
}

impl TryFrom<WireArtifact> for RestoreArtifactExpectation {
    type Error = RestoreMarkerContractError;

    fn try_from(value: WireArtifact) -> Result<Self, Self::Error> {
        Self::new(
            value.device,
            value.inode,
            value.byte_length,
            decode_hex_32(&value.sha256)?,
        )
    }
}

fn marker_checksum(payload: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(RESTORE_MARKER_CHECKSUM_DOMAIN);
    hasher.update(
        u64::try_from(payload.len())
            .expect("bounded marker payload")
            .to_be_bytes(),
    );
    hasher.update(payload);
    hasher.finalize().into()
}

fn encode_hex(bytes: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(64);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

fn decode_hex_32(value: &str) -> Result<[u8; 32], RestoreMarkerContractError> {
    if value.len() != 64 {
        return Err(RestoreMarkerContractError::InvalidIdentity);
    }
    let mut output = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        output[index] = (decode_nibble(pair[0])? << 4) | decode_nibble(pair[1])?;
    }
    Ok(output)
}

fn decode_nibble(value: u8) -> Result<u8, RestoreMarkerContractError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err(RestoreMarkerContractError::InvalidIdentity),
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod store {
    use std::{
        fs::File,
        io::{Read, Seek, SeekFrom, Write},
    };

    use rustix::{
        fs::{
            AtFlags, FileType, Mode, OFlags, fchmod, fstat, open, openat, renameat, statat,
            unlinkat,
        },
        io::Errno,
        process::geteuid,
    };

    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct FileIdentity {
        device: u64,
        inode: u64,
    }

    pub(crate) struct RestoreMarkerBinding {
        directory: File,
        directory_identity: FileIdentity,
        marker_file: File,
        marker_identity: FileIdentity,
        marker: RestoreRecoveryMarker,
    }

    impl fmt::Debug for RestoreMarkerBinding {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter
                .debug_struct("RestoreMarkerBinding")
                .field("phase", &self.marker.phase())
                .field("artifacts", &"[redacted]")
                .finish()
        }
    }

    impl RestoreMarkerBinding {
        #[cfg(test)]
        pub(crate) fn create(
            paths: &ServiceSqlitePaths,
            authority: &WriterAuthority,
            marker: &RestoreRecoveryMarker,
        ) -> Result<Self, ServiceSqliteError> {
            Self::create_with_durable_callback(paths, authority, marker, || {})
        }

        pub(crate) fn create_with_durable_callback(
            paths: &ServiceSqlitePaths,
            authority: &WriterAuthority,
            marker: &RestoreRecoveryMarker,
            on_durable: impl FnOnce(),
        ) -> Result<Self, ServiceSqliteError> {
            let failpoints = crate::failpoint::DurabilityFailpoints::default();
            Self::create_with_durable_callback_and_failpoints(
                paths,
                authority,
                marker,
                &failpoints,
                on_durable,
            )
        }

        pub(crate) fn create_with_durable_callback_and_failpoints(
            paths: &ServiceSqlitePaths,
            authority: &WriterAuthority,
            marker: &RestoreRecoveryMarker,
            failpoints: &crate::failpoint::DurabilityFailpoints,
            on_durable: impl FnOnce(),
        ) -> Result<Self, ServiceSqliteError> {
            Self::create_with_operations(
                paths,
                authority,
                marker,
                &SystemStoreOperations,
                failpoints,
                on_durable,
            )
        }

        #[cfg(test)]
        pub(crate) fn test_create_with_durable_authority_drift(
            paths: &ServiceSqlitePaths,
            authority: &WriterAuthority,
            marker: &RestoreRecoveryMarker,
            on_durable: impl FnOnce(),
        ) -> Result<Self, ServiceSqliteError> {
            Self::create_with_operations(
                paths,
                authority,
                marker,
                &AuthorityDriftAfterSyncStoreOperations,
                &crate::failpoint::DurabilityFailpoints::default(),
                on_durable,
            )
        }

        fn create_with_operations(
            paths: &ServiceSqlitePaths,
            authority: &WriterAuthority,
            marker: &RestoreRecoveryMarker,
            operations: &dyn StoreOperations,
            failpoints: &crate::failpoint::DurabilityFailpoints,
            on_durable: impl FnOnce(),
        ) -> Result<Self, ServiceSqliteError> {
            authority.validate_for(paths)?;
            if !marker.matches_paths(paths) {
                return Err(restore_contract(
                    RestoreMarkerContractError::InvalidIdentity,
                ));
            }
            if marker.phase() != RestoreRecoveryPhase::Prepared {
                return Err(restore_contract(
                    RestoreMarkerContractError::IllegalTransition,
                ));
            }
            let layout = RestoreRecoveryLayout::for_paths(paths).map_err(restore_contract)?;
            let directory = authority_checked(authority, paths, || {
                authority
                    .directory()
                    .try_clone()
                    .map_err(|_| StoreFailure::Directory)
            })?
            .map_err(restore_store)?;
            let directory_identity =
                authority_checked(authority, paths, || validate_directory(&directory))?
                    .map_err(restore_store)?;
            authority_checked(authority, paths, || {
                require_absent(&directory, MARKER_NEXT_FILE_NAME)
            })?
            .map_err(restore_store)?;
            authority_checked(authority, paths, || {
                hit(
                    failpoints,
                    crate::failpoint::DurabilityFailpoint::MarkerBeforeCreate,
                )
            })?
            .map_err(restore_store)?;
            let (marker_file, marker_identity) = authority_checked(authority, paths, || {
                let file = create_marker_file(&directory, MARKER_FILE_NAME)?;
                let identity = file_identity(&file)?;
                Ok::<_, StoreFailure>((file, identity))
            })?
            .map_err(restore_store)?;
            if let Err(cause) = hit(
                failpoints,
                crate::failpoint::DurabilityFailpoint::MarkerAfterCreate,
            ) {
                cleanup_with_authority(
                    authority,
                    paths,
                    &directory,
                    MARKER_FILE_NAME,
                    marker_identity,
                )?;
                return Err(restore_store(cause));
            }
            let write_result = authority_checked(authority, paths, || {
                write_and_sync(
                    &marker_file,
                    marker.canonical_bytes(),
                    &directory,
                    operations,
                    failpoints,
                    Some(crate::failpoint::DurabilityFailpoint::MarkerBeforeFileSync),
                    Some(crate::failpoint::DurabilityFailpoint::MarkerAfterFileSync),
                )
            })?;
            if let Err(cause) = write_result {
                cleanup_with_authority(
                    authority,
                    paths,
                    &directory,
                    MARKER_FILE_NAME,
                    marker_identity,
                )?;
                return Err(restore_store(cause));
            }
            authority.validate_for(paths)?;
            if let Err(cause) = hit(
                failpoints,
                crate::failpoint::DurabilityFailpoint::MarkerBeforeDirectorySync,
            ) {
                cleanup_with_authority(
                    authority,
                    paths,
                    &directory,
                    MARKER_FILE_NAME,
                    marker_identity,
                )?;
                return Err(restore_store(cause));
            }
            if operations.sync_directory(&directory).is_err() {
                authority.validate_for(paths)?;
                cleanup_with_authority(
                    authority,
                    paths,
                    &directory,
                    MARKER_FILE_NAME,
                    marker_identity,
                )?;
                return Err(restore_store(StoreFailure::Sync));
            }
            // The marker contents and its directory entry are durable from
            // this point. The caller must transfer ownership of every bound
            // artifact before any subsequent fallible validation.
            on_durable();
            let after_directory_sync = hit(
                failpoints,
                crate::failpoint::DurabilityFailpoint::MarkerAfterDirectorySync,
            );
            authority.validate_for(paths)?;
            after_directory_sync.map_err(restore_store)?;
            let binding = Self {
                directory,
                directory_identity,
                marker_file,
                marker_identity,
                marker: marker.clone(),
            };
            authority_checked(authority, paths, || binding.validate_for_restore(paths))??;
            if layout
                .marker
                .file_name()
                .is_none_or(|name| name != MARKER_FILE_NAME)
            {
                return Err(restore_contract(RestoreMarkerContractError::InvalidLayout));
            }
            Ok(binding)
        }

        pub(crate) fn load(paths: &ServiceSqlitePaths) -> Result<Option<Self>, ServiceSqliteError> {
            let layout = RestoreRecoveryLayout::for_paths(paths).map_err(recovery_contract)?;
            let directory = open(
                &layout.state_directory,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| recovery_store(StoreFailure::Directory))?;
            let directory = File::from(directory);
            let directory_identity = validate_directory(&directory).map_err(recovery_store)?;
            require_absent(&directory, MARKER_NEXT_FILE_NAME).map_err(recovery_store)?;
            let marker_file = match open_marker_file(&directory, MARKER_FILE_NAME) {
                Ok(file) => file,
                Err(StoreFailure::Missing) => return Ok(None),
                Err(cause) => return Err(recovery_store(cause)),
            };
            let marker_identity = file_identity(&marker_file).map_err(recovery_store)?;
            let marker = read_marker(&marker_file).map_err(recovery_store)?;
            if !marker.matches_paths(paths) {
                return Err(recovery_contract(
                    RestoreMarkerContractError::InvalidIdentity,
                ));
            }
            let binding = Self {
                directory,
                directory_identity,
                marker_file,
                marker_identity,
                marker,
            };
            binding.validate(paths)?;
            Ok(Some(binding))
        }

        pub(crate) fn load_for_recovery(
            paths: &ServiceSqlitePaths,
            authority: &WriterAuthority,
        ) -> Result<Option<Self>, ServiceSqliteError> {
            authority.validate_for(paths)?;
            let layout = RestoreRecoveryLayout::for_paths(paths).map_err(recovery_contract)?;
            let directory = authority_checked(authority, paths, || {
                authority
                    .directory()
                    .try_clone()
                    .map_err(|_| StoreFailure::Directory)
            })?
            .map_err(recovery_store)?;
            let directory_identity =
                authority_checked(authority, paths, || validate_directory(&directory))?
                    .map_err(authority_store)?;
            let marker_file = match authority_checked(authority, paths, || {
                open_marker_file(&directory, MARKER_FILE_NAME)
            })? {
                Ok(file) => file,
                Err(StoreFailure::Missing) => {
                    authority_checked(authority, paths, || {
                        require_absent(&directory, MARKER_NEXT_FILE_NAME)
                    })?
                    .map_err(recovery_store)?;
                    return Ok(None);
                }
                Err(cause) => return Err(recovery_store(cause)),
            };
            let marker_identity =
                authority_checked(authority, paths, || file_identity(&marker_file))?
                    .map_err(recovery_store)?;
            let marker = authority_checked(authority, paths, || read_marker(&marker_file))?
                .map_err(recovery_store)?;
            if !marker.matches_paths(paths) {
                return Err(recovery_contract(
                    RestoreMarkerContractError::InvalidIdentity,
                ));
            }
            let binding = Self {
                directory,
                directory_identity,
                marker_file,
                marker_identity,
                marker,
            };
            authority_checked(authority, paths, || {
                binding.validate_inner(paths, false, ServiceSqliteErrorKind::Recovery)
            })??;
            if layout
                .marker
                .file_name()
                .is_none_or(|name| name != MARKER_FILE_NAME)
            {
                return Err(recovery_contract(RestoreMarkerContractError::InvalidLayout));
            }
            authority.validate_for(paths)?;
            Ok(Some(binding))
        }

        pub(crate) fn advance(
            self,
            paths: &ServiceSqlitePaths,
            authority: &WriterAuthority,
            next: RestoreRecoveryPhase,
        ) -> Result<Self, ServiceSqliteError> {
            let failpoints = crate::failpoint::DurabilityFailpoints::default();
            self.advance_with_failpoints(paths, authority, next, &failpoints)
        }

        pub(crate) fn advance_with_failpoints(
            self,
            paths: &ServiceSqlitePaths,
            authority: &WriterAuthority,
            next: RestoreRecoveryPhase,
            failpoints: &crate::failpoint::DurabilityFailpoints,
        ) -> Result<Self, ServiceSqliteError> {
            self.advance_with_operations(
                paths,
                authority,
                next,
                &SystemStoreOperations,
                ServiceSqliteErrorKind::Restore,
                failpoints,
            )
        }

        pub(crate) fn advance_for_recovery(
            self,
            paths: &ServiceSqlitePaths,
            authority: &WriterAuthority,
            next: RestoreRecoveryPhase,
        ) -> Result<Self, ServiceSqliteError> {
            self.advance_with_operations(
                paths,
                authority,
                next,
                &SystemStoreOperations,
                ServiceSqliteErrorKind::Recovery,
                &crate::failpoint::DurabilityFailpoints::default(),
            )
        }

        fn advance_with_operations(
            self,
            paths: &ServiceSqlitePaths,
            authority: &WriterAuthority,
            next: RestoreRecoveryPhase,
            operations: &dyn StoreOperations,
            operation_kind: ServiceSqliteErrorKind,
            failpoints: &crate::failpoint::DurabilityFailpoints,
        ) -> Result<Self, ServiceSqliteError> {
            authority_checked(authority, paths, || {
                self.validate_inner(paths, true, operation_kind)
            })??;
            let current = authority_checked(authority, paths, || read_marker(&self.marker_file))?
                .map_err(|cause| operation_store(operation_kind, cause))?;
            if current.canonical_bytes() != self.marker.canonical_bytes() {
                return Err(operation_store(operation_kind, StoreFailure::Conflict));
            }
            let next_marker = self
                .marker
                .transitioned_to(next)
                .map_err(|cause| operation_contract(operation_kind, cause))?;
            if next_marker.canonical_bytes() == self.marker.canonical_bytes() {
                return Ok(self);
            }
            authority_checked(authority, paths, || {
                require_absent(&self.directory, MARKER_NEXT_FILE_NAME)
            })?
            .map_err(|cause| operation_store(operation_kind, cause))?;
            let (scratch, scratch_identity) = authority_checked(authority, paths, || {
                let file = create_marker_file(&self.directory, MARKER_NEXT_FILE_NAME)?;
                let identity = file_identity(&file)?;
                Ok::<_, StoreFailure>((file, identity))
            })?
            .map_err(|cause| operation_store(operation_kind, cause))?;
            let before_write = authority_checked(authority, paths, || {
                hit(
                    failpoints,
                    crate::failpoint::DurabilityFailpoint::MarkerAdvanceBeforeWriteAndFileSync,
                )
            })?;
            if let Err(cause) = before_write {
                cleanup_with_authority(
                    authority,
                    paths,
                    &self.directory,
                    MARKER_NEXT_FILE_NAME,
                    scratch_identity,
                )?;
                return Err(operation_store(operation_kind, cause));
            }
            let scratch_write = authority_checked(authority, paths, || {
                write_and_sync(
                    &scratch,
                    next_marker.canonical_bytes(),
                    &self.directory,
                    operations,
                    failpoints,
                    None,
                    None,
                )
            })?;
            if let Err(cause) = scratch_write {
                cleanup_with_authority(
                    authority,
                    paths,
                    &self.directory,
                    MARKER_NEXT_FILE_NAME,
                    scratch_identity,
                )?;
                return Err(operation_store(operation_kind, cause));
            }
            authority_checked(authority, paths, || {
                hit(
                    failpoints,
                    crate::failpoint::DurabilityFailpoint::MarkerAdvanceAfterWriteAndFileSync,
                )
            })?
            .map_err(|cause| operation_store(operation_kind, cause))?;
            authority_checked(authority, paths, || {
                self.validate_inner(paths, false, operation_kind)
            })??;
            let scratch_matches = authority_checked(authority, paths, || {
                let current = open_marker_file(&self.directory, MARKER_NEXT_FILE_NAME)?;
                Ok::<_, StoreFailure>(
                    file_identity(&scratch)? == scratch_identity
                        && file_identity(&current)? == scratch_identity,
                )
            })?
            .map_err(|cause| operation_store(operation_kind, cause))?;
            if !scratch_matches {
                cleanup_with_authority(
                    authority,
                    paths,
                    &self.directory,
                    MARKER_NEXT_FILE_NAME,
                    scratch_identity,
                )?;
                return Err(operation_store(operation_kind, StoreFailure::Conflict));
            }
            let before_replace = authority_checked(authority, paths, || {
                hit(
                    failpoints,
                    crate::failpoint::DurabilityFailpoint::MarkerAdvanceBeforeReplace,
                )
            })?;
            if let Err(cause) = before_replace {
                cleanup_with_authority(
                    authority,
                    paths,
                    &self.directory,
                    MARKER_NEXT_FILE_NAME,
                    scratch_identity,
                )?;
                return Err(operation_store(operation_kind, cause));
            }
            let replacement = authority_checked(authority, paths, || {
                operations
                    .replace_marker(&self.directory)
                    .map_err(|_| StoreFailure::Rename)
            })?;
            if replacement.is_err() {
                cleanup_with_authority(
                    authority,
                    paths,
                    &self.directory,
                    MARKER_NEXT_FILE_NAME,
                    scratch_identity,
                )?;
                return Err(operation_store(operation_kind, StoreFailure::Rename));
            }
            authority_checked(authority, paths, || {
                hit(
                    failpoints,
                    crate::failpoint::DurabilityFailpoint::MarkerAdvanceAfterReplace,
                )
            })?
            .map_err(|cause| operation_store(operation_kind, cause))?;
            authority_checked(authority, paths, || {
                hit(
                    failpoints,
                    crate::failpoint::DurabilityFailpoint::MarkerAdvanceBeforeDirectorySync,
                )
            })?
            .map_err(|cause| operation_store(operation_kind, cause))?;
            let parent_sync = authority_checked(authority, paths, || {
                operations
                    .sync_directory(&self.directory)
                    .map_err(|_| StoreFailure::Sync)
            })?;
            if parent_sync.is_err() {
                return Err(operation_store(operation_kind, StoreFailure::Sync));
            }
            authority_checked(authority, paths, || {
                hit(
                    failpoints,
                    crate::failpoint::DurabilityFailpoint::MarkerAdvanceAfterDirectorySync,
                )
            })?
            .map_err(|cause| operation_store(operation_kind, cause))?;
            let (marker_file, marker_identity, reread) =
                authority_checked(authority, paths, || {
                    let file = open_marker_file(&self.directory, MARKER_FILE_NAME)?;
                    let identity = file_identity(&file)?;
                    let marker = read_marker(&file)?;
                    Ok::<_, StoreFailure>((file, identity, marker))
                })?
                .map_err(|cause| operation_store(operation_kind, cause))?;
            if reread.canonical_bytes() != next_marker.canonical_bytes() {
                return Err(operation_store(operation_kind, StoreFailure::Conflict));
            }
            let binding = Self {
                directory: self.directory,
                directory_identity: self.directory_identity,
                marker_file,
                marker_identity,
                marker: next_marker,
            };
            authority_checked(authority, paths, || {
                binding.validate_inner(paths, true, operation_kind)
            })??;
            Ok(binding)
        }

        #[cfg(test)]
        pub(crate) fn test_advance_with_failure(
            self,
            paths: &ServiceSqlitePaths,
            authority: &WriterAuthority,
            next: RestoreRecoveryPhase,
            failure: TestStoreFailure,
        ) -> Result<Self, ServiceSqliteError> {
            self.advance_with_operations(
                paths,
                authority,
                next,
                &FailingStoreOperations { failure },
                ServiceSqliteErrorKind::Restore,
                &crate::failpoint::DurabilityFailpoints::default(),
            )
        }

        #[cfg(test)]
        pub(crate) fn test_advance_for_recovery_with_failure(
            self,
            paths: &ServiceSqlitePaths,
            authority: &WriterAuthority,
            next: RestoreRecoveryPhase,
            failure: TestStoreFailure,
        ) -> Result<Self, ServiceSqliteError> {
            self.advance_with_operations(
                paths,
                authority,
                next,
                &FailingStoreOperations { failure },
                ServiceSqliteErrorKind::Recovery,
                &crate::failpoint::DurabilityFailpoints::default(),
            )
        }

        pub(crate) const fn marker(&self) -> &RestoreRecoveryMarker {
            &self.marker
        }

        pub(crate) fn interrupted_transition(
            &self,
            paths: &ServiceSqlitePaths,
            authority: &WriterAuthority,
        ) -> Result<Option<RestoreRecoveryPhase>, ServiceSqliteError> {
            authority_checked(authority, paths, || {
                self.validate_inner(paths, false, ServiceSqliteErrorKind::Recovery)
            })??;
            let scratch = match authority_checked(authority, paths, || {
                open_marker_file(&self.directory, MARKER_NEXT_FILE_NAME)
            })? {
                Ok(file) => file,
                Err(StoreFailure::Missing) => return Ok(None),
                Err(cause) => return Err(recovery_store(cause)),
            };
            let scratch_marker = authority_checked(authority, paths, || read_marker(&scratch))?
                .map_err(recovery_store)?;
            let next = scratch_marker.phase();
            let expected = self.marker.transitioned_to(next);
            if next == self.marker.phase()
                || match expected {
                    Ok(expected) => expected.canonical_bytes() != scratch_marker.canonical_bytes(),
                    Err(_) => true,
                }
            {
                return Err(recovery_store(StoreFailure::Conflict));
            }
            Ok(Some(next))
        }

        pub(crate) fn promote_interrupted_transition(
            self,
            paths: &ServiceSqlitePaths,
            authority: &WriterAuthority,
            expected_phase: RestoreRecoveryPhase,
        ) -> Result<Self, ServiceSqliteError> {
            self.promote_interrupted_transition_with_hook(paths, authority, expected_phase, || {})
        }

        fn promote_interrupted_transition_with_hook(
            self,
            paths: &ServiceSqlitePaths,
            authority: &WriterAuthority,
            expected_phase: RestoreRecoveryPhase,
            before_exact_removal: impl FnOnce(),
        ) -> Result<Self, ServiceSqliteError> {
            authority.validate_for(paths)?;
            authority_checked(authority, paths, || {
                self.validate_inner(paths, false, ServiceSqliteErrorKind::Recovery)
            })??;
            let scratch = authority_checked(authority, paths, || {
                open_marker_file(&self.directory, MARKER_NEXT_FILE_NAME)
            })?
            .map_err(recovery_store)?;
            let scratch_identity = authority_checked(authority, paths, || file_identity(&scratch))?
                .map_err(recovery_store)?;
            let scratch_marker = authority_checked(authority, paths, || read_marker(&scratch))?
                .map_err(recovery_store)?;
            let expected = self
                .marker
                .transitioned_to(expected_phase)
                .map_err(recovery_contract)?;
            if scratch_marker.canonical_bytes() != expected.canonical_bytes() {
                return Err(recovery_store(StoreFailure::Conflict));
            }
            before_exact_removal();
            // Preserve the valid current marker even if the scratch pathname
            // was replaced after an interrupted advance. Remove only the
            // exact validated scratch, then recreate the governed transition.
            let removal = authority_checked(authority, paths, || {
                remove_exact_and_sync(&self.directory, MARKER_NEXT_FILE_NAME, scratch_identity)
            })?;
            removal.map_err(recovery_store)?;
            self.advance_for_recovery(paths, authority, expected_phase)
        }

        #[cfg(test)]
        pub(crate) fn test_promote_interrupted_transition_after_hook(
            self,
            paths: &ServiceSqlitePaths,
            authority: &WriterAuthority,
            expected_phase: RestoreRecoveryPhase,
            before_exact_removal: impl FnOnce(),
        ) -> Result<Self, ServiceSqliteError> {
            self.promote_interrupted_transition_with_hook(
                paths,
                authority,
                expected_phase,
                before_exact_removal,
            )
        }

        pub(crate) fn retire(
            self,
            paths: &ServiceSqlitePaths,
            authority: &WriterAuthority,
        ) -> Result<(), ServiceSqliteError> {
            authority.validate_for(paths)?;
            authority_checked(authority, paths, || {
                self.validate_inner(paths, true, ServiceSqliteErrorKind::Recovery)
            })??;
            authority_checked(authority, paths, || {
                remove_exact_and_sync(&self.directory, MARKER_FILE_NAME, self.marker_identity)
            })?
            .map_err(recovery_store)
        }

        pub(crate) fn validate(
            &self,
            paths: &ServiceSqlitePaths,
        ) -> Result<(), ServiceSqliteError> {
            self.validate_inner(paths, true, ServiceSqliteErrorKind::Recovery)
        }

        fn validate_for_restore(
            &self,
            paths: &ServiceSqlitePaths,
        ) -> Result<(), ServiceSqliteError> {
            self.validate_inner(paths, true, ServiceSqliteErrorKind::Restore)
        }

        fn validate_inner(
            &self,
            paths: &ServiceSqlitePaths,
            require_no_scratch: bool,
            operation_kind: ServiceSqliteErrorKind,
        ) -> Result<(), ServiceSqliteError> {
            let layout = RestoreRecoveryLayout::for_paths(paths)
                .map_err(|cause| operation_contract(operation_kind, cause))?;
            let current_directory = open(
                &layout.state_directory,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| authority_store(StoreFailure::Directory))?;
            let current_directory = File::from(current_directory);
            if validate_directory(&self.directory).map_err(authority_store)?
                != self.directory_identity
                || validate_directory(&current_directory).map_err(authority_store)?
                    != self.directory_identity
            {
                return Err(authority_store(StoreFailure::Conflict));
            }
            let current_marker = open_marker_file(&current_directory, MARKER_FILE_NAME)
                .map_err(|cause| operation_store(operation_kind, cause))?;
            if file_identity(&self.marker_file)
                .map_err(|cause| operation_store(operation_kind, cause))?
                != self.marker_identity
                || file_identity(&current_marker)
                    .map_err(|cause| operation_store(operation_kind, cause))?
                    != self.marker_identity
            {
                return Err(operation_store(operation_kind, StoreFailure::Conflict));
            }
            if read_marker(&self.marker_file)
                .map_err(|cause| operation_store(operation_kind, cause))?
                .canonical_bytes()
                != self.marker.canonical_bytes()
            {
                return Err(operation_store(operation_kind, StoreFailure::Conflict));
            }
            if require_no_scratch {
                require_absent(&current_directory, MARKER_NEXT_FILE_NAME)
                    .map_err(|cause| operation_store(operation_kind, cause))?;
            }
            Ok(())
        }
    }

    fn validate_directory(file: &File) -> Result<FileIdentity, StoreFailure> {
        let status = fstat(file).map_err(|_| StoreFailure::Directory)?;
        if !FileType::from_raw_mode(status.st_mode).is_dir()
            || status.st_uid != geteuid().as_raw()
            || crate::native_metadata::mode(status.st_mode) & 0o022 != 0
        {
            return Err(StoreFailure::Directory);
        }
        identity(status.st_dev, status.st_ino)
    }

    fn authority_checked<T, E>(
        authority: &WriterAuthority,
        paths: &ServiceSqlitePaths,
        operation: impl FnOnce() -> Result<T, E>,
    ) -> Result<Result<T, E>, ServiceSqliteError> {
        authority.validate_for(paths)?;
        let result = operation();
        authority.validate_for(paths)?;
        Ok(result)
    }

    fn cleanup_with_authority(
        authority: &WriterAuthority,
        paths: &ServiceSqlitePaths,
        directory: &File,
        name: &str,
        expected: FileIdentity,
    ) -> Result<(), ServiceSqliteError> {
        authority.validate_for(paths)?;
        cleanup_exact(directory, name, expected);
        authority.validate_for(paths)
    }

    fn create_marker_file(directory: &File, name: &str) -> Result<File, StoreFailure> {
        let descriptor = openat(
            directory,
            name,
            OFlags::RDWR
                | OFlags::CREATE
                | OFlags::EXCL
                | OFlags::NOFOLLOW
                | OFlags::CLOEXEC
                | OFlags::NONBLOCK,
            Mode::RUSR | Mode::WUSR,
        )
        .map_err(|_| StoreFailure::Collision)?;
        fchmod(&descriptor, Mode::RUSR | Mode::WUSR).map_err(|_| StoreFailure::Permissions)?;
        let file = File::from(descriptor);
        let _ = file_identity(&file)?;
        Ok(file)
    }

    fn open_marker_file(directory: &File, name: &str) -> Result<File, StoreFailure> {
        let descriptor = openat(
            directory,
            name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::empty(),
        )
        .map_err(|error| {
            if error == Errno::NOENT {
                StoreFailure::Missing
            } else {
                StoreFailure::Marker
            }
        })?;
        let file = File::from(descriptor);
        let _ = file_identity(&file)?;
        Ok(file)
    }

    fn file_identity(file: &File) -> Result<FileIdentity, StoreFailure> {
        let status = fstat(file).map_err(|_| StoreFailure::Marker)?;
        if !FileType::from_raw_mode(status.st_mode).is_file()
            || crate::native_metadata::link_count(status.st_nlink) != 1
            || status.st_uid != geteuid().as_raw()
            || crate::native_metadata::mode(status.st_mode) & 0o777 != 0o600
        {
            return Err(StoreFailure::Marker);
        }
        identity(status.st_dev, status.st_ino)
    }

    fn identity(device: impl TryInto<u64>, inode: u64) -> Result<FileIdentity, StoreFailure> {
        Ok(FileIdentity {
            device: device.try_into().map_err(|_| StoreFailure::Marker)?,
            inode,
        })
    }

    trait StoreOperations {
        fn sync_file(&self, file: &File, directory: &File) -> std::io::Result<()>;
        fn sync_directory(&self, directory: &File) -> std::io::Result<()>;
        fn replace_marker(&self, directory: &File) -> std::io::Result<()>;
    }

    struct SystemStoreOperations;

    impl StoreOperations for SystemStoreOperations {
        fn sync_file(&self, file: &File, _directory: &File) -> std::io::Result<()> {
            file.sync_all()
        }

        fn sync_directory(&self, directory: &File) -> std::io::Result<()> {
            directory.sync_all()
        }

        fn replace_marker(&self, directory: &File) -> std::io::Result<()> {
            renameat(
                directory,
                MARKER_NEXT_FILE_NAME,
                directory,
                MARKER_FILE_NAME,
            )
            .map_err(std::io::Error::from)
        }
    }

    #[cfg(test)]
    struct AuthorityDriftAfterSyncStoreOperations;

    #[cfg(test)]
    impl StoreOperations for AuthorityDriftAfterSyncStoreOperations {
        fn sync_file(&self, file: &File, _directory: &File) -> std::io::Result<()> {
            file.sync_all()
        }

        fn sync_directory(&self, directory: &File) -> std::io::Result<()> {
            directory.sync_all()?;
            fchmod(
                directory,
                Mode::RUSR | Mode::WUSR | Mode::XUSR | Mode::RGRP | Mode::WGRP | Mode::XGRP,
            )
            .map_err(std::io::Error::from)
        }

        fn replace_marker(&self, directory: &File) -> std::io::Result<()> {
            SystemStoreOperations.replace_marker(directory)
        }
    }

    #[cfg(test)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(crate) enum TestStoreFailure {
        ScratchSync,
        ParentSyncAfterRename,
        AuthorityDriftAndScratchSync,
    }

    #[cfg(test)]
    struct FailingStoreOperations {
        failure: TestStoreFailure,
    }

    #[cfg(test)]
    impl StoreOperations for FailingStoreOperations {
        fn sync_file(&self, file: &File, directory: &File) -> std::io::Result<()> {
            match self.failure {
                TestStoreFailure::ScratchSync => {
                    Err(std::io::Error::other("injected scratch sync failure"))
                }
                TestStoreFailure::AuthorityDriftAndScratchSync => {
                    fchmod(
                        directory,
                        Mode::RUSR | Mode::WUSR | Mode::XUSR | Mode::RGRP | Mode::WGRP | Mode::XGRP,
                    )
                    .map_err(std::io::Error::from)?;
                    Err(std::io::Error::other(
                        "injected authority drift and scratch sync failure",
                    ))
                }
                TestStoreFailure::ParentSyncAfterRename => file.sync_all(),
            }
        }

        fn sync_directory(&self, _directory: &File) -> std::io::Result<()> {
            Err(std::io::Error::other("injected parent sync failure"))
        }

        fn replace_marker(&self, directory: &File) -> std::io::Result<()> {
            SystemStoreOperations.replace_marker(directory)
        }
    }

    fn write_and_sync(
        file: &File,
        bytes: &[u8],
        directory: &File,
        operations: &dyn StoreOperations,
        failpoints: &crate::failpoint::DurabilityFailpoints,
        before_sync: Option<crate::failpoint::DurabilityFailpoint>,
        after_sync: Option<crate::failpoint::DurabilityFailpoint>,
    ) -> Result<(), StoreFailure> {
        let mut file = file.try_clone().map_err(|_| StoreFailure::Write)?;
        file.write_all(bytes).map_err(|_| StoreFailure::Write)?;
        if let Some(before_sync) = before_sync {
            hit(failpoints, before_sync)?;
        }
        operations
            .sync_file(&file, directory)
            .map_err(|_| StoreFailure::Sync)?;
        if let Some(after_sync) = after_sync {
            hit(failpoints, after_sync)?;
        }
        Ok(())
    }

    fn hit(
        failpoints: &crate::failpoint::DurabilityFailpoints,
        point: crate::failpoint::DurabilityFailpoint,
    ) -> Result<(), StoreFailure> {
        failpoints.hit(point).map_err(|_| StoreFailure::Injected)
    }

    fn read_marker(file: &File) -> Result<RestoreRecoveryMarker, StoreFailure> {
        let mut file = file.try_clone().map_err(|_| StoreFailure::Read)?;
        file.seek(SeekFrom::Start(0))
            .map_err(|_| StoreFailure::Read)?;
        let mut bytes = Vec::with_capacity(RESTORE_MARKER_MAX_BYTES.min(512));
        file.take(u64::try_from(RESTORE_MARKER_MAX_BYTES + 1).expect("marker bound"))
            .read_to_end(&mut bytes)
            .map_err(|_| StoreFailure::Read)?;
        if bytes.len() > RESTORE_MARKER_MAX_BYTES {
            return Err(StoreFailure::Contract(
                RestoreMarkerContractError::MarkerTooLarge,
            ));
        }
        RestoreRecoveryMarker::from_canonical_bytes(&bytes).map_err(StoreFailure::Contract)
    }

    fn require_absent(directory: &File, name: &str) -> Result<(), StoreFailure> {
        match statat(directory, name, AtFlags::SYMLINK_NOFOLLOW) {
            Err(error) if error == Errno::NOENT => Ok(()),
            _ => Err(StoreFailure::Collision),
        }
    }

    fn cleanup_exact(directory: &File, name: &str, expected: FileIdentity) {
        let Ok(file) = open_marker_file(directory, name) else {
            return;
        };
        if file_identity(&file).ok() == Some(expected) {
            let _ = unlinkat(directory, name, AtFlags::empty());
            let _ = directory.sync_all();
        }
    }

    fn remove_exact_and_sync(
        directory: &File,
        name: &str,
        expected: FileIdentity,
    ) -> Result<(), StoreFailure> {
        let current = open_marker_file(directory, name)?;
        if file_identity(&current)? != expected {
            return Err(StoreFailure::Conflict);
        }
        unlinkat(directory, name, AtFlags::empty()).map_err(|_| StoreFailure::Conflict)?;
        directory.sync_all().map_err(|_| StoreFailure::Sync)?;
        require_absent(directory, name)
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum StoreFailure {
        Directory,
        Marker,
        Missing,
        Collision,
        Permissions,
        Read,
        Write,
        Sync,
        Rename,
        Conflict,
        Injected,
        Contract(RestoreMarkerContractError),
    }

    impl fmt::Display for StoreFailure {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str(match self {
                Self::Directory => "restore marker directory is invalid",
                Self::Marker => "restore marker file is invalid",
                Self::Missing => "restore marker file is missing",
                Self::Collision => "restore marker artifact already exists",
                Self::Permissions => "restore marker permissions are invalid",
                Self::Read => "restore marker could not be read",
                Self::Write => "restore marker could not be written",
                Self::Sync => "restore marker durability could not be proven",
                Self::Rename => "restore marker replacement failed",
                Self::Conflict => "restore marker binding changed",
                Self::Injected => "restore marker durability boundary failed",
                Self::Contract(error) => return error.fmt(formatter),
            })
        }
    }
    impl Error for StoreFailure {}

    fn restore_store(cause: StoreFailure) -> ServiceSqliteError {
        ServiceSqliteError::with_source(ServiceSqliteErrorKind::Restore, cause)
    }
    fn recovery_store(cause: StoreFailure) -> ServiceSqliteError {
        ServiceSqliteError::with_source(ServiceSqliteErrorKind::Recovery, cause)
    }
    fn authority_store(cause: StoreFailure) -> ServiceSqliteError {
        ServiceSqliteError::with_source(ServiceSqliteErrorKind::Authority, cause)
    }
    fn restore_contract(cause: RestoreMarkerContractError) -> ServiceSqliteError {
        ServiceSqliteError::with_source(ServiceSqliteErrorKind::Restore, cause)
    }
    fn recovery_contract(cause: RestoreMarkerContractError) -> ServiceSqliteError {
        ServiceSqliteError::with_source(ServiceSqliteErrorKind::Recovery, cause)
    }
    fn operation_store(kind: ServiceSqliteErrorKind, cause: StoreFailure) -> ServiceSqliteError {
        debug_assert!(matches!(
            kind,
            ServiceSqliteErrorKind::Restore | ServiceSqliteErrorKind::Recovery
        ));
        ServiceSqliteError::with_source(kind, cause)
    }
    fn operation_contract(
        kind: ServiceSqliteErrorKind,
        cause: RestoreMarkerContractError,
    ) -> ServiceSqliteError {
        debug_assert!(matches!(
            kind,
            ServiceSqliteErrorKind::Restore | ServiceSqliteErrorKind::Recovery
        ));
        ServiceSqliteError::with_source(kind, cause)
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) use store::RestoreMarkerBinding;
#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
pub(crate) use store::TestStoreFailure;

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_runtime_paths::{
        InstanceId, RadrootsHostEnvironment, RadrootsPathProfile, RadrootsPathResolver,
        RadrootsPlatform, RuntimeContext, RuntimeContextBootstrap, RuntimeContextSource, ServiceId,
    };

    fn paths(root: &Path) -> ServiceSqlitePaths {
        let context = RuntimeContext::resolve(
            &RadrootsPathResolver::new(RadrootsPlatform::Linux, RadrootsHostEnvironment::default()),
            RuntimeContextBootstrap::new(
                RadrootsPathProfile::RepoLocal,
                Some(root.to_path_buf()),
                RuntimeContextSource::BootstrapCli,
                RuntimeContextSource::BootstrapCli,
            )
            .expect("bootstrap"),
            ServiceId::new("myc").expect("service"),
            InstanceId::new("primary").expect("instance"),
        )
        .expect("context");
        ServiceSqlitePaths::from_runtime_context(&context).expect("paths")
    }

    fn marker(paths: &ServiceSqlitePaths) -> RestoreRecoveryMarker {
        let metadata = ServiceDatabaseMetadata::new(
            paths,
            SourceGeneration::new([7; 32]).expect("generation"),
            NonZeroU32::new(3).expect("schema"),
            1_800_000_000_000,
            ServiceSqliteApplicationId::new(0x5244_5254).expect("application"),
        )
        .expect("metadata");
        RestoreRecoveryMarker::prepared(
            &metadata,
            BackupManifestSha256::from_bytes([8; 32]),
            RestoreArtifactExpectation::new(1, 2, 4096, [3; 32]).expect("live"),
            RestoreArtifactExpectation::new(1, 4, 4096, [5; 32]).expect("staged"),
        )
        .expect("marker")
    }

    #[test]
    fn canonical_vector_and_checksum_are_frozen() {
        let root = tempfile::tempdir().expect("root");
        let marker = marker(&paths(root.path()));
        let text = std::str::from_utf8(marker.canonical_bytes()).expect("UTF-8");
        assert_eq!(marker.canonical_bytes().len(), 820);
        assert!(text.starts_with("{\"schema\":\"radroots.service-sqlite.restore-marker\",\"schema_version\":1,\"phase\":\"prepared\""));
        assert!(text.ends_with("\"marker_sha256\":\"026e975f22d45df3b4f46d4d3958e82e3755f7cd2f17d742714e2b55bf381d0f\"}"));
        assert_eq!(
            RestoreRecoveryMarker::from_canonical_bytes(marker.canonical_bytes()).expect("parse"),
            marker
        );
    }

    #[test]
    fn prepared_marker_uses_actual_backup_schema_not_a_binary_ceiling() {
        let root = tempfile::tempdir().expect("root");
        let paths = paths(root.path());
        let actual = ServiceDatabaseMetadata::new(
            &paths,
            SourceGeneration::new([7; 32]).expect("generation"),
            NonZeroU32::new(1).expect("actual schema"),
            1_800_000_000_000,
            ServiceSqliteApplicationId::new(0x5244_5254).expect("application"),
        )
        .expect("metadata");
        let marker = RestoreRecoveryMarker::prepared(
            &actual,
            BackupManifestSha256::from_bytes([8; 32]),
            RestoreArtifactExpectation::new(1, 2, 4096, [3; 32]).expect("live"),
            RestoreArtifactExpectation::new(1, 4, 4096, [5; 32]).expect("staged"),
        )
        .expect("marker");
        let text = std::str::from_utf8(marker.canonical_bytes()).expect("text");
        assert!(text.contains("\"state_schema_version\":1"));
        assert!(!text.contains("\"state_schema_version\":3"));
    }

    #[test]
    fn all_phase_edges_and_idempotent_bytes_are_exact() {
        let root = tempfile::tempdir().expect("root");
        let prepared = marker(&paths(root.path()));
        let retained = prepared
            .transitioned_to(RestoreRecoveryPhase::LiveRetained)
            .expect("advance");
        let installed = retained
            .transitioned_to(RestoreRecoveryPhase::ReplacementInstalled)
            .expect("advance");
        let phases = [prepared, retained, installed];
        for current in &phases {
            for next in [
                RestoreRecoveryPhase::Prepared,
                RestoreRecoveryPhase::LiveRetained,
                RestoreRecoveryPhase::ReplacementInstalled,
            ] {
                let result = current.transitioned_to(next);
                let allowed = current.phase() == next
                    || matches!(
                        (current.phase(), next),
                        (
                            RestoreRecoveryPhase::Prepared,
                            RestoreRecoveryPhase::LiveRetained
                        ) | (
                            RestoreRecoveryPhase::LiveRetained,
                            RestoreRecoveryPhase::ReplacementInstalled
                        )
                    );
                assert_eq!(
                    result.is_ok(),
                    allowed,
                    "edge {:?}->{next:?}",
                    current.phase()
                );
                if current.phase() == next {
                    assert_eq!(
                        result.expect("same").canonical_bytes(),
                        current.canonical_bytes()
                    );
                }
            }
        }
    }

    #[test]
    fn strict_codec_rejects_tamper_and_noncanonical_inputs() {
        let root = tempfile::tempdir().expect("root");
        let marker = marker(&paths(root.path()));
        let text = std::str::from_utf8(marker.canonical_bytes()).expect("text");
        for altered in [
            format!(" {text}"),
            text.replace("\"schema_version\":1", "\"schema_version\":2"),
            text.replace("\"phase\":\"prepared\"", "\"phase\":null"),
            text.replace(
                "\"service\":\"myc\"",
                "\"service\":\"myc\",\"service\":\"myc\"",
            ),
            text.replace(
                "\"schema_version\":1,\"phase\"",
                "\"unknown\":1,\"schema_version\":1,\"phase\"",
            ),
            text.replace("\"device\":1,\"inode\":2", "\"inode\":2,\"device\":1"),
            text.replace("\"marker_sha256\":\"0", "\"marker_sha256\":\"A"),
        ] {
            assert!(
                RestoreRecoveryMarker::from_canonical_bytes(altered.as_bytes()).is_err(),
                "accepted {altered}"
            );
        }
        assert_eq!(
            RestoreRecoveryMarker::from_canonical_bytes(&vec![b'x'; RESTORE_MARKER_MAX_BYTES + 1]),
            Err(RestoreMarkerContractError::MarkerTooLarge)
        );
        assert_ne!(
            RestoreRecoveryMarker::from_canonical_bytes(&vec![b'x'; RESTORE_MARKER_MAX_BYTES]),
            Err(RestoreMarkerContractError::MarkerTooLarge)
        );
    }

    #[test]
    fn identity_bounds_layout_and_debug_are_closed() {
        assert!(RestoreArtifactExpectation::new(0, 0, 1, [0; 32]).is_ok());
        assert!(RestoreArtifactExpectation::new(u64::MAX, u64::MAX, 1, [0; 32]).is_ok());
        assert!(RestoreArtifactExpectation::new(1, 1, 0, [0; 32]).is_err());
        assert!(RestoreArtifactExpectation::new(1, 1, i64::MAX as u64 + 1, [0; 32]).is_err());
        let root = tempfile::tempdir().expect("root");
        let paths = paths(root.path());
        let layout = RestoreRecoveryLayout::for_paths(&paths).expect("layout");
        assert_eq!(
            layout.file_names(),
            [
                LIVE_FILE_NAME,
                STAGED_FILE_NAME,
                BACKUP_FILE_NAME,
                MARKER_FILE_NAME,
                MARKER_NEXT_FILE_NAME
            ]
        );
        assert!(layout.live.starts_with(&layout.state_directory));
        assert!(layout.staged.starts_with(&layout.state_directory));
        assert!(layout.backup.starts_with(&layout.state_directory));
        assert_eq!(format!("{layout:?}"), "RestoreRecoveryLayout([redacted])");
        assert_eq!(
            format!("{:?}", marker(&paths)),
            "RestoreRecoveryMarker { schema_version: 1, phase: Prepared, .. }"
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn durable_store_creates_loads_and_advances_owner_only_marker() {
        use std::{fs, os::unix::fs::PermissionsExt};
        let root = tempfile::tempdir().expect("root");
        let paths = paths(root.path());
        fs::create_dir_all(paths.state_database().parent().expect("parent")).expect("state dir");
        fs::set_permissions(
            paths.state_database().parent().expect("parent"),
            fs::Permissions::from_mode(0o700),
        )
        .expect("mode");
        let authority = WriterAuthority::acquire(&paths, crate::OpenMode::Initialize)
            .expect("authority")
            .expect("writer");
        let prepared = marker(&paths);
        let binding = RestoreMarkerBinding::create(&paths, &authority, &prepared).expect("create");
        assert_eq!(
            fs::metadata(paths.state_database().with_file_name(MARKER_FILE_NAME))
                .expect("metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        binding.validate(&paths).expect("validate");
        drop(binding);
        let loaded = RestoreMarkerBinding::load(&paths)
            .expect("load")
            .expect("present");
        assert_eq!(loaded.marker().phase(), RestoreRecoveryPhase::Prepared);
        let loaded = loaded
            .advance(&paths, &authority, RestoreRecoveryPhase::LiveRetained)
            .expect("advance");
        assert_eq!(loaded.marker().phase(), RestoreRecoveryPhase::LiveRetained);
        let same = loaded
            .advance(&paths, &authority, RestoreRecoveryPhase::LiveRetained)
            .expect("same");
        assert_eq!(same.marker().phase(), RestoreRecoveryPhase::LiveRetained);
        assert!(
            !paths
                .state_database()
                .with_file_name(MARKER_NEXT_FILE_NAME)
                .exists()
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn atomic_advance_failures_leave_exact_old_or_new_valid_marker() {
        use super::store::TestStoreFailure;
        use std::{fs, os::unix::fs::PermissionsExt};

        for (instance, failure, expected_phase) in [
            (
                "pre-rename",
                TestStoreFailure::ScratchSync,
                RestoreRecoveryPhase::Prepared,
            ),
            (
                "post-rename",
                TestStoreFailure::ParentSyncAfterRename,
                RestoreRecoveryPhase::LiveRetained,
            ),
        ] {
            let root = tempfile::tempdir().expect("root");
            let paths = paths(&root.path().join(instance));
            let parent = paths.state_database().parent().expect("parent");
            fs::create_dir_all(parent).expect("state dir");
            fs::set_permissions(parent, fs::Permissions::from_mode(0o700)).expect("mode");
            let authority = WriterAuthority::acquire(&paths, crate::OpenMode::Initialize)
                .expect("authority")
                .expect("writer");
            let binding =
                RestoreMarkerBinding::create(&paths, &authority, &marker(&paths)).expect("create");
            assert_eq!(
                binding
                    .test_advance_with_failure(
                        &paths,
                        &authority,
                        RestoreRecoveryPhase::LiveRetained,
                        failure,
                    )
                    .expect_err("injected failure")
                    .kind(),
                ServiceSqliteErrorKind::Restore
            );
            let recovered = RestoreMarkerBinding::load(&paths)
                .expect("read valid durable state")
                .expect("marker remains");
            assert_eq!(recovered.marker().phase(), expected_phase);
            assert!(
                !paths
                    .state_database()
                    .with_file_name(MARKER_NEXT_FILE_NAME)
                    .exists()
            );
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn initial_store_requires_prepared_and_authority_wins_combined_failure() {
        use super::store::TestStoreFailure;
        use std::{fs, os::unix::fs::PermissionsExt};

        for phase in [
            RestoreRecoveryPhase::LiveRetained,
            RestoreRecoveryPhase::ReplacementInstalled,
        ] {
            let root = tempfile::tempdir().expect("root");
            let paths = paths(root.path());
            let parent = paths.state_database().parent().expect("parent");
            fs::create_dir_all(parent).expect("state dir");
            fs::set_permissions(parent, fs::Permissions::from_mode(0o700)).expect("mode");
            let authority = WriterAuthority::acquire(&paths, crate::OpenMode::Initialize)
                .expect("authority")
                .expect("writer");
            let later = marker(&paths)
                .transitioned_to(RestoreRecoveryPhase::LiveRetained)
                .expect("retained");
            let later = if phase == RestoreRecoveryPhase::ReplacementInstalled {
                later.transitioned_to(phase).expect("installed")
            } else {
                later
            };
            assert_eq!(
                RestoreMarkerBinding::create(&paths, &authority, &later)
                    .expect_err("initial later phase")
                    .kind(),
                ServiceSqliteErrorKind::Restore
            );
            assert!(!parent.join(MARKER_FILE_NAME).exists());
        }

        let root = tempfile::tempdir().expect("root");
        let paths = paths(root.path());
        let parent = paths.state_database().parent().expect("parent");
        fs::create_dir_all(parent).expect("state dir");
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700)).expect("mode");
        let authority = WriterAuthority::acquire(&paths, crate::OpenMode::Initialize)
            .expect("authority")
            .expect("writer");
        let binding =
            RestoreMarkerBinding::create(&paths, &authority, &marker(&paths)).expect("create");
        assert_eq!(
            binding
                .test_advance_with_failure(
                    &paths,
                    &authority,
                    RestoreRecoveryPhase::LiveRetained,
                    TestStoreFailure::AuthorityDriftAndScratchSync,
                )
                .expect_err("authority drift")
                .kind(),
            ServiceSqliteErrorKind::Authority
        );
        assert!(parent.join(MARKER_NEXT_FILE_NAME).exists());
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn active_marker_conflict_is_restore_and_directory_replacement_is_authority() {
        use std::{fs, os::unix::fs::PermissionsExt};

        let root = tempfile::tempdir().expect("root");
        let marker_paths = paths(&root.path().join("marker-conflict"));
        let parent = marker_paths.state_database().parent().expect("parent");
        fs::create_dir_all(parent).expect("state dir");
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700)).expect("mode");
        let authority = WriterAuthority::acquire(&marker_paths, crate::OpenMode::Initialize)
            .expect("authority")
            .expect("writer");
        let binding =
            RestoreMarkerBinding::create(&marker_paths, &authority, &marker(&marker_paths))
                .expect("create");
        let marker_path = parent.join(MARKER_FILE_NAME);
        let replacement_bytes = fs::read(&marker_path).expect("marker bytes");
        fs::rename(&marker_path, parent.join("retained-marker")).expect("retain marker");
        fs::write(&marker_path, replacement_bytes).expect("replacement marker");
        fs::set_permissions(&marker_path, fs::Permissions::from_mode(0o600)).expect("mode");
        assert_eq!(
            binding
                .advance(
                    &marker_paths,
                    &authority,
                    RestoreRecoveryPhase::LiveRetained,
                )
                .expect_err("marker replacement")
                .kind(),
            ServiceSqliteErrorKind::Restore
        );

        let root = tempfile::tempdir().expect("root");
        let paths = paths(&root.path().join("directory-conflict"));
        let parent = paths
            .state_database()
            .parent()
            .expect("parent")
            .to_path_buf();
        fs::create_dir_all(&parent).expect("state dir");
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).expect("mode");
        let authority = WriterAuthority::acquire(&paths, crate::OpenMode::Initialize)
            .expect("authority")
            .expect("writer");
        let binding =
            RestoreMarkerBinding::create(&paths, &authority, &marker(&paths)).expect("create");
        let retained_parent = parent.with_file_name("state-retained");
        fs::rename(&parent, &retained_parent).expect("retain directory");
        fs::create_dir(&parent).expect("replacement directory");
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).expect("mode");
        assert_eq!(
            binding
                .advance(&paths, &authority, RestoreRecoveryPhase::LiveRetained,)
                .expect_err("directory replacement")
                .kind(),
            ServiceSqliteErrorKind::Authority
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn load_rejects_stale_scratch_hardlinks_and_wrong_mode() {
        use std::{fs, os::unix::fs::PermissionsExt};

        for shape in ["stale-next", "hardlink", "wrong-mode"] {
            let root = tempfile::tempdir().expect("root");
            let paths = paths(root.path());
            let parent = paths.state_database().parent().expect("parent");
            fs::create_dir_all(parent).expect("state dir");
            fs::set_permissions(parent, fs::Permissions::from_mode(0o700)).expect("mode");
            let authority = WriterAuthority::acquire(&paths, crate::OpenMode::Initialize)
                .expect("authority")
                .expect("writer");
            drop(
                RestoreMarkerBinding::create(&paths, &authority, &marker(&paths)).expect("create"),
            );
            let marker_path = parent.join(MARKER_FILE_NAME);
            match shape {
                "stale-next" => {
                    fs::write(parent.join(MARKER_NEXT_FILE_NAME), b"evidence").expect("stale next")
                }
                "hardlink" => {
                    fs::hard_link(&marker_path, parent.join("retained-link")).expect("hard link");
                }
                "wrong-mode" => {
                    fs::set_permissions(&marker_path, fs::Permissions::from_mode(0o640))
                        .expect("mode");
                }
                _ => unreachable!(),
            }
            assert_eq!(
                RestoreMarkerBinding::load(&paths)
                    .expect_err("invalid artifact")
                    .kind(),
                ServiceSqliteErrorKind::Recovery
            );
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn store_rejects_collisions_shapes_tamper_and_insecure_parent() {
        use std::{
            fs,
            os::unix::fs::{PermissionsExt, symlink},
        };
        for shape in ["file", "directory", "symlink"] {
            let root = tempfile::tempdir().expect("root");
            let paths = paths(root.path());
            let parent = paths.state_database().parent().expect("parent");
            fs::create_dir_all(parent).expect("parent");
            fs::set_permissions(parent, fs::Permissions::from_mode(0o700)).expect("mode");
            let marker_path = parent.join(MARKER_FILE_NAME);
            match shape {
                "file" => fs::write(&marker_path, b"collision").expect("file"),
                "directory" => fs::create_dir(&marker_path).expect("directory"),
                "symlink" => symlink(parent.join("missing"), &marker_path).expect("symlink"),
                _ => unreachable!(),
            }
            let authority = WriterAuthority::acquire(&paths, crate::OpenMode::Initialize)
                .expect("authority")
                .expect("writer");
            assert_eq!(
                RestoreMarkerBinding::create(&paths, &authority, &marker(&paths))
                    .expect_err("collision")
                    .kind(),
                ServiceSqliteErrorKind::Restore
            );
        }

        let root = tempfile::tempdir().expect("root");
        let paths = paths(root.path());
        let parent = paths.state_database().parent().expect("parent");
        fs::create_dir_all(parent).expect("parent");
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700)).expect("mode");
        let authority = WriterAuthority::acquire(&paths, crate::OpenMode::Initialize)
            .expect("authority")
            .expect("writer");
        drop(RestoreMarkerBinding::create(&paths, &authority, &marker(&paths)).expect("create"));
        fs::write(parent.join(MARKER_FILE_NAME), b"tampered").expect("tamper");
        assert_eq!(
            RestoreMarkerBinding::load(&paths)
                .expect_err("tamper")
                .kind(),
            ServiceSqliteErrorKind::Recovery
        );
        fs::remove_file(parent.join(MARKER_FILE_NAME)).expect("remove");
        fs::set_permissions(parent, fs::Permissions::from_mode(0o722)).expect("insecure");
        assert_eq!(
            RestoreMarkerBinding::load(&paths)
                .expect_err("directory")
                .kind(),
            ServiceSqliteErrorKind::Recovery
        );
    }

    #[test]
    fn marker_errors_and_debug_are_path_content_and_digest_free() {
        let root = tempfile::tempdir().expect("secret-root");
        let paths = paths(root.path());
        let marker = marker(&paths);
        let debug = format!("{marker:?}");
        for sensitive in [
            "secret-root",
            "state.sqlite",
            "myc",
            "primary",
            "07070707",
            "08080808",
        ] {
            assert!(!debug.contains(sensitive));
        }
        for error in [
            RestoreMarkerContractError::MarkerTooLarge,
            RestoreMarkerContractError::MalformedEncoding,
            RestoreMarkerContractError::NonCanonicalEncoding,
            RestoreMarkerContractError::EncodingFailure,
            RestoreMarkerContractError::UnsupportedValue,
            RestoreMarkerContractError::ChecksumMismatch,
            RestoreMarkerContractError::InvalidIdentity,
            RestoreMarkerContractError::InvalidLayout,
            RestoreMarkerContractError::IllegalTransition,
        ] {
            let rendered = format!("{error:?} {error}");
            assert!(rendered.is_ascii());
            assert!(!rendered.contains('/'));
            assert!(!rendered.contains(".sqlite"));
        }
    }
}
