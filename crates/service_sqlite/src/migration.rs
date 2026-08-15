//! Deterministic migration identity, ledger, and governed execution mechanics.

use core::fmt;
use std::{collections::BTreeSet, error::Error, future::Future, pin::Pin};

#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

use sqlx::SqliteConnection;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use sqlx::{Connection, Row};

use crate::ServiceSqliteError;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use crate::{SchemaCatalog, ServiceSqliteErrorKind};

const MIGRATION_CONTENT_DOMAIN: &[u8] = b"radroots.service_sqlite.migration_content.v1\0";
const MIGRATION_CATALOG_DOMAIN: &[u8] = b"radroots.service_sqlite.migration_catalog.v1\0";
const BASE_SCHEMA_VERSION: u32 = 1;
const MAX_MIGRATION_NAME_UTF8_BYTES: usize = 128;
const MAX_MIGRATION_CONTENT_BYTES: usize = 1024 * 1024;
const MAX_MIGRATION_COUNT: usize = 4096;
const MAX_MIGRATION_BUILD_ID_UTF8_BYTES: usize = 128;

/// A bounded stable lower-snake migration name.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MigrationName(&'static str);

impl MigrationName {
    /// Validates an embedded migration name.
    pub fn new(value: &'static str) -> Result<Self, MigrationContractError> {
        if !valid_name(value) {
            return Err(MigrationContractError::InvalidName);
        }
        Ok(Self(value))
    }

    /// Returns the validated stable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Debug for MigrationName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("MigrationName")
            .field(&self.0)
            .finish()
    }
}

/// The execution kind whose canonical content is bound by a checksum.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MigrationKind {
    Sql,
    Callback,
}

impl MigrationKind {
    const fn tag(self) -> u8 {
        match self {
            Self::Sql => 0,
            Self::Callback => 1,
        }
    }
}

/// A SHA-256 digest over one migration body or one ordered catalog.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct MigrationChecksum([u8; 32]);

impl MigrationChecksum {
    /// Constructs an independently pinned checksum from exact reviewed bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Computes the frozen SQL-content checksum.
    #[must_use]
    pub fn for_sql(sql: &str) -> Self {
        Self::for_content(MigrationKind::Sql, sql.as_bytes())
    }

    /// Computes the frozen callback-definition checksum.
    #[must_use]
    pub fn for_callback(callback_definition: &[u8]) -> Self {
        Self::for_content(MigrationKind::Callback, callback_definition)
    }

    /// Returns the exact digest bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    fn for_content(kind: MigrationKind, content: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(MIGRATION_CONTENT_DOMAIN);
        hasher.update([kind.tag()]);
        let content_len = u64::try_from(content.len()).expect("migration content bound fits u64");
        hasher.update(content_len.to_be_bytes());
        hasher.update(content);
        Self(hasher.finalize().into())
    }
}

impl fmt::Debug for MigrationChecksum {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("MigrationChecksum([redacted])")
    }
}

/// One immutable future-schema migration identity.
#[derive(Clone, PartialEq, Eq)]
pub struct MigrationDescriptor {
    target_version: u32,
    name: MigrationName,
    kind: MigrationKind,
    checksum: MigrationChecksum,
    content: &'static [u8],
}

impl MigrationDescriptor {
    /// Defines an embedded SQL migration after verifying its expected checksum.
    pub fn sql(
        target_version: u32,
        name: &'static str,
        sql: &'static str,
        expected_checksum: MigrationChecksum,
    ) -> Result<Self, MigrationContractError> {
        Self::new(
            target_version,
            name,
            MigrationKind::Sql,
            sql.as_bytes(),
            expected_checksum,
        )
    }

    /// Defines an execution-free callback identity from canonical embedded bytes.
    pub fn callback(
        target_version: u32,
        name: &'static str,
        callback_definition: &'static [u8],
        expected_checksum: MigrationChecksum,
    ) -> Result<Self, MigrationContractError> {
        Self::new(
            target_version,
            name,
            MigrationKind::Callback,
            callback_definition,
            expected_checksum,
        )
    }

    fn new(
        target_version: u32,
        name: &'static str,
        kind: MigrationKind,
        content: &'static [u8],
        expected_checksum: MigrationChecksum,
    ) -> Result<Self, MigrationContractError> {
        if target_version <= BASE_SCHEMA_VERSION {
            return Err(MigrationContractError::InvalidTargetVersion);
        }
        if content.is_empty() {
            return Err(MigrationContractError::EmptyContent);
        }
        if content.len() > MAX_MIGRATION_CONTENT_BYTES {
            return Err(MigrationContractError::ContentTooLarge);
        }
        let name = MigrationName::new(name)?;
        let actual_checksum = MigrationChecksum::for_content(kind, content);
        if actual_checksum != expected_checksum {
            return Err(MigrationContractError::ChecksumMismatch);
        }
        Ok(Self {
            target_version,
            name,
            kind,
            checksum: actual_checksum,
            content,
        })
    }

    /// Returns the schema version produced by this migration.
    #[must_use]
    pub const fn target_version(&self) -> u32 {
        self.target_version
    }

    /// Returns the stable migration name.
    #[must_use]
    pub const fn name(&self) -> MigrationName {
        self.name
    }

    /// Returns whether this descriptor binds SQL or callback-definition bytes.
    #[must_use]
    pub const fn kind(&self) -> MigrationKind {
        self.kind
    }

    /// Returns the verified content checksum.
    #[must_use]
    pub const fn checksum(&self) -> MigrationChecksum {
        self.checksum
    }
}

impl fmt::Debug for MigrationDescriptor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MigrationDescriptor")
            .field("target_version", &self.target_version)
            .field("name", &self.name)
            .field("kind", &self.kind)
            .field("checksum", &self.checksum)
            .field("content", &"[redacted]")
            .finish()
    }
}

/// One validated ordered migration catalog whose baseline is schema v1.
#[derive(Clone, PartialEq, Eq)]
pub struct MigrationCatalog {
    descriptors: Box<[MigrationDescriptor]>,
    current_version: u32,
    digest: MigrationChecksum,
}

impl MigrationCatalog {
    /// Validates and owns at most 4096 future migrations in exact caller order.
    pub fn new<I>(descriptors: I) -> Result<Self, MigrationContractError>
    where
        I: IntoIterator<Item = MigrationDescriptor>,
    {
        let descriptors: Vec<_> = descriptors
            .into_iter()
            .take(MAX_MIGRATION_COUNT + 1)
            .collect();
        if descriptors.len() > MAX_MIGRATION_COUNT {
            return Err(MigrationContractError::TooManyMigrations);
        }
        validate_catalog(&descriptors)?;
        let current_version = descriptors
            .last()
            .map_or(BASE_SCHEMA_VERSION, MigrationDescriptor::target_version);
        let digest = catalog_digest(&descriptors);
        Ok(Self {
            descriptors: descriptors.into_boxed_slice(),
            current_version,
            digest,
        })
    }

    /// Returns the immutable descriptors in exact execution order.
    #[must_use]
    pub fn descriptors(&self) -> &[MigrationDescriptor] {
        &self.descriptors
    }

    /// Returns schema v1 for an empty catalog or the last target version.
    #[must_use]
    pub const fn current_version(&self) -> u32 {
        self.current_version
    }

    /// Returns the deterministic digest of the ordered catalog identity.
    #[must_use]
    pub const fn digest(&self) -> MigrationChecksum {
        self.digest
    }
}

impl fmt::Debug for MigrationCatalog {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MigrationCatalog")
            .field("descriptor_count", &self.descriptors.len())
            .field("current_version", &self.current_version)
            .field("digest", &self.digest)
            .finish()
    }
}

/// Injected Unix timestamp recorded for one applied migration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MigrationAppliedAtUnixSeconds(u64);

impl MigrationAppliedAtUnixSeconds {
    /// Validates a timestamp representable by SQLite's signed integer storage.
    pub const fn new(value: u64) -> Result<Self, MigrationEvidenceError> {
        if value > i64::MAX as u64 {
            return Err(MigrationEvidenceError::InvalidAppliedTime);
        }
        Ok(Self(value))
    }

    /// Returns the injected Unix timestamp in seconds.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Complete deterministic application-build identity recorded with a migration.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct MigrationBuildIdentity {
    service_version: String,
    service_commit: String,
    lib_revision: String,
    rust_version: String,
    target: String,
    feature_profile: String,
    config_contract_version: u32,
    state_contract_version: u32,
    admin_contract_version: u32,
    status_contract_version: u32,
    provider_contract_version: u32,
}

impl MigrationBuildIdentity {
    /// Validates the complete timestamp-free build identity used by service hosts.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        service_version: impl AsRef<str>,
        service_commit: impl AsRef<str>,
        lib_revision: impl AsRef<str>,
        rust_version: impl AsRef<str>,
        target: impl AsRef<str>,
        feature_profile: impl AsRef<str>,
        config_contract_version: u32,
        state_contract_version: u32,
        admin_contract_version: u32,
        status_contract_version: u32,
        provider_contract_version: u32,
    ) -> Result<Self, MigrationEvidenceError> {
        let service_version = service_version.as_ref();
        let service_commit = service_commit.as_ref();
        let lib_revision = lib_revision.as_ref();
        let rust_version = rust_version.as_ref();
        let target = target.as_ref();
        let feature_profile = feature_profile.as_ref();
        if !valid_build_text(service_version)
            || !valid_revision(service_commit)
            || !valid_revision(lib_revision)
            || !valid_build_text(rust_version)
            || !valid_build_text(target)
            || !valid_build_text(feature_profile)
            || [
                config_contract_version,
                state_contract_version,
                admin_contract_version,
                status_contract_version,
                provider_contract_version,
            ]
            .contains(&0)
        {
            return Err(MigrationEvidenceError::InvalidBuildIdentity);
        }
        Ok(Self {
            service_version: service_version.to_owned(),
            service_commit: service_commit.to_owned(),
            lib_revision: lib_revision.to_owned(),
            rust_version: rust_version.to_owned(),
            target: target.to_owned(),
            feature_profile: feature_profile.to_owned(),
            config_contract_version,
            state_contract_version,
            admin_contract_version,
            status_contract_version,
            provider_contract_version,
        })
    }

    #[must_use]
    pub fn service_version(&self) -> &str {
        &self.service_version
    }

    #[must_use]
    pub fn service_commit(&self) -> &str {
        &self.service_commit
    }

    #[must_use]
    pub fn lib_revision(&self) -> &str {
        &self.lib_revision
    }

    #[must_use]
    pub fn rust_version(&self) -> &str {
        &self.rust_version
    }

    #[must_use]
    pub fn target(&self) -> &str {
        &self.target
    }

    #[must_use]
    pub fn feature_profile(&self) -> &str {
        &self.feature_profile
    }

    #[must_use]
    pub const fn config_contract_version(&self) -> u32 {
        self.config_contract_version
    }

    #[must_use]
    pub const fn state_contract_version(&self) -> u32 {
        self.state_contract_version
    }

    #[must_use]
    pub const fn admin_contract_version(&self) -> u32 {
        self.admin_contract_version
    }

    #[must_use]
    pub const fn status_contract_version(&self) -> u32 {
        self.status_contract_version
    }

    #[must_use]
    pub const fn provider_contract_version(&self) -> u32 {
        self.provider_contract_version
    }
}

impl fmt::Debug for MigrationBuildIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MigrationBuildIdentity")
            .field("text", &"[redacted]")
            .field("revisions", &"[redacted]")
            .field(
                "contract_versions",
                &[
                    self.config_contract_version,
                    self.state_contract_version,
                    self.admin_contract_version,
                    self.status_contract_version,
                    self.provider_contract_version,
                ],
            )
            .finish()
    }
}

/// Invalid injected migration ledger evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MigrationEvidenceError {
    InvalidAppliedTime,
    InvalidBuildIdentity,
}

impl fmt::Display for MigrationEvidenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidAppliedTime => "migration application time is invalid",
            Self::InvalidBuildIdentity => "migration application build identity is invalid",
        })
    }
}

impl Error for MigrationEvidenceError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MigrationApplicationOutcome {
    initial_version: u32,
    final_version: u32,
    applied_count: u32,
}

impl MigrationApplicationOutcome {
    /// Returns the schema version observed under the migration transaction.
    #[must_use]
    pub const fn initial_version(self) -> u32 {
        self.initial_version
    }

    /// Returns the schema version committed or already present.
    #[must_use]
    pub const fn final_version(self) -> u32 {
        self.final_version
    }

    /// Returns the number of newly committed migration rows.
    #[must_use]
    pub const fn applied_count(self) -> u32 {
        self.applied_count
    }
}

/// Stable, content-free migration contract failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MigrationContractError {
    InvalidName,
    InvalidTargetVersion,
    EmptyContent,
    ContentTooLarge,
    ChecksumMismatch,
    TooManyMigrations,
    DuplicateVersion,
    DuplicateName,
    OutOfOrder,
    VersionGap,
}

impl fmt::Display for MigrationContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidName => "migration name is invalid",
            Self::InvalidTargetVersion => "migration target version is invalid",
            Self::EmptyContent => "migration content is empty",
            Self::ContentTooLarge => "migration content exceeds the limit",
            Self::ChecksumMismatch => "migration checksum does not match",
            Self::TooManyMigrations => "migration catalog exceeds the limit",
            Self::DuplicateVersion => "migration target version is duplicated",
            Self::DuplicateName => "migration name is duplicated",
            Self::OutOfOrder => "migration catalog is out of order",
            Self::VersionGap => "migration catalog contains a version gap",
        })
    }
}

