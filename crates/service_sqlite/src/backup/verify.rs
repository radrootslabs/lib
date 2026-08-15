//! Untrusted backup bundle verification.

use core::{fmt, num::NonZeroU64};
use std::path::Path;

use crate::{
    BackupManifestSha256, ServiceBackupManifest, ServiceDatabaseIdentity, ServiceDatabaseMetadata,
    ServiceSqliteError, ServiceSqliteErrorKind,
};

#[cfg(any(target_os = "linux", target_os = "macos"))]
use {
    core::num::NonZeroU32,
    radroots_runtime_paths::{InstanceId, ServiceId},
    radroots_storage::event::SourceGeneration,
    rustix::{
        fs::{Dir, FileType, Mode, OFlags, fstat, open, openat},
        process::geteuid,
    },
    sha2::{Digest, Sha256},
    sqlx::{ConnectOptions, Connection as _, Row, SqliteConnection, sqlite::SqliteConnectOptions},
    std::{
        error::Error,
        fs::File,
        io::{Read, Seek, SeekFrom},
        os::fd::AsRawFd,
        os::unix::ffi::OsStrExt,
        path::PathBuf,
    },
};

#[cfg(any(target_os = "linux", target_os = "macos"))]
const MAX_BUNDLE_PATH_BYTES: usize = 4_096;
#[cfg(any(target_os = "linux", target_os = "macos"))]
const HASH_BUFFER_BYTES: usize = 64 * 1_024;

/// Non-forgeable proof that one retained backup member passed v1 verification.
///
/// The proof is bound to retained directory and file descriptors, not only to
/// their pathnames. It is not restore or replacement authority. Later restore
/// work must copy from the retained member and reverify the staged copy.
///
/// External callers cannot construct the proof or obtain its raw handles:
///
/// ```compile_fail
/// use radroots_service_sqlite::VerifiedServiceBackup;
/// let _forged = VerifiedServiceBackup {};
/// ```
///
/// ```compile_fail
/// # fn inspect(proof: &radroots_service_sqlite::VerifiedServiceBackup) {
/// let _raw = proof.state_file();
/// # }
/// ```
pub struct VerifiedServiceBackup {
    manifest: ServiceBackupManifest,
    database_metadata: ServiceDatabaseMetadata,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    binding: VerifiedBundleBinding,
}

impl VerifiedServiceBackup {
    /// Returns the exact canonical manifest bound by this proof.
    #[must_use]
    pub const fn manifest(&self) -> &ServiceBackupManifest {
        &self.manifest
    }

