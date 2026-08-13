//! Immutable expected SQLite schema-object catalogs.

use core::fmt;
use std::{collections::BTreeSet, error::Error};

use sha2::{Digest, Sha256};

use crate::{MigrationCatalog, MigrationChecksum};

pub(crate) const MAX_SCHEMA_OBJECT_COUNT: usize = 4096;
pub(crate) const MAX_SCHEMA_SQL_UTF8_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_SCHEMA_CATALOG_UTF8_BYTES: usize = 16 * 1024 * 1024;
const MAX_SCHEMA_NAME_UTF8_BYTES: usize = 128;
const MAX_SCHEMA_VERSION_COUNT: usize = 4097;

const OBJECT_DOMAIN: &[u8] = b"radroots.service_sqlite.schema_object.v1\0";
const SNAPSHOT_DOMAIN: &[u8] = b"radroots.service_sqlite.schema_snapshot.v1\0";
const CATALOG_DOMAIN: &[u8] = b"radroots.service_sqlite.schema_catalog.v1\0";

pub(crate) const CREATE_METADATA_TABLE_SQL: &str = r#"CREATE TABLE radroots_service_metadata (
    singleton INTEGER NOT NULL PRIMARY KEY CHECK (singleton = 1),
    service_id TEXT NOT NULL,
    instance_id TEXT NOT NULL,
    source_generation BLOB NOT NULL CHECK (length(source_generation) = 32),
    state_schema_version INTEGER NOT NULL
        CHECK (state_schema_version BETWEEN 1 AND 4294967295),
    created_at_unix_ms INTEGER NOT NULL CHECK (created_at_unix_ms > 0)
) STRICT"#;
pub(crate) const CREATE_METADATA_GUARD_TRIGGER_SQL: &str = r#"CREATE TRIGGER radroots_service_metadata_guard_update
BEFORE UPDATE ON radroots_service_metadata
WHEN NEW.singleton != OLD.singleton
    OR NEW.service_id != OLD.service_id
    OR NEW.instance_id != OLD.instance_id
    OR NEW.source_generation != OLD.source_generation
    OR NEW.created_at_unix_ms != OLD.created_at_unix_ms
    OR NEW.state_schema_version <= OLD.state_schema_version
BEGIN
    SELECT RAISE(ABORT, 'service metadata identity is immutable');
END"#;
pub(crate) const CREATE_METADATA_NO_DELETE_TRIGGER_SQL: &str = r#"CREATE TRIGGER radroots_service_metadata_no_delete
BEFORE DELETE ON radroots_service_metadata
BEGIN
    SELECT RAISE(ABORT, 'service metadata is immutable');
END"#;
pub(crate) const CREATE_MIGRATION_LEDGER_TABLE_SQL: &str = r#"CREATE TABLE schema_migrations (
    version INTEGER NOT NULL PRIMARY KEY
        CHECK (version BETWEEN 2 AND 4294967295),
    name TEXT NOT NULL UNIQUE
        CHECK (length(CAST(name AS BLOB)) BETWEEN 1 AND 128),
    checksum BLOB NOT NULL CHECK (length(checksum) = 32),
    applied_at_unix_s INTEGER NOT NULL CHECK (applied_at_unix_s BETWEEN 0 AND 9223372036854775807),
    service_version TEXT NOT NULL CHECK (length(CAST(service_version AS BLOB)) BETWEEN 1 AND 128),
    service_commit TEXT NOT NULL CHECK (length(CAST(service_commit AS BLOB)) = 40),
    lib_revision TEXT NOT NULL CHECK (length(CAST(lib_revision AS BLOB)) = 40),
    rust_version TEXT NOT NULL CHECK (length(CAST(rust_version AS BLOB)) BETWEEN 1 AND 128),
    target TEXT NOT NULL CHECK (length(CAST(target AS BLOB)) BETWEEN 1 AND 128),
    feature_profile TEXT NOT NULL CHECK (length(CAST(feature_profile AS BLOB)) BETWEEN 1 AND 128),
    config_contract_version INTEGER NOT NULL CHECK (config_contract_version BETWEEN 1 AND 4294967295),
    state_contract_version INTEGER NOT NULL CHECK (state_contract_version BETWEEN 1 AND 4294967295),
    admin_contract_version INTEGER NOT NULL CHECK (admin_contract_version BETWEEN 1 AND 4294967295),
    status_contract_version INTEGER NOT NULL CHECK (status_contract_version BETWEEN 1 AND 4294967295),
    provider_contract_version INTEGER NOT NULL CHECK (provider_contract_version BETWEEN 1 AND 4294967295)
) STRICT"#;
pub(crate) const CREATE_MIGRATION_NO_UPDATE_TRIGGER_SQL: &str = r#"CREATE TRIGGER schema_migrations_no_update
BEFORE UPDATE ON schema_migrations
BEGIN
    SELECT RAISE(ABORT, 'migration history is immutable');
