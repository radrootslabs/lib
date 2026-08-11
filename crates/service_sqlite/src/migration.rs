//! Deterministic, execution-free migration identity contracts.

use core::fmt;
use std::{collections::BTreeSet, error::Error};

use sha2::{Digest, Sha256};

const MIGRATION_CONTENT_DOMAIN: &[u8] = b"radroots.service_sqlite.migration_content.v1\0";
const MIGRATION_CATALOG_DOMAIN: &[u8] = b"radroots.service_sqlite.migration_catalog.v1\0";
const BASE_SCHEMA_VERSION: u32 = 1;
const MAX_MIGRATION_NAME_UTF8_BYTES: usize = 128;
const MAX_MIGRATION_CONTENT_BYTES: usize = 1024 * 1024;
const MAX_MIGRATION_COUNT: usize = 4096;

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

#[cfg(test)]
mod tests {
    use super::*;

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
