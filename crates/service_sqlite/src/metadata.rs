//! Immutable database identity metadata for one service instance.

use core::{fmt, num::NonZeroU32};
use std::error::Error;

use radroots_runtime_paths::{InstanceId, ServiceId};
use radroots_storage::event::SourceGeneration;

use crate::ServiceSqlitePaths;

#[cfg(any(target_os = "linux", target_os = "macos"))]
use crate::{ServiceSqliteError, ServiceSqliteErrorKind};

#[cfg(any(target_os = "linux", target_os = "macos"))]
use sqlx::{Connection, Row, SqliteConnection};

const MAX_APPLICATION_ID: u32 = i32::MAX as u32;
const MAX_CREATED_AT_UNIX_MS: u64 = i64::MAX as u64;

#[cfg(any(target_os = "linux", target_os = "macos"))]
const CREATE_METADATA_SQL: &str = r#"
CREATE TABLE radroots_service_metadata (
    singleton INTEGER NOT NULL PRIMARY KEY CHECK (singleton = 1),
    service_id TEXT NOT NULL,
    instance_id TEXT NOT NULL,
    source_generation BLOB NOT NULL CHECK (length(source_generation) = 32),
    state_schema_version INTEGER NOT NULL
        CHECK (state_schema_version BETWEEN 1 AND 4294967295),
    created_at_unix_ms INTEGER NOT NULL CHECK (created_at_unix_ms > 0)
) STRICT;
CREATE TRIGGER radroots_service_metadata_guard_update
BEFORE UPDATE ON radroots_service_metadata
WHEN NEW.singleton != OLD.singleton
    OR NEW.service_id != OLD.service_id
    OR NEW.instance_id != OLD.instance_id
    OR NEW.source_generation != OLD.source_generation
    OR NEW.created_at_unix_ms != OLD.created_at_unix_ms
    OR NEW.state_schema_version <= OLD.state_schema_version
BEGIN
    SELECT RAISE(ABORT, 'service metadata identity is immutable');
END;
CREATE TRIGGER radroots_service_metadata_no_delete
BEFORE DELETE ON radroots_service_metadata
BEGIN
    SELECT RAISE(ABORT, 'service metadata is immutable');
END;
"#;

/// A validated nonzero SQLite application identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ServiceSqliteApplicationId(u32);

impl ServiceSqliteApplicationId {
    /// Validates a caller-owned application identifier for SQLite's signed range.
    pub const fn new(value: u32) -> Result<Self, ServiceSqliteMetadataValueError> {
        if value == 0 || value > MAX_APPLICATION_ID {
            return Err(ServiceSqliteMetadataValueError::InvalidApplicationId);
        }
        Ok(Self(value))
    }

    /// Returns the validated application identifier.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Invalid caller-supplied database metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceSqliteMetadataValueError {
    InvalidApplicationId,
    InvalidCreationTime,
}

impl fmt::Display for ServiceSqliteMetadataValueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidApplicationId => "SQLite application ID is out of range",
            Self::InvalidCreationTime => "SQLite creation time is out of range",
        })
    }
}

impl Error for ServiceSqliteMetadataValueError {}

/// Exact immutable identity expected from one service database.
#[derive(Clone, PartialEq, Eq)]
pub struct ServiceDatabaseMetadata {
    service: ServiceId,
    instance: InstanceId,
    source_generation: SourceGeneration,
    state_schema_version: NonZeroU32,
    created_at_unix_ms: u64,
    application_id: ServiceSqliteApplicationId,
}

/// Exact mount identity and maximum schema version accepted by one service binary.
#[derive(Clone, PartialEq, Eq)]
pub struct ServiceDatabaseIdentity {
    service: ServiceId,
    instance: InstanceId,
    source_generation: SourceGeneration,
    supported_state_schema_version: NonZeroU32,
    application_id: ServiceSqliteApplicationId,
}

