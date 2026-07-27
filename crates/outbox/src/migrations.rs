#![forbid(unsafe_code)]

use crate::RadrootsOutboxError;
pub(crate) use crate::generated::outbox_migration_registry::OUTBOX_MIGRATIONS;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub(crate) const OUTBOX_LEDGER_NAME: &str = "radroots_outbox_schema_migrations";
pub(crate) const OUTBOX_RESERVED_PREFIX: &str = "outbox_";

/// Oldest managed schema version that the runtime can preserve or target.
pub const RADROOTS_OUTBOX_SCHEMA_VERSION_MIN: u32 = OUTBOX_MIGRATIONS[0].version;
/// Latest managed schema version understood by this runtime.
pub const RADROOTS_OUTBOX_SCHEMA_VERSION_CURRENT: u32 =
    OUTBOX_MIGRATIONS[OUTBOX_MIGRATIONS.len() - 1].version;

pub(crate) const OUTBOX_LEDGER_DDL: &str = "CREATE TABLE radroots_outbox_schema_migrations (
  version INTEGER PRIMARY KEY NOT NULL CHECK (version > 0),
  name TEXT NOT NULL UNIQUE CHECK (length(name) > 0),
  up_sha256 TEXT NOT NULL CHECK (length(up_sha256) = 64 AND up_sha256 NOT GLOB '*[^0-9a-f]*'),
  down_sha256 TEXT NOT NULL CHECK (length(down_sha256) = 64 AND down_sha256 NOT GLOB '*[^0-9a-f]*'),
  schema_sha256 TEXT NOT NULL CHECK (length(schema_sha256) = 64 AND schema_sha256 NOT GLOB '*[^0-9a-f]*')
) STRICT, WITHOUT ROWID";

pub(crate) const OUTBOX_LEDGER_CREATE_DDL: &str =
    "CREATE TABLE main.radroots_outbox_schema_migrations (
  version INTEGER PRIMARY KEY NOT NULL CHECK (version > 0),
  name TEXT NOT NULL UNIQUE CHECK (length(name) > 0),
  up_sha256 TEXT NOT NULL CHECK (length(up_sha256) = 64 AND up_sha256 NOT GLOB '*[^0-9a-f]*'),
  down_sha256 TEXT NOT NULL CHECK (length(down_sha256) = 64 AND down_sha256 NOT GLOB '*[^0-9a-f]*'),
  schema_sha256 TEXT NOT NULL CHECK (length(schema_sha256) = 64 AND schema_sha256 NOT GLOB '*[^0-9a-f]*')
) STRICT, WITHOUT ROWID";

#[derive(Clone, Copy)]
pub(crate) struct OutboxMigration {
    pub(crate) version: u32,
    pub(crate) name: &'static str,
    pub(crate) up_sql: &'static str,
    pub(crate) down_sql: &'static str,
    pub(crate) up_len: usize,
    pub(crate) down_len: usize,
    pub(crate) up_sha256: &'static str,
    pub(crate) down_sha256: &'static str,
    pub(crate) schema_sha256: &'static str,
    pub(crate) owned_object_names: &'static [&'static str],
    pub(crate) owned_table_names: &'static [&'static str],
}

pub(crate) fn migration_for_version(
    registry: &[OutboxMigration],
    version: u32,
) -> Option<&OutboxMigration> {
    registry
        .iter()
        .find(|migration| migration.version == version)
}

#[cfg(test)]
pub(crate) fn is_outbox_owned_table_name(registry: &[OutboxMigration], name: &str) -> bool {
    sqlite_identifier_starts_with(name, OUTBOX_RESERVED_PREFIX)
        || registry
            .iter()
            .flat_map(|migration| migration.owned_table_names)
            .any(|owned| name.eq_ignore_ascii_case(owned))
}

pub(crate) fn is_outbox_governed_schema_name(registry: &[OutboxMigration], name: &str) -> bool {
    name.eq_ignore_ascii_case(OUTBOX_LEDGER_NAME)
        || sqlite_identifier_starts_with(name, OUTBOX_RESERVED_PREFIX)
        || registry
            .iter()
            .flat_map(|migration| migration.owned_object_names)
            .any(|owned| name.eq_ignore_ascii_case(owned))
}

pub(crate) fn sqlite_identifier_starts_with(name: &str, prefix: &str) -> bool {
    name.get(..prefix.len())
        .is_some_and(|candidate| candidate.eq_ignore_ascii_case(prefix))
}

