use crate::RadrootsEventStoreError;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub(crate) const EVENT_STORE_LEDGER_NAME: &str = "radroots_event_store_schema_migrations";
pub(crate) const EVENT_STORE_RESERVED_PREFIX: &str = "radroots_event_store_";

pub const RADROOTS_EVENT_STORE_SCHEMA_VERSION_MIN: u32 = 1;
pub const RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT: u32 = 1;

pub(crate) const EVENT_STORE_LEDGER_DDL: &str = "CREATE TABLE radroots_event_store_schema_migrations (
  version INTEGER PRIMARY KEY NOT NULL CHECK (version > 0),
  name TEXT NOT NULL UNIQUE CHECK (length(name) > 0),
  up_sha256 TEXT NOT NULL CHECK (length(up_sha256) = 64 AND up_sha256 NOT GLOB '*[^0-9a-f]*'),
  down_sha256 TEXT NOT NULL CHECK (length(down_sha256) = 64 AND down_sha256 NOT GLOB '*[^0-9a-f]*'),
  schema_sha256 TEXT NOT NULL CHECK (length(schema_sha256) = 64 AND schema_sha256 NOT GLOB '*[^0-9a-f]*')
) STRICT, WITHOUT ROWID";

pub(crate) const EVENT_STORE_BASELINE_OBJECT_NAMES: &[&str] = &[
    "event_envelope_contract_idx",
    "event_envelope_head",
    "event_envelope_head_addressable_idx",
    "event_envelope_head_event_idx",
    "event_envelope_head_replaceable_idx",
    "event_envelope_kind_created_idx",
    "event_envelope_projection_idx",
    "event_envelope_tag_lookup_idx",
    "event_envelope_tag_relay_idx",
    "event_envelope_tags",
    "event_envelope_verification_contract_idx",
    "event_envelopes",
    "event_transport_observation",
    "event_transport_observation_endpoint_idx",
    "listing_projection",
    "listing_projection_geohash_idx",
    "listing_projection_seller_idx",
    "listing_search_fts",
    "listing_search_fts_config",
    "listing_search_fts_content",
    "listing_search_fts_data",
    "listing_search_fts_docsize",
    "listing_search_fts_idx",
    "projection_cursor",
    "seller_inventory_reservation",
    "seller_inventory_reservation_authority_idx",
    "seller_inventory_reservation_line",
    "seller_inventory_reservation_line_bin_idx",
    "seller_inventory_reservation_trade_idx",
    "trade_missing_parent",
    "trade_missing_parent_lookup_idx",
    "trade_mutation",
    "trade_mutation_actor_idx",
    "trade_mutation_candidate_idx",
    "trade_mutation_parent",
    "trade_mutation_parent_lookup_idx",
    "trade_mutation_trade_idx",
    "trade_projection_checkpoint",
    "trade_projection_checkpoint_actor_idx",
    "trade_projection_checkpoint_agreement_idx",
    "trade_projection_quarantine",
    "trade_projection_quarantine_mutation_idx",
    "trade_projection_quarantine_trade_idx",
    "trade_transport_envelope",
    "trade_transport_envelope_mutation_idx",
    "trade_transport_envelope_trade_idx",
];

pub(crate) const EVENT_STORE_BASELINE_TABLE_NAMES: &[&str] = &[
    "event_envelope_head",
    "event_envelope_tags",
    "event_envelopes",
    "event_transport_observation",
    "listing_projection",
    "listing_search_fts",
    "listing_search_fts_config",
    "listing_search_fts_content",
    "listing_search_fts_data",
    "listing_search_fts_docsize",
    "listing_search_fts_idx",
    "projection_cursor",
    "seller_inventory_reservation",
    "seller_inventory_reservation_line",
    "trade_missing_parent",
    "trade_mutation",
    "trade_mutation_parent",
    "trade_projection_checkpoint",
    "trade_projection_quarantine",
    "trade_transport_envelope",
];

pub(crate) const EVENT_STORE_BASELINE_FTS5_TABLE_NAMES: &[&str] = &["listing_search_fts"];

#[derive(Clone, Copy)]
pub(crate) struct EventStoreMigration {
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
    pub(crate) fts5_table_names: &'static [&'static str],
}