impl ServiceDatabaseMetadata {
    /// Constructs metadata bound to the service and instance in canonical paths.
    pub fn new(
        paths: &ServiceSqlitePaths,
        source_generation: SourceGeneration,
        state_schema_version: NonZeroU32,
        created_at_unix_ms: u64,
        application_id: ServiceSqliteApplicationId,
    ) -> Result<Self, ServiceSqliteMetadataValueError> {
        if created_at_unix_ms == 0 || created_at_unix_ms > MAX_CREATED_AT_UNIX_MS {
            return Err(ServiceSqliteMetadataValueError::InvalidCreationTime);
        }
        Ok(Self {
            service: paths.service().clone(),
            instance: paths.instance().clone(),
            source_generation,
            state_schema_version,
            created_at_unix_ms,
            application_id,
        })
    }

    /// Returns the bound service identity.
    #[must_use]
    pub fn service(&self) -> &ServiceId {
        &self.service
    }

    /// Returns the bound instance identity.
    #[must_use]
    pub fn instance(&self) -> &InstanceId {
        &self.instance
    }

    /// Returns the opaque nonzero source generation.
    #[must_use]
    pub const fn source_generation(&self) -> SourceGeneration {
        self.source_generation
    }

    /// Returns the expected nonzero state schema version.
    #[must_use]
    pub const fn state_schema_version(&self) -> NonZeroU32 {
        self.state_schema_version
    }

    /// Returns the injected positive creation time in Unix milliseconds.
    #[must_use]
    pub const fn created_at_unix_ms(&self) -> u64 {
        self.created_at_unix_ms
    }

    /// Returns the caller-owned SQLite application identifier.
    #[must_use]
    pub const fn application_id(&self) -> ServiceSqliteApplicationId {
        self.application_id
    }

    /// Returns the reopen identity derived from this initialization record.
    #[must_use]
    pub fn identity(&self) -> ServiceDatabaseIdentity {
        ServiceDatabaseIdentity {
            service: self.service.clone(),
            instance: self.instance.clone(),
            source_generation: self.source_generation,
            supported_state_schema_version: self.state_schema_version,
            application_id: self.application_id,
        }
    }

    pub(crate) fn matches_paths(&self, paths: &ServiceSqlitePaths) -> bool {
        self.service == *paths.service() && self.instance == *paths.instance()
    }
}

impl ServiceDatabaseIdentity {
    /// Constructs a reopen expectation bound to canonical service-instance paths.
    #[must_use]
    pub fn new(
        paths: &ServiceSqlitePaths,
        source_generation: SourceGeneration,
        supported_state_schema_version: NonZeroU32,
        application_id: ServiceSqliteApplicationId,
    ) -> Self {
        Self {
            service: paths.service().clone(),
            instance: paths.instance().clone(),
            source_generation,
            supported_state_schema_version,
            application_id,
        }
    }

    /// Returns the bound service identity.
    #[must_use]
    pub fn service(&self) -> &ServiceId {
        &self.service
    }

    /// Returns the bound instance identity.
    #[must_use]
    pub fn instance(&self) -> &InstanceId {
        &self.instance
    }

    /// Returns the expected opaque source generation.
    #[must_use]
    pub const fn source_generation(&self) -> SourceGeneration {
        self.source_generation
    }

    /// Returns the newest state schema version this binary accepts.
    #[must_use]
    pub const fn supported_state_schema_version(&self) -> NonZeroU32 {
        self.supported_state_schema_version
    }

    /// Returns the expected SQLite application identifier.
    #[must_use]
    pub const fn application_id(&self) -> ServiceSqliteApplicationId {
        self.application_id
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    pub(crate) fn matches_paths(&self, paths: &ServiceSqlitePaths) -> bool {
        self.service == *paths.service() && self.instance == *paths.instance()
    }
}

impl fmt::Debug for ServiceDatabaseIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ServiceDatabaseIdentity")
            .field("service", &"[redacted]")
            .field("instance", &"[redacted]")
            .field("source_generation", &"[redacted]")
            .field(
                "supported_state_schema_version",
                &self.supported_state_schema_version,
            )
            .field("application_id", &self.application_id)
            .finish()
    }
}