impl Error for MigrationContractError {}

fn valid_name(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.is_empty() || bytes.len() > MAX_MIGRATION_NAME_UTF8_BYTES {
        return false;
    }
    let is_boundary = |byte: u8| byte.is_ascii_lowercase() || byte.is_ascii_digit();
    if !is_boundary(bytes[0]) || !is_boundary(bytes[bytes.len() - 1]) {
        return false;
    }
    let mut previous_underscore = false;
    for byte in bytes {
        if *byte == b'_' {
            if previous_underscore {
                return false;
            }
            previous_underscore = true;
        } else if is_boundary(*byte) {
            previous_underscore = false;
        } else {
            return false;
        }
    }
    true
}

fn valid_build_text(value: &str) -> bool {
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    value.len() <= MAX_MIGRATION_BUILD_ID_UTF8_BYTES
        && first.is_ascii_alphanumeric()
        && bytes
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-'))
}

fn valid_revision(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn validate_catalog(descriptors: &[MigrationDescriptor]) -> Result<(), MigrationContractError> {
    let mut versions = BTreeSet::new();
    let mut names = BTreeSet::new();
    for descriptor in descriptors {
        if !versions.insert(descriptor.target_version) {
            return Err(MigrationContractError::DuplicateVersion);
        }
        if !names.insert(descriptor.name) {
            return Err(MigrationContractError::DuplicateName);
        }
    }
    if descriptors
        .windows(2)
        .any(|pair| pair[0].target_version > pair[1].target_version)
    {
        return Err(MigrationContractError::OutOfOrder);
    }
    for (index, descriptor) in descriptors.iter().enumerate() {
        let expected =
            BASE_SCHEMA_VERSION + u32::try_from(index).expect("catalog bound fits u32") + 1;
        if descriptor.target_version != expected {
            return Err(MigrationContractError::VersionGap);
        }
    }
    Ok(())
}

fn catalog_digest(descriptors: &[MigrationDescriptor]) -> MigrationChecksum {
    let mut hasher = Sha256::new();
    hasher.update(MIGRATION_CATALOG_DOMAIN);
    let descriptor_count =
        u32::try_from(descriptors.len()).expect("migration catalog bound fits u32");
    hasher.update(descriptor_count.to_be_bytes());
    for descriptor in descriptors {
        hasher.update(descriptor.target_version.to_be_bytes());
        let name = descriptor.name.as_str().as_bytes();
        let name_len = u64::try_from(name.len()).expect("migration name bound fits u64");
        hasher.update(name_len.to_be_bytes());
        hasher.update(name);
        hasher.update([descriptor.kind.tag()]);
        hasher.update(descriptor.checksum.as_bytes());
    }
    MigrationChecksum(hasher.finalize().into())
}

pub type MigrationCallbackFuture<'a> =
    Pin<Box<dyn Future<Output = Result<(), ServiceSqliteError>> + Send + 'a>>;

pub type MigrationCallback =
    for<'a> fn(&'a mut MigrationTransactionExecutor<'_>) -> MigrationCallbackFuture<'a>;

pub struct MigrationTransactionExecutor<'a> {
    connection: &'a mut SqliteConnection,
    statement_control_rejected: bool,
}