pub(crate) const EVENT_STORE_MIGRATIONS: &[EventStoreMigration] = &[EventStoreMigration {
    version: 1,
    name: "event_store",
    up_sql: include_str!("../migrations/0001_event_store.up.sql"),
    down_sql: include_str!("../migrations/0001_event_store.down.sql"),
    up_len: 10_712,
    down_len: 522,
    up_sha256: "4c03906a1cffd418a48d40907aa9a1ca51bb41766cff7250c4dfc7c2fd6eddde",
    down_sha256: "fa84d587f657f601947eaeb9cd239c962a48f6fcdce723588476e8d22f3c1f53",
    schema_sha256: "5b1f92779640f1a2dbd75e37a96996bda6c8be58883190f69eb3eced22a48f03",
    owned_object_names: EVENT_STORE_BASELINE_OBJECT_NAMES,
    owned_table_names: EVENT_STORE_BASELINE_TABLE_NAMES,
    fts5_table_names: EVENT_STORE_BASELINE_FTS5_TABLE_NAMES,
}];

pub(crate) fn migration_for_version(
    registry: &[EventStoreMigration],
    version: u32,
) -> Option<&EventStoreMigration> {
    registry
        .iter()
        .find(|migration| migration.version == version)
}

pub(crate) fn validate_embedded_migration_registry() -> Result<(), RadrootsEventStoreError> {
    validate_migration_registry(
        EVENT_STORE_MIGRATIONS,
        RADROOTS_EVENT_STORE_SCHEMA_VERSION_MIN,
        RADROOTS_EVENT_STORE_SCHEMA_VERSION_CURRENT,
    )
}