impl fmt::Debug for ServiceDatabaseMetadata {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ServiceDatabaseMetadata")
            .field("service", &"[redacted]")
            .field("instance", &"[redacted]")
            .field("source_generation", &"[redacted]")
            .field("state_schema_version", &self.state_schema_version)
            .field("created_at_unix_ms", &self.created_at_unix_ms)
            .field("application_id", &self.application_id)
            .finish()
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MetadataFailureKind {
    AlreadyPresent,
    Missing,
    Corrupt,
    Mismatch,
    Storage,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[derive(Debug)]
struct MetadataFailure(MetadataFailureKind);

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl fmt::Display for MetadataFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.0 {
            MetadataFailureKind::AlreadyPresent => "SQLite metadata already exists",
            MetadataFailureKind::Missing => "SQLite metadata is missing",
            MetadataFailureKind::Corrupt => "SQLite metadata is corrupt",
            MetadataFailureKind::Mismatch => "SQLite metadata identity does not match",
            MetadataFailureKind::Storage => "SQLite metadata could not be accessed",
        })
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl Error for MetadataFailure {}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn metadata_error(kind: MetadataFailureKind) -> ServiceSqliteError {
    ServiceSqliteError::with_source(ServiceSqliteErrorKind::Metadata, MetadataFailure(kind))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) async fn write_database_metadata(
    connection: &mut SqliteConnection,
    expected: &ServiceDatabaseMetadata,
) -> Result<(), ServiceSqliteError> {
    if read_application_id(connection).await? != 0 {
        return Err(metadata_error(MetadataFailureKind::AlreadyPresent));
    }

    let mut transaction = connection
        .begin()
        .await
        .map_err(|_| metadata_error(MetadataFailureKind::Storage))?;
    sqlx::raw_sql(CREATE_METADATA_SQL)
        .execute(&mut *transaction)
        .await
        .map_err(|_| metadata_error(MetadataFailureKind::AlreadyPresent))?;
    sqlx::query(
        "INSERT INTO radroots_service_metadata (
            singleton, service_id, instance_id, source_generation,
            state_schema_version, created_at_unix_ms
         ) VALUES (1, ?, ?, ?, ?, ?)",
    )
    .bind(expected.service().as_str())
    .bind(expected.instance().as_str())
    .bind(expected.source_generation().as_bytes().as_slice())
    .bind(i64::from(expected.state_schema_version().get()))
    .bind(
        i64::try_from(expected.created_at_unix_ms())
            .map_err(|_| metadata_error(MetadataFailureKind::Corrupt))?,
    )
    .execute(&mut *transaction)
    .await
    .map_err(|_| metadata_error(MetadataFailureKind::Storage))?;
    let set_application_id = format!(
        "PRAGMA application_id = {}",
        expected.application_id().get()
    );
    // The only dynamic token is a validated decimal u31 value.
    sqlx::query(sqlx::AssertSqlSafe(set_application_id.as_str()))
        .execute(&mut *transaction)
        .await
        .map_err(|_| metadata_error(MetadataFailureKind::Storage))?;
    transaction
        .commit()
        .await
        .map_err(|_| metadata_error(MetadataFailureKind::Storage))?;