pub(crate) fn validate_embedded_migration_registry() -> Result<(), RadrootsOutboxError> {
    validate_migration_registry(
        OUTBOX_MIGRATIONS,
        RADROOTS_OUTBOX_SCHEMA_VERSION_MIN,
        RADROOTS_OUTBOX_SCHEMA_VERSION_CURRENT,
    )
}

pub(crate) fn validate_migration_registry(
    registry: &[OutboxMigration],
    minimum: u32,
    current: u32,
) -> Result<(), RadrootsOutboxError> {
    validate_ledger_ddl_identity(OUTBOX_LEDGER_DDL, OUTBOX_LEDGER_CREATE_DDL)?;
    if minimum == 0 || current < minimum || registry.is_empty() {
        return Err(RadrootsOutboxError::MigrationRegistryDefect {
            reason: format!(
                "migration version range {minimum}..={current} requires a non-empty positive registry"
            ),
        });
    }
    let mut expected_version = minimum;
    let mut object_names = BTreeSet::new();
    let mut table_names = BTreeSet::new();
    for (index, migration) in registry.iter().enumerate() {
        if migration.version != expected_version {
            return Err(RadrootsOutboxError::MigrationRegistryDefect {
                reason: format!(
                    "expected migration version {expected_version}, found {}",
                    migration.version
                ),
            });
        }
        if migration.name.is_empty()
            || registry[..index]
                .iter()
                .any(|prior| prior.name == migration.name)
        {
            return Err(RadrootsOutboxError::MigrationRegistryDefect {
                reason: format!(
                    "migration version {} has an invalid or duplicate name",
                    migration.version
                ),
            });
        }
        if migration.owned_object_names.is_empty() || migration.owned_table_names.is_empty() {
            return Err(RadrootsOutboxError::MigrationRegistryDefect {
                reason: format!(
                    "migration version {} must own schema objects and tables",
                    migration.version
                ),
            });
        }
        for name in migration.owned_object_names {
            validate_owned_schema_name(migration.version, "object", name)?;
            if !object_names.insert(*name) {
                return Err(RadrootsOutboxError::MigrationRegistryDefect {
                    reason: format!("owned schema object `{name}` is declared more than once"),
                });
            }
        }
        for name in migration.owned_table_names {
            validate_owned_schema_name(migration.version, "table", name)?;
            if !migration.owned_object_names.contains(name) || !table_names.insert(*name) {
                return Err(RadrootsOutboxError::MigrationRegistryDefect {
                    reason: format!(
                        "owned table `{name}` is missing from the object inventory or is duplicated"
                    ),
                });
            }
        }
        validate_embedded_migration_input(
            migration.version,
            "up",
            migration.up_sql,
            migration.up_len,
            migration.up_sha256,
        )?;
        validate_embedded_migration_input(
            migration.version,
            "down",
            migration.down_sql,
            migration.down_len,
            migration.down_sha256,
        )?;
        validate_sha256_literal(migration.version, "schema", migration.schema_sha256)?;
        expected_version = expected_version.checked_add(1).ok_or_else(|| {
            RadrootsOutboxError::MigrationRegistryDefect {
                reason: "migration version overflow".to_owned(),
            }
        })?;
    }
    if expected_version - 1 != current {
        return Err(RadrootsOutboxError::MigrationRegistryDefect {
            reason: format!(
                "migration registry ends at {}, expected {current}",
                expected_version - 1
            ),
        });
    }
    Ok(())
}

fn validate_ledger_ddl_identity(
    catalog_ddl: &str,
    create_ddl: &str,
) -> Result<(), RadrootsOutboxError> {
    let catalog_ddl = catalog_ddl.strip_prefix("CREATE TABLE ");
    let create_ddl = create_ddl.strip_prefix("CREATE TABLE main.");
    if catalog_ddl.is_none() || create_ddl.is_none() || create_ddl != catalog_ddl {
        return Err(RadrootsOutboxError::MigrationRegistryDefect {
            reason: "main-qualified ledger creation DDL does not match canonical catalog DDL"
                .to_owned(),
        });
    }
    Ok(())
}

fn validate_owned_schema_name(
    version: u32,
    object_kind: &'static str,
    name: &str,
) -> Result<(), RadrootsOutboxError> {
    if name.is_empty()
        || name == OUTBOX_LEDGER_NAME
        || !sqlite_identifier_starts_with(name, OUTBOX_RESERVED_PREFIX)
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(RadrootsOutboxError::MigrationRegistryDefect {
            reason: format!(
                "migration version {version} has invalid owned {object_kind} name `{name}`"
            ),
        });
    }
    Ok(())
}