impl MigrationTransactionExecutor<'_> {
    pub async fn execute(&mut self, sql: &'static str) -> Result<(), ServiceSqliteError> {
        if crate::statement_policy::contains_forbidden_statement_control(sql) {
            self.statement_control_rejected = true;
            return Err(ServiceSqliteError::new(
                crate::ServiceSqliteErrorKind::Migration,
            ));
        }
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            assert_governed_transaction(self.connection).await?;
            let execution = sqlx::raw_sql(sql).execute(&mut *self.connection).await;
            let transaction = assert_governed_transaction(self.connection).await;
            transaction?;
            execution
                .map(|_| ())
                .map_err(|source| migration_source(MigrationFailureKind::Execution, source))
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let _ = (&mut self.connection, sql);
            Err(ServiceSqliteError::new(
                crate::ServiceSqliteErrorKind::Migration,
            ))
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[derive(PartialEq, Eq)]
pub(crate) struct MigrationConnectionPolicy {
    application_id: i64,
    journal_mode: String,
    synchronous: i64,
    foreign_keys: i64,
    trusted_schema: i64,
    busy_timeout: i64,
    query_only: i64,
}

#[derive(Clone, Copy)]
pub struct MigrationCallbackBinding {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    target_version: u32,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    name: MigrationName,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    checksum: MigrationChecksum,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    callback: MigrationCallback,
}

impl MigrationCallbackBinding {
    pub const fn new(
        target_version: u32,
        name: MigrationName,
        checksum: MigrationChecksum,
        callback: MigrationCallback,
    ) -> Self {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            Self {
                target_version,
                name,
                checksum,
                callback,
            }
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let _ = (target_version, name, checksum, callback);
            Self {}
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MigrationFailureKind {
    CatalogMismatch,
    HistoryCorrupt,
    CallbackBinding,
    Execution,
    LedgerWrite,
    MetadataAdvance,
    Commit,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[derive(Debug)]
struct MigrationFailure(MigrationFailureKind);

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl fmt::Display for MigrationFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.0 {
            MigrationFailureKind::CatalogMismatch => "migration catalog does not match state",
            MigrationFailureKind::HistoryCorrupt => "migration history is corrupt",
            MigrationFailureKind::CallbackBinding => "migration callback binding is invalid",
            MigrationFailureKind::Execution => "migration execution failed",
            MigrationFailureKind::LedgerWrite => "migration ledger write failed",
            MigrationFailureKind::MetadataAdvance => "migration metadata advance failed",
            MigrationFailureKind::Commit => "migration commit outcome is unavailable",
        })
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl Error for MigrationFailure {}

#[cfg(any(target_os = "linux", target_os = "macos"))]
struct MigrationSource {
    kind: MigrationFailureKind,
    source: Box<dyn Error + Send + Sync + 'static>,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl fmt::Debug for MigrationSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MigrationSource")
            .field("kind", &self.kind)
            .field("source", &"[redacted]")
            .finish()
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl fmt::Display for MigrationSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        MigrationFailure(self.kind).fmt(formatter)
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl Error for MigrationSource {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.source.as_ref())
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn migration_error(kind: MigrationFailureKind) -> ServiceSqliteError {
    ServiceSqliteError::with_source(ServiceSqliteErrorKind::Migration, MigrationFailure(kind))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn migration_source(
    kind: MigrationFailureKind,
    source: impl Error + Send + Sync + 'static,
) -> ServiceSqliteError {
    ServiceSqliteError::with_source(
        ServiceSqliteErrorKind::Migration,
        MigrationSource {
            kind,
            source: Box::new(source),
        },
    )
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn require_migration_condition(
    condition: bool,
    kind: MigrationFailureKind,
) -> Result<(), ServiceSqliteError> {
    condition.then_some(()).ok_or_else(|| migration_error(kind))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[derive(Clone, PartialEq, Eq)]
struct AppliedMigration {
    version: u32,
    name: String,
    checksum: MigrationChecksum,
    applied_at: MigrationAppliedAtUnixSeconds,
    build: MigrationBuildIdentity,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) async fn verify_migration_history(
    connection: &mut SqliteConnection,
    catalog: &MigrationCatalog,
    schema_catalog: &SchemaCatalog,
    require_current: bool,
) -> Result<u32, ServiceSqliteError> {
    schema_catalog
        .matches_migrations(catalog)
        .then_some(())
        .ok_or_else(|| ServiceSqliteError::new(ServiceSqliteErrorKind::Integrity))?;
    let mut transaction = connection
        .begin()
        .await
        .map_err(|source| migration_source(MigrationFailureKind::HistoryCorrupt, source))?;
    let result = verify_migration_history_snapshot(
        &mut transaction,
        catalog,
        schema_catalog,
        require_current,
    )
    .await;
    let rollback = transaction.rollback().await;
    match (result, rollback) {
        (Err(error), _) => Err(error),
        (Ok(_), Err(source)) => Err(migration_source(
            MigrationFailureKind::HistoryCorrupt,
            source,
        )),
        (Ok(version), Ok(())) => Ok(version),
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) async fn verify_migration_history_snapshot(
    connection: &mut SqliteConnection,
    catalog: &MigrationCatalog,
    schema_catalog: &SchemaCatalog,
    require_current: bool,
) -> Result<u32, ServiceSqliteError> {
    let version = read_state_schema_version(connection).await?;
    let history = read_migration_history(connection).await?;
    validate_migration_prefix(catalog, version, &history)?;
    crate::integrity::verify_schema_catalog(connection, schema_catalog, version).await?;
    if require_current && version != catalog.current_version() {
        return Err(migration_error(MigrationFailureKind::CatalogMismatch));
    }
    Ok(version)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) async fn apply_governed_migrations<V>(
    connection: &mut SqliteConnection,
    catalog: &MigrationCatalog,
    schema_catalog: &SchemaCatalog,
    applied_at: MigrationAppliedAtUnixSeconds,
    build: &MigrationBuildIdentity,
    callback_bindings: &[MigrationCallbackBinding],
    validate_authority: &mut V,
) -> Result<MigrationApplicationOutcome, ServiceSqliteError>
where
    V: FnMut() -> Result<(), ServiceSqliteError>,
{
    let mut after_commit = || Ok(());
    apply_governed_migrations_with_observer(
        connection,
        catalog,
        schema_catalog,
        applied_at,
        build,
        callback_bindings,
        validate_authority,
        &mut after_commit,
    )
    .await
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[allow(clippy::too_many_arguments)]
async fn apply_governed_migrations_with_observer<V, O>(
    connection: &mut SqliteConnection,
    catalog: &MigrationCatalog,
    schema_catalog: &SchemaCatalog,
    applied_at: MigrationAppliedAtUnixSeconds,
    build: &MigrationBuildIdentity,
    callback_bindings: &[MigrationCallbackBinding],
    validate_authority: &mut V,
    after_commit: &mut O,
) -> Result<MigrationApplicationOutcome, ServiceSqliteError>
where
    V: FnMut() -> Result<(), ServiceSqliteError>,
    O: FnMut() -> Result<(), ServiceSqliteError>,
{
    schema_catalog
        .matches_migrations(catalog)
        .then_some(())
        .ok_or_else(|| ServiceSqliteError::new(ServiceSqliteErrorKind::Integrity))?;
    let callbacks = validate_callback_bindings(catalog, callback_bindings)?;
    let mut initial_version = None;
    let mut applied_count = 0_u32;
    validate_authority()?;
    let initial_policy_result = read_connection_policy(connection).await;
    validate_authority()?;
    let initial_policy = initial_policy_result?;

    loop {
        validate_authority()?;
        let gate_result = crate::transaction_control::TransactionControlGate::install(connection)
            .await
            .map_err(|source| migration_source(MigrationFailureKind::Execution, source));
        validate_authority()?;
        let commit_gate = gate_result?;
        let transaction_result = connection.begin_with("BEGIN IMMEDIATE").await;
        validate_authority()?;
        let mut transaction = transaction_result
            .map_err(|source| migration_source(MigrationFailureKind::Execution, source))?;
        let transactional_result =
            verify_migration_history_snapshot(&mut transaction, catalog, schema_catalog, false)
                .await;
        validate_authority()?;
        let current = transactional_result?;
        initial_version.get_or_insert(current);
        if current == catalog.current_version() {
            let rollback_permit = commit_gate.permit_runner_rollback();
            let rollback_result = transaction.rollback().await;
            drop(rollback_permit);
            validate_authority()?;
            rollback_result
                .map_err(|source| migration_source(MigrationFailureKind::Commit, source))?;
            let remove_result = commit_gate
                .remove(connection)
                .await
                .map_err(|source| migration_source(MigrationFailureKind::Commit, source));
            validate_authority()?;
            remove_result?;
            break;
        }
        let descriptor_index = usize::try_from(current.saturating_sub(BASE_SCHEMA_VERSION))
            .map_err(|_| migration_error(MigrationFailureKind::CatalogMismatch))?;
        let descriptor = catalog
            .descriptors()
            .get(descriptor_index)
            .ok_or_else(|| migration_error(MigrationFailureKind::CatalogMismatch))?;
        validate_authority()?;
        let execution_result = execute_descriptor(&mut transaction, descriptor, &callbacks).await;
        validate_authority()?;
        execution_result?;
        require_migration_condition(
            !commit_gate.control_violation_observed(),
            MigrationFailureKind::Execution,
        )?;
        let transaction_result = assert_governed_transaction(&mut transaction).await;
        validate_authority()?;
        transaction_result?;
        let policy_result = read_connection_policy(&mut transaction).await;
        validate_authority()?;
        require_migration_condition(
            policy_result? == initial_policy,
            MigrationFailureKind::Execution,
        )?;
        let transaction_result = assert_governed_transaction(&mut transaction).await;
        validate_authority()?;
        transaction_result?;
        let schema_result = crate::integrity::verify_schema_catalog(
            &mut transaction,
            schema_catalog,
            descriptor.target_version(),
        )
        .await;
        validate_authority()?;
        schema_result?;
        let insert_result =
            insert_migration_row(&mut transaction, descriptor, applied_at, build).await;
        validate_authority()?;
        insert_result?;
        let transaction_result = assert_governed_transaction(&mut transaction).await;
        validate_authority()?;
        transaction_result?;
        let advance_result =
            advance_schema_version(&mut transaction, current, descriptor.target_version()).await;
        validate_authority()?;
        advance_result?;
        let transaction_result = assert_governed_transaction(&mut transaction).await;
        validate_authority()?;
        transaction_result?;
        require_migration_condition(
            !commit_gate.control_violation_observed(),
            MigrationFailureKind::Execution,
        )?;
        let policy_result = read_connection_policy(&mut transaction).await;
        validate_authority()?;
        require_migration_condition(
            policy_result? == initial_policy,
            MigrationFailureKind::Execution,
        )?;
        let transaction_result = assert_governed_transaction(&mut transaction).await;
        validate_authority()?;
        transaction_result?;
        require_migration_condition(
            !commit_gate.control_violation_observed(),
            MigrationFailureKind::Execution,
        )?;
        let permit = commit_gate.permit_outer_commit();
        let commit_result = transaction.commit().await;
        drop(permit);
        validate_authority()?;
        commit_result.map_err(|source| migration_source(MigrationFailureKind::Commit, source))?;
        let remove_result = commit_gate
            .remove(connection)
            .await
            .map_err(|source| migration_source(MigrationFailureKind::Commit, source));
        validate_authority()?;
        remove_result?;
        let observed = after_commit();
        validate_authority()?;
        observed?;
        applied_count = applied_count.saturating_add(1);
    }

    validate_authority()?;
    let final_result = verify_migration_history(connection, catalog, schema_catalog, true).await;
    validate_authority()?;
    let final_version = final_result?;
    Ok(MigrationApplicationOutcome {
        initial_version: initial_version.unwrap_or(final_version),
        final_version,
        applied_count,
    })
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn validate_callback_bindings(
    catalog: &MigrationCatalog,
    bindings: &[MigrationCallbackBinding],
) -> Result<BTreeMap<u32, MigrationCallback>, ServiceSqliteError> {
    let expected = catalog
        .descriptors()
        .iter()
        .filter(|descriptor| descriptor.kind() == MigrationKind::Callback)
        .count();
    if bindings.len() != expected {
        return Err(migration_error(MigrationFailureKind::CallbackBinding));
    }
    let mut callbacks = BTreeMap::new();
    for binding in bindings {
        let descriptor = catalog
            .descriptors()
            .iter()
            .find(|descriptor| descriptor.target_version() == binding.target_version)
            .ok_or_else(|| migration_error(MigrationFailureKind::CallbackBinding))?;
        let unique = callbacks
            .insert(binding.target_version, binding.callback)
            .is_none();
        if !crate::all_constraints([
            descriptor.kind() == MigrationKind::Callback,
            descriptor.name() == binding.name,
            descriptor.checksum() == binding.checksum,
            unique,
        ]) {
            return Err(migration_error(MigrationFailureKind::CallbackBinding));
        }
    }
    Ok(callbacks)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
async fn execute_descriptor(
    connection: &mut SqliteConnection,
    descriptor: &MigrationDescriptor,
    callbacks: &BTreeMap<u32, MigrationCallback>,
) -> Result<(), ServiceSqliteError> {
    let mut executor = MigrationTransactionExecutor {
        connection,
        statement_control_rejected: false,
    };
    match descriptor.kind() {
        MigrationKind::Sql => {
            let sql = core::str::from_utf8(descriptor.content)
                .map_err(|source| migration_source(MigrationFailureKind::Execution, source))?;
            executor.execute(sql).await?;
        }
        MigrationKind::Callback => {
            let callback = callbacks
                .get(&descriptor.target_version())
                .ok_or_else(|| migration_error(MigrationFailureKind::CallbackBinding))?;
            callback(&mut executor)
                .await
                .map_err(|source| migration_source(MigrationFailureKind::Execution, source))?;
            assert_governed_transaction(executor.connection).await?;
        }
    }
    if executor.statement_control_rejected {
        return Err(migration_error(MigrationFailureKind::Execution));
    }
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) async fn assert_governed_transaction(
    connection: &mut SqliteConnection,
) -> Result<(), ServiceSqliteError> {
    sqlx::raw_sql(
        "SAVEPOINT radroots_migration_transaction_probe;
         RELEASE SAVEPOINT radroots_migration_transaction_probe;",
    )
    .execute(connection)
    .await
    .map(|_| ())
    .map_err(|source| migration_source(MigrationFailureKind::Execution, source))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) async fn read_connection_policy(
    connection: &mut SqliteConnection,
) -> Result<MigrationConnectionPolicy, ServiceSqliteError> {
    let text = |source| migration_source(MigrationFailureKind::Execution, source);
    let application_id = sqlx::query_scalar::<_, i64>("PRAGMA application_id")
        .fetch_one(&mut *connection)
        .await
        .map_err(text)?;
    let journal_mode = sqlx::query_scalar::<_, String>("PRAGMA journal_mode")
        .fetch_one(&mut *connection)
        .await
        .map_err(text)?;
    require_migration_condition(journal_mode.len() <= 16, MigrationFailureKind::Execution)?;
    let synchronous = sqlx::query_scalar::<_, i64>("PRAGMA synchronous")
        .fetch_one(&mut *connection)
        .await
        .map_err(text)?;
    let foreign_keys = sqlx::query_scalar::<_, i64>("PRAGMA foreign_keys")
        .fetch_one(&mut *connection)
        .await
        .map_err(text)?;
    let trusted_schema = sqlx::query_scalar::<_, i64>("PRAGMA trusted_schema")
        .fetch_one(&mut *connection)
        .await
        .map_err(text)?;
    let busy_timeout = sqlx::query_scalar::<_, i64>("PRAGMA busy_timeout")
        .fetch_one(&mut *connection)
        .await
        .map_err(text)?;
    let query_only = sqlx::query_scalar::<_, i64>("PRAGMA query_only")
        .fetch_one(&mut *connection)
        .await
        .map_err(text)?;
    let databases = sqlx::query_scalar::<_, String>(
        "SELECT CASE
             WHEN typeof(name) = 'text' AND length(CAST(name AS BLOB)) <= 4 THEN name
             ELSE ''
         END
         FROM pragma_database_list
         ORDER BY seq
         LIMIT 2",
    )
    .fetch_all(connection)
    .await
    .map_err(text)?;
    require_migration_condition(databases == ["main"], MigrationFailureKind::Execution)?;
    Ok(MigrationConnectionPolicy {
        application_id,
        journal_mode,
        synchronous,
        foreign_keys,
        trusted_schema,
        busy_timeout,
        query_only,
    })
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
async fn insert_migration_row(
    connection: &mut SqliteConnection,
    descriptor: &MigrationDescriptor,
    applied_at: MigrationAppliedAtUnixSeconds,
    build: &MigrationBuildIdentity,
) -> Result<(), ServiceSqliteError> {
    let result = sqlx::query(
        "INSERT INTO schema_migrations (
            version, name, checksum, applied_at_unix_s,
            service_version, service_commit, lib_revision, rust_version, target, feature_profile,
            config_contract_version, state_contract_version, admin_contract_version,
            status_contract_version, provider_contract_version
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(i64::from(descriptor.target_version()))
    .bind(descriptor.name().as_str())
    .bind(descriptor.checksum().as_bytes().as_slice())
    .bind(
        i64::try_from(applied_at.get())
            .map_err(|_| migration_error(MigrationFailureKind::LedgerWrite))?,
    )
    .bind(build.service_version())
    .bind(build.service_commit())
    .bind(build.lib_revision())
    .bind(build.rust_version())
    .bind(build.target())
    .bind(build.feature_profile())
    .bind(i64::from(build.config_contract_version()))
    .bind(i64::from(build.state_contract_version()))
    .bind(i64::from(build.admin_contract_version()))
    .bind(i64::from(build.status_contract_version()))
    .bind(i64::from(build.provider_contract_version()))
    .execute(connection)
    .await
    .map_err(|source| migration_source(MigrationFailureKind::LedgerWrite, source))?;
    require_migration_condition(
        result.rows_affected() == 1,
        MigrationFailureKind::LedgerWrite,
    )?;
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
async fn advance_schema_version(
    connection: &mut SqliteConnection,
    current: u32,
    target: u32,
) -> Result<(), ServiceSqliteError> {
    let result = sqlx::query(
        "UPDATE radroots_service_metadata
         SET state_schema_version = ?
         WHERE singleton = 1 AND state_schema_version = ?",
    )
    .bind(i64::from(target))
    .bind(i64::from(current))
    .execute(connection)
    .await
    .map_err(|source| migration_source(MigrationFailureKind::MetadataAdvance, source))?;
    require_migration_condition(
        result.rows_affected() == 1,
        MigrationFailureKind::MetadataAdvance,
    )?;
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
async fn read_state_schema_version(
    connection: &mut SqliteConnection,
) -> Result<u32, ServiceSqliteError> {
    let rows = sqlx::query(
        "SELECT state_schema_version, typeof(state_schema_version) AS version_type
         FROM radroots_service_metadata
         WHERE singleton = 1
         LIMIT 2",
    )
    .fetch_all(connection)
    .await
    .map_err(|source| migration_source(MigrationFailureKind::HistoryCorrupt, source))?;
    let [row] = rows.as_slice() else {
        return Err(migration_error(MigrationFailureKind::HistoryCorrupt));
    };
    if row
        .try_get::<String, _>("version_type")
        .map_err(|source| migration_source(MigrationFailureKind::HistoryCorrupt, source))?
        != "integer"
    {
        return Err(migration_error(MigrationFailureKind::HistoryCorrupt));
    }
    u32::try_from(
        row.try_get::<i64, _>("state_schema_version")
            .map_err(|source| migration_source(MigrationFailureKind::HistoryCorrupt, source))?,
    )
    .map_err(|_| migration_error(MigrationFailureKind::HistoryCorrupt))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
async fn read_migration_history(
    connection: &mut SqliteConnection,
) -> Result<Vec<AppliedMigration>, ServiceSqliteError> {
    let rows = sqlx::query(
        "SELECT
            version,
            CASE WHEN typeof(name) = 'text' AND length(CAST(name AS BLOB)) <= 128
                 THEN name END AS name,
            CASE WHEN typeof(checksum) = 'blob' AND length(checksum) <= 32
                 THEN checksum END AS checksum,
            applied_at_unix_s,
            CASE WHEN typeof(service_version) = 'text'
                           AND length(CAST(service_version AS BLOB)) <= 128
                 THEN service_version END AS service_version,
            CASE WHEN typeof(service_commit) = 'text'
                           AND length(CAST(service_commit AS BLOB)) <= 40
                 THEN service_commit END AS service_commit,
            CASE WHEN typeof(lib_revision) = 'text'
                           AND length(CAST(lib_revision AS BLOB)) <= 40
                 THEN lib_revision END AS lib_revision,
            CASE WHEN typeof(rust_version) = 'text'
                           AND length(CAST(rust_version AS BLOB)) <= 128
                 THEN rust_version END AS rust_version,
            CASE WHEN typeof(target) = 'text' AND length(CAST(target AS BLOB)) <= 128
                 THEN target END AS target,
            CASE WHEN typeof(feature_profile) = 'text'
                           AND length(CAST(feature_profile AS BLOB)) <= 128
                 THEN feature_profile END AS feature_profile,
            config_contract_version, state_contract_version, admin_contract_version,
            status_contract_version, provider_contract_version,
            typeof(version) AS version_type, typeof(name) AS name_type,
            typeof(checksum) AS checksum_type, typeof(applied_at_unix_s) AS applied_at_type,
            typeof(service_version) AS service_version_type,
            typeof(service_commit) AS service_commit_type,
            typeof(lib_revision) AS lib_revision_type,
            typeof(rust_version) AS rust_version_type,
            typeof(target) AS target_type, typeof(feature_profile) AS feature_profile_type,
            typeof(config_contract_version) AS config_contract_version_type,
            typeof(state_contract_version) AS state_contract_version_type,
            typeof(admin_contract_version) AS admin_contract_version_type,
            typeof(status_contract_version) AS status_contract_version_type,
            typeof(provider_contract_version) AS provider_contract_version_type
         FROM schema_migrations
         ORDER BY version
         LIMIT 4097",
    )
    .fetch_all(connection)
    .await
    .map_err(|source| migration_source(MigrationFailureKind::HistoryCorrupt, source))?;
    require_migration_condition(
        rows.len() <= MAX_MIGRATION_COUNT,
        MigrationFailureKind::HistoryCorrupt,
    )?;
    rows.iter().map(parse_applied_migration).collect()
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn parse_applied_migration(
    row: &sqlx::sqlite::SqliteRow,
) -> Result<AppliedMigration, ServiceSqliteError> {
    for (column, expected_type) in [
        ("version_type", "integer"),
        ("name_type", "text"),
        ("checksum_type", "blob"),
        ("applied_at_type", "integer"),
        ("service_version_type", "text"),
        ("service_commit_type", "text"),
        ("lib_revision_type", "text"),
        ("rust_version_type", "text"),
        ("target_type", "text"),
        ("feature_profile_type", "text"),
        ("config_contract_version_type", "integer"),
        ("state_contract_version_type", "integer"),
        ("admin_contract_version_type", "integer"),
        ("status_contract_version_type", "integer"),
        ("provider_contract_version_type", "integer"),
    ] {
        if row
            .try_get::<String, _>(column)
            .map_err(|source| migration_source(MigrationFailureKind::HistoryCorrupt, source))?
            != expected_type
        {
            return Err(migration_error(MigrationFailureKind::HistoryCorrupt));
        }
    }
    let version = u32::try_from(
        row.try_get::<i64, _>("version")
            .map_err(|source| migration_source(MigrationFailureKind::HistoryCorrupt, source))?,
    )
    .map_err(|_| migration_error(MigrationFailureKind::HistoryCorrupt))?;
    let name = row
        .try_get::<String, _>("name")
        .map_err(|source| migration_source(MigrationFailureKind::HistoryCorrupt, source))?;
    if !valid_name(&name) {
        return Err(migration_error(MigrationFailureKind::HistoryCorrupt));
    }
    let checksum: [u8; 32] = row
        .try_get::<Vec<u8>, _>("checksum")
        .map_err(|source| migration_source(MigrationFailureKind::HistoryCorrupt, source))?
        .try_into()
        .map_err(|_| migration_error(MigrationFailureKind::HistoryCorrupt))?;
    let applied_at = MigrationAppliedAtUnixSeconds::new(
        u64::try_from(
            row.try_get::<i64, _>("applied_at_unix_s")
                .map_err(|source| migration_source(MigrationFailureKind::HistoryCorrupt, source))?,
        )
        .map_err(|_| migration_error(MigrationFailureKind::HistoryCorrupt))?,
    )
    .map_err(|_| migration_error(MigrationFailureKind::HistoryCorrupt))?;
    let text = |column| {
        row.try_get::<String, _>(column)
            .map_err(|source| migration_source(MigrationFailureKind::HistoryCorrupt, source))
    };
    let version_field = |column| {
        u32::try_from(
            row.try_get::<i64, _>(column)
                .map_err(|source| migration_source(MigrationFailureKind::HistoryCorrupt, source))?,
        )
        .map_err(|_| migration_error(MigrationFailureKind::HistoryCorrupt))
    };
    let build = MigrationBuildIdentity::new(
        text("service_version")?,
        text("service_commit")?,
        text("lib_revision")?,
        text("rust_version")?,
        text("target")?,
        text("feature_profile")?,
        version_field("config_contract_version")?,
        version_field("state_contract_version")?,
        version_field("admin_contract_version")?,
        version_field("status_contract_version")?,
        version_field("provider_contract_version")?,
    )
    .map_err(|_| migration_error(MigrationFailureKind::HistoryCorrupt))?;
    Ok(AppliedMigration {
        version,
        name,
        checksum: MigrationChecksum::from_bytes(checksum),
        applied_at,
        build,
    })
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn validate_migration_prefix(
    catalog: &MigrationCatalog,
    version: u32,
    history: &[AppliedMigration],
) -> Result<(), ServiceSqliteError> {
    require_migration_condition(
        version >= BASE_SCHEMA_VERSION && version <= catalog.current_version(),
        MigrationFailureKind::CatalogMismatch,
    )?;
    let expected_len = usize::try_from(version - BASE_SCHEMA_VERSION)
        .map_err(|_| migration_error(MigrationFailureKind::HistoryCorrupt))?;
    require_migration_condition(
        history.len() == expected_len,
        MigrationFailureKind::CatalogMismatch,
    )?;
    for (applied, descriptor) in history.iter().zip(catalog.descriptors()) {
        if !crate::all_constraints([
            applied.version == descriptor.target_version(),
            applied.name == descriptor.name().as_str(),
            applied.checksum == descriptor.checksum(),
        ]) {
            return Err(migration_error(MigrationFailureKind::CatalogMismatch));
        }
        let _ = (applied.applied_at, &applied.build);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn migration_failure_inventory_is_complete_and_source_aware() {
        use std::error::Error as _;

        let cases = [
            (
                MigrationFailureKind::CatalogMismatch,
                "migration catalog does not match state",
            ),
            (
                MigrationFailureKind::HistoryCorrupt,
                "migration history is corrupt",
            ),
            (
                MigrationFailureKind::CallbackBinding,
                "migration callback binding is invalid",
            ),
            (
                MigrationFailureKind::Execution,
                "migration execution failed",
            ),
            (
                MigrationFailureKind::LedgerWrite,
                "migration ledger write failed",
            ),
            (
                MigrationFailureKind::MetadataAdvance,
                "migration metadata advance failed",
            ),
            (
                MigrationFailureKind::Commit,
                "migration commit outcome is unavailable",
            ),
        ];
        for (kind, message) in cases {
            let plain = MigrationFailure(kind);
            assert_eq!(plain.to_string(), message);
            assert!(plain.source().is_none());

            let sourced = MigrationSource {
                kind,
                source: Box::new(std::io::Error::other("private-cause")),
            };
            assert_eq!(sourced.to_string(), message);
            assert!(sourced.source().is_some());
            let debug = format!("{sourced:?}");
            assert!(debug.contains("[redacted]"));
            assert!(!debug.contains("private-cause"));
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn migration_condition_classifier_preserves_every_stable_kind() {
        for kind in [
            MigrationFailureKind::CatalogMismatch,
            MigrationFailureKind::CallbackBinding,
            MigrationFailureKind::HistoryCorrupt,
            MigrationFailureKind::Execution,
            MigrationFailureKind::LedgerWrite,
            MigrationFailureKind::MetadataAdvance,
            MigrationFailureKind::Commit,
        ] {
            assert!(require_migration_condition(true, kind).is_ok());
            let error = require_migration_condition(false, kind).expect_err("failure");
            assert_eq!(error.kind(), ServiceSqliteErrorKind::Migration);
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    use std::{
        num::NonZeroU32,
        path::{Path, PathBuf},
        sync::{
            Mutex,
            atomic::{AtomicUsize, Ordering as AtomicOrdering},
        },
    };

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    use radroots_runtime_paths::{
        InstanceId, RadrootsHostEnvironment, RadrootsPathProfile, RadrootsPathResolver,
        RadrootsPlatform, RuntimeContext, RuntimeContextBootstrap, RuntimeContextSource, ServiceId,
    };
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    use radroots_storage::event::SourceGeneration;
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    use sqlx::sqlite::SqliteConnectOptions;

    const SQL_TWO: &str = "CREATE TABLE alpha (id INTEGER PRIMARY KEY);";
    const CALLBACK_THREE: &[u8] = b"callback:rebuild_projection:v1";
    const SQL_TWO_CHECKSUM: MigrationChecksum = MigrationChecksum::from_bytes([
        0xd9, 0xa8, 0x5f, 0x7a, 0x59, 0x04, 0x0b, 0x3b, 0x25, 0x86, 0x56, 0x48, 0x02, 0x44, 0x10,
        0x93, 0x07, 0xaa, 0x3d, 0x1a, 0x5d, 0xec, 0x04, 0x06, 0xa7, 0x50, 0x99, 0x4f, 0x17, 0xe8,
        0x91, 0x13,
    ]);
    const CALLBACK_SQL_BYTES_CHECKSUM: MigrationChecksum = MigrationChecksum::from_bytes([
        0x7a, 0x6e, 0x62, 0xf7, 0xf7, 0xa4, 0xf6, 0x1a, 0xb9, 0x14, 0x84, 0xbf, 0xe6, 0xa1, 0x2f,
        0xf5, 0x0d, 0x62, 0x3d, 0x8d, 0xa2, 0x74, 0x8a, 0x16, 0xf9, 0x18, 0xd9, 0x9a, 0x52, 0xae,
        0xf6, 0x15,
    ]);
    const CALLBACK_THREE_CHECKSUM: MigrationChecksum = MigrationChecksum::from_bytes([
        0x7d, 0xca, 0x22, 0x77, 0x1b, 0x17, 0xa9, 0xf2, 0xc8, 0x04, 0x4b, 0xdc, 0xf6, 0xa6, 0xfa,
        0xea, 0x41, 0x46, 0xc3, 0x56, 0xb2, 0x20, 0x17, 0xe1, 0x91, 0xd1, 0xe5, 0x42, 0xbb, 0x69,
        0x47, 0x66,
    ]);

    fn sql(version: u32, name: &'static str, source: &'static str) -> MigrationDescriptor {
        MigrationDescriptor::sql(version, name, source, MigrationChecksum::for_sql(source))
            .expect("valid SQL descriptor")
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn table_object(name: &'static str, sql: &'static str) -> crate::SchemaObject {
        crate::SchemaObject::new(
            crate::SchemaObjectKind::Table,
            name,
            name,
            sql,
            crate::SchemaObject::computed_digest(crate::SchemaObjectKind::Table, name, name, sql)
                .expect("schema table digest"),
        )
        .expect("schema table")
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn schema_catalog(
        migrations: &MigrationCatalog,
        versions: Vec<Vec<crate::SchemaObject>>,
    ) -> crate::SchemaCatalog {
        let versions = versions
            .into_iter()
            .enumerate()
            .map(|(index, objects)| {
                let version = u32::try_from(index + 1).expect("schema version");
                let digest =
                    crate::SchemaVersionCatalog::computed_digest(version, objects.iter().cloned())
                        .expect("schema digest");
                crate::SchemaVersionCatalog::new(version, objects, digest)
                    .expect("schema version catalog")
            })
            .collect::<Vec<_>>();
        crate::SchemaCatalog::new(migrations, versions).expect("schema catalog")
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn unchanged_schema_catalog(migrations: &MigrationCatalog) -> crate::SchemaCatalog {
        schema_catalog(
            migrations,
            (0..migrations.current_version())
                .map(|_| Vec::new())
                .collect(),
        )
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn alpha_schema_catalog(migrations: &MigrationCatalog) -> crate::SchemaCatalog {
        const ALPHA_SQL: &str = "CREATE TABLE alpha (id INTEGER PRIMARY KEY)";
        let alpha = table_object("alpha", ALPHA_SQL);
        let mut versions = vec![Vec::new()];
        versions.extend((1..migrations.current_version()).map(|_| vec![alpha.clone()]));
        schema_catalog(migrations, versions)
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn alpha_beta_schema_catalog(
        migrations: &MigrationCatalog,
        beta_sql: &'static str,
    ) -> crate::SchemaCatalog {
        const ALPHA_SQL: &str = "CREATE TABLE alpha (id INTEGER PRIMARY KEY)";
        let alpha = table_object("alpha", ALPHA_SQL);
        let beta = table_object("beta", beta_sql);
        schema_catalog(
            migrations,
            vec![Vec::new(), vec![alpha.clone()], vec![alpha, beta]],
        )
    }

    fn build_identity() -> MigrationBuildIdentity {
        MigrationBuildIdentity::new(
            "0.1.0-alpha",
            "0123456789abcdef0123456789abcdef01234567",
            "89abcdef0123456789abcdef0123456789abcdef",
            "1.97.1",
            "x86_64-unknown-linux-gnu",
            "service-host",
            1,
            2,
            3,
            4,
            5,
        )
        .expect("valid build identity")
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    async fn initialized_memory_database() -> SqliteConnection {
        initialized_database(SqliteConnectOptions::new().filename(":memory:")).await
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    async fn initialized_file_database(path: &Path) -> SqliteConnection {
        initialized_database(
            SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true),
        )
        .await
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    async fn initialized_database(options: SqliteConnectOptions) -> SqliteConnection {
        let context = RuntimeContext::resolve(
            &RadrootsPathResolver::new(RadrootsPlatform::Linux, RadrootsHostEnvironment::default()),
            RuntimeContextBootstrap::new(
                RadrootsPathProfile::RepoLocal,
                Some(PathBuf::from("/isolated/migration-tests")),
                RuntimeContextSource::BootstrapCli,
                RuntimeContextSource::BootstrapCli,
            )
            .expect("runtime bootstrap"),
            ServiceId::new("myc").expect("service"),
            InstanceId::new("primary").expect("instance"),
        )
        .expect("runtime context");
        let paths =
            crate::ServiceSqlitePaths::from_runtime_context(&context).expect("SQLite paths");
        let metadata = crate::ServiceDatabaseMetadata::new(
            &paths,
            SourceGeneration::new([7; 32]).expect("generation"),
            NonZeroU32::new(1).expect("schema"),
            1_700_000_000_000,
            crate::ServiceSqliteApplicationId::new(0x5244_5351).expect("application ID"),
        )
        .expect("metadata");
        let mut connection = SqliteConnection::connect_with(&options)
            .await
            .expect("test SQLite");
        let migrations = MigrationCatalog::new([]).expect("empty migration catalog");
        let schema_catalog = unchanged_schema_catalog(&migrations);
        crate::metadata::write_database_metadata(&mut connection, &metadata, &schema_catalog)
            .await
            .expect("initialize metadata and ledger");
        connection
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn insert_projection_callback<'a>(
        executor: &'a mut MigrationTransactionExecutor<'_>,
    ) -> MigrationCallbackFuture<'a> {
        Box::pin(async move { executor.execute("INSERT INTO alpha (id) VALUES (41)").await })
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn pending_projection_callback<'a>(
        executor: &'a mut MigrationTransactionExecutor<'_>,
    ) -> MigrationCallbackFuture<'a> {
        Box::pin(async move {
            executor
                .execute("INSERT INTO alpha (id) VALUES (99)")
                .await?;
            PENDING_CALLBACK_COUNT.fetch_add(1, AtomicOrdering::SeqCst);
            core::future::pending::<Result<(), ServiceSqliteError>>().await
        })
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn rollback_escape_callback<'a>(
        executor: &'a mut MigrationTransactionExecutor<'_>,
    ) -> MigrationCallbackFuture<'a> {
        Box::pin(async move {
            let _ = executor
                .execute(
                    "CREATE TABLE callback_rolled_back (id INTEGER PRIMARY KEY);
                     ROLLBACK;
                     BEGIN DEFERRED;
                     CREATE TABLE callback_leaked (id INTEGER PRIMARY KEY);",
                )
                .await;
            Ok(())
        })
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn ignored_statement_control_callback<'a>(
        executor: &'a mut MigrationTransactionExecutor<'_>,
    ) -> MigrationCallbackFuture<'a> {
        let sql = IGNORED_STATEMENT_CONTROL_SQL
            .lock()
            .expect("statement-control SQL mutex")
            .expect("statement-control SQL is installed");
        Box::pin(async move {
            let _ = executor.execute(sql).await;
            Ok(())
        })
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    static PENDING_CALLBACK_COUNT: AtomicUsize = AtomicUsize::new(0);

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    static IGNORED_STATEMENT_CONTROL_SQL: Mutex<Option<&'static str>> = Mutex::new(None);

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    async fn replace_with_permissive_ledger(connection: &mut SqliteConnection) {
        sqlx::raw_sql(
            "DROP TRIGGER schema_migrations_no_update;
             DROP TRIGGER schema_migrations_no_delete;
             DROP TABLE schema_migrations;
             CREATE TABLE schema_migrations (
                version, name, checksum, applied_at_unix_s,
                service_version, service_commit, lib_revision, rust_version, target,
                feature_profile, config_contract_version, state_contract_version,
                admin_contract_version, status_contract_version, provider_contract_version
             );",
        )
        .execute(connection)
        .await
        .expect("replace ledger for corrupt-state test");
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    async fn insert_permissive_history_row(
        connection: &mut SqliteConnection,
        version: i64,
        name: &str,
        checksum: &[u8],
        applied_at: i64,
        service_version: &str,
    ) {
        let build = build_identity();
        sqlx::query(
            "INSERT INTO schema_migrations (
                version, name, checksum, applied_at_unix_s,
                service_version, service_commit, lib_revision, rust_version, target,
                feature_profile, config_contract_version, state_contract_version,
                admin_contract_version, status_contract_version, provider_contract_version
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1, 2, 3, 4, 5)",
        )
        .bind(version)
        .bind(name)
        .bind(checksum)
        .bind(applied_at)
        .bind(service_version)
        .bind(build.service_commit())
        .bind(build.lib_revision())
        .bind(build.rust_version())
        .bind(build.target())
        .bind(build.feature_profile())
        .execute(connection)
        .await
        .expect("insert corrupt-state row");
    }

    #[test]
    fn applied_time_and_complete_build_identity_are_exact_and_bounded() {
        assert_eq!(MigrationAppliedAtUnixSeconds::new(0).unwrap().get(), 0);
        assert_eq!(
            MigrationAppliedAtUnixSeconds::new(i64::MAX as u64)
                .unwrap()
                .get(),
            i64::MAX as u64
        );
        assert_eq!(
            MigrationAppliedAtUnixSeconds::new(i64::MAX as u64 + 1),
            Err(MigrationEvidenceError::InvalidAppliedTime)
        );

        let build = build_identity();
        assert_eq!(build.service_version(), "0.1.0-alpha");
        assert_eq!(
            build.service_commit(),
            "0123456789abcdef0123456789abcdef01234567"
        );
        assert_eq!(
            build.lib_revision(),
            "89abcdef0123456789abcdef0123456789abcdef"
        );
        assert_eq!(build.rust_version(), "1.97.1");
        assert_eq!(build.target(), "x86_64-unknown-linux-gnu");
        assert_eq!(build.feature_profile(), "service-host");
        assert_eq!(
            [
                build.config_contract_version(),
                build.state_contract_version(),
                build.admin_contract_version(),
                build.status_contract_version(),
                build.provider_contract_version(),
            ],
            [1, 2, 3, 4, 5]
        );
        let debug = format!("{build:?}");
        for hidden in [
            build.service_version(),
            build.service_commit(),
            build.lib_revision(),
            build.rust_version(),
            build.target(),
            build.feature_profile(),
        ] {
            assert!(!debug.contains(hidden));
        }

        for invalid in ["", ".bad", "bad value", "bad/value", "é"] {
            assert_eq!(
                MigrationBuildIdentity::new(
                    invalid,
                    "0123456789abcdef0123456789abcdef01234567",
                    "89abcdef0123456789abcdef0123456789abcdef",
                    "1.97.1",
                    "x86_64-unknown-linux-gnu",
                    "service-host",
                    1,
                    2,
                    3,
                    4,
                    5,
                ),
                Err(MigrationEvidenceError::InvalidBuildIdentity)
            );
        }
        for invalid_revision in [
            "0123456789abcdef0123456789abcdef0123456",
            "0123456789ABCDEF0123456789abcdef01234567",
            "g123456789abcdef0123456789abcdef01234567",
        ] {
            assert_eq!(
                MigrationBuildIdentity::new(
                    "0.1.0-alpha",
                    invalid_revision,
                    "89abcdef0123456789abcdef0123456789abcdef",
                    "1.97.1",
                    "x86_64-unknown-linux-gnu",
                    "service-host",
                    1,
                    2,
                    3,
                    4,
                    5,
                ),
                Err(MigrationEvidenceError::InvalidBuildIdentity)
            );
        }
        let maximum = "a".repeat(MAX_MIGRATION_BUILD_ID_UTF8_BYTES);
        assert!(
            MigrationBuildIdentity::new(
                &maximum,
                "0123456789abcdef0123456789abcdef01234567",
                "89abcdef0123456789abcdef0123456789abcdef",
                &maximum,
                &maximum,
                &maximum,
                1,
                2,
                3,
                4,
                5,
            )
            .is_ok()
        );
        let maximum_plus_one = "a".repeat(MAX_MIGRATION_BUILD_ID_UTF8_BYTES + 1);
        for field in [0, 3, 4, 5] {
            let mut values = [
                "0.1.0-alpha",
                "0123456789abcdef0123456789abcdef01234567",
                "89abcdef0123456789abcdef0123456789abcdef",
                "1.97.1",
                "x86_64-unknown-linux-gnu",
                "service-host",
            ];
            values[field] = &maximum_plus_one;
            assert_eq!(
                MigrationBuildIdentity::new(
                    values[0], values[1], values[2], values[3], values[4], values[5], 1, 2, 3, 4,
                    5,
                ),
                Err(MigrationEvidenceError::InvalidBuildIdentity)
            );
        }
        let very_large = "a".repeat(4 * 1024 * 1024);
        for field in 0..6 {
            let mut values = [
                "0.1.0-alpha",
                "0123456789abcdef0123456789abcdef01234567",
                "89abcdef0123456789abcdef0123456789abcdef",
                "1.97.1",
                "x86_64-unknown-linux-gnu",
                "service-host",
            ];
            values[field] = &very_large;
            assert_eq!(
                MigrationBuildIdentity::new(
                    values[0], values[1], values[2], values[3], values[4], values[5], 1, 2, 3, 4,
                    5,
                ),
                Err(MigrationEvidenceError::InvalidBuildIdentity),
                "field {field} allocated before validation"
            );
        }
        assert_eq!(
            MigrationBuildIdentity::new(
                "0.1.0-alpha",
                "0123456789abcdef0123456789abcdef01234567",
                "89abcdef0123456789abcdef0123456789abcdef",
                "1.97.1",
                "x86_64-unknown-linux-gnu",
                "service-host",
                1,
                2,
                3,
                4,
                0,
            ),
            Err(MigrationEvidenceError::InvalidBuildIdentity)
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test(flavor = "current_thread")]
    async fn sql_and_callback_migrations_commit_exact_restart_safe_ledger() {
        let sql_descriptor =
            MigrationDescriptor::sql(2, "create_alpha", SQL_TWO, SQL_TWO_CHECKSUM).unwrap();
        let callback_descriptor = MigrationDescriptor::callback(
            3,
            "rebuild_projection",
            CALLBACK_THREE,
            CALLBACK_THREE_CHECKSUM,
        )
        .unwrap();
        let callback = MigrationCallbackBinding::new(
            callback_descriptor.target_version(),
            callback_descriptor.name(),
            callback_descriptor.checksum(),
            insert_projection_callback,
        );
        let catalog =
            MigrationCatalog::new([sql_descriptor, callback_descriptor]).expect("catalog");
        let schema_catalog = alpha_schema_catalog(&catalog);
        let applied_at = MigrationAppliedAtUnixSeconds::new(1_800_000_000).unwrap();
        let build = build_identity();
        let mut connection = initialized_memory_database().await;
        let mut validate = || Ok(());

        let outcome = apply_governed_migrations(
            &mut connection,
            &catalog,
            &schema_catalog,
            applied_at,
            &build,
            &[callback],
            &mut validate,
        )
        .await
        .expect("apply catalog");
        assert_eq!(outcome.initial_version(), 1);
        assert_eq!(outcome.final_version(), 3);
        assert_eq!(outcome.applied_count(), 2);
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT id FROM alpha")
                .fetch_one(&mut connection)
                .await
                .unwrap(),
            41
        );
        let rows = sqlx::query(
            "SELECT version, name, checksum, applied_at_unix_s,
                    service_version, service_commit, lib_revision, rust_version,
                    target, feature_profile, config_contract_version,
                    state_contract_version, admin_contract_version,
                    status_contract_version, provider_contract_version
             FROM schema_migrations ORDER BY version",
        )
        .fetch_all(&mut connection)
        .await
        .unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].try_get::<i64, _>("version").unwrap(), 2);
        assert_eq!(
            rows[0].try_get::<String, _>("name").unwrap(),
            "create_alpha"
        );
        assert_eq!(
            rows[0].try_get::<Vec<u8>, _>("checksum").unwrap(),
            SQL_TWO_CHECKSUM.as_bytes().as_slice()
        );
        assert_eq!(rows[1].try_get::<i64, _>("version").unwrap(), 3);
        assert_eq!(
            rows[1].try_get::<String, _>("name").unwrap(),
            "rebuild_projection"
        );
        for row in &rows {
            assert_eq!(
                row.try_get::<i64, _>("applied_at_unix_s").unwrap(),
                1_800_000_000
            );
            assert_eq!(
                row.try_get::<String, _>("service_version").unwrap(),
                build.service_version()
            );
            assert_eq!(
                row.try_get::<String, _>("service_commit").unwrap(),
                build.service_commit()
            );
            assert_eq!(
                row.try_get::<String, _>("lib_revision").unwrap(),
                build.lib_revision()
            );
            assert_eq!(
                row.try_get::<String, _>("rust_version").unwrap(),
                build.rust_version()
            );
            assert_eq!(row.try_get::<String, _>("target").unwrap(), build.target());
            assert_eq!(
                row.try_get::<String, _>("feature_profile").unwrap(),
                build.feature_profile()
            );
            assert_eq!(row.try_get::<i64, _>("config_contract_version").unwrap(), 1);
            assert_eq!(row.try_get::<i64, _>("state_contract_version").unwrap(), 2);
            assert_eq!(row.try_get::<i64, _>("admin_contract_version").unwrap(), 3);
            assert_eq!(row.try_get::<i64, _>("status_contract_version").unwrap(), 4);
            assert_eq!(
                row.try_get::<i64, _>("provider_contract_version").unwrap(),
                5
            );
        }
        for statement in [
            "UPDATE schema_migrations SET name = 'changed' WHERE version = 2",
            "DELETE FROM schema_migrations WHERE version = 2",
        ] {
            assert!(
                sqlx::query(statement)
                    .execute(&mut connection)
                    .await
                    .is_err(),
                "append-only ledger accepted `{statement}`"
            );
        }
        assert_eq!(read_state_schema_version(&mut connection).await.unwrap(), 3);

        let reopened = apply_governed_migrations(
            &mut connection,
            &catalog,
            &schema_catalog,
            MigrationAppliedAtUnixSeconds::new(1_900_000_000).unwrap(),
            &build,
            &[callback],
            &mut validate,
        )
        .await
        .expect("lost response converges on exact committed history");
        assert_eq!(reopened.initial_version(), 3);
        assert_eq!(reopened.final_version(), 3);
        assert_eq!(reopened.applied_count(), 0);
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM alpha")
                .fetch_one(&mut connection)
                .await
                .unwrap(),
            1
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test(flavor = "current_thread")]
    async fn failing_step_rolls_back_only_that_step_and_exact_prefix_resumes() {
        const INVALID_SQL: &str =
            "CREATE TABLE broken (id INTEGER PRIMARY KEY); SELECT no_such_function();";
        const RECOVERY_SQL: &str = "CREATE TABLE beta (id INTEGER PRIMARY KEY);";
        let first = sql(2, "create_alpha", SQL_TWO);
        let invalid = sql(3, "create_beta", INVALID_SQL);
        let invalid_catalog = MigrationCatalog::new([first.clone(), invalid]).unwrap();
        let invalid_schema_catalog = alpha_schema_catalog(&invalid_catalog);
        let applied_at = MigrationAppliedAtUnixSeconds::new(1_800_000_000).unwrap();
        let build = build_identity();
        let mut connection = initialized_memory_database().await;
        let mut validate = || Ok(());

        let error = apply_governed_migrations(
            &mut connection,
            &invalid_catalog,
            &invalid_schema_catalog,
            applied_at,
            &build,
            &[],
            &mut validate,
        )
        .await
        .expect_err("invalid second step must roll back");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Migration);
        assert_eq!(read_state_schema_version(&mut connection).await.unwrap(), 2);
        assert_eq!(
            read_migration_history(&mut connection).await.unwrap().len(),
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM sqlite_schema WHERE type = 'table' AND name = 'broken'",
            )
            .fetch_one(&mut connection)
            .await
            .unwrap(),
            0
        );

        let recovered_catalog =
            MigrationCatalog::new([first, sql(3, "create_beta", RECOVERY_SQL)]).unwrap();
        let recovered_schema_catalog = alpha_beta_schema_catalog(
            &recovered_catalog,
            "CREATE TABLE beta (id INTEGER PRIMARY KEY)",
        );
        let recovered = apply_governed_migrations(
            &mut connection,
            &recovered_catalog,
            &recovered_schema_catalog,
            applied_at,
            &build,
            &[],
            &mut validate,
        )
        .await
        .expect("resume exact prefix");
        assert_eq!(recovered.initial_version(), 2);
        assert_eq!(recovered.final_version(), 3);
        assert_eq!(recovered.applied_count(), 1);
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test(flavor = "current_thread")]
    async fn schema_mismatch_before_or_after_execution_never_commits_a_step() {
        let catalog = MigrationCatalog::new([sql(2, "create_alpha", SQL_TWO)]).unwrap();
        let wrong_target_catalog = unchanged_schema_catalog(&catalog);
        let applied_at = MigrationAppliedAtUnixSeconds::new(1_800_000_000).unwrap();
        let build = build_identity();
        let mut validate = || Ok(());

        let mut after_execution = initialized_memory_database().await;
        let error = apply_governed_migrations(
            &mut after_execution,
            &catalog,
            &wrong_target_catalog,
            applied_at,
            &build,
            &[],
            &mut validate,
        )
        .await
        .expect_err("target schema mismatch must roll back");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Integrity);
        assert_eq!(
            read_state_schema_version(&mut after_execution)
                .await
                .unwrap(),
            1
        );
        assert!(
            read_migration_history(&mut after_execution)
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM sqlite_schema WHERE type = 'table' AND name = 'alpha'",
            )
            .fetch_one(&mut after_execution)
            .await
            .unwrap(),
            0
        );

        let mut before_execution = initialized_memory_database().await;
        sqlx::query("CREATE TABLE unexpected (value INTEGER)")
            .execute(&mut before_execution)
            .await
            .unwrap();
        let expected_target = alpha_schema_catalog(&catalog);
        let error = apply_governed_migrations(
            &mut before_execution,
            &catalog,
            &expected_target,
            applied_at,
            &build,
            &[],
            &mut validate,
        )
        .await
        .expect_err("current schema mismatch must fail before execution");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Integrity);
        assert_eq!(
            read_state_schema_version(&mut before_execution)
                .await
                .unwrap(),
            1
        );
        assert!(
            read_migration_history(&mut before_execution)
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM sqlite_schema WHERE type = 'table' AND name = 'alpha'",
            )
            .fetch_one(&mut before_execution)
            .await
            .unwrap(),
            0
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test(flavor = "current_thread")]
    async fn transaction_control_cannot_escape_schema_ledger_metadata_atomicity() {
        const COMMIT_ESCAPE_SQL: &str =
            "CREATE TABLE sql_leaked (id INTEGER PRIMARY KEY); COMMIT; SELECT no_such_function();";
        const REPLACEMENT_ESCAPE_SQL: &str =
            "CREATE TABLE sql_rolled_back (id INTEGER PRIMARY KEY);
             ROLLBACK;
             BEGIN DEFERRED;
             CREATE TABLE sql_replacement_leaked (id INTEGER PRIMARY KEY);";
        const ROLLBACK_CALLBACK_DEFINITION: &[u8] = b"callback:rollback_escape:v1";
        let directory = tempfile::tempdir().unwrap();
        let build = build_identity();
        let applied_at = MigrationAppliedAtUnixSeconds::new(1_800_000_000).unwrap();

        let sql_path = directory.path().join("sql-escape.sqlite");
        let mut sql_connection = initialized_file_database(&sql_path).await;
        let sql_catalog =
            MigrationCatalog::new([sql(2, "attempt_commit_escape", COMMIT_ESCAPE_SQL)]).unwrap();
        let sql_schema_catalog = unchanged_schema_catalog(&sql_catalog);
        let mut validate = || Ok(());
        let sql_error = apply_governed_migrations(
            &mut sql_connection,
            &sql_catalog,
            &sql_schema_catalog,
            applied_at,
            &build,
            &[],
            &mut validate,
        )
        .await
        .expect_err("embedded COMMIT must be rejected");
        assert_eq!(sql_error.kind(), ServiceSqliteErrorKind::Migration);
        drop(sql_connection);
        assert_fresh_connection_has_no_migration_effect(&sql_path, "sql_leaked").await;

        let replacement_path = directory.path().join("sql-replacement-escape.sqlite");
        let mut replacement_connection = initialized_file_database(&replacement_path).await;
        let replacement_catalog = MigrationCatalog::new([sql(
            2,
            "attempt_transaction_replacement",
            REPLACEMENT_ESCAPE_SQL,
        )])
        .unwrap();
        let replacement_schema_catalog = unchanged_schema_catalog(&replacement_catalog);
        let replacement_error = apply_governed_migrations(
            &mut replacement_connection,
            &replacement_catalog,
            &replacement_schema_catalog,
            applied_at,
            &build,
            &[],
            &mut validate,
        )
        .await
        .expect_err("replacement transaction must not inherit the governed commit permit");
        assert_eq!(replacement_error.kind(), ServiceSqliteErrorKind::Migration);
        drop(replacement_connection);
        assert_fresh_connection_has_no_migration_effect(
            &replacement_path,
            "sql_replacement_leaked",
        )
        .await;

        let callback_path = directory.path().join("callback-escape.sqlite");
        let mut callback_connection = initialized_file_database(&callback_path).await;
        let callback_descriptor = MigrationDescriptor::callback(
            2,
            "attempt_rollback_escape",
            ROLLBACK_CALLBACK_DEFINITION,
            MigrationChecksum::for_callback(ROLLBACK_CALLBACK_DEFINITION),
        )
        .unwrap();
        let callback = MigrationCallbackBinding::new(
            callback_descriptor.target_version(),
            callback_descriptor.name(),
            callback_descriptor.checksum(),
            rollback_escape_callback,
        );
        let callback_catalog = MigrationCatalog::new([callback_descriptor]).unwrap();
        let callback_schema_catalog = unchanged_schema_catalog(&callback_catalog);
        let callback_error = apply_governed_migrations(
            &mut callback_connection,
            &callback_catalog,
            &callback_schema_catalog,
            applied_at,
            &build,
            &[callback],
            &mut validate,
        )
        .await
        .expect_err("callback ROLLBACK must be rejected even when its error is ignored");
        assert_eq!(callback_error.kind(), ServiceSqliteErrorKind::Migration);
        drop(callback_connection);
        assert_fresh_connection_has_no_migration_effect(&callback_path, "callback_leaked").await;

        const POLICY_ESCAPE_SQL: &str = "CREATE TABLE policy_leaked (id INTEGER PRIMARY KEY);
             PRAGMA busy_timeout = 1;
             ATTACH ':memory:' AS escaped;";
        let policy_path = directory.path().join("policy-escape.sqlite");
        let mut policy_connection = initialized_file_database(&policy_path).await;
        let policy_catalog =
            MigrationCatalog::new([sql(2, "attempt_policy_escape", POLICY_ESCAPE_SQL)]).unwrap();
        let policy_schema_catalog = unchanged_schema_catalog(&policy_catalog);
        let policy_error = apply_governed_migrations(
            &mut policy_connection,
            &policy_catalog,
            &policy_schema_catalog,
            applied_at,
            &build,
            &[],
            &mut validate,
        )
        .await
        .expect_err("connection-policy or attachment drift must block commit");
        assert_eq!(policy_error.kind(), ServiceSqliteErrorKind::Migration);
        drop(policy_connection);
        assert_fresh_connection_has_no_migration_effect(&policy_path, "policy_leaked").await;
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test(flavor = "current_thread")]
    async fn migration_sql_rejects_complete_statement_control_inventory_before_execution() {
        for (name, statement) in [
            (
                "reject_pragma",
                "CREATE TABLE leaked (id INTEGER); /* policy */ PrAgMa\ntrusted_schema=ON",
            ),
            (
                "reject_attach",
                "CREATE TABLE leaked (id INTEGER); ATTACH ':memory:' AS escaped",
            ),
            (
                "reject_detach",
                "CREATE TABLE leaked (id INTEGER); DETACH DATABASE escaped",
            ),
            (
                "reject_begin",
                "CREATE TABLE leaked (id INTEGER); BEGIN DEFERRED",
            ),
            ("reject_commit", "CREATE TABLE leaked (id INTEGER); COMMIT"),
            (
                "reject_end",
                "CREATE TABLE leaked (id INTEGER); END TRANSACTION",
            ),
            (
                "reject_rollback",
                "CREATE TABLE leaked (id INTEGER); ROLLBACK",
            ),
            (
                "reject_savepoint",
                "CREATE TABLE leaked (id INTEGER); SAVEPOINT escaped",
            ),
            (
                "reject_release",
                "CREATE TABLE leaked (id INTEGER); RELEASE SAVEPOINT escaped",
            ),
        ] {
            let directory = tempfile::tempdir().unwrap();
            let database_path = directory.path().join("statement-control.sqlite");
            let mut connection = initialized_file_database(&database_path).await;
            let catalog = MigrationCatalog::new([sql(2, name, statement)]).unwrap();
            let schema_catalog = unchanged_schema_catalog(&catalog);
            let mut validate = || Ok(());
            let error = apply_governed_migrations(
                &mut connection,
                &catalog,
                &schema_catalog,
                MigrationAppliedAtUnixSeconds::new(1_800_000_000).unwrap(),
                &build_identity(),
                &[],
                &mut validate,
            )
            .await
            .expect_err("statement-control migration must be rejected");
            assert_eq!(error.kind(), ServiceSqliteErrorKind::Migration);
            assert_eq!(read_state_schema_version(&mut connection).await.unwrap(), 1);
            assert!(
                read_migration_history(&mut connection)
                    .await
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(
                sqlx::query_scalar::<_, i64>(
                    "SELECT COUNT(*) FROM sqlite_schema WHERE type = 'table' AND name = 'leaked'",
                )
                .fetch_one(&mut connection)
                .await
                .unwrap(),
                0
            );
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test(flavor = "current_thread")]
    async fn migration_executor_rejects_transient_attachment_before_file_creation() {
        let directory = tempfile::tempdir().unwrap();
        let database_path = directory.path().join("main.sqlite");
        let external_path = directory.path().join("external.sqlite");
        let migration_sql = Box::leak(
            format!(
                "ATTACH DATABASE '{}' AS extra; DETACH DATABASE extra",
                external_path.display()
            )
            .into_boxed_str(),
        );
        let mut connection = initialized_file_database(&database_path).await;
        let catalog =
            MigrationCatalog::new([sql(2, "reject_transient_attachment", migration_sql)]).unwrap();
        let schema_catalog = unchanged_schema_catalog(&catalog);
        let mut validate = || Ok(());
        let error = apply_governed_migrations(
            &mut connection,
            &catalog,
            &schema_catalog,
            MigrationAppliedAtUnixSeconds::new(1_800_000_000).unwrap(),
            &build_identity(),
            &[],
            &mut validate,
        )
        .await
        .expect_err("migration ATTACH/DETACH must fail before SQLite compilation");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Migration);
        assert!(!external_path.exists());
        assert_eq!(read_state_schema_version(&mut connection).await.unwrap(), 1);
        assert!(
            read_migration_history(&mut connection)
                .await
                .unwrap()
                .is_empty()
        );

        const CALLBACK_DEFINITION: &[u8] = b"callback:reject_transient_attachment:v1";
        let callback_database_path = directory.path().join("callback-main.sqlite");
        let callback_external_path = directory.path().join("callback-external.sqlite");
        let callback_sql = Box::leak(
            format!(
                "ATTACH DATABASE '{}' AS extra; DETACH DATABASE extra",
                callback_external_path.display()
            )
            .into_boxed_str(),
        );
        *IGNORED_STATEMENT_CONTROL_SQL
            .lock()
            .expect("statement-control SQL mutex") = Some(callback_sql);
        let mut callback_connection = initialized_file_database(&callback_database_path).await;
        let callback_descriptor = MigrationDescriptor::callback(
            2,
            "reject_callback_attachment",
            CALLBACK_DEFINITION,
            MigrationChecksum::for_callback(CALLBACK_DEFINITION),
        )
        .unwrap();
        let callback = MigrationCallbackBinding::new(
            callback_descriptor.target_version(),
            callback_descriptor.name(),
            callback_descriptor.checksum(),
            ignored_statement_control_callback,
        );
        let callback_catalog = MigrationCatalog::new([callback_descriptor]).unwrap();
        let callback_schema_catalog = unchanged_schema_catalog(&callback_catalog);
        let callback_error = apply_governed_migrations(
            &mut callback_connection,
            &callback_catalog,
            &callback_schema_catalog,
            MigrationAppliedAtUnixSeconds::new(1_800_000_001).unwrap(),
            &build_identity(),
            &[callback],
            &mut validate,
        )
        .await
        .expect_err("ignored callback ATTACH/DETACH must still fail the migration");
        *IGNORED_STATEMENT_CONTROL_SQL
            .lock()
            .expect("statement-control SQL mutex") = None;
        assert_eq!(callback_error.kind(), ServiceSqliteErrorKind::Migration);
        assert!(!callback_external_path.exists());
        assert_eq!(
            read_state_schema_version(&mut callback_connection)
                .await
                .unwrap(),
            1
        );
        assert!(
            read_migration_history(&mut callback_connection)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    async fn assert_fresh_connection_has_no_migration_effect(path: &Path, table: &str) {
        let mut connection = SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(false),
        )
        .await
        .expect("fresh verification connection");
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM sqlite_schema WHERE type = 'table' AND name = ?",
            )
            .bind(table)
            .fetch_one(&mut connection)
            .await
            .unwrap(),
            0
        );
        assert_eq!(read_state_schema_version(&mut connection).await.unwrap(), 1);
        assert!(
            read_migration_history(&mut connection)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test(flavor = "current_thread")]
    async fn cancellation_before_commit_leaves_an_exact_resumable_prefix() {
        PENDING_CALLBACK_COUNT.store(0, AtomicOrdering::SeqCst);
        let sql_descriptor =
            MigrationDescriptor::sql(2, "create_alpha", SQL_TWO, SQL_TWO_CHECKSUM).unwrap();
        let callback_descriptor = MigrationDescriptor::callback(
            3,
            "rebuild_projection",
            CALLBACK_THREE,
            CALLBACK_THREE_CHECKSUM,
        )
        .unwrap();
        let catalog = MigrationCatalog::new([sql_descriptor, callback_descriptor.clone()]).unwrap();
        let schema_catalog = alpha_schema_catalog(&catalog);
        let pending_binding = MigrationCallbackBinding::new(
            3,
            callback_descriptor.name(),
            callback_descriptor.checksum(),
            pending_projection_callback,
        );
        let working_binding = MigrationCallbackBinding::new(
            3,
            callback_descriptor.name(),
            callback_descriptor.checksum(),
            insert_projection_callback,
        );
        let applied_at = MigrationAppliedAtUnixSeconds::new(1_800_000_000).unwrap();
        let build = build_identity();
        let mut connection = initialized_memory_database().await;
        let mut validate = || Ok(());
        let pending_bindings = [pending_binding];

        let mut application = Box::pin(apply_governed_migrations(
            &mut connection,
            &catalog,
            &schema_catalog,
            applied_at,
            &build,
            &pending_bindings,
            &mut validate,
        ));
        tokio::select! {
            outcome = &mut application => panic!("pending callback completed: {outcome:?}"),
            () = async {
                while PENDING_CALLBACK_COUNT.load(AtomicOrdering::SeqCst) == 0 {
                    tokio::task::yield_now().await;
                }
            } => {}
        }
        drop(application);

        assert_eq!(read_state_schema_version(&mut connection).await.unwrap(), 2);
        assert_eq!(
            read_migration_history(&mut connection).await.unwrap().len(),
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM alpha")
                .fetch_one(&mut connection)
                .await
                .unwrap(),
            0,
            "callback write survived cancellation before commit"
        );
        let recovered = apply_governed_migrations(
            &mut connection,
            &catalog,
            &schema_catalog,
            applied_at,
            &build,
            &[working_binding],
            &mut validate,
        )
        .await
        .expect("resume cancelled callback");
        assert_eq!(recovered.initial_version(), 2);
        assert_eq!(recovered.final_version(), 3);
        assert_eq!(recovered.applied_count(), 1);
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM alpha")
                .fetch_one(&mut connection)
                .await
                .unwrap(),
            1
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test(flavor = "current_thread")]
    async fn commit_response_loss_is_resolved_from_history_without_replay() {
        let first = sql(2, "create_alpha", SQL_TWO);
        let second = sql(3, "create_beta", "CREATE TABLE beta (id INTEGER);");
        let catalog = MigrationCatalog::new([first, second]).unwrap();
        let schema_catalog = alpha_beta_schema_catalog(&catalog, "CREATE TABLE beta (id INTEGER)");
        let applied_at = MigrationAppliedAtUnixSeconds::new(1_800_000_000).unwrap();
        let build = build_identity();
        let mut connection = initialized_memory_database().await;
        let mut validate = || Ok(());
        let mut lose_first_response = || Err(migration_error(MigrationFailureKind::Commit));

        let error = apply_governed_migrations_with_observer(
            &mut connection,
            &catalog,
            &schema_catalog,
            applied_at,
            &build,
            &[],
            &mut validate,
            &mut lose_first_response,
        )
        .await
        .expect_err("first committed response is lost");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Migration);
        assert_eq!(read_state_schema_version(&mut connection).await.unwrap(), 2);
        assert_eq!(
            read_migration_history(&mut connection).await.unwrap().len(),
            1
        );

        let resumed = apply_governed_migrations(
            &mut connection,
            &catalog,
            &schema_catalog,
            applied_at,
            &build,
            &[],
            &mut validate,
        )
        .await
        .expect("resolve committed prefix and continue");
        assert_eq!(resumed.initial_version(), 2);
        assert_eq!(resumed.final_version(), 3);
        assert_eq!(resumed.applied_count(), 1);
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM sqlite_schema WHERE type = 'table' AND name = 'alpha'",
            )
            .fetch_one(&mut connection)
            .await
            .unwrap(),
            1
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test(flavor = "current_thread")]
    async fn callback_bindings_and_history_mismatches_fail_before_replay() {
        let callback_descriptor = MigrationDescriptor::callback(
            2,
            "rebuild_projection",
            CALLBACK_THREE,
            CALLBACK_THREE_CHECKSUM,
        )
        .unwrap();
        let catalog = MigrationCatalog::new([callback_descriptor.clone()]).unwrap();
        let schema_catalog = unchanged_schema_catalog(&catalog);
        let correct = MigrationCallbackBinding::new(
            2,
            callback_descriptor.name(),
            callback_descriptor.checksum(),
            insert_projection_callback,
        );
        let wrong = MigrationCallbackBinding::new(
            3,
            callback_descriptor.name(),
            callback_descriptor.checksum(),
            insert_projection_callback,
        );
        let applied_at = MigrationAppliedAtUnixSeconds::new(1_800_000_000).unwrap();
        let build = build_identity();

        for bindings in [Vec::new(), vec![wrong], vec![correct, correct]] {
            let mut connection = initialized_memory_database().await;
            let mut validate = || Ok(());
            let error = apply_governed_migrations(
                &mut connection,
                &catalog,
                &schema_catalog,
                applied_at,
                &build,
                &bindings,
                &mut validate,
            )
            .await
            .expect_err("callback registry mismatch");
            assert_eq!(error.kind(), ServiceSqliteErrorKind::Migration);
            assert_eq!(read_state_schema_version(&mut connection).await.unwrap(), 1);
            assert!(
                read_migration_history(&mut connection)
                    .await
                    .unwrap()
                    .is_empty()
            );
        }

        let mut connection = initialized_memory_database().await;
        sqlx::query(
            "INSERT INTO schema_migrations (
                version, name, checksum, applied_at_unix_s,
                service_version, service_commit, lib_revision, rust_version, target,
                feature_profile, config_contract_version, state_contract_version,
                admin_contract_version, status_contract_version, provider_contract_version
             ) VALUES (2, 'wrong_name', zeroblob(32), 0, ?, ?, ?, ?, ?, ?, 1, 2, 3, 4, 5)",
        )
        .bind(build.service_version())
        .bind(build.service_commit())
        .bind(build.lib_revision())
        .bind(build.rust_version())
        .bind(build.target())
        .bind(build.feature_profile())
        .execute(&mut connection)
        .await
        .unwrap();
        sqlx::query(
            "UPDATE radroots_service_metadata SET state_schema_version = 2 WHERE singleton = 1",
        )
        .execute(&mut connection)
        .await
        .unwrap();
        let error = verify_migration_history(&mut connection, &catalog, &schema_catalog, true)
            .await
            .expect_err("mismatched history");
        assert_eq!(error.kind(), ServiceSqliteErrorKind::Migration);
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test(flavor = "current_thread")]
    async fn missing_extra_reordered_newer_and_corrupt_history_fail_closed() {
        let first = sql(2, "create_alpha", SQL_TWO);
        let second = sql(3, "create_beta", "CREATE TABLE beta (id INTEGER);");
        let catalog = MigrationCatalog::new([first.clone(), second.clone()]).unwrap();
        let schema_catalog = unchanged_schema_catalog(&catalog);

        let mut missing = initialized_memory_database().await;
        sqlx::query(
            "UPDATE radroots_service_metadata SET state_schema_version = 2 WHERE singleton = 1",
        )
        .execute(&mut missing)
        .await
        .unwrap();
        assert_eq!(
            verify_migration_history(&mut missing, &catalog, &schema_catalog, false)
                .await
                .expect_err("missing row")
                .kind(),
            ServiceSqliteErrorKind::Migration
        );

        let mut extra = initialized_memory_database().await;
        replace_with_permissive_ledger(&mut extra).await;
        insert_permissive_history_row(
            &mut extra,
            2,
            first.name().as_str(),
            first.checksum().as_bytes(),
            0,
            "0.1.0-alpha",
        )
        .await;
        assert_eq!(
            verify_migration_history(&mut extra, &catalog, &schema_catalog, false)
                .await
                .expect_err("extra row")
                .kind(),
            ServiceSqliteErrorKind::Migration
        );

        let mut reordered = initialized_memory_database().await;
        replace_with_permissive_ledger(&mut reordered).await;
        insert_permissive_history_row(
            &mut reordered,
            2,
            second.name().as_str(),
            second.checksum().as_bytes(),
            0,
            "0.1.0-alpha",
        )
        .await;
        insert_permissive_history_row(
            &mut reordered,
            3,
            first.name().as_str(),
            first.checksum().as_bytes(),
            0,
            "0.1.0-alpha",
        )
        .await;
        sqlx::query(
            "UPDATE radroots_service_metadata SET state_schema_version = 3 WHERE singleton = 1",
        )
        .execute(&mut reordered)
        .await
        .unwrap();
        assert_eq!(
            verify_migration_history(&mut reordered, &catalog, &schema_catalog, true)
                .await
                .expect_err("reordered names and checksums")
                .kind(),
            ServiceSqliteErrorKind::Migration
        );

        let mut newer = initialized_memory_database().await;
        sqlx::query(
            "UPDATE radroots_service_metadata SET state_schema_version = 4 WHERE singleton = 1",
        )
        .execute(&mut newer)
        .await
        .unwrap();
        assert_eq!(
            verify_migration_history(&mut newer, &catalog, &schema_catalog, false)
                .await
                .expect_err("newer schema")
                .kind(),
            ServiceSqliteErrorKind::Migration
        );

        for (applied_at, service_version) in [(0_i64, "bad value"), (-1, "0.1.0-alpha")] {
            let mut corrupt = initialized_memory_database().await;
            replace_with_permissive_ledger(&mut corrupt).await;
            insert_permissive_history_row(
                &mut corrupt,
                2,
                first.name().as_str(),
                first.checksum().as_bytes(),
                applied_at,
                service_version,
            )
            .await;
            sqlx::query(
                "UPDATE radroots_service_metadata SET state_schema_version = 2 WHERE singleton = 1",
            )
            .execute(&mut corrupt)
            .await
            .unwrap();
            assert_eq!(
                verify_migration_history(&mut corrupt, &catalog, &schema_catalog, false)
                    .await
                    .expect_err("corrupt time or build")
                    .kind(),
                ServiceSqliteErrorKind::Migration
            );
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test(flavor = "current_thread")]
    async fn every_migration_ledger_projection_rejects_wrong_storage_and_values() {
        let descriptor = sql(2, "create_alpha", SQL_TWO);
        let catalog = MigrationCatalog::new([descriptor.clone()]).unwrap();
        let schema_catalog = unchanged_schema_catalog(&catalog);
        let wrong_storage_updates = [
            "UPDATE schema_migrations SET version = '2' WHERE rowid = 1",
            "UPDATE schema_migrations SET name = 2 WHERE rowid = 1",
            "UPDATE schema_migrations SET checksum = 'checksum' WHERE rowid = 1",
            "UPDATE schema_migrations SET applied_at_unix_s = '0' WHERE rowid = 1",
            "UPDATE schema_migrations SET service_version = 2 WHERE rowid = 1",
            "UPDATE schema_migrations SET service_commit = 2 WHERE rowid = 1",
            "UPDATE schema_migrations SET lib_revision = 2 WHERE rowid = 1",
            "UPDATE schema_migrations SET rust_version = 2 WHERE rowid = 1",
            "UPDATE schema_migrations SET target = 2 WHERE rowid = 1",
            "UPDATE schema_migrations SET feature_profile = 2 WHERE rowid = 1",
            "UPDATE schema_migrations SET config_contract_version = '1' WHERE rowid = 1",
            "UPDATE schema_migrations SET state_contract_version = '2' WHERE rowid = 1",
            "UPDATE schema_migrations SET admin_contract_version = '3' WHERE rowid = 1",
            "UPDATE schema_migrations SET status_contract_version = '4' WHERE rowid = 1",
            "UPDATE schema_migrations SET provider_contract_version = '5' WHERE rowid = 1",
        ];
        let invalid_value_updates = [
            "UPDATE schema_migrations SET version = -1 WHERE rowid = 1",
            "UPDATE schema_migrations SET version = 4294967296 WHERE rowid = 1",
            "UPDATE schema_migrations SET name = 'BadName' WHERE rowid = 1",
            "UPDATE schema_migrations SET checksum = zeroblob(31) WHERE rowid = 1",
            "UPDATE schema_migrations SET applied_at_unix_s = -1 WHERE rowid = 1",
            "UPDATE schema_migrations SET service_version = 'bad value' WHERE rowid = 1",
            "UPDATE schema_migrations SET service_commit = 'bad' WHERE rowid = 1",
            "UPDATE schema_migrations SET lib_revision = 'bad' WHERE rowid = 1",
            "UPDATE schema_migrations SET rust_version = 'bad value' WHERE rowid = 1",
            "UPDATE schema_migrations SET target = 'bad value' WHERE rowid = 1",
            "UPDATE schema_migrations SET feature_profile = 'bad value' WHERE rowid = 1",
            "UPDATE schema_migrations SET config_contract_version = 0 WHERE rowid = 1",
            "UPDATE schema_migrations SET state_contract_version = -1 WHERE rowid = 1",
            "UPDATE schema_migrations SET admin_contract_version = 4294967296 WHERE rowid = 1",
            "UPDATE schema_migrations SET status_contract_version = 0 WHERE rowid = 1",
            "UPDATE schema_migrations SET provider_contract_version = 0 WHERE rowid = 1",
        ];

        for update in wrong_storage_updates
            .into_iter()
            .chain(invalid_value_updates)
        {
            let mut connection = initialized_memory_database().await;
            replace_with_permissive_ledger(&mut connection).await;
            insert_permissive_history_row(
                &mut connection,
                2,
                descriptor.name().as_str(),
                descriptor.checksum().as_bytes(),
                0,
                "0.1.0-alpha",
            )
            .await;
            sqlx::raw_sql(update)
                .execute(&mut connection)
                .await
                .expect("corrupt ledger projection");
            sqlx::query(
                "UPDATE radroots_service_metadata SET state_schema_version = 2 WHERE singleton = 1",
            )
            .execute(&mut connection)
            .await
            .unwrap();
            assert_eq!(
                verify_migration_history(&mut connection, &catalog, &schema_catalog, false)
                    .await
                    .expect_err("corrupt ledger projection")
                    .kind(),
                ServiceSqliteErrorKind::Migration,
                "accepted corrupt projection update `{update}`"
            );
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test(flavor = "current_thread")]
    async fn oversized_corrupt_history_is_bounded_before_decode() {
        let descriptor = sql(2, "create_alpha", SQL_TWO);
        let catalog = MigrationCatalog::new([descriptor.clone()]).unwrap();
        let schema_catalog = unchanged_schema_catalog(&catalog);
        let oversized_text = "a".repeat(4 * 1024 * 1024);
        for (column, update) in [
            (
                "name",
                "UPDATE schema_migrations SET name = ? WHERE version = 2",
            ),
            (
                "service_version",
                "UPDATE schema_migrations SET service_version = ? WHERE version = 2",
            ),
            (
                "service_commit",
                "UPDATE schema_migrations SET service_commit = ? WHERE version = 2",
            ),
            (
                "lib_revision",
                "UPDATE schema_migrations SET lib_revision = ? WHERE version = 2",
            ),
            (
                "rust_version",
                "UPDATE schema_migrations SET rust_version = ? WHERE version = 2",
            ),
            (
                "target",
                "UPDATE schema_migrations SET target = ? WHERE version = 2",
            ),
            (
                "feature_profile",
                "UPDATE schema_migrations SET feature_profile = ? WHERE version = 2",
            ),
        ] {
            let mut connection = initialized_memory_database().await;
            replace_with_permissive_ledger(&mut connection).await;
            insert_permissive_history_row(
                &mut connection,
                2,
                descriptor.name().as_str(),
                descriptor.checksum().as_bytes(),
                0,
                "0.1.0-alpha",
            )
            .await;
            sqlx::query(update)
                .bind(&oversized_text)
                .execute(&mut connection)
                .await
                .unwrap();
            sqlx::query(
                "UPDATE radroots_service_metadata SET state_schema_version = 2 WHERE singleton = 1",
            )
            .execute(&mut connection)
            .await
            .unwrap();
            assert_eq!(
                verify_migration_history(&mut connection, &catalog, &schema_catalog, true)
                    .await
                    .expect_err("oversized text must fail before decode")
                    .kind(),
                ServiceSqliteErrorKind::Migration,
                "column {column}"
            );
        }

        let mut checksum = initialized_memory_database().await;
        replace_with_permissive_ledger(&mut checksum).await;
        insert_permissive_history_row(
            &mut checksum,
            2,
            descriptor.name().as_str(),
            &vec![0_u8; 4 * 1024 * 1024],
            0,
            "0.1.0-alpha",
        )
        .await;
        sqlx::query(
            "UPDATE radroots_service_metadata SET state_schema_version = 2 WHERE singleton = 1",
        )
        .execute(&mut checksum)
        .await
        .unwrap();
        assert_eq!(
            verify_migration_history(&mut checksum, &catalog, &schema_catalog, true)
                .await
                .expect_err("oversized checksum must fail before decode")
                .kind(),
            ServiceSqliteErrorKind::Migration
        );
    }

    #[test]
    fn exact_content_checksums_are_deterministic_and_kind_separated() {
        let sql = MigrationChecksum::for_sql(SQL_TWO);
        let callback = MigrationChecksum::for_callback(SQL_TWO.as_bytes());
        assert_eq!(sql, SQL_TWO_CHECKSUM);
        assert_eq!(callback, CALLBACK_SQL_BYTES_CHECKSUM);
        assert_eq!(sql, MigrationChecksum::for_sql(SQL_TWO));
        assert_ne!(sql, callback);
        assert_ne!(
            sql,
            MigrationChecksum::for_sql(" CREATE TABLE alpha (id INTEGER PRIMARY KEY);")
        );
        assert_ne!(
            sql,
            MigrationChecksum::for_sql("CREATE TABLE alpha (id INTEGER PRIMARY KEY);\n")
        );
        assert_eq!(
            MigrationDescriptor::sql(2, "create_alpha", SQL_TWO, SQL_TWO_CHECKSUM)
                .unwrap()
                .checksum(),
            SQL_TWO_CHECKSUM
        );
        assert_eq!(
            MigrationDescriptor::callback(
                2,
                "callback_alpha",
                SQL_TWO.as_bytes(),
                CALLBACK_SQL_BYTES_CHECKSUM
            )
            .unwrap()
            .checksum(),
            CALLBACK_SQL_BYTES_CHECKSUM
        );
    }

    #[test]
    fn descriptor_checksum_name_version_and_content_bounds_fail_closed() {
        assert_eq!(
            MigrationDescriptor::sql(2, "alpha", SQL_TWO, MigrationChecksum::for_sql("other")),
            Err(MigrationContractError::ChecksumMismatch)
        );
        assert_eq!(
            MigrationDescriptor::callback(
                2,
                "alpha",
                CALLBACK_THREE,
                MigrationChecksum::for_callback(b"other")
            ),
            Err(MigrationContractError::ChecksumMismatch)
        );
        for invalid in ["", "_alpha", "alpha_", "Alpha", "alpha-beta", "alpha__beta"] {
            assert_eq!(
                MigrationName::new(invalid),
                Err(MigrationContractError::InvalidName)
            );
        }
        let max_name = Box::leak("a".repeat(MAX_MIGRATION_NAME_UTF8_BYTES).into_boxed_str());
        assert_eq!(MigrationName::new(max_name).unwrap().as_str(), max_name);
        let long_name = Box::leak(
            "a".repeat(MAX_MIGRATION_NAME_UTF8_BYTES + 1)
                .into_boxed_str(),
        );
        assert_eq!(
            MigrationName::new(long_name),
            Err(MigrationContractError::InvalidName)
        );
        for version in [0, 1] {
            assert_eq!(
                MigrationDescriptor::sql(
                    version,
                    "alpha",
                    SQL_TWO,
                    MigrationChecksum::for_sql(SQL_TWO)
                ),
                Err(MigrationContractError::InvalidTargetVersion)
            );
        }
        assert_eq!(
            MigrationDescriptor::sql(2, "alpha", "", MigrationChecksum::for_sql("")),
            Err(MigrationContractError::EmptyContent)
        );
        let max_content = Box::leak(vec![b'x'; MAX_MIGRATION_CONTENT_BYTES].into_boxed_slice());
        assert!(
            MigrationDescriptor::callback(
                2,
                "alpha",
                max_content,
                MigrationChecksum::for_callback(max_content)
            )
            .is_ok()
        );
        let oversized = Box::leak(vec![b'x'; MAX_MIGRATION_CONTENT_BYTES + 1].into_boxed_slice());
        assert_eq!(
            MigrationDescriptor::callback(
                2,
                "alpha",
                oversized,
                MigrationChecksum::for_callback(oversized)
            ),
            Err(MigrationContractError::ContentTooLarge)
        );
    }

    #[test]
    fn empty_v1_and_ordered_catalog_digest_are_exact() {
        let empty = MigrationCatalog::new([]).expect("empty v1 catalog");
        assert!(empty.descriptors().is_empty());
        assert_eq!(empty.current_version(), 1);
        assert_eq!(
            hex(empty.digest()),
            "ec89dc8f7b6c2a11b967e33808e4031e29b3970ffee4959bff9bad352877ee9b"
        );

        let catalog = MigrationCatalog::new([
            MigrationDescriptor::sql(2, "create_alpha", SQL_TWO, SQL_TWO_CHECKSUM).unwrap(),
            MigrationDescriptor::callback(
                3,
                "rebuild_projection",
                CALLBACK_THREE,
                CALLBACK_THREE_CHECKSUM,
            )
            .unwrap(),
        ])
        .expect("ordered catalog");
        assert_eq!(catalog.current_version(), 3);
        assert_eq!(
            catalog
                .descriptors()
                .iter()
                .map(MigrationDescriptor::target_version)
                .collect::<Vec<_>>(),
            vec![2, 3]
        );
        assert_eq!(
            hex(catalog.digest()),
            "318e8b0143859e58ffe995b7d97d1cc2488097d307367979666cb47b98665838"
        );
    }

    #[test]
    fn duplicate_gap_and_ordering_failures_are_distinct() {
        assert_eq!(
            MigrationCatalog::new([sql(2, "alpha", "alpha"), sql(2, "beta", "beta")]),
            Err(MigrationContractError::DuplicateVersion)
        );
        assert_eq!(
            MigrationCatalog::new([sql(2, "alpha", "alpha"), sql(3, "alpha", "beta")]),
            Err(MigrationContractError::DuplicateName)
        );
        assert_eq!(
            MigrationCatalog::new([sql(3, "alpha", "alpha")]),
            Err(MigrationContractError::VersionGap)
        );
        assert_eq!(
            MigrationCatalog::new([sql(2, "alpha", "alpha"), sql(4, "beta", "beta")]),
            Err(MigrationContractError::VersionGap)
        );
        assert_eq!(
            MigrationCatalog::new([sql(3, "alpha", "alpha"), sql(2, "beta", "beta")]),
            Err(MigrationContractError::OutOfOrder)
        );
        assert_eq!(
            MigrationCatalog::new([sql(u32::MAX, "alpha", "alpha")]),
            Err(MigrationContractError::VersionGap)
        );
    }

    #[test]
    fn catalog_count_is_bounded_during_ingestion() {
        let maximum = (0..MAX_MIGRATION_COUNT).map(|index| {
            let version = u32::try_from(index).unwrap() + 2;
            let name = Box::leak(format!("migration_{version}").into_boxed_str());
            sql(version, name, "SELECT 1;")
        });
        assert_eq!(
            MigrationCatalog::new(maximum).unwrap().descriptors().len(),
            MAX_MIGRATION_COUNT
        );

        let excessive = (0..=MAX_MIGRATION_COUNT).map(|index| {
            let version = u32::try_from(index).unwrap() + 2;
            let name = Box::leak(format!("migration_{version}").into_boxed_str());
            sql(version, name, "SELECT 1;")
        });
        assert_eq!(
            MigrationCatalog::new(excessive),
            Err(MigrationContractError::TooManyMigrations)
        );

        let infinite = (2_u32..).map(|version| {
            let name = Box::leak(format!("migration_{version}").into_boxed_str());
            sql(version, name, "SELECT 1;")
        });
        assert_eq!(
            MigrationCatalog::new(infinite),
            Err(MigrationContractError::TooManyMigrations)
        );
    }

    #[test]
    fn debug_and_errors_never_expose_migration_content() {
        const SECRET_SQL: &str = "SELECT 'migration-secret';";
        let descriptor = sql(2, "safe_name", SECRET_SQL);
        let catalog = MigrationCatalog::new([descriptor.clone()]).unwrap();
        for rendered in [format!("{descriptor:?}"), format!("{catalog:?}")] {
            assert!(!rendered.contains(SECRET_SQL));
            assert!(!rendered.contains("migration-secret"));
        }
        for error in [
            MigrationContractError::InvalidName,
            MigrationContractError::InvalidTargetVersion,
            MigrationContractError::EmptyContent,
            MigrationContractError::ContentTooLarge,
            MigrationContractError::ChecksumMismatch,
            MigrationContractError::TooManyMigrations,
            MigrationContractError::DuplicateVersion,
            MigrationContractError::DuplicateName,
            MigrationContractError::OutOfOrder,
            MigrationContractError::VersionGap,
        ] {
            assert!(!error.to_string().contains("secret"));
            assert!(error.source().is_none());
        }
    }

    fn hex(checksum: MigrationChecksum) -> String {
        checksum
            .as_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
}