    let actual = read_database_metadata(connection).await?;
    if actual != *expected {
        return Err(metadata_error(MetadataFailureKind::Mismatch));
    }
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) async fn verify_database_metadata(
    connection: &mut SqliteConnection,
    expected: &ServiceDatabaseIdentity,
) -> Result<ServiceDatabaseMetadata, ServiceSqliteError> {
    let actual = read_database_metadata(connection).await?;
    if actual.service != expected.service
        || actual.instance != expected.instance
        || actual.source_generation != expected.source_generation
        || actual.application_id != expected.application_id
        || actual.state_schema_version > expected.supported_state_schema_version
    {
        return Err(metadata_error(MetadataFailureKind::Mismatch));
    }
    Ok(actual)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
async fn read_database_metadata(
    connection: &mut SqliteConnection,
) -> Result<ServiceDatabaseMetadata, ServiceSqliteError> {
    let application_id = read_application_id(connection).await?;
    let application_id = ServiceSqliteApplicationId::new(
        u32::try_from(application_id).map_err(|_| metadata_error(MetadataFailureKind::Corrupt))?,
    )
    .map_err(|_| metadata_error(MetadataFailureKind::Corrupt))?;
    let rows = sqlx::query(
        "SELECT
            singleton, service_id, instance_id, source_generation,
            state_schema_version, created_at_unix_ms,
            typeof(singleton) AS singleton_type,
            typeof(service_id) AS service_id_type,
            typeof(instance_id) AS instance_id_type,
            typeof(source_generation) AS source_generation_type,
            typeof(state_schema_version) AS state_schema_version_type,
            typeof(created_at_unix_ms) AS created_at_unix_ms_type
         FROM radroots_service_metadata
         ORDER BY singleton
         LIMIT 2",
    )
    .fetch_all(&mut *connection)
    .await
    .map_err(|_| metadata_error(MetadataFailureKind::Missing))?;
    let [row] = rows.as_slice() else {
        return Err(metadata_error(if rows.is_empty() {
            MetadataFailureKind::Missing
        } else {
            MetadataFailureKind::Corrupt
        }));
    };
    for (column, expected_type) in [
        ("singleton_type", "integer"),
        ("service_id_type", "text"),
        ("instance_id_type", "text"),
        ("source_generation_type", "blob"),
        ("state_schema_version_type", "integer"),
        ("created_at_unix_ms_type", "integer"),
    ] {
        if row
            .try_get::<String, _>(column)
            .map_err(|_| metadata_error(MetadataFailureKind::Corrupt))?
            != expected_type
        {
            return Err(metadata_error(MetadataFailureKind::Corrupt));
        }
    }
    if row
        .try_get::<i64, _>("singleton")
        .map_err(|_| metadata_error(MetadataFailureKind::Corrupt))?
        != 1
    {
        return Err(metadata_error(MetadataFailureKind::Corrupt));
    }
    let service = ServiceId::new(
        row.try_get::<String, _>("service_id")
            .map_err(|_| metadata_error(MetadataFailureKind::Corrupt))?,
    )
    .map_err(|_| metadata_error(MetadataFailureKind::Corrupt))?;
    let instance = InstanceId::new(
        row.try_get::<String, _>("instance_id")
            .map_err(|_| metadata_error(MetadataFailureKind::Corrupt))?,
    )
    .map_err(|_| metadata_error(MetadataFailureKind::Corrupt))?;
    let source_generation = SourceGeneration::new(
        row.try_get::<Vec<u8>, _>("source_generation")
            .map_err(|_| metadata_error(MetadataFailureKind::Corrupt))?
            .try_into()
            .map_err(|_| metadata_error(MetadataFailureKind::Corrupt))?,
    )
    .map_err(|_| metadata_error(MetadataFailureKind::Corrupt))?;
    let state_schema_version = NonZeroU32::new(
        u32::try_from(
            row.try_get::<i64, _>("state_schema_version")
                .map_err(|_| metadata_error(MetadataFailureKind::Corrupt))?,
        )
        .map_err(|_| metadata_error(MetadataFailureKind::Corrupt))?,
    )
    .ok_or_else(|| metadata_error(MetadataFailureKind::Corrupt))?;
    let created_at_unix_ms = u64::try_from(
        row.try_get::<i64, _>("created_at_unix_ms")
            .map_err(|_| metadata_error(MetadataFailureKind::Corrupt))?,
    )
    .map_err(|_| metadata_error(MetadataFailureKind::Corrupt))?;
    if created_at_unix_ms == 0 {
        return Err(metadata_error(MetadataFailureKind::Corrupt));
    }

    Ok(ServiceDatabaseMetadata {
        service,
        instance,
        source_generation,
        state_schema_version,
        created_at_unix_ms,
        application_id,
    })
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
async fn read_application_id(connection: &mut SqliteConnection) -> Result<i64, ServiceSqliteError> {
    sqlx::query_scalar::<_, i64>("PRAGMA application_id")
        .fetch_one(connection)
        .await
        .map_err(|_| metadata_error(MetadataFailureKind::Storage))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use radroots_runtime_paths::{
        RadrootsHostEnvironment, RadrootsPathProfile, RadrootsPathResolver, RadrootsPlatform,
        RuntimeContext, RuntimeContextBootstrap, RuntimeContextSource,
    };

    use super::*;

    fn sqlite_paths(service: &str, instance: &str) -> ServiceSqlitePaths {
        let context = RuntimeContext::resolve(
            &RadrootsPathResolver::new(RadrootsPlatform::Linux, RadrootsHostEnvironment::default()),
            RuntimeContextBootstrap::new(
                RadrootsPathProfile::RepoLocal,
                Some(PathBuf::from("/isolated/service-metadata")),
                RuntimeContextSource::BootstrapCli,
                RuntimeContextSource::BootstrapCli,
            )
            .expect("bootstrap"),
            ServiceId::new(service).expect("service"),
            InstanceId::new(instance).expect("instance"),
        )
        .expect("runtime context");
        ServiceSqlitePaths::from_runtime_context(&context).expect("SQLite paths")
    }

    fn metadata(
        paths: &ServiceSqlitePaths,
        generation_byte: u8,
        schema_version: u32,
        creation_time: u64,
        application_id: u32,
    ) -> ServiceDatabaseMetadata {
        ServiceDatabaseMetadata::new(
            paths,
            SourceGeneration::new([generation_byte; 32]).expect("source generation"),
            NonZeroU32::new(schema_version).expect("schema version"),
            creation_time,
            ServiceSqliteApplicationId::new(application_id).expect("application ID"),
        )
        .expect("database metadata")
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    async fn memory_connection() -> SqliteConnection {
        SqliteConnection::connect("sqlite::memory:")
            .await
            .expect("memory SQLite")
    }

    #[test]
    fn application_id_and_creation_time_bounds_are_exact() {
        assert_eq!(
            ServiceSqliteApplicationId::new(0),
            Err(ServiceSqliteMetadataValueError::InvalidApplicationId)
        );
        assert_eq!(
            ServiceSqliteApplicationId::new(MAX_APPLICATION_ID + 1),
            Err(ServiceSqliteMetadataValueError::InvalidApplicationId)
        );
        assert_eq!(
            ServiceSqliteApplicationId::new(MAX_APPLICATION_ID)
                .expect("maximum application ID")
                .get(),
            MAX_APPLICATION_ID
        );

        let paths = sqlite_paths("myc", "primary");
        let generation = SourceGeneration::new([7; 32]).expect("source generation");
        let schema = NonZeroU32::new(1).expect("schema version");
        let application = ServiceSqliteApplicationId::new(1).expect("application ID");
        assert_eq!(
            ServiceDatabaseMetadata::new(&paths, generation, schema, 0, application),
            Err(ServiceSqliteMetadataValueError::InvalidCreationTime)
        );
        assert_eq!(
            ServiceDatabaseMetadata::new(
                &paths,
                generation,
                schema,
                MAX_CREATED_AT_UNIX_MS + 1,
                application,
            ),
            Err(ServiceSqliteMetadataValueError::InvalidCreationTime)
        );
        assert!(
            ServiceDatabaseMetadata::new(
                &paths,
                generation,
                schema,
                MAX_CREATED_AT_UNIX_MS,
                application,
            )
            .is_ok()
        );
        assert!(SourceGeneration::new([0; 32]).is_err());
        assert!(NonZeroU32::new(0).is_none());
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test(flavor = "current_thread")]
    async fn fresh_metadata_write_read_and_immutability_are_exact() {
        let paths = sqlite_paths("myc", "primary");
        let expected = metadata(&paths, 7, 1, 1_700_000_000_000, 0x5244_5351);
        let mut connection = memory_connection().await;

        write_database_metadata(&mut connection, &expected)
            .await
            .expect("write metadata");
        verify_database_metadata(&mut connection, &expected.identity())
            .await
            .expect("verify metadata");
        let actual = read_database_metadata(&mut connection)
            .await
            .expect("read metadata");
        assert_eq!(actual, expected);
        assert_eq!(actual.service().as_str(), "myc");
        assert_eq!(actual.instance().as_str(), "primary");
        assert_eq!(actual.source_generation().as_bytes(), &[7; 32]);
        assert_eq!(actual.state_schema_version().get(), 1);
        assert_eq!(actual.created_at_unix_ms(), 1_700_000_000_000);
        assert_eq!(actual.application_id().get(), 0x5244_5351);
        assert_eq!(
            read_application_id(&mut connection).await.unwrap(),
            0x5244_5351
        );

        sqlx::query(
            "UPDATE radroots_service_metadata SET state_schema_version = 2 WHERE singleton = 1",
        )
        .execute(&mut connection)
        .await
        .expect("monotonic schema advance");
        assert_eq!(
            read_database_metadata(&mut connection)
                .await
                .expect("advanced metadata")
                .state_schema_version()
                .get(),
            2
        );
        for statement in [
            "UPDATE radroots_service_metadata SET service_id = 'rhi' WHERE singleton = 1",
            "UPDATE radroots_service_metadata SET state_schema_version = 2 WHERE singleton = 1",
            "UPDATE radroots_service_metadata SET state_schema_version = 1 WHERE singleton = 1",
            "DELETE FROM radroots_service_metadata WHERE singleton = 1",
            "INSERT INTO radroots_service_metadata VALUES (2, 'rhi', 'default', zeroblob(32), 1, 1)",
        ] {
            assert!(
                sqlx::query(statement)
                    .execute(&mut connection)
                    .await
                    .is_err(),
                "immutable metadata accepted `{statement}`"
            );
        }
        assert_eq!(
            write_database_metadata(&mut connection, &expected)
                .await
                .expect_err("second write must fail")
                .kind(),
            ServiceSqliteErrorKind::Metadata
        );

        let debug = format!("{actual:?}");
        for sensitive in ["myc", "primary", "07070707"] {
            assert!(!debug.contains(sensitive));
        }
        assert!(debug.contains("source_generation: \"[redacted]\""));
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test(flavor = "current_thread")]
    async fn every_identity_dimension_must_match() {
        let paths = sqlite_paths("myc", "primary");
        let expected = metadata(&paths, 7, 1, 1_700_000_000_000, 0x5244_5351);
        let mut connection = memory_connection().await;
        write_database_metadata(&mut connection, &expected)
            .await
            .expect("write metadata");

        let alternatives = [
            ServiceDatabaseIdentity::new(
                &sqlite_paths("rhi", "primary"),
                SourceGeneration::new([7; 32]).unwrap(),
                NonZeroU32::new(1).unwrap(),
                ServiceSqliteApplicationId::new(0x5244_5351).unwrap(),
            ),
            ServiceDatabaseIdentity::new(
                &sqlite_paths("myc", "secondary"),
                SourceGeneration::new([7; 32]).unwrap(),
                NonZeroU32::new(1).unwrap(),
                ServiceSqliteApplicationId::new(0x5244_5351).unwrap(),
            ),
            ServiceDatabaseIdentity::new(
                &paths,
                SourceGeneration::new([8; 32]).unwrap(),
                NonZeroU32::new(1).unwrap(),
                ServiceSqliteApplicationId::new(0x5244_5351).unwrap(),
            ),
            ServiceDatabaseIdentity::new(
                &paths,
                SourceGeneration::new([7; 32]).unwrap(),
                NonZeroU32::new(1).unwrap(),
                ServiceSqliteApplicationId::new(0x5244_5352).unwrap(),
            ),
        ];
        for alternative in alternatives {
            assert_eq!(
                verify_database_metadata(&mut connection, &alternative)
                    .await
                    .expect_err("identity mismatch")
                    .kind(),
                ServiceSqliteErrorKind::Metadata
            );
        }

        let newer_binary = ServiceDatabaseIdentity::new(
            &paths,
            expected.source_generation(),
            NonZeroU32::new(2).unwrap(),
            expected.application_id(),
        );
        let stored = verify_database_metadata(&mut connection, &newer_binary)
            .await
            .expect("older schema is migration eligible");
        assert_eq!(stored.created_at_unix_ms(), 1_700_000_000_000);
        assert_eq!(stored.state_schema_version().get(), 1);

        let mut newer_state = memory_connection().await;
        let version_two = metadata(&paths, 7, 2, 1_700_000_000_001, 0x5244_5351);
        write_database_metadata(&mut newer_state, &version_two)
            .await
            .expect("write newer metadata");
        assert_eq!(
            verify_database_metadata(&mut newer_state, &expected.identity())
                .await
                .expect_err("newer state must fail closed")
                .kind(),
            ServiceSqliteErrorKind::Metadata
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test(flavor = "current_thread")]
    async fn missing_duplicate_and_corrupt_metadata_fail_closed() {
        const PERMISSIVE_TABLE: &str = "CREATE TABLE radroots_service_metadata (
            singleton, service_id, instance_id, source_generation,
            state_schema_version, created_at_unix_ms
        )";
        let paths = sqlite_paths("myc", "primary");
        let expected = metadata(&paths, 7, 1, 1_700_000_000_000, 0x5244_5351);

        let mut missing_table = memory_connection().await;
        assert_eq!(
            verify_database_metadata(&mut missing_table, &expected.identity())
                .await
                .expect_err("missing table")
                .kind(),
            ServiceSqliteErrorKind::Metadata
        );

        let corrupt_rows = [
            "",
            "INSERT INTO radroots_service_metadata VALUES
                (1, 'myc', 'primary', randomblob(32), 1, 1700000000000),
                (2, 'myc', 'primary', randomblob(32), 1, 1700000000000)",
            "INSERT INTO radroots_service_metadata VALUES
                (1, NULL, 'primary', randomblob(32), 1, 1700000000000)",
            "INSERT INTO radroots_service_metadata VALUES
                (1, 'Myc', 'primary', randomblob(32), 1, 1700000000000)",
            "INSERT INTO radroots_service_metadata VALUES
                (1, 'myc', 'Primary', randomblob(32), 1, 1700000000000)",
            "INSERT INTO radroots_service_metadata VALUES
                (1, 'myc', 'primary', zeroblob(31), 1, 1700000000000)",
            "INSERT INTO radroots_service_metadata VALUES
                (1, 'myc', 'primary', zeroblob(32), 1, 1700000000000)",
            "INSERT INTO radroots_service_metadata VALUES
                (1, 'myc', 'primary', randomblob(33), 1, 1700000000000)",
            "INSERT INTO radroots_service_metadata VALUES
                (1, 'myc', 'primary', randomblob(32), 0, 1700000000000)",
            "INSERT INTO radroots_service_metadata VALUES
                (1, 'myc', 'primary', randomblob(32), 1, 0)",
            "INSERT INTO radroots_service_metadata VALUES
                ('1', 'myc', 'primary', randomblob(32), 1, 1700000000000)",
        ];
        for corrupt_row in corrupt_rows {
            let mut connection = memory_connection().await;
            sqlx::raw_sql(PERMISSIVE_TABLE)
                .execute(&mut connection)
                .await
                .expect("permissive metadata table");
            sqlx::query("PRAGMA application_id = 1380209489")
                .execute(&mut connection)
                .await
                .expect("application ID");
            if !corrupt_row.is_empty() {
                sqlx::raw_sql(corrupt_row)
                    .execute(&mut connection)
                    .await
                    .expect("corrupt metadata row");
            }
            assert_eq!(
                verify_database_metadata(&mut connection, &expected.identity())
                    .await
                    .expect_err("corrupt metadata")
                    .kind(),
                ServiceSqliteErrorKind::Metadata,
                "accepted corrupt fixture `{corrupt_row}`"
            );
        }
    }
}