END"#;
pub(crate) const CREATE_MIGRATION_NO_DELETE_TRIGGER_SQL: &str = r#"CREATE TRIGGER schema_migrations_no_delete
BEFORE DELETE ON schema_migrations
BEGIN
    SELECT RAISE(ABORT, 'migration history is immutable');
END"#;

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) const METADATA_SCHEMA_SQL: [&str; 3] = [
    CREATE_METADATA_TABLE_SQL,
    CREATE_METADATA_GUARD_TRIGGER_SQL,
    CREATE_METADATA_NO_DELETE_TRIGGER_SQL,
];
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) const MIGRATION_LEDGER_SCHEMA_SQL: [&str; 3] = [
    CREATE_MIGRATION_LEDGER_TABLE_SQL,
    CREATE_MIGRATION_NO_UPDATE_TRIGGER_SQL,
    CREATE_MIGRATION_NO_DELETE_TRIGGER_SQL,
];

/// Supported persistent object kinds in a governed service schema.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SchemaObjectKind {
    Table,
    Index,
    Trigger,
}

impl SchemaObjectKind {
    pub(crate) const fn tag(self) -> u8 {
        match self {
            Self::Table => 0,
            Self::Index => 1,
            Self::Trigger => 2,
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    pub(crate) fn from_sqlite(value: &str) -> Option<Self> {
        match value {
            "table" => Some(Self::Table),
            "index" => Some(Self::Index),
            "trigger" => Some(Self::Trigger),
            _ => None,
        }
    }
}

/// A SHA-256 digest over an object, version snapshot, or bound schema catalog.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct SchemaDigest([u8; 32]);

impl SchemaDigest {
    /// Constructs an independently reviewed digest from exact bytes.
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

impl fmt::Debug for SchemaDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SchemaDigest([redacted])")
    }
}

/// One immutable service-owned SQLite schema object definition.
#[derive(Clone, PartialEq, Eq)]
pub struct SchemaObject {
    kind: SchemaObjectKind,
    name: &'static str,
    table_name: &'static str,
    sql: &'static str,
    digest: SchemaDigest,
}

impl SchemaObject {
    /// Validates an embedded object definition and its independently pinned digest.
    pub fn new(
        kind: SchemaObjectKind,
        name: &'static str,
        table_name: &'static str,
        sql: &'static str,
        expected_digest: SchemaDigest,
    ) -> Result<Self, SchemaCatalogContractError> {
        validate_service_object(kind, name, table_name, sql)?;
        let actual_digest = object_digest(kind, name, table_name, sql);
        if actual_digest != expected_digest {
            return Err(SchemaCatalogContractError::ObjectDigestMismatch);
        }
        Ok(Self {
            kind,
            name,
            table_name,
            sql,
            digest: actual_digest,
        })
    }

    #[must_use]
    pub const fn kind(&self) -> SchemaObjectKind {
        self.kind
    }

    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    #[must_use]
    pub const fn table_name(&self) -> &'static str {
        self.table_name
    }

    #[must_use]
    pub const fn digest(&self) -> SchemaDigest {
        self.digest
    }

    /// Computes the frozen object digest for independent pin generation.
    pub fn computed_digest(
        kind: SchemaObjectKind,
        name: &str,
        table_name: &str,
        sql: &str,
    ) -> Result<SchemaDigest, SchemaCatalogContractError> {
        validate_service_object(kind, name, table_name, sql)?;
        Ok(object_digest(kind, name, table_name, sql))
    }
}

impl fmt::Debug for SchemaObject {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SchemaObject")
            .field("kind", &self.kind)
            .field("name", &self.name)
            .field("table_name", &self.table_name)
            .field("digest", &self.digest)
            .field("sql", &"[redacted]")
            .finish()
    }
}