pub(crate) fn validate_migration_registry(
    registry: &[EventStoreMigration],
    minimum: u32,
    current: u32,
) -> Result<(), RadrootsEventStoreError> {
    if minimum == 0 || current < minimum || registry.is_empty() {
        return Err(RadrootsEventStoreError::MigrationRegistryDefect {
            reason: format!(
                "migration version range {minimum}..={current} requires a non-empty positive registry"
            ),
        });
    }

    let mut expected_version = minimum;
    let mut owned_object_names = BTreeSet::new();
    let mut owned_table_names = BTreeSet::new();
    for (index, migration) in registry.iter().enumerate() {
        if migration.version != expected_version {
            return Err(RadrootsEventStoreError::MigrationRegistryDefect {
                reason: format!(
                    "expected migration version {expected_version}, found {}",
                    migration.version
                ),
            });
        }
        if migration.name.is_empty() {
            return Err(RadrootsEventStoreError::MigrationRegistryDefect {
                reason: format!("migration version {} has an empty name", migration.version),
            });
        }
        if registry[..index]
            .iter()
            .any(|prior| prior.name == migration.name)
        {
            return Err(RadrootsEventStoreError::MigrationRegistryDefect {
                reason: format!("migration name `{}` is duplicated", migration.name),
            });
        }
        if migration.owned_object_names.is_empty() {
            return Err(RadrootsEventStoreError::MigrationRegistryDefect {
                reason: format!(
                    "migration version {} declares no owned schema objects",
                    migration.version
                ),
            });
        }
        for object_name in migration.owned_object_names {
            validate_owned_schema_name(migration.version, "object", object_name)?;
            if index > 0 && !object_name.starts_with(EVENT_STORE_RESERVED_PREFIX) {
                return Err(RadrootsEventStoreError::MigrationRegistryDefect {
                    reason: format!(
                        "migration version {} object `{object_name}` is outside the reserved `{EVENT_STORE_RESERVED_PREFIX}` namespace",
                        migration.version
                    ),
                });
            }
            if !owned_object_names.insert(*object_name) {
                return Err(RadrootsEventStoreError::MigrationRegistryDefect {
                    reason: format!(
                        "owned schema object `{object_name}` is declared more than once"
                    ),
                });
            }
        }
        for table_name in migration.owned_table_names {
            validate_owned_schema_name(migration.version, "table", table_name)?;
            if !migration.owned_object_names.contains(table_name) {
                return Err(RadrootsEventStoreError::MigrationRegistryDefect {
                    reason: format!(
                        "migration version {} owned table `{table_name}` is not also an owned object",
                        migration.version
                    ),
                });
            }
            if !owned_table_names.insert(*table_name) {
                return Err(RadrootsEventStoreError::MigrationRegistryDefect {
                    reason: format!("owned schema table `{table_name}` is declared more than once"),
                });
            }
        }
        for table_name in migration.fts5_table_names {
            if !migration.owned_table_names.contains(table_name) {
                return Err(RadrootsEventStoreError::MigrationRegistryDefect {
                    reason: format!(
                        "migration version {} FTS5 table `{table_name}` is not also an owned table",
                        migration.version
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
            RadrootsEventStoreError::MigrationRegistryDefect {
                reason: "migration version overflow".to_owned(),
            }
        })?;
    }
    if expected_version - 1 != current {
        return Err(RadrootsEventStoreError::MigrationRegistryDefect {
            reason: format!(
                "registry ends at version {}, declared current version is {}",
                expected_version - 1,
                current
            ),
        });
    }
    Ok(())
}

fn validate_owned_schema_name(
    version: u32,
    object_kind: &'static str,
    name: &str,
) -> Result<(), RadrootsEventStoreError> {
    if name.is_empty()
        || name == EVENT_STORE_LEDGER_NAME
        || name.to_ascii_lowercase().starts_with("sqlite_")
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(RadrootsEventStoreError::MigrationRegistryDefect {
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
) -> Result<(), RadrootsEventStoreError> {
    if sql.len() != expected_len {
        return Err(RadrootsEventStoreError::EmbeddedMigrationLengthMismatch {
            version,
            direction,
            expected: expected_len,
            actual: sql.len(),
        });
    }
    validate_sha256_literal(version, direction, expected_sha256)?;
    let actual = sha256_hex(sql.as_bytes());
    if actual != expected_sha256 {
        return Err(RadrootsEventStoreError::EmbeddedMigrationChecksumMismatch {
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
) -> Result<(), RadrootsEventStoreError> {
    if value.len() != 64
        || !value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(RadrootsEventStoreError::MigrationRegistryDefect {
            reason: format!("migration version {version} has an invalid {field} SHA-256 literal"),
        });
    }
    Ok(())
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod migration_framework {
    use super::*;
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};

    #[derive(Debug, PartialEq, Eq)]
    struct DiscoveredMigration {
        version: u32,
        name: String,
        up: PathBuf,
        down: PathBuf,
    }

    const FROZEN_V1_UP_LEN: usize = 10_712;
    const FROZEN_V1_DOWN_LEN: usize = 522;
    const FROZEN_V1_UP_SHA256: &str =
        "4c03906a1cffd418a48d40907aa9a1ca51bb41766cff7250c4dfc7c2fd6eddde";
    const FROZEN_V1_DOWN_SHA256: &str =
        "fa84d587f657f601947eaeb9cd239c962a48f6fcdce723588476e8d22f3c1f53";
    const FROZEN_V1_SCHEMA_SHA256: &str =
        "5b1f92779640f1a2dbd75e37a96996bda6c8be58883190f69eb3eced22a48f03";
    const FROZEN_V1_OBJECT_COUNT: usize = 46;

    fn discover_migration_directory(directory: &Path) -> Result<Vec<DiscoveredMigration>, String> {
        let directory_metadata = fs::symlink_metadata(directory).map_err(|error| {
            format!(
                "cannot inspect migration directory {}: {error}",
                directory.display()
            )
        })?;
        if directory_metadata.file_type().is_symlink() {
            return Err(format!(
                "migration directory must not be a symlink: {}",
                directory.display()
            ));
        }
        if !directory_metadata.is_dir() {
            return Err(format!(
                "migration source is not a directory: {}",
                directory.display()
            ));
        }
        let canonical_directory = fs::canonicalize(directory).map_err(|error| {
            format!(
                "cannot canonicalize migration directory {}: {error}",
                directory.display()
            )
        })?;
        let mut paths = Vec::new();
        for entry in fs::read_dir(directory)
            .map_err(|error| format!("cannot read migration directory: {error}"))?
        {
            let entry = entry.map_err(|error| format!("cannot read migration entry: {error}"))?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path).map_err(|error| {
                format!(
                    "cannot inspect migration source {}: {error}",
                    path.display()
                )
            })?;
            if metadata.file_type().is_symlink() {
                return Err(format!(
                    "migration source must not be a symlink: {}",
                    path.display()
                ));
            }
            if !metadata.is_file() {
                return Err(format!(
                    "migration source is not a regular file: {}",
                    path.display()
                ));
            }
            let canonical_path = fs::canonicalize(&path).map_err(|error| {
                format!(
                    "cannot canonicalize migration source {}: {error}",
                    path.display()
                )
            })?;
            if canonical_path.parent() != Some(canonical_directory.as_path()) {
                return Err(format!(
                    "migration source escapes its directory: {}",
                    path.display()
                ));
            }
            paths.push(path);
        }
        discover_migration_files(paths)
    }

    fn discover_migration_files(
        paths: impl IntoIterator<Item = PathBuf>,
    ) -> Result<Vec<DiscoveredMigration>, String> {
        let mut discovered = BTreeMap::<(u32, String), (Option<PathBuf>, Option<PathBuf>)>::new();
        for path in paths {
            let filename = path
                .file_name()
                .and_then(|value| value.to_str())
                .ok_or_else(|| format!("migration path is not valid UTF-8: {}", path.display()))?;
            let (stem, direction) = if let Some(stem) = filename.strip_suffix(".up.sql") {
                (stem, "up")
            } else if let Some(stem) = filename.strip_suffix(".down.sql") {
                (stem, "down")
            } else {
                return Err(format!("unsupported migration filename `{filename}`"));
            };
            let (version, name) = stem
                .split_once('_')
                .ok_or_else(|| format!("migration filename `{filename}` has no name"))?;
            if version.len() != 4 || !version.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(format!(
                    "migration filename `{filename}` has an invalid version"
                ));
            }
            if name.is_empty()
                || !name
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
            {
                return Err(format!(
                    "migration filename `{filename}` has an invalid name"
                ));
            }
            let version = version
                .parse::<u32>()
                .map_err(|error| format!("invalid migration version: {error}"))?;
            let pair = discovered.entry((version, name.to_owned())).or_default();
            let slot = if direction == "up" {
                &mut pair.0
            } else {
                &mut pair.1
            };
            if slot.replace(path).is_some() {
                return Err(format!(
                    "duplicate {direction} migration for version {version}"
                ));
            }
        }

        let mut migrations = Vec::with_capacity(discovered.len());
        for (expected_version, ((version, name), (up, down))) in
            (RADROOTS_EVENT_STORE_SCHEMA_VERSION_MIN..).zip(discovered)
        {
            if version != expected_version {
                return Err(format!(
                    "migration version gap: expected {expected_version}, found {version}"
                ));
            }
            let up = up.ok_or_else(|| format!("migration version {version} is missing up SQL"))?;
            let down =
                down.ok_or_else(|| format!("migration version {version} is missing down SQL"))?;
            migrations.push(DiscoveredMigration {
                version,
                name,
                up,
                down,
            });
        }
        Ok(migrations)
    }

    #[test]
    fn embedded_registry_is_contiguous_and_byte_pinned() {
        validate_embedded_migration_registry().expect("valid registry");
        assert_eq!(EVENT_STORE_MIGRATIONS.len(), 1);
        assert_eq!(EVENT_STORE_MIGRATIONS[0].version, 1);
        assert_eq!(EVENT_STORE_MIGRATIONS[0].name, "event_store");
        assert_eq!(EVENT_STORE_MIGRATIONS[0].up_len, FROZEN_V1_UP_LEN);
        assert_eq!(EVENT_STORE_MIGRATIONS[0].down_len, FROZEN_V1_DOWN_LEN);
        assert_eq!(EVENT_STORE_MIGRATIONS[0].up_sha256, FROZEN_V1_UP_SHA256);
        assert_eq!(EVENT_STORE_MIGRATIONS[0].down_sha256, FROZEN_V1_DOWN_SHA256);
        assert_eq!(
            EVENT_STORE_MIGRATIONS[0].schema_sha256,
            FROZEN_V1_SCHEMA_SHA256
        );
        assert_eq!(
            EVENT_STORE_MIGRATIONS[0].owned_object_names.len(),
            FROZEN_V1_OBJECT_COUNT
        );
    }

    #[test]
    fn migration_directory_exactly_matches_embedded_registry() {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
        let discovered =
            discover_migration_directory(&directory).expect("bounded migration discovery");

        assert_eq!(discovered.len(), EVENT_STORE_MIGRATIONS.len());
        for (disk, embedded) in discovered.iter().zip(EVENT_STORE_MIGRATIONS) {
            assert_eq!(disk.version, embedded.version);
            assert_eq!(disk.name, embedded.name);
            let up = fs::read(&disk.up).expect("up migration bytes");
            let down = fs::read(&disk.down).expect("down migration bytes");
            assert_eq!(up.len(), embedded.up_len);
            assert_eq!(down.len(), embedded.down_len);
            assert_eq!(sha256_hex(&up), embedded.up_sha256);
            assert_eq!(sha256_hex(&down), embedded.down_sha256);
        }

        let frozen_v1 = discovered
            .iter()
            .find(|migration| migration.version == 1)
            .expect("frozen v1 migration");
        let frozen_v1_up = fs::read(&frozen_v1.up).expect("frozen v1 up bytes");
        let frozen_v1_down = fs::read(&frozen_v1.down).expect("frozen v1 down bytes");
        assert_eq!(frozen_v1_up.len(), FROZEN_V1_UP_LEN);
        assert_eq!(frozen_v1_down.len(), FROZEN_V1_DOWN_LEN);
        assert_eq!(sha256_hex(&frozen_v1_up), FROZEN_V1_UP_SHA256);
        assert_eq!(sha256_hex(&frozen_v1_down), FROZEN_V1_DOWN_SHA256);
    }

    #[test]
    fn migration_discovery_rejects_missing_pairs_gaps_and_invalid_names() {
        let missing_pair =
            discover_migration_files([PathBuf::from("migrations/0001_event_store.up.sql")])
                .expect_err("missing pair");
        assert!(missing_pair.contains("missing down SQL"));

        let gap = discover_migration_files([
            PathBuf::from("migrations/0001_event_store.up.sql"),
            PathBuf::from("migrations/0001_event_store.down.sql"),
            PathBuf::from("migrations/0003_future.up.sql"),
            PathBuf::from("migrations/0003_future.down.sql"),
        ])
        .expect_err("gap");
        assert!(gap.contains("expected 2, found 3"));

        let invalid = discover_migration_files([PathBuf::from("migrations/1_event-store.up.sql")])
            .expect_err("invalid filename");
        assert!(invalid.contains("invalid version"));
    }

    #[test]
    fn migration_directory_rejects_extra_and_nonregular_sources() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let directory = tempdir.path().join("migrations");
        fs::create_dir(&directory).expect("migration directory");
        fs::write(directory.join("0001_event_store.up.sql"), "SELECT 1;").expect("up source");
        fs::write(directory.join("0001_event_store.down.sql"), "SELECT 1;").expect("down source");
        fs::write(directory.join("README"), "unexpected").expect("extra source");

        let extra = discover_migration_directory(&directory).expect_err("extra source");
        assert!(extra.contains("unsupported migration filename"));

        fs::remove_file(directory.join("README")).expect("remove extra source");
        fs::create_dir(directory.join("0002_future.up.sql")).expect("directory source");
        let nonregular = discover_migration_directory(&directory).expect_err("nonregular source");
        assert!(nonregular.contains("not a regular file"));
    }

    #[cfg(unix)]
    #[test]
    fn migration_directory_rejects_symlinked_roots_and_files() {
        use std::os::unix::fs::symlink;

        let tempdir = tempfile::tempdir().expect("tempdir");
        let directory = tempdir.path().join("migrations");
        fs::create_dir(&directory).expect("migration directory");
        let outside = tempdir.path().join("outside.sql");
        fs::write(&outside, "SELECT 1;").expect("outside source");
        symlink(&outside, directory.join("0001_event_store.up.sql")).expect("source symlink");

        let source_error =
            discover_migration_directory(&directory).expect_err("source symlink rejected");
        assert!(source_error.contains("must not be a symlink"));

        let linked_directory = tempdir.path().join("linked-migrations");
        symlink(&directory, &linked_directory).expect("directory symlink");
        let directory_error = discover_migration_directory(&linked_directory)
            .expect_err("directory symlink rejected");
        assert!(directory_error.contains("directory must not be a symlink"));
    }
}