    /// Returns the actual validated application metadata read from the member.
    #[must_use]
    pub const fn database_metadata(&self) -> &ServiceDatabaseMetadata {
        &self.database_metadata
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[allow(dead_code)]
    pub(crate) fn state_file(&self) -> &File {
        &self.binding.state
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[allow(dead_code)]
    pub(crate) fn validate_binding(&self) -> Result<(), ServiceSqliteError> {
        self.binding.validate()
    }
}

impl fmt::Debug for VerifiedServiceBackup {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedServiceBackup")
            .field("service", &"[redacted]")
            .field("instance", &"[redacted]")
            .field("manifest_digest", &"[redacted]")
            .field(
                "state_schema_version",
                &self.database_metadata.state_schema_version(),
            )
            .field("member", &"[retained]")
            .finish()
    }
}

/// Verifies untrusted canonical manifest bytes and their exact singleton bundle.
///
/// The expected digest, expected database identity, and positive maximum member
/// size are trusted caller inputs. This function is deliberately synchronous:
/// callers own the supervised blocking worker and positive deadline. It never
/// creates, copies, deletes, restores, or replaces filesystem state.
pub fn verify_backup_bundle(
    manifest_bytes: &[u8],
    expected_manifest_digest: BackupManifestSha256,
    bundle_directory: &Path,
    expected_identity: &ServiceDatabaseIdentity,
    maximum_state_bytes: NonZeroU64,
) -> Result<VerifiedServiceBackup, ServiceSqliteError> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        verify_backup_bundle_native(
            manifest_bytes,
            expected_manifest_digest,
            bundle_directory,
            expected_identity,
            maximum_state_bytes,
        )
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = (
            manifest_bytes,
            expected_manifest_digest,
            bundle_directory,
            expected_identity,
            maximum_state_bytes,
        );
        Err(ServiceSqliteError::new(ServiceSqliteErrorKind::Backup))
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn verify_backup_bundle_native(
    manifest_bytes: &[u8],
    expected_manifest_digest: BackupManifestSha256,
    bundle_directory: &Path,
    expected_identity: &ServiceDatabaseIdentity,
    maximum_state_bytes: NonZeroU64,
) -> Result<VerifiedServiceBackup, ServiceSqliteError> {
    require_verification_condition(
        manifest_bytes.len() <= crate::BACKUP_MANIFEST_CANONICAL_MAX_BYTES,
        VerificationFailureKind::Manifest,
    )?;
    let actual_manifest_digest: [u8; 32] = Sha256::digest(manifest_bytes).into();
    require_verification_condition(
        &actual_manifest_digest == expected_manifest_digest.as_bytes(),
        VerificationFailureKind::ManifestDigest,
    )?;
    let manifest = ServiceBackupManifest::from_canonical_bytes(manifest_bytes)
        .map_err(|source| verification_source(VerificationFailureKind::Manifest, source))?;
    verify_manifest_intent(&manifest, expected_identity)?;

    let member = manifest
        .members()
        .first()
        .ok_or_else(|| verification_error(VerificationFailureKind::Inventory))?;
    require_verification_condition(
        member.byte_length() <= i64::MAX as u64
            && member.byte_length() <= maximum_state_bytes.get(),
        VerificationFailureKind::MemberLength,
    )?;

    let binding = VerifiedBundleBinding::open(bundle_directory, member.byte_length())?;
    binding.validate_inventory()?;
    let first_digest = binding.hash_state(maximum_state_bytes)?;
    require_verification_condition(
        &first_digest == member.sha256().as_bytes(),
        VerificationFailureKind::MemberDigest,
    )?;
    binding.validate()?;

    let database_metadata = futures::executor::block_on(async {
        let mut connection = open_sqlite_from_retained_state(&binding).await?;
        apply_connection_policy(&mut connection).await?;
        binding.validate()?;
        verify_database_inventory(&mut connection).await?;
        binding.validate()?;
        let database_metadata =
            verify_database_metadata(&mut connection, &manifest, expected_identity).await?;
        binding.validate()?;
        verify_integrity(&mut connection).await?;
        binding.validate()?;
        connection.close().await.map_err(integrity_source)?;
        Ok::<_, ServiceSqliteError>(database_metadata)
    })?;

    binding.validate_inventory()?;
    let final_digest = binding.hash_state(maximum_state_bytes)?;
    require_backup_digests(&first_digest, &final_digest, member.sha256().as_bytes())?;
    binding.validate()?;

    Ok(VerifiedServiceBackup {
        manifest,
        database_metadata,
        binding,
    })
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn require_backup_digests(
    first: &[u8; 32],
    final_digest: &[u8; 32],
    expected: &[u8; 32],
) -> Result<(), ServiceSqliteError> {
    require_verification_condition(
        crate::all_constraints([
            first == expected,
            final_digest == expected,
            first == final_digest,
        ]),
        VerificationFailureKind::MemberDigest,
    )
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn verify_manifest_intent(
    manifest: &ServiceBackupManifest,
    expected: &ServiceDatabaseIdentity,
) -> Result<(), ServiceSqliteError> {
    require_verification_condition(
        crate::all_constraints([
            manifest.service() == expected.service(),
            manifest.instance() == expected.instance(),
            manifest.source_generation() == expected.source_generation(),
            manifest.state_schema_version() <= expected.supported_state_schema_version(),
        ]),
        VerificationFailureKind::Intent,
    )?;
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FileIdentity {
    device: u64,
    inode: u64,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
struct VerifiedBundleBinding {
    path: PathBuf,
    directory: File,
    directory_identity: FileIdentity,
    state: File,
    state_identity: FileIdentity,
    state_length: u64,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl VerifiedBundleBinding {
    fn open(path: &Path, expected_length: u64) -> Result<Self, ServiceSqliteError> {
        validate_bundle_path(path)?;
        let directory = File::from(
            open(
                path,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|source| {
                verification_source(VerificationFailureKind::BundleDirectory, source)
            })?,
        );
        let directory_identity = validate_directory(&directory)?;
        let state = File::from(
            openat(
                &directory,
                crate::BACKUP_STATE_MEMBER_NAME,
                OFlags::RDONLY | OFlags::NONBLOCK | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|source| verification_source(VerificationFailureKind::Inventory, source))?,
        );
        let (state_identity, state_length) = validate_state(&state)?;
        require_verification_condition(
            state_length == expected_length,
            VerificationFailureKind::MemberLength,
        )?;
        let binding = Self {
            path: path.to_path_buf(),
            directory,
            directory_identity,
            state,
            state_identity,
            state_length,
        };
        binding.validate()?;
        Ok(binding)
    }

    fn validate(&self) -> Result<(), ServiceSqliteError> {
        let current_directory = File::from(
            open(
                &self.path,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|source| {
                verification_source(VerificationFailureKind::BindingChanged, source)
            })?,
        );
        require_verification_condition(
            validate_directory(&self.directory)? == self.directory_identity,
            VerificationFailureKind::BindingChanged,
        )?;
        require_verification_condition(
            validate_directory(&current_directory)? == self.directory_identity,
            VerificationFailureKind::BindingChanged,
        )?;
        let current_state = File::from(
            openat(
                &self.directory,
                crate::BACKUP_STATE_MEMBER_NAME,
                OFlags::RDONLY | OFlags::NONBLOCK | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|source| {
                verification_source(VerificationFailureKind::BindingChanged, source)
            })?,
        );
        for state in [&self.state, &current_state] {
            let (identity, length) = validate_state(state)?;
            require_verification_condition(
                (identity, length) == (self.state_identity, self.state_length),
                VerificationFailureKind::BindingChanged,
            )?;
        }
        Ok(())
    }

    fn validate_inventory(&self) -> Result<(), ServiceSqliteError> {
        self.validate()?;
        let mut directory = Dir::read_from(&self.directory)
            .map_err(|source| verification_source(VerificationFailureKind::Inventory, source))?;
        let mut seen_state = false;
        let mut meaningful = 0_u8;
        while let Some(entry) = directory.read() {
            let entry = entry.map_err(|source| {
                verification_source(VerificationFailureKind::Inventory, source)
            })?;
            let name = entry.file_name().to_bytes();
            if name == b"." || name == b".." {
                continue;
            }
            meaningful = meaningful
                .checked_add(1)
                .ok_or_else(|| verification_error(VerificationFailureKind::Inventory))?;
            require_verification_condition(
                crate::all_constraints([
                    meaningful <= 1,
                    name == crate::BACKUP_STATE_MEMBER_NAME.as_bytes(),
                ]),
                VerificationFailureKind::Inventory,
            )?;
            seen_state = true;
        }
        require_verification_condition(seen_state, VerificationFailureKind::Inventory)?;
        self.validate()
    }

    fn hash_state(&self, maximum: NonZeroU64) -> Result<[u8; 32], ServiceSqliteError> {
        self.validate()?;
        let mut state = self
            .state
            .try_clone()
            .map_err(|source| verification_source(VerificationFailureKind::MemberDigest, source))?;
        state
            .seek(SeekFrom::Start(0))
            .map_err(|source| verification_source(VerificationFailureKind::MemberDigest, source))?;
        let mut hasher = Sha256::new();
        let mut length = 0_u64;
        let mut buffer = [0_u8; HASH_BUFFER_BYTES];
        loop {
            let count = state.read(&mut buffer).map_err(|source| {
                verification_source(VerificationFailureKind::MemberDigest, source)
            })?;
            if count == 0 {
                break;
            }
            length = length
                .checked_add(
                    u64::try_from(count)
                        .map_err(|_| verification_error(VerificationFailureKind::MemberLength))?,
                )
                .ok_or_else(|| verification_error(VerificationFailureKind::MemberLength))?;
            require_verification_condition(
                crate::all_constraints([length <= i64::MAX as u64, length <= maximum.get()]),
                VerificationFailureKind::MemberLength,
            )?;
            hasher.update(&buffer[..count]);
        }
        require_verification_condition(
            crate::all_constraints([length != 0, length == self.state_length]),
            VerificationFailureKind::MemberLength,
        )?;
        self.validate()?;
        Ok(hasher.finalize().into())
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn validate_bundle_path(path: &Path) -> Result<(), ServiceSqliteError> {
    require_verification_condition(
        crate::all_constraints([
            path.is_absolute(),
            !path.as_os_str().as_bytes().is_empty(),
            path.as_os_str().as_bytes().len() <= MAX_BUNDLE_PATH_BYTES,
            !path.components().any(|part| {
                matches!(
                    part,
                    std::path::Component::CurDir | std::path::Component::ParentDir
                )
            }),
        ]),
        VerificationFailureKind::BundleDirectory,
    )?;
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn validate_directory(directory: &File) -> Result<FileIdentity, ServiceSqliteError> {
    let status = fstat(directory)
        .map_err(|source| verification_source(VerificationFailureKind::BundleDirectory, source))?;
    let mode = crate::native_metadata::mode(status.st_mode) & 0o777;
    require_verification_condition(
        crate::native_metadata::restrictive_directory(
            FileType::from_raw_mode(status.st_mode).is_dir(),
            status.st_uid,
            geteuid().as_raw(),
            mode,
        ),
        VerificationFailureKind::Permissions,
    )?;
    file_identity(&status)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn validate_state(file: &File) -> Result<(FileIdentity, u64), ServiceSqliteError> {
    let status = fstat(file)
        .map_err(|source| verification_source(VerificationFailureKind::Inventory, source))?;
    let mode = crate::native_metadata::mode(status.st_mode) & 0o777;
    let length = u64::try_from(status.st_size)
        .map_err(|_| verification_error(VerificationFailureKind::MemberLength))?;
    require_verification_condition(
        crate::native_metadata::restrictive_regular_file(
            FileType::from_raw_mode(status.st_mode).is_file(),
            crate::native_metadata::link_count(status.st_nlink),
            status.st_uid,
            geteuid().as_raw(),
            mode,
        ),
        VerificationFailureKind::Permissions,
    )?;
    require_verification_condition(
        crate::native_metadata::valid_artifact_length(length, None),
        VerificationFailureKind::MemberLength,
    )?;
    Ok((file_identity(&status)?, length))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn file_identity(status: &rustix::fs::Stat) -> Result<FileIdentity, ServiceSqliteError> {
    Ok(FileIdentity {
        device: crate::native_metadata::device(status.st_dev)
            .map_err(|_| verification_error(VerificationFailureKind::BindingChanged))?,
        inode: status.st_ino,
    })
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
async fn open_sqlite_from_retained_state(
    binding: &VerifiedBundleBinding,
) -> Result<SqliteConnection, ServiceSqliteError> {
    let descriptor = binding.state.as_raw_fd();
    #[cfg(target_os = "linux")]
    let descriptor_path = format!("/proc/self/fd/{descriptor}");
    #[cfg(target_os = "macos")]
    let descriptor_path = format!("/dev/fd/{descriptor}");
    let options = SqliteConnectOptions::new()
        .filename(descriptor_path)
        .read_only(true)
        .immutable(true)
        .create_if_missing(false)
        .foreign_keys(false)
        .disable_statement_logging();
    SqliteConnection::connect_with(&options)
        .await
        .map_err(|source| verification_source(VerificationFailureKind::Inventory, source))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
async fn apply_connection_policy(
    connection: &mut SqliteConnection,
) -> Result<(), ServiceSqliteError> {
    sqlx::query("PRAGMA query_only = ON")
        .execute(&mut *connection)
        .await
        .map_err(integrity_source)?;
    sqlx::query("PRAGMA trusted_schema = OFF")
        .execute(&mut *connection)
        .await
        .map_err(integrity_source)?;
    verify_connection_policy(connection).await
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
async fn verify_connection_policy(
    connection: &mut SqliteConnection,
) -> Result<(), ServiceSqliteError> {
    let query_only = sqlx::query_scalar::<_, i64>("PRAGMA query_only")
        .fetch_one(&mut *connection)
        .await
        .map_err(integrity_source)?;
    let trusted_schema = sqlx::query_scalar::<_, i64>("PRAGMA trusted_schema")
        .fetch_one(connection)
        .await
        .map_err(integrity_source)?;
    require_verification_connection_policy(query_only, trusted_schema)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn require_verification_connection_policy(
    query_only: i64,
    trusted_schema: i64,
) -> Result<(), ServiceSqliteError> {
    crate::all_constraints([query_only == 1, trusted_schema == 0])
        .then_some(())
        .ok_or_else(|| integrity_error(IntegrityFailureKind::Policy))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
async fn verify_database_inventory(
    connection: &mut SqliteConnection,
) -> Result<(), ServiceSqliteError> {
    let rows = sqlx::query(
        "SELECT
            seq,
            typeof(name) = 'text' AS name_type_ok,
            length(CAST(name AS BLOB)) AS name_length,
            substr(CAST(name AS BLOB), 1, 5) AS name_prefix
         FROM pragma_database_list
         LIMIT 2",
    )
    .fetch_all(connection)
    .await
    .map_err(integrity_source)?;
    let Some(first) = rows.first() else {
        return Err(integrity_error(IntegrityFailureKind::DatabaseInventory));
    };
    let sequence_matches = first.try_get::<i64, _>(0).ok() == Some(0);
    let name_matches = crate::persisted_value::bounded_utf8(
        first,
        "name_type_ok",
        "name_length",
        "name_prefix",
        1,
        4,
    ) == Some("main");
    require_verification_database_inventory(sequence_matches, name_matches, rows.len() > 1)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn require_verification_database_inventory(
    sequence_matches: bool,
    name_matches: bool,
    has_extra: bool,
) -> Result<(), ServiceSqliteError> {
    crate::all_constraints([sequence_matches, name_matches, !has_extra])
        .then_some(())
        .ok_or_else(|| integrity_error(IntegrityFailureKind::DatabaseInventory))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
async fn verify_database_metadata(
    connection: &mut SqliteConnection,
    manifest: &ServiceBackupManifest,
    expected: &ServiceDatabaseIdentity,
) -> Result<ServiceDatabaseMetadata, ServiceSqliteError> {
    let object_rows = sqlx::query(
        "SELECT type
             FROM main.sqlite_schema
             WHERE name = 'radroots_service_metadata'
             LIMIT 2",
    )
    .fetch_all(&mut *connection)
    .await
    .map_err(metadata_source)?;
    let object = object_rows.first().ok_or_else(metadata_error)?;
    crate::require_condition(
        object.try_get::<&str, _>(0).ok() == Some("table") && object_rows.len() == 1,
        ServiceSqliteErrorKind::Metadata,
    )?;

    let application_id = sqlx::query_scalar::<_, i64>("PRAGMA application_id")
        .fetch_one(&mut *connection)
        .await
        .map_err(metadata_source)?;
    let application_id = u32::try_from(application_id)
        .ok()
        .and_then(|value| crate::ServiceSqliteApplicationId::new(value).ok())
        .ok_or_else(metadata_error)?;

    let rows = sqlx::query(
        "SELECT
                CASE WHEN typeof(singleton) = 'integer' THEN singleton END,
                typeof(service_id) = 'text' AS service_id_type_ok,
                length(CAST(service_id AS BLOB)) AS service_id_length,
                substr(CAST(service_id AS BLOB), 1, 129) AS service_id_prefix,
                typeof(instance_id) = 'text' AS instance_id_type_ok,
                length(CAST(instance_id AS BLOB)) AS instance_id_length,
                substr(CAST(instance_id AS BLOB), 1, 129) AS instance_id_prefix,
                typeof(source_generation) = 'blob' AS source_generation_type_ok,
                length(source_generation) AS source_generation_length,
                substr(source_generation, 1, 33) AS source_generation_prefix,
                CASE WHEN typeof(state_schema_version) = 'integer'
                     THEN state_schema_version END,
                CASE WHEN typeof(created_at_unix_ms) = 'integer'
                     THEN created_at_unix_ms END
             FROM radroots_service_metadata
             LIMIT 2",
    )
    .fetch_all(connection)
    .await
    .map_err(metadata_source)?;
    let row = rows.first().ok_or_else(metadata_error)?;
    let singleton = row.try_get::<Option<i64>, _>(0).map_err(metadata_source)?;
    crate::require_condition(rows.len() == 1, ServiceSqliteErrorKind::Metadata)?;
    if singleton != Some(1) {
        return Err(metadata_error());
    }
    let service = crate::persisted_value::bounded_utf8(
        row,
        "service_id_type_ok",
        "service_id_length",
        "service_id_prefix",
        1,
        crate::persisted_value::MAX_IDENTIFIER_UTF8_BYTES,
    )
    .and_then(|value| ServiceId::new(value).ok())
    .ok_or_else(metadata_error)?;
    let instance = crate::persisted_value::bounded_utf8(
        row,
        "instance_id_type_ok",
        "instance_id_length",
        "instance_id_prefix",
        1,
        crate::persisted_value::MAX_IDENTIFIER_UTF8_BYTES,
    )
    .and_then(|value| InstanceId::new(value).ok())
    .ok_or_else(metadata_error)?;
    let generation = crate::persisted_value::bounded_bytes(
        row,
        "source_generation_type_ok",
        "source_generation_length",
        "source_generation_prefix",
        32,
        32,
    )
    .and_then(|value| <[u8; 32]>::try_from(value).ok())
    .and_then(|value| SourceGeneration::new(value).ok())
    .ok_or_else(metadata_error)?;
    let schema = row.try_get::<Option<i64>, _>(10).map_err(metadata_source)?;
    let created_at = row.try_get::<Option<i64>, _>(11).map_err(metadata_source)?;
    let (Some(schema), Some(created_at)) = (schema, created_at) else {
        return Err(metadata_error());
    };
    let schema = NonZeroU32::new(u32::try_from(schema).map_err(|_| metadata_error())?)
        .ok_or_else(metadata_error)?;
    let created_at = u64::try_from(created_at).map_err(|_| metadata_error())?;

    require_verification_metadata_projection([
        service == *expected.service(),
        instance == *expected.instance(),
        generation == expected.source_generation(),
        application_id == expected.application_id(),
        service == *manifest.service(),
        instance == *manifest.instance(),
        generation == manifest.source_generation(),
        schema == manifest.state_schema_version(),
        schema <= expected.supported_state_schema_version(),
    ])?;
    ServiceDatabaseMetadata::from_verified_backup(
        service,
        instance,
        generation,
        schema,
        created_at,
        application_id,
    )
    .map_err(|_| metadata_error())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
async fn verify_integrity(connection: &mut SqliteConnection) -> Result<(), ServiceSqliteError> {
    let rows = sqlx::query(crate::persisted_value::INTEGRITY_CHECK_SQL)
        .fetch_all(&mut *connection)
        .await
        .map_err(integrity_source)?;
    let row = rows
        .first()
        .ok_or_else(|| integrity_error(IntegrityFailureKind::Sqlite))?;
    let value = crate::persisted_value::bounded_integrity_bytes(row);
    require_verification_integrity_projection(value == Some(b"ok"), rows.len() > 1)?;
    let violation = sqlx::query_scalar::<_, i64>("SELECT 1 FROM pragma_foreign_key_check LIMIT 1")
        .fetch_optional(connection)
        .await
        .map_err(integrity_source)?;
    require_integrity_condition(violation.is_none(), IntegrityFailureKind::ForeignKeys)?;
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn require_verification_metadata_projection(matches: [bool; 9]) -> Result<(), ServiceSqliteError> {
    crate::require_condition(
        crate::all_constraints(matches),
        ServiceSqliteErrorKind::Metadata,
    )
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn require_verification_integrity_projection(
    value_matches: bool,
    has_extra: bool,
) -> Result<(), ServiceSqliteError> {
    crate::all_constraints([value_matches, !has_extra])
        .then_some(())
        .ok_or_else(|| integrity_error(IntegrityFailureKind::Sqlite))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VerificationFailureKind {
    Manifest,
    ManifestDigest,
    Intent,
    BundleDirectory,
    Inventory,
    Permissions,
    MemberLength,
    MemberDigest,
    BindingChanged,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
struct VerificationFailure {
    kind: VerificationFailureKind,
    source: Option<Box<dyn Error + Send + Sync + 'static>>,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl fmt::Debug for VerificationFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BackupVerificationFailure")
            .field("kind", &self.kind)
            .field("source", &self.source.as_ref().map(|_| "[redacted]"))
            .finish()
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl fmt::Display for VerificationFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.kind {
            VerificationFailureKind::Manifest => "backup manifest is invalid",
            VerificationFailureKind::ManifestDigest => "backup manifest digest does not match",
            VerificationFailureKind::Intent => "backup intent does not match",
            VerificationFailureKind::BundleDirectory => "backup bundle directory is invalid",
            VerificationFailureKind::Inventory => "backup member inventory is invalid",
            VerificationFailureKind::Permissions => "backup permissions are invalid",
            VerificationFailureKind::MemberLength => "backup member length is invalid",
            VerificationFailureKind::MemberDigest => "backup member digest does not match",
            VerificationFailureKind::BindingChanged => "backup member binding changed",
        })
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl Error for VerificationFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn Error + 'static))
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn verification_error(kind: VerificationFailureKind) -> ServiceSqliteError {
    ServiceSqliteError::with_source(
        ServiceSqliteErrorKind::Backup,
        VerificationFailure { kind, source: None },
    )
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn require_verification_condition(
    condition: bool,
    kind: VerificationFailureKind,
) -> Result<(), ServiceSqliteError> {
    if condition {
        Ok(())
    } else {
        Err(verification_error(kind))
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn verification_source(
    kind: VerificationFailureKind,
    source: impl Error + Send + Sync + 'static,
) -> ServiceSqliteError {
    ServiceSqliteError::with_source(
        ServiceSqliteErrorKind::Backup,
        VerificationFailure {
            kind,
            source: Some(Box::new(source)),
        },
    )
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum IntegrityFailureKind {
    Policy,
    DatabaseInventory,
    Sqlite,
    ForeignKeys,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[derive(Debug)]
struct IntegrityFailure(IntegrityFailureKind);

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl fmt::Display for IntegrityFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.0 {
            IntegrityFailureKind::Policy => "backup SQLite policy is invalid",
            IntegrityFailureKind::DatabaseInventory => "backup database inventory is invalid",
            IntegrityFailureKind::Sqlite => "backup SQLite integrity is invalid",
            IntegrityFailureKind::ForeignKeys => "backup foreign-key integrity is invalid",
        })
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl Error for IntegrityFailure {}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn integrity_error(kind: IntegrityFailureKind) -> ServiceSqliteError {
    ServiceSqliteError::with_source(ServiceSqliteErrorKind::Integrity, IntegrityFailure(kind))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn require_integrity_condition(
    condition: bool,
    kind: IntegrityFailureKind,
) -> Result<(), ServiceSqliteError> {
    if condition {
        Ok(())
    } else {
        Err(integrity_error(kind))
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn integrity_source(source: sqlx::Error) -> ServiceSqliteError {
    ServiceSqliteError::with_source(ServiceSqliteErrorKind::Integrity, source)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn metadata_error() -> ServiceSqliteError {
    ServiceSqliteError::new(ServiceSqliteErrorKind::Metadata)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn metadata_source(source: sqlx::Error) -> ServiceSqliteError {
    ServiceSqliteError::with_source(ServiceSqliteErrorKind::Metadata, source)
}

#[cfg(test)]
mod tests {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    use {
        super::*,
        crate::{ServiceSqliteApplicationId, ServiceSqlitePaths},
        radroots_runtime_paths::{
            RadrootsHostEnvironment, RadrootsPathProfile, RadrootsPathResolver, RadrootsPlatform,
            RuntimeContext, RuntimeContextBootstrap, RuntimeContextSource,
        },
        std::{
            collections::BTreeSet, fs, io::Write, os::unix::fs::PermissionsExt, path::Path,
            process::Command,
        },
    };

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn open_test_database(path: &Path) -> SqliteConnection {
        futures::executor::block_on(SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true)
                .foreign_keys(false)
                .disable_statement_logging(),
        ))
        .expect("database")
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn verification_projection_helpers_reject_every_independent_drift() {
        let digest = [7_u8; 32];
        assert!(require_backup_digests(&digest, &digest, &digest).is_ok());
        for changed in 0..3 {
            let mut values = [digest; 3];
            values[changed][0] ^= 1;
            assert!(require_backup_digests(&values[0], &values[1], &values[2]).is_err());
        }

        assert!(require_verification_connection_policy(1, 0).is_ok());
        for values in [(0, 0), (1, 1), (0, 1)] {
            assert!(require_verification_connection_policy(values.0, values.1).is_err());
        }

        assert!(require_verification_database_inventory(true, true, false).is_ok());
        for (sequence, name, extra) in [
            (false, true, false),
            (true, false, false),
            (false, false, false),
            (true, true, true),
        ] {
            assert!(require_verification_database_inventory(sequence, name, extra).is_err());
        }

        assert!(require_verification_metadata_projection([true; 9]).is_ok());
        for changed in 0..9 {
            let mut matches = [true; 9];
            matches[changed] = false;
            assert!(require_verification_metadata_projection(matches).is_err());
        }

        assert!(require_verification_integrity_projection(true, false).is_ok());
        for (value_matches, extra) in [(false, false), (true, true)] {
            assert!(require_verification_integrity_projection(value_matches, extra).is_err());
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn verification_and_integrity_failure_inventories_are_complete() {
        let verification_cases = [
            (
                VerificationFailureKind::Manifest,
                "backup manifest is invalid",
            ),
            (
                VerificationFailureKind::ManifestDigest,
                "backup manifest digest does not match",
            ),
            (
                VerificationFailureKind::Intent,
                "backup intent does not match",
            ),
            (
                VerificationFailureKind::BundleDirectory,
                "backup bundle directory is invalid",
            ),
            (
                VerificationFailureKind::Inventory,
                "backup member inventory is invalid",
            ),
            (
                VerificationFailureKind::Permissions,
                "backup permissions are invalid",
            ),
            (
                VerificationFailureKind::MemberLength,
                "backup member length is invalid",
            ),
            (
                VerificationFailureKind::MemberDigest,
                "backup member digest does not match",
            ),
            (
                VerificationFailureKind::BindingChanged,
                "backup member binding changed",
            ),
        ];
        for (kind, message) in verification_cases {
            let plain = VerificationFailure { kind, source: None };
            assert_eq!(plain.to_string(), message);
            assert!(plain.source().is_none());
            let sourced = VerificationFailure {
                kind,
                source: Some(Box::new(std::io::Error::other("private-cause"))),
            };
            assert_eq!(sourced.to_string(), message);
            assert!(sourced.source().is_some());
            assert!(format!("{sourced:?}").contains("[redacted]"));
            assert!(require_verification_condition(true, kind).is_ok());
            assert_eq!(
                require_verification_condition(false, kind)
                    .expect_err("false condition")
                    .kind(),
                ServiceSqliteErrorKind::Backup
            );
        }

        for (kind, message) in [
            (
                IntegrityFailureKind::Policy,
                "backup SQLite policy is invalid",
            ),
            (
                IntegrityFailureKind::DatabaseInventory,
                "backup database inventory is invalid",
            ),
            (
                IntegrityFailureKind::Sqlite,
                "backup SQLite integrity is invalid",
            ),
            (
                IntegrityFailureKind::ForeignKeys,
                "backup foreign-key integrity is invalid",
            ),
        ] {
            let failure = IntegrityFailure(kind);
            assert_eq!(failure.to_string(), message);
            assert!(failure.source().is_none());
            assert!(format!("{failure:?}").contains(&format!("{kind:?}")));
            assert!(require_integrity_condition(true, kind).is_ok());
            assert_eq!(
                require_integrity_condition(false, kind)
                    .expect_err("false condition")
                    .kind(),
                ServiceSqliteErrorKind::Integrity
            );
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    struct Fixture {
        _root: tempfile::TempDir,
        bundle: PathBuf,
        paths: ServiceSqlitePaths,
        metadata: ServiceDatabaseMetadata,
        identity: ServiceDatabaseIdentity,
        manifest: ServiceBackupManifest,
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    impl Fixture {
        fn new(bundle_name: &str) -> Self {
            let root = tempfile::tempdir().expect("temporary root");
            let paths = paths(root.path(), "myc", "primary");
            let metadata = ServiceDatabaseMetadata::new(
                &paths,
                SourceGeneration::new([9; 32]).expect("source generation"),
                NonZeroU32::new(1).expect("schema"),
                1_700_000_000_000,
                ServiceSqliteApplicationId::new(0x5244_5351).expect("application ID"),
            )
            .expect("metadata");
            let bundle = root.path().join(bundle_name);
            fs::create_dir(&bundle).expect("bundle");
            fs::set_permissions(&bundle, fs::Permissions::from_mode(0o700)).expect("bundle mode");
            create_database(&bundle.join(crate::BACKUP_STATE_MEMBER_NAME), &metadata);
            let manifest = manifest_for(&bundle, &metadata);
            let identity = metadata.identity();
            Self {
                _root: root,
                bundle,
                paths,
                metadata,
                identity,
                manifest,
            }
        }

        fn maximum(&self) -> NonZeroU64 {
            NonZeroU64::new(self.manifest.members()[0].byte_length()).expect("member length")
        }

        fn verify(&self) -> Result<VerifiedServiceBackup, ServiceSqliteError> {
            verify_backup_bundle(
                self.manifest.canonical_bytes(),
                self.manifest.digest(),
                &self.bundle,
                &self.identity,
                self.maximum(),
            )
        }

        fn refresh_manifest(&mut self) {
            self.manifest = manifest_for(&self.bundle, &self.metadata);
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn paths(root: &Path, service: &str, instance: &str) -> ServiceSqlitePaths {
        let context = RuntimeContext::resolve(
            &RadrootsPathResolver::new(RadrootsPlatform::Linux, RadrootsHostEnvironment::default()),
            RuntimeContextBootstrap::new(
                RadrootsPathProfile::RepoLocal,
                Some(root.to_path_buf()),
                RuntimeContextSource::BootstrapCli,
                RuntimeContextSource::BootstrapCli,
            )
            .expect("bootstrap"),
            ServiceId::new(service).expect("service"),
            InstanceId::new(instance).expect("instance"),
        )
        .expect("context");
        ServiceSqlitePaths::from_runtime_context(&context).expect("paths")
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn create_database(path: &Path, metadata: &ServiceDatabaseMetadata) {
        let mut connection = open_test_database(path);
        futures::executor::block_on(async {
            sqlx::raw_sql(
                "CREATE TABLE radroots_service_metadata (
                    singleton INTEGER PRIMARY KEY,
                    service_id TEXT NOT NULL,
                    instance_id TEXT NOT NULL,
                    source_generation BLOB NOT NULL,
                    state_schema_version INTEGER NOT NULL,
                    created_at_unix_ms INTEGER NOT NULL
                 );
                 CREATE TABLE verify_probe (value INTEGER NOT NULL);
                 INSERT INTO verify_probe (value) VALUES (41), (42);",
            )
            .execute(&mut connection)
            .await
            .expect("schema");
            sqlx::query(
                "INSERT INTO radroots_service_metadata (
                    singleton, service_id, instance_id, source_generation,
                    state_schema_version, created_at_unix_ms
                 ) VALUES (1, ?, ?, ?, ?, ?)",
            )
            .bind(metadata.service().as_str())
            .bind(metadata.instance().as_str())
            .bind(metadata.source_generation().as_bytes().as_slice())
            .bind(i64::from(metadata.state_schema_version().get()))
            .bind(i64::try_from(metadata.created_at_unix_ms()).expect("creation time"))
            .execute(&mut connection)
            .await
            .expect("metadata row");
            let application_id = format!(
                "PRAGMA application_id = {}",
                metadata.application_id().get()
            );
            sqlx::query(sqlx::AssertSqlSafe(application_id.as_str()))
                .execute(&mut connection)
                .await
                .expect("application ID");
            connection.close().await.expect("close fixture");
        });
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).expect("state mode");
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn manifest_for(bundle: &Path, metadata: &ServiceDatabaseMetadata) -> ServiceBackupManifest {
        let bytes = fs::read(bundle.join(crate::BACKUP_STATE_MEMBER_NAME)).expect("state bytes");
        ServiceBackupManifest::from_capture(
            metadata,
            crate::BackupCreatedAtUnixMs::new(1_700_000_000_100).expect("capture time"),
            u64::try_from(bytes.len()).expect("length"),
            crate::BackupMemberSha256::from_bytes(Sha256::digest(&bytes).into()),
        )
        .expect("manifest")
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn digest(bytes: &[u8]) -> BackupManifestSha256 {
        BackupManifestSha256::from_bytes(Sha256::digest(bytes).into())
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn inventory(path: &Path) -> BTreeSet<std::ffi::OsString> {
        fs::read_dir(path)
            .expect("inventory")
            .map(|entry| entry.expect("entry").file_name())
            .collect()
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn verifies_exact_bundle_without_mutating_it_and_redacts_the_capability() {
        let fixture = Fixture::new("verified bundle %?#");
        let state = fixture.bundle.join(crate::BACKUP_STATE_MEMBER_NAME);
        let before_bytes = fs::read(&state).expect("before bytes");
        let before_modified = fs::metadata(&state)
            .expect("before metadata")
            .modified()
            .expect("before mtime");
        let before_modes = (
            fs::metadata(&fixture.bundle)
                .expect("directory metadata")
                .permissions()
                .mode()
                & 0o777,
            fs::metadata(&state)
                .expect("state metadata")
                .permissions()
                .mode()
                & 0o777,
        );
        let before_inventory = inventory(&fixture.bundle);

        let verified = fixture.verify().expect("verified backup");
        assert_eq!(verified.manifest(), &fixture.manifest);
        assert_eq!(verified.database_metadata(), &fixture.metadata);
        let debug = format!("{verified:?}");
        for forbidden in [
            fixture.bundle.to_string_lossy().as_ref(),
            "myc",
            "primary",
            "52445351",
        ] {
            assert!(!debug.contains(forbidden));
        }
        assert_eq!(fs::read(&state).expect("after bytes"), before_bytes);
        assert_eq!(
            fs::metadata(&state)
                .expect("after metadata")
                .modified()
                .expect("after mtime"),
            before_modified
        );
        assert_eq!(
            (
                fs::metadata(&fixture.bundle)
                    .expect("directory metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                fs::metadata(&state)
                    .expect("state metadata")
                    .permissions()
                    .mode()
                    & 0o777,
            ),
            before_modes
        );
        assert_eq!(inventory(&fixture.bundle), before_inventory);
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn digest_intent_schema_and_member_size_fail_closed() {
        let fixture = Fixture::new("intent");
        let wrong_digest = BackupManifestSha256::from_bytes([7; 32]);
        assert_eq!(
            verify_backup_bundle(
                fixture.manifest.canonical_bytes(),
                wrong_digest,
                &fixture.bundle,
                &fixture.identity,
                fixture.maximum(),
            )
            .expect_err("digest mismatch")
            .kind(),
            ServiceSqliteErrorKind::Backup
        );

        let tampered_time = String::from_utf8(fixture.manifest.canonical_bytes().to_vec())
            .expect("manifest text")
            .replace("1700000000100", "1700000000101");
        assert_eq!(
            verify_backup_bundle(
                tampered_time.as_bytes(),
                fixture.manifest.digest(),
                &fixture.bundle,
                &fixture.identity,
                fixture.maximum(),
            )
            .expect_err("creation time tamper")
            .kind(),
            ServiceSqliteErrorKind::Backup
        );

        let smaller = NonZeroU64::new(fixture.maximum().get() - 1).expect("smaller limit");
        assert_eq!(
            verify_backup_bundle(
                fixture.manifest.canonical_bytes(),
                fixture.manifest.digest(),
                &fixture.bundle,
                &fixture.identity,
                smaller,
            )
            .expect_err("member over caller limit")
            .kind(),
            ServiceSqliteErrorKind::Backup
        );

        let wrong_generation = ServiceDatabaseIdentity::new(
            &fixture.paths,
            SourceGeneration::new([8; 32]).expect("generation"),
            NonZeroU32::new(1).expect("schema"),
            fixture.identity.application_id(),
        );
        assert_eq!(
            verify_backup_bundle(
                fixture.manifest.canonical_bytes(),
                fixture.manifest.digest(),
                &fixture.bundle,
                &wrong_generation,
                fixture.maximum(),
            )
            .expect_err("generation intent")
            .kind(),
            ServiceSqliteErrorKind::Backup
        );

        for (service, instance) in [("rhi", "primary"), ("myc", "secondary")] {
            let wrong_paths = paths(fixture._root.path(), service, instance);
            let wrong_identity = ServiceDatabaseIdentity::new(
                &wrong_paths,
                fixture.identity.source_generation(),
                fixture.identity.supported_state_schema_version(),
                fixture.identity.application_id(),
            );
            assert_eq!(
                verify_backup_bundle(
                    fixture.manifest.canonical_bytes(),
                    fixture.manifest.digest(),
                    &fixture.bundle,
                    &wrong_identity,
                    fixture.maximum(),
                )
                .expect_err("service-instance intent")
                .kind(),
                ServiceSqliteErrorKind::Backup
            );
        }

        let wrong_application = ServiceDatabaseIdentity::new(
            &fixture.paths,
            fixture.identity.source_generation(),
            NonZeroU32::new(1).expect("schema"),
            ServiceSqliteApplicationId::new(7).expect("application"),
        );
        assert_eq!(
            verify_backup_bundle(
                fixture.manifest.canonical_bytes(),
                fixture.manifest.digest(),
                &fixture.bundle,
                &wrong_application,
                fixture.maximum(),
            )
            .expect_err("application intent")
            .kind(),
            ServiceSqliteErrorKind::Metadata
        );

        let schema_two = String::from_utf8(fixture.manifest.canonical_bytes().to_vec())
            .expect("manifest text")
            .replace("\"state_schema_version\":1", "\"state_schema_version\":2");
        let supports_two = ServiceDatabaseIdentity::new(
            &fixture.paths,
            fixture.identity.source_generation(),
            NonZeroU32::new(2).expect("schema"),
            fixture.identity.application_id(),
        );
        assert_eq!(
            verify_backup_bundle(
                schema_two.as_bytes(),
                digest(schema_two.as_bytes()),
                &fixture.bundle,
                &supports_two,
                fixture.maximum(),
            )
            .expect_err("manifest and database schema mismatch")
            .kind(),
            ServiceSqliteErrorKind::Metadata
        );

        let too_large = String::from_utf8(fixture.manifest.canonical_bytes().to_vec())
            .expect("manifest text")
            .replace(
                &format!("\"byte_length\":{}", fixture.maximum().get()),
                "\"byte_length\":9223372036854775808",
            );
        assert_eq!(
            verify_backup_bundle(
                too_large.as_bytes(),
                digest(too_large.as_bytes()),
                &fixture.bundle,
                &fixture.identity,
                NonZeroU64::new(u64::MAX).expect("maximum"),
            )
            .expect_err("signed representation overflow")
            .kind(),
            ServiceSqliteErrorKind::Backup
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn restrictive_read_only_modes_are_allowed_and_broader_modes_reject() {
        for (directory_mode, state_mode) in [
            (0o700, 0o600),
            (0o700, 0o400),
            (0o500, 0o600),
            (0o500, 0o400),
        ] {
            let fixture = Fixture::new("allowed-modes");
            let state = fixture.bundle.join(crate::BACKUP_STATE_MEMBER_NAME);
            fs::set_permissions(&state, fs::Permissions::from_mode(state_mode))
                .expect("state mode");
            fs::set_permissions(&fixture.bundle, fs::Permissions::from_mode(directory_mode))
                .expect("directory mode");
            fixture.verify().expect("restrictive mode verifies");
        }
        for directory_mode in [0o400, 0o710, 0o750] {
            let fixture = Fixture::new("bad-directory-mode");
            fs::set_permissions(&fixture.bundle, fs::Permissions::from_mode(directory_mode))
                .expect("directory mode");
            assert_eq!(
                fixture.verify().expect_err("invalid directory mode").kind(),
                ServiceSqliteErrorKind::Backup
            );
        }
        for state_mode in [0o000, 0o500, 0o640, 0o700] {
            let fixture = Fixture::new("bad-state-mode");
            fs::set_permissions(
                fixture.bundle.join(crate::BACKUP_STATE_MEMBER_NAME),
                fs::Permissions::from_mode(state_mode),
            )
            .expect("state mode");
            assert_eq!(
                fixture.verify().expect_err("invalid state mode").kind(),
                ServiceSqliteErrorKind::Backup
            );
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn inventory_links_and_same_length_tampering_reject() {
        let fixture = Fixture::new("extra");
        fs::write(fixture.bundle.join("state.sqlite-wal"), b"foreign").expect("sidecar");
        assert_eq!(
            fixture.verify().expect_err("extra member").kind(),
            ServiceSqliteErrorKind::Backup
        );

        let fixture = Fixture::new("hardlink");
        fs::hard_link(
            fixture.bundle.join(crate::BACKUP_STATE_MEMBER_NAME),
            fixture.bundle.join("other"),
        )
        .expect("hard link");
        assert_eq!(
            fixture.verify().expect_err("hard-linked member").kind(),
            ServiceSqliteErrorKind::Backup
        );

        let fixture = Fixture::new("missing");
        fs::remove_file(fixture.bundle.join(crate::BACKUP_STATE_MEMBER_NAME))
            .expect("remove state");
        assert_eq!(
            fixture.verify().expect_err("missing member").kind(),
            ServiceSqliteErrorKind::Backup
        );

        let fixture = Fixture::new("symlink");
        let state = fixture.bundle.join(crate::BACKUP_STATE_MEMBER_NAME);
        let held = fixture.bundle.join("held-state");
        fs::rename(&state, &held).expect("move state");
        std::os::unix::fs::symlink(&held, &state).expect("symlink state");
        assert_eq!(
            fixture.verify().expect_err("symlink member").kind(),
            ServiceSqliteErrorKind::Backup
        );

        let fixture = Fixture::new("directory-member");
        let state = fixture.bundle.join(crate::BACKUP_STATE_MEMBER_NAME);
        fs::remove_file(&state).expect("remove state");
        fs::create_dir(&state).expect("directory member");
        fs::set_permissions(&state, fs::Permissions::from_mode(0o700))
            .expect("directory member mode");
        assert_eq!(
            fixture.verify().expect_err("directory member").kind(),
            ServiceSqliteErrorKind::Backup
        );

        let fixture = Fixture::new("fifo-member");
        let state = fixture.bundle.join(crate::BACKUP_STATE_MEMBER_NAME);
        fs::remove_file(&state).expect("remove state");
        assert!(
            Command::new("mkfifo")
                .arg(&state)
                .status()
                .expect("mkfifo")
                .success()
        );
        fs::set_permissions(&state, fs::Permissions::from_mode(0o600)).expect("fifo mode");
        assert_eq!(
            fixture.verify().expect_err("fifo member").kind(),
            ServiceSqliteErrorKind::Backup
        );

        let fixture = Fixture::new("tamper");
        let state = fixture.bundle.join(crate::BACKUP_STATE_MEMBER_NAME);
        let mut bytes = fs::read(&state).expect("state");
        let index = bytes.len() - 1;
        bytes[index] ^= 1;
        fs::write(&state, bytes).expect("same-length tamper");
        fs::set_permissions(&state, fs::Permissions::from_mode(0o600)).expect("state mode");
        assert_eq!(
            fixture.verify().expect_err("member digest mismatch").kind(),
            ServiceSqliteErrorKind::Backup
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn bounded_metadata_and_foreign_key_integrity_reject() {
        let mut metadata_fixture = Fixture::new("oversized-metadata");
        let state = metadata_fixture
            .bundle
            .join(crate::BACKUP_STATE_MEMBER_NAME);
        let mut connection = open_test_database(&state);
        futures::executor::block_on(async {
            sqlx::query("UPDATE radroots_service_metadata SET service_id = ?")
                .bind("x".repeat(129))
                .execute(&mut connection)
                .await
                .expect("oversized metadata");
            connection.close().await.expect("close");
        });
        metadata_fixture.refresh_manifest();
        assert_eq!(
            metadata_fixture
                .verify()
                .expect_err("oversized metadata")
                .kind(),
            ServiceSqliteErrorKind::Metadata
        );

        let mut view_fixture = Fixture::new("metadata-view");
        let state = view_fixture.bundle.join(crate::BACKUP_STATE_MEMBER_NAME);
        let mut connection = open_test_database(&state);
        let generation = "09".repeat(32);
        futures::executor::block_on(async {
            let statement = format!(
                "DROP TABLE radroots_service_metadata;
                 CREATE VIEW radroots_service_metadata AS
                 SELECT
                     1 AS singleton,
                     'myc' AS service_id,
                     'primary' AS instance_id,
                     X'{generation}' AS source_generation,
                     1 AS state_schema_version,
                     1700000000000 AS created_at_unix_ms;"
            );
            sqlx::raw_sql(sqlx::AssertSqlSafe(statement.as_str()))
                .execute(&mut connection)
                .await
                .expect("metadata view");
            connection.close().await.expect("close");
        });
        view_fixture.refresh_manifest();
        assert_eq!(
            view_fixture.verify().expect_err("metadata view").kind(),
            ServiceSqliteErrorKind::Metadata
        );

        let mut foreign_key_fixture = Fixture::new("foreign-key");
        let state = foreign_key_fixture
            .bundle
            .join(crate::BACKUP_STATE_MEMBER_NAME);
        let mut connection = open_test_database(&state);
        futures::executor::block_on(async {
            sqlx::raw_sql(
                "PRAGMA foreign_keys = OFF;
                 CREATE TABLE parent (id INTEGER PRIMARY KEY);
                 CREATE TABLE child (
                     id INTEGER PRIMARY KEY,
                     parent_id INTEGER NOT NULL REFERENCES parent(id)
                 );
                 INSERT INTO child (id, parent_id) VALUES (1, 99);",
            )
            .execute(&mut connection)
            .await
            .expect("foreign-key violation");
            connection.close().await.expect("close");
        });
        foreign_key_fixture.refresh_manifest();
        assert_eq!(
            foreign_key_fixture
                .verify()
                .expect_err("foreign-key violation")
                .kind(),
            ServiceSqliteErrorKind::Integrity
        );

        let mut corrupt_fixture = Fixture::new("corrupt-sqlite");
        let state = corrupt_fixture.bundle.join(crate::BACKUP_STATE_MEMBER_NAME);
        let mut connection = open_test_database(&state);
        let (page_size, root_page) = futures::executor::block_on(async {
            let page_size = sqlx::query_scalar::<_, i64>("PRAGMA page_size")
                .fetch_one(&mut connection)
                .await
                .expect("page size");
            let root_page = sqlx::query_scalar::<_, i64>(
                "SELECT rootpage FROM sqlite_schema WHERE name = 'verify_probe'",
            )
            .fetch_one(&mut connection)
            .await
            .expect("probe root page");
            connection.close().await.expect("close");
            (page_size, root_page)
        });
        let page_size = u64::try_from(page_size).expect("positive page size");
        let root_page = u64::try_from(root_page).expect("positive root page");
        let corrupt_offset = root_page
            .checked_sub(1)
            .and_then(|page| page.checked_mul(page_size))
            .expect("corrupt offset");
        let mut state_file = fs::OpenOptions::new()
            .write(true)
            .open(&state)
            .expect("open corrupt state");
        state_file
            .seek(SeekFrom::Start(corrupt_offset))
            .expect("seek corrupt page");
        state_file.write_all(&[0xff]).expect("corrupt page type");
        state_file.sync_all().expect("sync corruption");
        drop(state_file);
        corrupt_fixture.refresh_manifest();
        assert_eq!(
            corrupt_fixture
                .verify()
                .expect_err("corrupt SQLite with trusted hashes")
                .kind(),
            ServiceSqliteErrorKind::Integrity
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn sqlite_connection_policy_drift_fails_closed() {
        futures::executor::block_on(async {
            let mut connection = SqliteConnection::connect("sqlite::memory:")
                .await
                .expect("memory database");
            apply_connection_policy(&mut connection)
                .await
                .expect("governed policy");
            verify_connection_policy(&mut connection)
                .await
                .expect("policy readback");

            sqlx::query("PRAGMA trusted_schema = ON")
                .execute(&mut connection)
                .await
                .expect("drift trusted schema");
            assert_eq!(
                verify_connection_policy(&mut connection)
                    .await
                    .expect_err("trusted-schema drift")
                    .kind(),
                ServiceSqliteErrorKind::Integrity
            );

            sqlx::query("PRAGMA trusted_schema = OFF")
                .execute(&mut connection)
                .await
                .expect("restore trusted schema");
            sqlx::query("PRAGMA query_only = OFF")
                .execute(&mut connection)
                .await
                .expect("drift query-only");
            assert_eq!(
                verify_connection_policy(&mut connection)
                    .await
                    .expect_err("query-only drift")
                    .kind(),
                ServiceSqliteErrorKind::Integrity
            );
        });
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn attached_database_and_replaced_bindings_fail_closed() {
        futures::executor::block_on(async {
            let mut connection = SqliteConnection::connect("sqlite::memory:")
                .await
                .expect("memory database");
            sqlx::query("ATTACH DATABASE ':memory:' AS extra")
                .execute(&mut connection)
                .await
                .expect("attach");
            assert_eq!(
                verify_database_inventory(&mut connection)
                    .await
                    .expect_err("extra attachment")
                    .kind(),
                ServiceSqliteErrorKind::Integrity
            );
        });

        let fixture = Fixture::new("replace-directory");
        let binding = VerifiedBundleBinding::open(
            &fixture.bundle,
            fixture.manifest.members()[0].byte_length(),
        )
        .expect("binding");
        let moved = fixture.bundle.with_extension("held");
        fs::rename(&fixture.bundle, &moved).expect("move held directory");
        fs::create_dir(&fixture.bundle).expect("replacement directory");
        fs::set_permissions(&fixture.bundle, fs::Permissions::from_mode(0o700))
            .expect("replacement mode");
        fs::write(
            fixture.bundle.join(crate::BACKUP_STATE_MEMBER_NAME),
            b"replacement",
        )
        .expect("replacement state");
        fs::set_permissions(
            fixture.bundle.join(crate::BACKUP_STATE_MEMBER_NAME),
            fs::Permissions::from_mode(0o600),
        )
        .expect("replacement state mode");

        futures::executor::block_on(async {
            let mut retained_connection = open_sqlite_from_retained_state(&binding)
                .await
                .expect("open retained member");
            apply_connection_policy(&mut retained_connection)
                .await
                .expect("retained connection policy");
            verify_database_inventory(&mut retained_connection)
                .await
                .expect("retained database inventory");
            assert_eq!(
                verify_database_metadata(
                    &mut retained_connection,
                    &fixture.manifest,
                    &fixture.identity,
                )
                .await
                .expect("retained metadata"),
                fixture.metadata
            );
            verify_integrity(&mut retained_connection)
                .await
                .expect("retained integrity");
            retained_connection
                .close()
                .await
                .expect("close retained member");
        });

        assert_eq!(
            binding
                .validate()
                .expect_err("directory replacement")
                .kind(),
            ServiceSqliteErrorKind::Backup
        );
        let retained = fs::read(moved.join(crate::BACKUP_STATE_MEMBER_NAME))
            .expect("retained member through moved directory");
        let replacement = fs::read(fixture.bundle.join(crate::BACKUP_STATE_MEMBER_NAME))
            .expect("replacement member");
        assert_ne!(retained, replacement);
        assert_eq!(replacement, b"replacement");

        let fixture = Fixture::new("replace-member");
        let binding = VerifiedBundleBinding::open(
            &fixture.bundle,
            fixture.manifest.members()[0].byte_length(),
        )
        .expect("binding");
        let state = fixture.bundle.join(crate::BACKUP_STATE_MEMBER_NAME);
        let moved = fixture.bundle.join("held-state");
        fs::rename(&state, &moved).expect("move held member");
        fs::copy(&moved, &state).expect("replacement member");
        fs::set_permissions(&state, fs::Permissions::from_mode(0o600)).expect("replacement mode");
        assert_eq!(
            binding.validate().expect_err("member replacement").kind(),
            ServiceSqliteErrorKind::Backup
        );
        assert_eq!(
            fs::read(&moved).expect("held member"),
            fs::read(&state).expect("replacement")
        );

        fs::remove_file(&state).expect("remove replacement");
        assert!(
            Command::new("mkfifo")
                .arg(&state)
                .status()
                .expect("mkfifo")
                .success()
        );
        fs::set_permissions(&state, fs::Permissions::from_mode(0o600)).expect("fifo mode");
        assert_eq!(
            binding.validate().expect_err("replacement fifo").kind(),
            ServiceSqliteErrorKind::Backup
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn public_diagnostics_do_not_render_untrusted_paths_or_contents() {
        let fixture = Fixture::new("secret-path-value");
        fs::write(fixture.bundle.join("secret-extra"), b"secret-content").expect("extra");
        let error = fixture.verify().expect_err("invalid inventory");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Backup);
        for rendered in [error.to_string(), format!("{error:?}")] {
            assert!(!rendered.contains("secret-path-value"));
            assert!(!rendered.contains("secret-extra"));
            assert!(!rendered.contains("secret-content"));
        }
    }
}