/// Exact expected non-internal object snapshot for one schema version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SchemaVersionCatalog {
    version: u32,
    object_count: u32,
    digest: SchemaDigest,
}

impl SchemaVersionCatalog {
    /// Validates service-owned objects, adds the shared objects, and pins the snapshot.
    pub fn new<I>(
        version: u32,
        service_objects: I,
        expected_digest: SchemaDigest,
    ) -> Result<Self, SchemaCatalogContractError>
    where
        I: IntoIterator<Item = SchemaObject>,
    {
        if version == 0 {
            return Err(SchemaCatalogContractError::InvalidVersionSequence);
        }
        let service_objects: Vec<_> = service_objects
            .into_iter()
            .take(MAX_SCHEMA_OBJECT_COUNT + 1)
            .collect();
        if service_objects.len() + shared_objects().len() > MAX_SCHEMA_OBJECT_COUNT {
            return Err(SchemaCatalogContractError::TooManyObjects);
        }
        validate_object_set(&service_objects)?;
        let mut objects = shared_objects();
        objects.extend(service_objects.iter().map(ObjectRef::from));
        let actual_digest = snapshot_digest(version, &objects);
        if actual_digest != expected_digest {
            return Err(SchemaCatalogContractError::SnapshotDigestMismatch);
        }
        Ok(Self {
            version,
            object_count: u32::try_from(objects.len()).expect("schema object bound fits in u32"),
            digest: actual_digest,
        })
    }

    /// Computes a snapshot digest for pin generation after validating the object set.
    pub fn computed_digest<I>(
        version: u32,
        service_objects: I,
    ) -> Result<SchemaDigest, SchemaCatalogContractError>
    where
        I: IntoIterator<Item = SchemaObject>,
    {
        if version == 0 {
            return Err(SchemaCatalogContractError::InvalidVersionSequence);
        }
        let service_objects: Vec<_> = service_objects
            .into_iter()
            .take(MAX_SCHEMA_OBJECT_COUNT + 1)
            .collect();
        if service_objects.len() + shared_objects().len() > MAX_SCHEMA_OBJECT_COUNT {
            return Err(SchemaCatalogContractError::TooManyObjects);
        }
        validate_object_set(&service_objects)?;
        let mut objects = shared_objects();
        objects.extend(service_objects.iter().map(ObjectRef::from));
        Ok(snapshot_digest(version, &objects))
    }

    #[must_use]
    pub const fn version(self) -> u32 {
        self.version
    }

    #[must_use]
    pub const fn object_count(self) -> u32 {
        self.object_count
    }

    #[must_use]
    pub const fn digest(self) -> SchemaDigest {
        self.digest
    }
}

/// Ordered exact schema snapshots bound to one migration catalog.
#[derive(Clone, PartialEq, Eq)]
pub struct SchemaCatalog {
    versions: Box<[SchemaVersionCatalog]>,
    migration_catalog_digest: MigrationChecksum,
    digest: SchemaDigest,
}

impl SchemaCatalog {
    /// Validates one exact snapshot for every migration-catalog schema version.
    pub fn new<I>(
        migrations: &MigrationCatalog,
        versions: I,
    ) -> Result<Self, SchemaCatalogContractError>
    where
        I: IntoIterator<Item = SchemaVersionCatalog>,
    {
        let versions: Vec<_> = versions
            .into_iter()
            .take(MAX_SCHEMA_VERSION_COUNT + 1)
            .collect();
        if versions.len() > MAX_SCHEMA_VERSION_COUNT {
            return Err(SchemaCatalogContractError::TooManyVersions);
        }
        let expected_len = usize::try_from(migrations.current_version())
            .map_err(|_| SchemaCatalogContractError::MigrationCatalogMismatch)?;
        if versions.len() != expected_len
            || versions
                .iter()
                .enumerate()
                .any(|(index, entry)| entry.version != u32::try_from(index + 1).unwrap_or(0))
        {
            return Err(SchemaCatalogContractError::InvalidVersionSequence);
        }
        let migration_catalog_digest = migrations.digest();
        let digest = catalog_digest(migration_catalog_digest, &versions);
        Ok(Self {
            versions: versions.into_boxed_slice(),
            migration_catalog_digest,
            digest,
        })
    }

    #[must_use]
    pub fn versions(&self) -> &[SchemaVersionCatalog] {
        &self.versions
    }