fn validate_embedded_migration_input(
    version: u32,
    direction: &'static str,
    sql: &str,
    expected_len: usize,
    expected_sha256: &'static str,
) -> Result<(), RadrootsOutboxError> {
    if sql.len() != expected_len {
        return Err(RadrootsOutboxError::EmbeddedMigrationLengthMismatch {
            version,
            direction,
            expected: expected_len,
            actual: sql.len(),
        });
    }
    validate_sha256_literal(version, direction, expected_sha256)?;
    let actual = sha256_hex(sql.as_bytes());
    if actual != expected_sha256 {
        return Err(RadrootsOutboxError::EmbeddedMigrationChecksumMismatch {
            version,
            direction,
            expected: expected_sha256,
            actual,
        });
    }
    Ok(())
}

fn validate_sha256_literal(
    version: u32,
    field: &'static str,
    value: &str,
) -> Result<(), RadrootsOutboxError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(RadrootsOutboxError::MigrationRegistryDefect {
            reason: format!("migration version {version} has an invalid {field} SHA-256 literal"),
        });
    }
    Ok(())
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};

    #[derive(Debug, PartialEq, Eq)]
    struct DiscoveredMigration {
        version: u32,
        name: String,
        up: PathBuf,
        down: PathBuf,
    }

    fn discover(root: &Path) -> Result<Vec<DiscoveredMigration>, String> {
        let directory = root.join("migrations");
        let mut entries = fs::read_dir(&directory)
            .map_err(|error| format!("read {}: {error}", directory.display()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("read migration entry: {error}"))?;
        entries.sort_by_key(|entry| entry.file_name());
        let mut pairs =
            std::collections::BTreeMap::<(u32, String), (Option<PathBuf>, Option<PathBuf>)>::new();
        for entry in entries {
            let file_type = entry.file_type().map_err(|error| error.to_string())?;
            if !file_type.is_file() || file_type.is_symlink() {
                return Err(format!(
                    "migration input must be a regular file: {}",
                    entry.path().display()
                ));
            }
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| "migration filename is not UTF-8".to_owned())?;
            let (stem, direction) = if let Some(stem) = name.strip_suffix(".up.sql") {
                (stem, "up")
            } else if let Some(stem) = name.strip_suffix(".down.sql") {
                (stem, "down")
            } else {
                return Err(format!("unknown migration file `{name}`"));
            };
            let (version, migration_name) = stem
                .split_once('_')
                .ok_or_else(|| format!("invalid migration filename `{name}`"))?;
            if version.len() != 4
                || !version.bytes().all(|byte| byte.is_ascii_digit())
                || migration_name.is_empty()
                || !migration_name
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
            {
                return Err(format!("invalid migration filename `{name}`"));
            }
            let version = version.parse::<u32>().map_err(|error| error.to_string())?;
            let pair = pairs
                .entry((version, migration_name.to_owned()))
                .or_default();
            let slot = if direction == "up" {
                &mut pair.0
            } else {
                &mut pair.1
            };
            if slot.replace(entry.path()).is_some() {
                return Err(format!(
                    "duplicate {direction} migration for version {version}"
                ));
            }
        }
        pairs
            .into_iter()
            .map(|((version, name), (up, down))| {
                Ok(DiscoveredMigration {
                    version,
                    name,
                    up: up.ok_or_else(|| format!("migration {version} is missing up SQL"))?,
                    down: down.ok_or_else(|| format!("migration {version} is missing down SQL"))?,
                })
            })
            .collect()
    }

    #[test]
    fn migration_source_discovery_is_exact_and_fail_closed() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let discovered = discover(root).expect("canonical migration discovery");
        assert_eq!(discovered.len(), OUTBOX_MIGRATIONS.len());
        for (discovered, embedded) in discovered.iter().zip(OUTBOX_MIGRATIONS) {
            assert_eq!(discovered.version, embedded.version);
            assert_eq!(discovered.name, embedded.name);
            assert_eq!(
                fs::read(&discovered.up).expect("up bytes"),
                embedded.up_sql.as_bytes()
            );
            assert_eq!(
                fs::read(&discovered.down).expect("down bytes"),
                embedded.down_sql.as_bytes()
            );
        }

        let temp = tempfile::tempdir().expect("tempdir");
        fs::create_dir(temp.path().join("migrations")).expect("migrations");
        for (name, bytes) in [
            ("0001_outbox.up.sql", OUTBOX_MIGRATIONS[0].up_sql.as_bytes()),
            (
                "0001_outbox.down.sql",
                OUTBOX_MIGRATIONS[0].down_sql.as_bytes(),
            ),
        ] {
            fs::write(temp.path().join("migrations").join(name), bytes).expect("fixture");
        }
        fs::write(
            temp.path().join("migrations/0002_unknown.txt"),
            b"SELECT 1;",
        )
        .expect("unknown");
        assert!(
            discover(temp.path())
                .expect_err("unknown file")
                .contains("unknown migration file")
        );
        fs::remove_file(temp.path().join("migrations/0002_unknown.txt")).expect("remove");
        fs::remove_file(temp.path().join("migrations/0001_outbox.down.sql")).expect("remove down");
        assert!(
            discover(temp.path())
                .expect_err("missing pair")
                .contains("missing down SQL")
        );
    }

    #[test]
    fn embedded_registry_and_frozen_baseline_are_exact() {
        validate_embedded_migration_registry().expect("registry");
        assert_eq!(OUTBOX_MIGRATIONS[0].up_len, 5_470);
        assert_eq!(OUTBOX_MIGRATIONS[0].down_len, 159);
        assert_eq!(
            OUTBOX_MIGRATIONS[0].up_sha256,
            "a7ee775d32c2b9f845961425362e1b1e558ce0d025f7d22dd58f118ba4dab4fa"
        );
        assert_eq!(
            OUTBOX_MIGRATIONS[0].down_sha256,
            "5d56f978f9172dc5ecbc5043a6c286c75926974d8a2a9e44fffa7c134829af61"
        );
        assert_eq!(OUTBOX_MIGRATIONS[0].owned_object_names.len(), 13);
        assert_eq!(OUTBOX_MIGRATIONS[0].owned_table_names.len(), 5);
    }

    fn assert_registry_defect(result: Result<(), RadrootsOutboxError>) -> String {
        match result.expect_err("registry defect") {
            RadrootsOutboxError::MigrationRegistryDefect { reason } => reason,
            other => panic!("unexpected error: {other}"),
        }
    }

    fn future_migration() -> OutboxMigration {
        let mut migration = OUTBOX_MIGRATIONS[0];
        migration.version = 2;
        migration.name = "future";
        migration.owned_object_names = &["outbox_future"];
        migration.owned_table_names = &["outbox_future"];
        migration
    }

    #[test]
    fn namespace_predicates_cover_reserved_ledger_registry_and_unrelated_names() {
        let mut legacy = OUTBOX_MIGRATIONS[0];
        legacy.owned_object_names = &["legacy_object"];
        legacy.owned_table_names = &["legacy_table"];
        let registry = [legacy];

        assert!(migration_for_version(OUTBOX_MIGRATIONS, 1).is_some());
        assert!(migration_for_version(OUTBOX_MIGRATIONS, 2).is_none());
        assert!(sqlite_identifier_starts_with("OUTBOX_EVENT", "outbox_"));
        assert!(!sqlite_identifier_starts_with("short", "outbox_"));
        assert!(is_outbox_owned_table_name(&registry, "outbox_new"));
        assert!(is_outbox_owned_table_name(&registry, "LEGACY_TABLE"));
        assert!(!is_outbox_owned_table_name(&registry, "caller_table"));
        assert!(is_outbox_governed_schema_name(
            &registry,
            "RADROOTS_OUTBOX_SCHEMA_MIGRATIONS"
        ));
        assert!(is_outbox_governed_schema_name(&registry, "outbox_new"));
        assert!(is_outbox_governed_schema_name(&registry, "LEGACY_OBJECT"));
        assert!(!is_outbox_governed_schema_name(&registry, "caller_object"));
    }

    #[test]
    fn ledger_identifiers_fail_closed() {
        validate_ledger_ddl_identity(OUTBOX_LEDGER_DDL, OUTBOX_LEDGER_CREATE_DDL)
            .expect("ledger DDL");
        for (catalog, create) in [
            (OUTBOX_LEDGER_DDL, "CREATE TABLE main.counterfeit"),
            ("counterfeit", OUTBOX_LEDGER_CREATE_DDL),
            (OUTBOX_LEDGER_DDL, "counterfeit"),
            ("counterfeit", "counterfeit"),
        ] {
            assert_registry_defect(validate_ledger_ddl_identity(catalog, create));
        }
    }

    #[test]
    fn registry_shape_validation_rejects_every_structural_defect() {
        assert_registry_defect(validate_migration_registry(OUTBOX_MIGRATIONS, 0, 1));
        assert_registry_defect(validate_migration_registry(OUTBOX_MIGRATIONS, 2, 1));
        assert_registry_defect(validate_migration_registry(&[], 1, 1));

        let mut migration = OUTBOX_MIGRATIONS[0];
        migration.version = 2;
        assert_registry_defect(validate_migration_registry(&[migration], 1, 1));

        let mut migration = OUTBOX_MIGRATIONS[0];
        migration.name = "";
        assert_registry_defect(validate_migration_registry(&[migration], 1, 1));

        let mut duplicate_name = future_migration();
        duplicate_name.name = OUTBOX_MIGRATIONS[0].name;
        assert_registry_defect(validate_migration_registry(
            &[OUTBOX_MIGRATIONS[0], duplicate_name],
            1,
            2,
        ));

        let mut migration = OUTBOX_MIGRATIONS[0];
        migration.owned_object_names = &[];
        assert_registry_defect(validate_migration_registry(&[migration], 1, 1));
        let mut migration = OUTBOX_MIGRATIONS[0];
        migration.owned_table_names = &[];
        assert_registry_defect(validate_migration_registry(&[migration], 1, 1));

        for invalid_name in ["", OUTBOX_LEDGER_NAME, "caller_object", "outbox_UPPER"] {
            assert_registry_defect(validate_owned_schema_name(1, "object", invalid_name));
        }
        validate_owned_schema_name(1, "object", "outbox_123").expect("numeric identifier");
        let mut migration = OUTBOX_MIGRATIONS[0];
        migration.owned_object_names = &["outbox_valid"];
        migration.owned_table_names = &["outbox_UPPER"];
        assert_registry_defect(validate_migration_registry(&[migration], 1, 1));

        let mut duplicate_object = future_migration();
        duplicate_object.owned_object_names = &["outbox_event"];
        duplicate_object.owned_table_names = &["outbox_event"];
        assert_registry_defect(validate_migration_registry(
            &[OUTBOX_MIGRATIONS[0], duplicate_object],
            1,
            2,
        ));

        let mut missing_table = future_migration();
        missing_table.owned_object_names = &["outbox_future_index"];
        assert_registry_defect(validate_migration_registry(
            &[OUTBOX_MIGRATIONS[0], missing_table],
            1,
            2,
        ));

        let mut duplicate_table = future_migration();
        duplicate_table.owned_table_names = &["outbox_future", "outbox_future"];
        assert_registry_defect(validate_migration_registry(
            &[OUTBOX_MIGRATIONS[0], duplicate_table],
            1,
            2,
        ));

        assert_registry_defect(validate_migration_registry(OUTBOX_MIGRATIONS, 1, 2));
        validate_migration_registry(&[OUTBOX_MIGRATIONS[0], future_migration()], 1, 2)
            .expect("contiguous synthetic registry");

        let mut overflow = OUTBOX_MIGRATIONS[0];
        overflow.version = u32::MAX;
        validate_migration_registry(&[overflow], u32::MAX, u32::MAX).expect_err("version overflow");
    }

    #[test]
    fn registry_checksum_validation_rejects_lengths_literals_and_bytes() {
        let mut migration = OUTBOX_MIGRATIONS[0];
        migration.up_len += 1;
        assert!(matches!(
            validate_migration_registry(&[migration], 1, 1),
            Err(RadrootsOutboxError::EmbeddedMigrationLengthMismatch {
                direction: "up",
                ..
            })
        ));

        let mut migration = OUTBOX_MIGRATIONS[0];
        migration.down_len += 1;
        assert!(matches!(
            validate_migration_registry(&[migration], 1, 1),
            Err(RadrootsOutboxError::EmbeddedMigrationLengthMismatch {
                direction: "down",
                ..
            })
        ));

        for invalid_sha in [
            "short",
            "gggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg",
        ] {
            let mut migration = OUTBOX_MIGRATIONS[0];
            migration.up_sha256 = invalid_sha;
            assert_registry_defect(validate_migration_registry(&[migration], 1, 1));
        }

        let mut migration = OUTBOX_MIGRATIONS[0];
        migration.up_sha256 = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        assert!(matches!(
            validate_migration_registry(&[migration], 1, 1),
            Err(RadrootsOutboxError::EmbeddedMigrationChecksumMismatch {
                direction: "up",
                ..
            })
        ));

        let mut migration = OUTBOX_MIGRATIONS[0];
        migration.schema_sha256 = "short";
        assert_registry_defect(validate_migration_registry(&[migration], 1, 1));
    }
}