    #[must_use]
    pub const fn migration_catalog_digest(&self) -> MigrationChecksum {
        self.migration_catalog_digest
    }

    #[must_use]
    pub const fn digest(&self) -> SchemaDigest {
        self.digest
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    pub(crate) fn matches_migrations(&self, migrations: &MigrationCatalog) -> bool {
        crate::all_constraints([
            self.migration_catalog_digest == migrations.digest(),
            self.versions.len()
                == usize::try_from(migrations.current_version()).unwrap_or(usize::MAX),
        ])
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    pub(crate) fn version(&self, version: u32) -> Option<SchemaVersionCatalog> {
        let index = usize::try_from(version.checked_sub(1)?).ok()?;
        self.versions.get(index).copied()
    }
}

impl fmt::Debug for SchemaCatalog {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SchemaCatalog")
            .field("version_count", &self.versions.len())
            .field("migration_catalog_digest", &self.migration_catalog_digest)
            .field("digest", &self.digest)
            .finish()
    }
}

/// Invalid immutable schema-catalog construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SchemaCatalogContractError {
    InvalidName,
    ReservedName,
    InvalidBinding,
    InvalidSql,
    ObjectDigestMismatch,
    DuplicateObject,
    TooManyObjects,
    SnapshotDigestMismatch,
    InvalidVersionSequence,
    TooManyVersions,
    MigrationCatalogMismatch,
}

impl fmt::Display for SchemaCatalogContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidName => "SQLite schema object name is invalid",
            Self::ReservedName => "SQLite schema object name is reserved",
            Self::InvalidBinding => "SQLite schema object table binding is invalid",
            Self::InvalidSql => "SQLite schema object definition is invalid",
            Self::ObjectDigestMismatch => "SQLite schema object digest does not match",
            Self::DuplicateObject => "SQLite schema object identity is duplicated",
            Self::TooManyObjects => "SQLite schema snapshot has too many objects",
            Self::SnapshotDigestMismatch => "SQLite schema snapshot digest does not match",
            Self::InvalidVersionSequence => "SQLite schema catalog version sequence is invalid",
            Self::TooManyVersions => "SQLite schema catalog has too many versions",
            Self::MigrationCatalogMismatch => {
                "SQLite schema catalog does not match the migration catalog"
            }
        })
    }
}

impl Error for SchemaCatalogContractError {}

#[derive(Clone, Copy)]
pub(crate) struct ObjectRef<'a> {
    pub(crate) kind: SchemaObjectKind,
    pub(crate) name: &'a str,
    pub(crate) table_name: &'a str,
    pub(crate) sql: &'a str,
    pub(crate) digest: SchemaDigest,
}

impl<'a> From<&'a SchemaObject> for ObjectRef<'a> {
    fn from(value: &'a SchemaObject) -> Self {
        Self {
            kind: value.kind,
            name: value.name,
            table_name: value.table_name,
            sql: value.sql,
            digest: value.digest,
        }
    }
}

pub(crate) fn object_digest(
    kind: SchemaObjectKind,
    name: &str,
    table_name: &str,
    sql: &str,
) -> SchemaDigest {
    let mut hasher = Sha256::new();
    hasher.update(OBJECT_DOMAIN);
    hasher.update([kind.tag()]);
    update_length_prefixed(&mut hasher, name.as_bytes());
    update_length_prefixed(&mut hasher, table_name.as_bytes());
    update_length_prefixed(&mut hasher, sql.as_bytes());
    SchemaDigest(hasher.finalize().into())
}

pub(crate) fn snapshot_digest(version: u32, objects: &[ObjectRef<'_>]) -> SchemaDigest {
    let mut objects = objects.to_vec();
    objects.sort_by(|left, right| {
        (
            left.kind.tag(),
            left.name.as_bytes(),
            left.table_name.as_bytes(),
        )
            .cmp(&(
                right.kind.tag(),
                right.name.as_bytes(),
                right.table_name.as_bytes(),
            ))
    });
    let mut hasher = Sha256::new();
    hasher.update(SNAPSHOT_DOMAIN);
    hasher.update(version.to_be_bytes());
    hasher.update(
        u32::try_from(objects.len())
            .expect("schema object bound fits in u32")
            .to_be_bytes(),
    );
    for object in objects {
        hasher.update([object.kind.tag()]);
        update_length_prefixed(&mut hasher, object.name.as_bytes());
        update_length_prefixed(&mut hasher, object.table_name.as_bytes());
        hasher.update(object.digest.as_bytes());
    }
    SchemaDigest(hasher.finalize().into())
}

fn catalog_digest(
    migration_digest: MigrationChecksum,
    versions: &[SchemaVersionCatalog],
) -> SchemaDigest {
    let mut hasher = Sha256::new();
    hasher.update(CATALOG_DOMAIN);
    hasher.update(migration_digest.as_bytes());
    hasher.update(
        u32::try_from(versions.len())
            .expect("schema version bound fits in u32")
            .to_be_bytes(),
    );
    for version in versions {
        hasher.update(version.version.to_be_bytes());
        hasher.update(version.object_count.to_be_bytes());
        hasher.update(version.digest.as_bytes());
    }
    SchemaDigest(hasher.finalize().into())
}

fn update_length_prefixed(hasher: &mut Sha256, value: &[u8]) {
    hasher.update(
        u64::try_from(value.len())
            .expect("bounded schema field fits in u64")
            .to_be_bytes(),
    );
    hasher.update(value);
}

fn validate_service_object(
    kind: SchemaObjectKind,
    name: &str,
    table_name: &str,
    sql: &str,
) -> Result<(), SchemaCatalogContractError> {
    if !crate::all_constraints([valid_name(name), valid_name(table_name)]) {
        return Err(SchemaCatalogContractError::InvalidName);
    }
    if !crate::all_constraints([!is_reserved(name), !is_reserved(table_name)]) {
        return Err(SchemaCatalogContractError::ReservedName);
    }
    if (kind == SchemaObjectKind::Table) != (name == table_name) {
        return Err(SchemaCatalogContractError::InvalidBinding);
    }
    if !crate::all_constraints([
        !sql.is_empty(),
        sql.len() <= MAX_SCHEMA_SQL_UTF8_BYTES,
        !sql.as_bytes().contains(&0),
    ]) {
        return Err(SchemaCatalogContractError::InvalidSql);
    }
    Ok(())
}

fn validate_object_set(objects: &[SchemaObject]) -> Result<(), SchemaCatalogContractError> {
    let mut identities = BTreeSet::new();
    let tables = objects
        .iter()
        .filter(|object| object.kind == SchemaObjectKind::Table)
        .map(|object| object.name)
        .collect::<BTreeSet<_>>();
    let mut total_sql_bytes = shared_objects()
        .iter()
        .map(|object| object.sql.len())
        .sum::<usize>();
    for object in objects {
        if !identities.insert((object.kind, object.name)) {
            return Err(SchemaCatalogContractError::DuplicateObject);
        }
        if object.kind != SchemaObjectKind::Table && !tables.contains(object.table_name) {
            return Err(SchemaCatalogContractError::InvalidBinding);
        }
        total_sql_bytes = total_sql_bytes
            .checked_add(object.sql.len())
            .ok_or(SchemaCatalogContractError::TooManyObjects)?;
    }
    if total_sql_bytes > MAX_SCHEMA_CATALOG_UTF8_BYTES {
        return Err(SchemaCatalogContractError::TooManyObjects);
    }
    Ok(())
}

fn valid_name(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    crate::all_constraints([
        bytes.len() <= MAX_SCHEMA_NAME_UTF8_BYTES,
        bytes[0].is_ascii_lowercase(),
        bytes[bytes.len() - 1].is_ascii_alphanumeric(),
        !bytes.windows(2).any(|pair| pair == b"__"),
        bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_'),
    ])
}

fn is_reserved(value: &str) -> bool {
    value.starts_with("sqlite_") || shared_objects().iter().any(|object| object.name == value)
}

fn shared_objects() -> Vec<ObjectRef<'static>> {
    [
        (
            SchemaObjectKind::Table,
            "radroots_service_metadata",
            "radroots_service_metadata",
            CREATE_METADATA_TABLE_SQL,
        ),
        (
            SchemaObjectKind::Trigger,
            "radroots_service_metadata_guard_update",
            "radroots_service_metadata",
            CREATE_METADATA_GUARD_TRIGGER_SQL,
        ),
        (
            SchemaObjectKind::Trigger,
            "radroots_service_metadata_no_delete",
            "radroots_service_metadata",
            CREATE_METADATA_NO_DELETE_TRIGGER_SQL,
        ),
        (
            SchemaObjectKind::Table,
            "schema_migrations",
            "schema_migrations",
            CREATE_MIGRATION_LEDGER_TABLE_SQL,
        ),
        (
            SchemaObjectKind::Trigger,
            "schema_migrations_no_update",
            "schema_migrations",
            CREATE_MIGRATION_NO_UPDATE_TRIGGER_SQL,
        ),
        (
            SchemaObjectKind::Trigger,
            "schema_migrations_no_delete",
            "schema_migrations",
            CREATE_MIGRATION_NO_DELETE_TRIGGER_SQL,
        ),
    ]
    .into_iter()
    .map(|(kind, name, table_name, sql)| ObjectRef {
        kind,
        name,
        table_name,
        sql,
        digest: object_digest(kind, name, table_name, sql),
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE_SQL: &str = "CREATE TABLE alpha (value INTEGER NOT NULL) STRICT";

    fn table() -> SchemaObject {
        SchemaObject::new(
            SchemaObjectKind::Table,
            "alpha",
            "alpha",
            TABLE_SQL,
            SchemaObject::computed_digest(SchemaObjectKind::Table, "alpha", "alpha", TABLE_SQL)
                .unwrap(),
        )
        .unwrap()
    }

    fn empty_migrations() -> MigrationCatalog {
        MigrationCatalog::new([]).unwrap()
    }

    #[test]
    fn exact_object_snapshot_and_catalog_vectors_are_stable() {
        let object = table();
        assert_eq!(
            object.digest().as_bytes(),
            &[
                0xf1, 0xa0, 0x6b, 0x60, 0x76, 0x0f, 0x73, 0xae, 0x0b, 0x43, 0x44, 0x1b, 0xb8, 0xfc,
                0x12, 0x56, 0x48, 0xa9, 0xcb, 0xf5, 0x63, 0xdb, 0x59, 0xd6, 0xc1, 0xfc, 0x60, 0x3b,
                0x5e, 0x92, 0x0b, 0x7c,
            ]
        );
        let snapshot_digest = SchemaVersionCatalog::computed_digest(1, [object.clone()]).unwrap();
        assert_eq!(
            snapshot_digest.as_bytes(),
            &[
                0x9e, 0x50, 0xd2, 0x25, 0xfe, 0xdc, 0xe1, 0x4f, 0x0d, 0x40, 0x41, 0x84, 0xb2, 0x00,
                0xbc, 0xd5, 0xcf, 0xe9, 0xb3, 0x56, 0x98, 0x16, 0xf6, 0x29, 0xc2, 0x40, 0x86, 0xdf,
                0x93, 0x2f, 0x52, 0x4a,
            ]
        );
        let version = SchemaVersionCatalog::new(1, [object], snapshot_digest).unwrap();
        let catalog = SchemaCatalog::new(&empty_migrations(), [version]).unwrap();
        assert_eq!(version.version(), 1);
        assert_eq!(version.object_count(), 7);
        assert_eq!(catalog.versions(), &[version]);
        assert_eq!(
            catalog.digest().as_bytes(),
            &[
                0xff, 0x9d, 0xbe, 0x4f, 0x32, 0x42, 0xb3, 0x3f, 0x6f, 0xd8, 0x76, 0x9d, 0x3e, 0x11,
                0x21, 0x7b, 0x38, 0xb2, 0x77, 0x3e, 0xa6, 0xa2, 0x98, 0x6b, 0x85, 0xb6, 0xed, 0x8d,
                0xe2, 0x4b, 0x52, 0x51,
            ]
        );
    }

    #[test]
    fn object_validation_is_closed_and_redacted() {
        let digest = SchemaDigest::from_bytes([0; 32]);
        for result in [
            SchemaObject::new(SchemaObjectKind::Table, "", "", "x", digest),
            SchemaObject::new(SchemaObjectKind::Table, "Bad", "Bad", "x", digest),
            SchemaObject::new(
                SchemaObjectKind::Table,
                "sqlite_bad",
                "sqlite_bad",
                "x",
                digest,
            ),
            SchemaObject::new(
                SchemaObjectKind::Table,
                "schema_migrations",
                "schema_migrations",
                "x",
                digest,
            ),
            SchemaObject::new(
                SchemaObjectKind::Index,
                "alpha_idx",
                "alpha_idx",
                "x",
                digest,
            ),
            SchemaObject::new(SchemaObjectKind::Table, "alpha", "alpha", "", digest),
        ] {
            assert!(result.is_err());
        }
        let debug = format!("{:?}", table());
        assert!(!debug.contains(TABLE_SQL));
        assert!(debug.contains("[redacted]"));

        let maximum_name = Box::leak("a".repeat(MAX_SCHEMA_NAME_UTF8_BYTES).into_boxed_str());
        let maximum_digest =
            SchemaObject::computed_digest(SchemaObjectKind::Table, maximum_name, maximum_name, "x")
                .unwrap();
        assert!(
            SchemaObject::new(
                SchemaObjectKind::Table,
                maximum_name,
                maximum_name,
                "x",
                maximum_digest,
            )
            .is_ok()
        );
        let excessive_name = Box::leak("a".repeat(MAX_SCHEMA_NAME_UTF8_BYTES + 1).into_boxed_str());
        assert_eq!(
            SchemaObject::computed_digest(
                SchemaObjectKind::Table,
                excessive_name,
                excessive_name,
                "x",
            ),
            Err(SchemaCatalogContractError::InvalidName)
        );
        assert_eq!(
            SchemaObject::computed_digest(
                SchemaObjectKind::Table,
                "alpha__beta",
                "alpha__beta",
                "x",
            ),
            Err(SchemaCatalogContractError::InvalidName)
        );
        for invalid in ["2alpha", "alpha_", "alpha-beta"] {
            assert_eq!(
                SchemaObject::computed_digest(SchemaObjectKind::Table, invalid, invalid, "x"),
                Err(SchemaCatalogContractError::InvalidName)
            );
        }

        let maximum_sql = Box::leak("x".repeat(MAX_SCHEMA_SQL_UTF8_BYTES).into_boxed_str());
        assert!(
            SchemaObject::computed_digest(
                SchemaObjectKind::Table,
                "maximum_sql",
                "maximum_sql",
                maximum_sql,
            )
            .is_ok()
        );
        let excessive_sql = Box::leak("x".repeat(MAX_SCHEMA_SQL_UTF8_BYTES + 1).into_boxed_str());
        assert_eq!(
            SchemaObject::computed_digest(
                SchemaObjectKind::Table,
                "excessive_sql",
                "excessive_sql",
                excessive_sql,
            ),
            Err(SchemaCatalogContractError::InvalidSql)
        );
        assert_eq!(
            SchemaObject::computed_digest(SchemaObjectKind::Table, "nul_sql", "nul_sql", "x\0y",),
            Err(SchemaCatalogContractError::InvalidSql)
        );

        assert_eq!(
            SchemaObject::new(
                SchemaObjectKind::Table,
                "alpha",
                "alpha",
                TABLE_SQL,
                SchemaDigest::from_bytes([0; 32]),
            ),
            Err(SchemaCatalogContractError::ObjectDigestMismatch)
        );
    }

    #[test]
    fn object_sets_reject_duplicates_missing_tables_and_bounds() {
        let duplicate_digest = SchemaVersionCatalog::computed_digest(1, [table(), table()]);
        assert_eq!(
            duplicate_digest,
            Err(SchemaCatalogContractError::DuplicateObject)
        );

        const INDEX_SQL: &str = "CREATE INDEX alpha_idx ON missing(value)";
        let index = SchemaObject::new(
            SchemaObjectKind::Index,
            "alpha_idx",
            "missing",
            INDEX_SQL,
            SchemaObject::computed_digest(
                SchemaObjectKind::Index,
                "alpha_idx",
                "missing",
                INDEX_SQL,
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            SchemaVersionCatalog::computed_digest(1, [index]),
            Err(SchemaCatalogContractError::InvalidBinding)
        );

        let excessive = std::iter::repeat_with(table).take(MAX_SCHEMA_OBJECT_COUNT + 1);
        assert_eq!(
            SchemaVersionCatalog::computed_digest(1, excessive),
            Err(SchemaCatalogContractError::TooManyObjects)
        );
        let infinite = std::iter::repeat_with(table);
        assert_eq!(
            SchemaVersionCatalog::computed_digest(1, infinite),
            Err(SchemaCatalogContractError::TooManyObjects)
        );

        let maximum = (0..(MAX_SCHEMA_OBJECT_COUNT - shared_objects().len()))
            .map(|index| {
                let name = Box::leak(format!("table_{index}").into_boxed_str());
                let sql =
                    Box::leak(format!("CREATE TABLE {name} (value INTEGER)").into_boxed_str());
                SchemaObject::new(
                    SchemaObjectKind::Table,
                    name,
                    name,
                    sql,
                    SchemaObject::computed_digest(SchemaObjectKind::Table, name, name, sql)
                        .unwrap(),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        assert!(SchemaVersionCatalog::computed_digest(1, maximum.iter().cloned()).is_ok());
        let mut excessive = maximum;
        excessive.push(table());
        assert_eq!(
            SchemaVersionCatalog::computed_digest(1, excessive),
            Err(SchemaCatalogContractError::TooManyObjects)
        );
    }

    #[test]
    fn catalog_requires_every_exact_migration_version_and_terminates() {
        let migrations = empty_migrations();
        let digest = SchemaVersionCatalog::computed_digest(1, []).unwrap();
        let v1 = SchemaVersionCatalog::new(1, [], digest).unwrap();
        assert_eq!(
            SchemaVersionCatalog::new(0, [], SchemaDigest::from_bytes([0; 32])),
            Err(SchemaCatalogContractError::InvalidVersionSequence)
        );
        assert_eq!(
            SchemaVersionCatalog::computed_digest(0, []),
            Err(SchemaCatalogContractError::InvalidVersionSequence)
        );
        assert_eq!(
            SchemaVersionCatalog::new(1, [], SchemaDigest::from_bytes([0; 32])),
            Err(SchemaCatalogContractError::SnapshotDigestMismatch)
        );
        assert!(SchemaCatalog::new(&migrations, [v1]).is_ok());
        assert_eq!(
            SchemaCatalog::new(&migrations, []),
            Err(SchemaCatalogContractError::InvalidVersionSequence)
        );
        let infinite = std::iter::repeat(v1);
        assert_eq!(
            SchemaCatalog::new(&migrations, infinite),
            Err(SchemaCatalogContractError::TooManyVersions)
        );

        let descriptors = (0..4096_u32)
            .map(|index| {
                let name = Box::leak(format!("migration_{index}").into_boxed_str());
                crate::MigrationDescriptor::callback(
                    index + 2,
                    name,
                    b"x",
                    MigrationChecksum::for_callback(b"x"),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let maximum_migrations = MigrationCatalog::new(descriptors).unwrap();
        let maximum_versions = (1..=4097_u32)
            .map(|version| {
                let digest = SchemaVersionCatalog::computed_digest(version, []).unwrap();
                SchemaVersionCatalog::new(version, [], digest).unwrap()
            })
            .collect::<Vec<_>>();
        assert!(SchemaCatalog::new(&maximum_migrations, maximum_versions).is_ok());

        let digest = SchemaVersionCatalog::computed_digest(1, []).unwrap();
        let v1 = SchemaVersionCatalog::new(1, [], digest).unwrap();
        let duplicate = SchemaVersionCatalog { version: 1, ..v1 };
        assert_eq!(
            SchemaCatalog::new(&maximum_migrations, [v1, duplicate]),
            Err(SchemaCatalogContractError::InvalidVersionSequence)
        );

        let excessive_objects = std::iter::repeat_with(table).take(MAX_SCHEMA_OBJECT_COUNT + 1);
        assert_eq!(
            SchemaVersionCatalog::new(1, excessive_objects, SchemaDigest::from_bytes([0; 32])),
            Err(SchemaCatalogContractError::TooManyObjects)
        );

        let gap_v2 = SchemaVersionCatalog { version: 2, ..v1 };
        assert_eq!(
            SchemaCatalog::new(&migrations, [gap_v2]),
            Err(SchemaCatalogContractError::InvalidVersionSequence)
        );

        let v1_for_exact = v1;
        let exact = SchemaCatalog::new(&migrations, [v1_for_exact]).expect("exact catalog");
        assert!(exact.matches_migrations(&migrations));
        assert_eq!(exact.version(0), None);
        assert_eq!(exact.version(1), Some(v1));
        assert_eq!(exact.version(2), None);

        let callback = crate::MigrationDescriptor::callback(
            2,
            "next_schema",
            b"definition",
            MigrationChecksum::for_callback(b"definition"),
        )
        .expect("migration");
        let other_migrations = MigrationCatalog::new([callback]).expect("other migrations");
        assert!(!exact.matches_migrations(&other_migrations));
    }
}
