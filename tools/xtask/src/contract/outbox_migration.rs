use super::artifact_bundle::{
    GeneratedArtifact, read_regular_file, with_artifact_bundle_transaction,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use syn::{Expr, Item};

const CONTRACT_ID: &str = "radroots_outbox.migration_authority.v1";
const AUTHORITY_ID: &str = "versioned_outbox_migration_authority_v1";
const SCHEMA_VERSION: u32 = 1;
const MINIMUM_VERSION: u32 = 1;
const CURRENT_VERSION: u32 = 1;
const LEDGER_NAME: &str = "radroots_outbox_schema_migrations";
const RESERVED_PREFIX: &str = "outbox_";
const CATALOG_ROW_LIMIT: u32 = 14;
const CATALOG_REJECTION_PROBE_LIMIT: u32 = 15;
const HISTORY_ROW_LIMIT: u32 = 1;
const HISTORY_REJECTION_PROBE_LIMIT: u32 = 2;
const HASH_ALGORITHM: &str = "sha256_bytes_v1";
const CATALOG_FINGERPRINT_ALGORITHM: &str =
    "sha256_type_nul_name_nul_table_name_nul_sql_nul_sorted_v1";
const WRITE_COMMAND: &str = "cargo xtask contract outbox-migration-manifest --write";

const UP_RELATIVE: &str = "crates/outbox/migrations/0001_outbox.up.sql";
const DOWN_RELATIVE: &str = "crates/outbox/migrations/0001_outbox.down.sql";
const UP_BYTE_LENGTH: usize = 5_470;
const DOWN_BYTE_LENGTH: usize = 159;
const UP_SHA256: &str = "a7ee775d32c2b9f845961425362e1b1e558ce0d025f7d22dd58f118ba4dab4fa";
const DOWN_SHA256: &str = "5d56f978f9172dc5ecbc5043a6c286c75926974d8a2a9e44fffa7c134829af61";
const SCHEMA_SHA256: &str = "e7eeba00de78ec6d990c620e7c056018166e8a00bb703e472ef6f67a00870293";

const MANIFEST_RELATIVE: &str = "crates/outbox/contracts/migration_authority_v1.manifest.json";
const MANIFEST_SCHEMA_RELATIVE: &str =
    "crates/outbox/contracts/migration_authority_v1.manifest.schema.json";
const MANIFEST_SHA256_RELATIVE: &str =
    "crates/outbox/contracts/migration_authority_v1.manifest.sha256";
const GENERATED_DESCRIPTOR_RELATIVE: &str =
    "crates/outbox/src/generated/outbox_migration_manifest.rs";
const VECTOR_RELATIVE: &str = "contracts/conformance/vectors/outbox/migration_authority.v1.json";
const VECTOR_MIRROR_RELATIVE: &str = "crates/outbox/tests/fixtures/migration_authority.v1.json";
const VECTOR_EXECUTOR_RELATIVE: &str =
    "crates/outbox/tests/migration_authority_v1_result_vector.rs";
const RELEASE_RECORD_RELATIVE: &str = "contracts/releases/1.0.0-alpha.1.toml";
const CHANGELOG_RELATIVE: &str = "CHANGELOG.md";
const RELEASE_CHANGE_ID: &str = "outbox-versioned-migration-authority";
const CHANGELOG_MARKER: &str = "<!-- release-change: outbox-versioned-migration-authority -->";

const OBJECTS: &[&str] = &[
    "outbox_delivery_attempt",
    "outbox_delivery_attempt_target_idx",
    "outbox_delivery_plan",
    "outbox_delivery_plan_event_idx",
    "outbox_delivery_target",
    "outbox_delivery_target_ready_idx",
    "outbox_event",
    "outbox_event_event_id_idx",
    "outbox_event_ready_idx",
    "outbox_operation_idempotency_idx",
    "outbox_operation_status_idx",
    "outbox_operation_trade_mutation_idx",
    "outbox_operations",
];
const TABLES: &[&str] = &[
    "outbox_delivery_attempt",
    "outbox_delivery_plan",
    "outbox_delivery_target",
    "outbox_event",
    "outbox_operations",
];
const INDEXES: &[&str] = &[
    "outbox_delivery_attempt_target_idx",
    "outbox_delivery_plan_event_idx",
    "outbox_delivery_target_ready_idx",
    "outbox_event_event_id_idx",
    "outbox_event_ready_idx",
    "outbox_operation_idempotency_idx",
    "outbox_operation_status_idx",
    "outbox_operation_trade_mutation_idx",
];

const PUBLIC_SYMBOLS: &[&str] = &[
    "RADROOTS_OUTBOX_SCHEMA_VERSION_CURRENT",
    "RADROOTS_OUTBOX_SCHEMA_VERSION_MIN",
    "RadrootsOutboxSchemaStatus",
    "inspect_outbox_schema_status",
];
const PUBLIC_METHODS: &[&str] = &[
    "RadrootsOutbox::migrate_to_current_schema",
    "RadrootsOutbox::rollback_to_schema_version_and_close",
    "RadrootsOutbox::schema_status",
];
const REMOVED_SYMBOLS: &[&str] = &["OUTBOX_MIGRATION_DOWN", "OUTBOX_MIGRATION_UP"];
const REMOVED_METHODS: &[&str] = &["RadrootsOutbox::migrate_down"];

const ERROR_VARIANTS: &[&str] = &[
    "EmbeddedMigrationChecksumMismatch",
    "EmbeddedMigrationLengthMismatch",
    "ForeignKeyViolation",
    "GovernedCatalogCapacityExceeded",
    "IntegrityCheckFailed",
    "MigrationCatalogDeltaMismatch",
    "MigrationHistoryChecksumDrift",
    "MigrationHistoryGap",
    "MigrationHistoryNameDrift",
    "MigrationLedgerDrift",
    "MigrationRegistryDefect",
    "MigrationTransactionRollbackFailed",
    "RollbackAhead",
    "RollbackBelowVersionFloor",
    "RollbackUnmanaged",
    "SchemaFingerprintMismatch",
    "SchemaTooNew",
    "SqliteForeignKeysNotEnabled",
    "SqliteMainDatabaseEncodingNotUtf8",
    "SqliteMainDatabaseUnavailable",
    "TemporarySchemaCollision",
    "UnknownMigration",
    "UnmanagedSchema",
];

const SOURCE_SPECS: &[(&str, &str)] = &[
    ("outbox_package_manifest", "crates/outbox/Cargo.toml"),
    ("outbox_public_surface", "crates/outbox/src/lib.rs"),
    ("outbox_error_surface", "crates/outbox/src/error.rs"),
    (
        "generated_descriptor_registration",
        "crates/outbox/src/generated.rs",
    ),
    (
        "outbox_migration_registry",
        "crates/outbox/src/migrations.rs",
    ),
    ("outbox_schema_runtime", "crates/outbox/src/schema.rs"),
    ("outbox_store_runtime", "crates/outbox/src/store.rs"),
    ("outbox_package_readme", "crates/outbox/README"),
    ("vector_executor", VECTOR_EXECUTOR_RELATIVE),
    (
        "contract_governance",
        "tools/xtask/src/contract/outbox_migration.rs",
    ),
    ("contract_dispatch", "tools/xtask/src/contract.rs"),
    ("xtask_dispatch", "tools/xtask/src/main.rs"),
    ("nix_contract_lane", "build/nix/common.nix"),
    ("release_record", RELEASE_RECORD_RELATIVE),
    ("release_notes", CHANGELOG_RELATIVE),
];

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Vector {
    schema_version: u32,
    contract_id: String,
    executor: VectorExecutor,
    delegated_suite: DelegatedSuite,
    cases: Vec<VectorCase>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct VectorExecutor {
    id: String,
    path: String,
    test: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DelegatedSuite {
    lane: String,
    package: String,
    authorities: Vec<DelegatedAuthority>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DelegatedAuthority {
    authority: String,
    authority_path: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct VectorCase {
    id: String,
    execution: String,
    expected_outcome: String,
    expected_error: Option<String>,
}

#[derive(Debug, Eq, PartialEq)]
struct DiscoveredMigration {
    version: u32,
    name: String,
    up_relative: String,
    down_relative: String,
}

pub(crate) fn write_outbox_migration_manifest(workspace_root: &Path) -> Result<(), String> {
    with_artifact_bundle_transaction(workspace_root, |transaction| {
        transaction.write(expected_artifacts(workspace_root)?)?;
        validate_under_lock(workspace_root)
    })
}

pub(crate) fn validate_outbox_migration_manifest(workspace_root: &Path) -> Result<(), String> {
    with_artifact_bundle_transaction(workspace_root, |_| validate_under_lock(workspace_root))
}

fn validate_under_lock(workspace_root: &Path) -> Result<(), String> {
    for artifact in expected_artifacts(workspace_root)? {
        let actual = read_regular_file(workspace_root, artifact.relative)?;
        if actual != artifact.contents {
            return Err(format!(
                "generated outbox migration artifact {} is stale; run `{WRITE_COMMAND}`",
                artifact.relative
            ));
        }
    }
    let manifest_bytes = read_regular_file(workspace_root, MANIFEST_RELATIVE)?;
    let manifest: Value = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| format!("parse {MANIFEST_RELATIVE}: {error}"))?;
    let schema_bytes = read_regular_file(workspace_root, MANIFEST_SCHEMA_RELATIVE)?;
    let schema: Value = serde_json::from_slice(&schema_bytes)
        .map_err(|error| format!("parse {MANIFEST_SCHEMA_RELATIVE}: {error}"))?;
    let validator = jsonschema::validator_for(&schema)
        .map_err(|error| format!("compile {MANIFEST_SCHEMA_RELATIVE}: {error}"))?;
    let errors = validator
        .iter_errors(&manifest)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    if !errors.is_empty() {
        return Err(format!(
            "{MANIFEST_RELATIVE} violates its schema: {}",
            errors.join("; ")
        ));
    }
    let sidecar = read_regular_file(workspace_root, MANIFEST_SHA256_RELATIVE)?;
    if sidecar != format!("{}\n", sha256_hex(&manifest_bytes)).as_bytes() {
        return Err(format!(
            "{MANIFEST_SHA256_RELATIVE} must authenticate the exact manifest bytes"
        ));
    }
    validate_source_authority(workspace_root)?;
    validate_vector(workspace_root)?;
    validate_release_authority(workspace_root)
}

fn expected_artifacts(workspace_root: &Path) -> Result<Vec<GeneratedArtifact>, String> {
    validate_source_authority(workspace_root)?;
    validate_vector(workspace_root)?;
    validate_release_authority(workspace_root)?;
    let schema_bytes = canonical_json_bytes(&manifest_schema())?;
    let manifest = expected_manifest(workspace_root, &schema_bytes)?;
    let manifest_bytes = canonical_json_bytes(&manifest)?;
    let manifest_sha256 = sha256_hex(&manifest_bytes);
    let descriptor = generated_descriptor(&manifest_sha256);
    let vector = read_regular_file(workspace_root, VECTOR_RELATIVE)?;
    Ok(vec![
        GeneratedArtifact {
            relative: MANIFEST_RELATIVE,
            contents: manifest_bytes,
        },
        GeneratedArtifact {
            relative: MANIFEST_SCHEMA_RELATIVE,
            contents: schema_bytes,
        },
        GeneratedArtifact {
            relative: MANIFEST_SHA256_RELATIVE,
            contents: format!("{manifest_sha256}\n").into_bytes(),
        },
        GeneratedArtifact {
            relative: GENERATED_DESCRIPTOR_RELATIVE,
            contents: descriptor.into_bytes(),
        },
        GeneratedArtifact {
            relative: VECTOR_MIRROR_RELATIVE,
            contents: vector,
        },
    ])
}

fn expected_manifest(workspace_root: &Path, schema_bytes: &[u8]) -> Result<Value, String> {
    let vector_bytes = read_regular_file(workspace_root, VECTOR_RELATIVE)?;
    let executor_bytes = read_regular_file(workspace_root, VECTOR_EXECUTOR_RELATIVE)?;
    let ledger_ddl = extract_string_const(
        &parse_rust(workspace_root, "crates/outbox/src/migrations.rs")?,
        "OUTBOX_LEDGER_DDL",
    )?;
    Ok(json!({
        "schema_version": SCHEMA_VERSION,
        "contract_id": CONTRACT_ID,
        "authority_id": AUTHORITY_ID,
        "manifest_schema": descriptor_for_bytes(MANIFEST_SCHEMA_RELATIVE, schema_bytes)?,
        "runtime": {
            "minimum_version": MINIMUM_VERSION,
            "current_version": CURRENT_VERSION,
            "reserved_prefix": RESERVED_PREFIX,
            "ledger_name": LEDGER_NAME,
            "ledger_ddl_sha256": sha256_hex(ledger_ddl.as_bytes()),
            "catalog_fingerprint_algorithm": CATALOG_FINGERPRINT_ALGORITHM,
            "migration_transaction": "begin_immediate_v1",
            "rollback_transaction": "begin_exclusive_terminal_close_v1",
            "catalog_row_limit": CATALOG_ROW_LIMIT,
            "catalog_rejection_probe_limit": CATALOG_REJECTION_PROBE_LIMIT,
            "history_row_limit": HISTORY_ROW_LIMIT,
            "history_rejection_probe_limit": HISTORY_REJECTION_PROBE_LIMIT,
            "open_validations": [
                "main_database_backing_identity",
                "utf8_encoding_before_journal_or_schema_mutation",
                "foreign_keys_enabled",
                "file_wal_result",
                "temporary_schema_authority",
                "bounded_managed_catalog",
                "tamper_evident_history",
                "catalog_fingerprint",
                "integrity_check_one",
                "outbox_scoped_foreign_key_check"
            ],
            "adoption_policy": "exact_unledgered_0001_catalog_only_v1",
            "rollback_floor": 1,
            "test_destruction": "cfg_test_destroy_outbox_schema_for_migration_test"
        },
        "migrations": [{
            "version": 1,
            "name": "outbox",
            "up": descriptor_for_file(workspace_root, UP_RELATIVE)?,
            "down": descriptor_for_file(workspace_root, DOWN_RELATIVE)?,
            "schema_sha256": SCHEMA_SHA256,
            "catalog": {
                "objects": OBJECTS,
                "tables": TABLES,
                "indexes": INDEXES
            }
        }],
        "public_api": {
            "added_symbols": PUBLIC_SYMBOLS,
            "methods": PUBLIC_METHODS,
            "removed_symbols": REMOVED_SYMBOLS,
            "removed_methods": REMOVED_METHODS,
            "error_variants": ERROR_VARIANTS,
            "error_enum_non_exhaustive": true
        },
        "source_files": SOURCE_SPECS.iter().map(|(role, path)| {
            Ok(json!({
                "role": role,
                "file": descriptor_for_file(workspace_root, path)?
            }))
        }).collect::<Result<Vec<Value>, String>>()?,
        "result_vector": {
            "canonical": descriptor_for_bytes(VECTOR_RELATIVE, &vector_bytes)?,
            "mirror_path": VECTOR_MIRROR_RELATIVE,
            "executor": descriptor_for_bytes(VECTOR_EXECUTOR_RELATIVE, &executor_bytes)?,
            "executor_id": "radroots_outbox.migration_authority_v1.result_vector_executor.v1",
            "executor_test": "migration_authority_v1_result_vector"
        },
        "release": {
            "change_id": RELEASE_CHANGE_ID,
            "release_record": RELEASE_RECORD_RELATIVE,
            "changelog": CHANGELOG_RELATIVE
        }
    }))
}

fn manifest_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://radroots.org/contracts/outbox/migration_authority_v1.manifest.schema.json",
        "type": "object",
        "additionalProperties": false,
        "required": [
            "schema_version", "contract_id", "authority_id", "manifest_schema", "runtime",
            "migrations", "public_api", "source_files", "result_vector", "release"
        ],
        "properties": {
            "schema_version": { "const": 1 },
            "contract_id": { "const": CONTRACT_ID },
            "authority_id": { "const": AUTHORITY_ID },
            "manifest_schema": { "$ref": "#/$defs/file" },
            "runtime": {
                "type": "object",
                "additionalProperties": false,
                "required": [
                    "minimum_version", "current_version", "reserved_prefix", "ledger_name",
                    "ledger_ddl_sha256", "catalog_fingerprint_algorithm", "migration_transaction",
                    "rollback_transaction", "catalog_row_limit", "catalog_rejection_probe_limit",
                    "history_row_limit", "history_rejection_probe_limit", "open_validations",
                    "adoption_policy", "rollback_floor", "test_destruction"
                ],
                "properties": {
                    "minimum_version": { "const": 1 },
                    "current_version": { "const": 1 },
                    "reserved_prefix": { "const": "outbox_" },
                    "ledger_name": { "const": LEDGER_NAME },
                    "ledger_ddl_sha256": { "$ref": "#/$defs/sha256" },
                    "catalog_fingerprint_algorithm": { "type": "string", "minLength": 1 },
                    "migration_transaction": { "const": "begin_immediate_v1" },
                    "rollback_transaction": { "const": "begin_exclusive_terminal_close_v1" },
                    "catalog_row_limit": { "const": 14 },
                    "catalog_rejection_probe_limit": { "const": 15 },
                    "history_row_limit": { "const": 1 },
                    "history_rejection_probe_limit": { "const": 2 },
                    "open_validations": { "type": "array", "minItems": 10, "uniqueItems": true, "items": { "type": "string", "minLength": 1 } },
                    "adoption_policy": { "const": "exact_unledgered_0001_catalog_only_v1" },
                    "rollback_floor": { "const": 1 },
                    "test_destruction": { "const": "cfg_test_destroy_outbox_schema_for_migration_test" }
                }
            },
            "migrations": {
                "type": "array", "minItems": 1, "maxItems": 1,
                "items": {
                    "type": "object", "additionalProperties": false,
                    "required": ["version", "name", "up", "down", "schema_sha256", "catalog"],
                    "properties": {
                        "version": { "const": 1 }, "name": { "const": "outbox" },
                        "up": { "$ref": "#/$defs/file" }, "down": { "$ref": "#/$defs/file" },
                        "schema_sha256": { "$ref": "#/$defs/sha256" },
                        "catalog": {
                            "type": "object", "additionalProperties": false,
                            "required": ["objects", "tables", "indexes"],
                            "properties": {
                                "objects": { "type": "array", "minItems": 13, "maxItems": 13, "uniqueItems": true, "items": { "type": "string" } },
                                "tables": { "type": "array", "minItems": 5, "maxItems": 5, "uniqueItems": true, "items": { "type": "string" } },
                                "indexes": { "type": "array", "minItems": 8, "maxItems": 8, "uniqueItems": true, "items": { "type": "string" } }
                            }
                        }
                    }
                }
            },
            "public_api": {
                "type": "object", "additionalProperties": false,
                "required": ["added_symbols", "methods", "removed_symbols", "removed_methods", "error_variants", "error_enum_non_exhaustive"],
                "properties": {
                    "added_symbols": { "type": "array", "minItems": 4, "maxItems": 4, "uniqueItems": true, "items": { "type": "string", "minLength": 1 } },
                    "methods": { "type": "array", "minItems": 3, "maxItems": 3, "uniqueItems": true, "items": { "type": "string", "minLength": 1 } },
                    "removed_symbols": { "type": "array", "minItems": 2, "maxItems": 2, "uniqueItems": true, "items": { "type": "string", "minLength": 1 } },
                    "removed_methods": { "type": "array", "minItems": 1, "maxItems": 1, "uniqueItems": true, "items": { "type": "string", "minLength": 1 } },
                    "error_variants": { "type": "array", "minItems": 23, "maxItems": 23, "uniqueItems": true, "items": { "type": "string", "minLength": 1 } },
                    "error_enum_non_exhaustive": { "const": true }
                }
            },
            "source_files": {
                "type": "array", "minItems": 15, "maxItems": 15,
                "items": {
                    "type": "object", "additionalProperties": false,
                    "required": ["role", "file"],
                    "properties": {
                        "role": { "type": "string", "minLength": 1 },
                        "file": { "$ref": "#/$defs/file" }
                    }
                }
            },
            "result_vector": {
                "type": "object", "additionalProperties": false,
                "required": ["canonical", "mirror_path", "executor", "executor_id", "executor_test"],
                "properties": {
                    "canonical": { "$ref": "#/$defs/file" },
                    "mirror_path": { "const": VECTOR_MIRROR_RELATIVE },
                    "executor": { "$ref": "#/$defs/file" },
                    "executor_id": { "const": "radroots_outbox.migration_authority_v1.result_vector_executor.v1" },
                    "executor_test": { "const": "migration_authority_v1_result_vector" }
                }
            },
            "release": {
                "type": "object", "additionalProperties": false,
                "required": ["change_id", "release_record", "changelog"],
                "properties": {
                    "change_id": { "const": RELEASE_CHANGE_ID },
                    "release_record": { "const": RELEASE_RECORD_RELATIVE },
                    "changelog": { "const": CHANGELOG_RELATIVE }
                }
            }
        },
        "$defs": {
            "sha256": { "type": "string", "pattern": "^[0-9a-f]{64}$" },
            "file": {
                "type": "object", "additionalProperties": false,
                "required": ["path", "byte_length", "sha256", "hash_algorithm"],
                "properties": {
                    "path": { "type": "string", "minLength": 1 },
                    "byte_length": { "type": "integer", "minimum": 1 },
                    "sha256": { "$ref": "#/$defs/sha256" },
                    "hash_algorithm": { "const": HASH_ALGORITHM }
                }
            }
        }
    })
}

fn generated_descriptor(manifest_sha256: &str) -> String {
    format!(
        "// @generated by `cargo xtask contract outbox-migration-manifest --write`; do not edit.\n\n\
pub(crate) const OUTBOX_MIGRATION_CONTRACT_ID: &str = \"{CONTRACT_ID}\";\n\
pub(crate) const OUTBOX_MIGRATION_SCHEMA_VERSION: u32 = {SCHEMA_VERSION};\n\
pub(crate) const OUTBOX_MIGRATION_CURRENT_VERSION: u32 = {CURRENT_VERSION};\n\
pub(crate) const OUTBOX_MIGRATION_0001_UP_BYTE_LENGTH: usize = {UP_BYTE_LENGTH};\n\
pub(crate) const OUTBOX_MIGRATION_0001_DOWN_BYTE_LENGTH: usize = {DOWN_BYTE_LENGTH};\n\
pub(crate) const OUTBOX_MIGRATION_0001_UP_SHA256: &str =\n    \"{UP_SHA256}\";\n\
pub(crate) const OUTBOX_MIGRATION_0001_DOWN_SHA256: &str =\n    \"{DOWN_SHA256}\";\n\
pub(crate) const OUTBOX_MIGRATION_0001_SCHEMA_SHA256: &str =\n    \"{SCHEMA_SHA256}\";\n\
pub(crate) const OUTBOX_MIGRATION_MANIFEST_SHA256: &str =\n    \"{manifest_sha256}\";\n"
    )
}

fn validate_source_authority(workspace_root: &Path) -> Result<(), String> {
    let source_roles = SOURCE_SPECS
        .iter()
        .map(|(role, _)| *role)
        .collect::<BTreeSet<_>>();
    let source_paths = SOURCE_SPECS
        .iter()
        .map(|(_, path)| *path)
        .collect::<BTreeSet<_>>();
    if source_roles.len() != SOURCE_SPECS.len() || source_paths.len() != SOURCE_SPECS.len() {
        return Err("outbox migration source roles and paths must be unique".to_owned());
    }
    let objects = OBJECTS.iter().copied().collect::<BTreeSet<_>>();
    let tables = TABLES.iter().copied().collect::<BTreeSet<_>>();
    let indexes = INDEXES.iter().copied().collect::<BTreeSet<_>>();
    if objects.len() != 13
        || tables.len() != 5
        || indexes.len() != 8
        || !tables.is_disjoint(&indexes)
        || tables.union(&indexes).copied().collect::<BTreeSet<_>>() != objects
    {
        return Err(
            "outbox catalog contract must contain exactly five tables and eight indexes".to_owned(),
        );
    }
    let discovered = discover_migrations(workspace_root)?;
    if discovered
        != [DiscoveredMigration {
            version: 1,
            name: "outbox".to_owned(),
            up_relative: UP_RELATIVE.to_owned(),
            down_relative: DOWN_RELATIVE.to_owned(),
        }]
    {
        return Err(
            "outbox migration discovery must contain exactly the 0001 outbox pair".to_owned(),
        );
    }
    validate_frozen_file(workspace_root, UP_RELATIVE, UP_BYTE_LENGTH, UP_SHA256)?;
    validate_frozen_file(workspace_root, DOWN_RELATIVE, DOWN_BYTE_LENGTH, DOWN_SHA256)?;

    let migrations = parse_rust(workspace_root, "crates/outbox/src/migrations.rs")?;
    if extract_string_array_const(&migrations, "OUTBOX_BASELINE_OBJECT_NAMES")? != OBJECTS
        || extract_string_array_const(&migrations, "OUTBOX_BASELINE_TABLE_NAMES")? != TABLES
    {
        return Err("outbox migration source catalog inventory is not exact".to_owned());
    }
    let migration_source = read_utf8(workspace_root, "crates/outbox/src/migrations.rs")?;
    for marker in [
        "include_str!(\"../migrations/0001_outbox.up.sql\")",
        "include_str!(\"../migrations/0001_outbox.down.sql\")",
        "validate_embedded_migration_input",
        "validate_migration_registry",
        "RADROOTS_OUTBOX_SCHEMA_VERSION_CURRENT",
    ] {
        if !migration_source.contains(marker) {
            return Err(format!(
                "outbox migration registry is missing `{marker}` authority"
            ));
        }
    }
    let schema = read_utf8(workspace_root, "crates/outbox/src/schema.rs")?;
    for marker in [
        "begin_with(\"BEGIN IMMEDIATE\")",
        "begin_with(\"BEGIN EXCLUSIVE\")",
        "main.sqlite_schema",
        "PRAGMA integrity_check(1)",
        "pragma_foreign_key_check",
        "LIMIT ?",
        "UnledgeredBaseline",
        "destroy_outbox_schema_for_migration_test",
    ] {
        if !schema.contains(marker) {
            return Err(format!(
                "outbox schema runtime is missing `{marker}` authority"
            ));
        }
    }
    let store = read_utf8(workspace_root, "crates/outbox/src/store.rs")?;
    for marker in [
        "rollback_to_schema_version_and_close",
        "migrate_to_current_schema",
        "validate_main_database_encoding",
        "PRAGMA foreign_keys",
        "PRAGMA main.journal_mode = WAL",
        "PRAGMA database_list",
    ] {
        if !store.contains(marker) {
            return Err(format!(
                "outbox store runtime is missing `{marker}` authority"
            ));
        }
    }
    if store.contains("pub async fn migrate_down") {
        return Err("unrestricted public outbox migrate_down remains reachable".to_owned());
    }
    let store_ast = parse_rust(workspace_root, "crates/outbox/src/store.rs")?;
    let public_async_methods = store_ast
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Impl(item) if item.trait_.is_none() => Some(item),
            _ => None,
        })
        .filter(|item| match item.self_ty.as_ref() {
            syn::Type::Path(path) => path
                .path
                .segments
                .last()
                .is_some_and(|segment| segment.ident == "RadrootsOutbox"),
            _ => false,
        })
        .flat_map(|item| item.items.iter())
        .filter_map(|item| match item {
            syn::ImplItem::Fn(method)
                if matches!(method.vis, syn::Visibility::Public(_))
                    && method.sig.asyncness.is_some() =>
            {
                Some(method.sig.ident.to_string())
            }
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    for qualified in PUBLIC_METHODS {
        let method = qualified
            .rsplit_once("::")
            .map(|(_, method)| method)
            .ok_or_else(|| format!("invalid public outbox method identity `{qualified}`"))?;
        if !public_async_methods.contains(method) {
            return Err(format!(
                "outbox public method `{qualified}` is not reachable"
            ));
        }
    }
    for qualified in REMOVED_METHODS {
        let method = qualified
            .rsplit_once("::")
            .map(|(_, method)| method)
            .ok_or_else(|| format!("invalid removed outbox method identity `{qualified}`"))?;
        if public_async_methods.contains(method) {
            return Err(format!(
                "removed outbox method `{qualified}` remains reachable"
            ));
        }
    }

    let errors = parse_rust(workspace_root, "crates/outbox/src/error.rs")?;
    let error_enum = errors
        .items
        .iter()
        .find_map(|item| match item {
            Item::Enum(item) if item.ident == "RadrootsOutboxError" => Some(item),
            _ => None,
        })
        .ok_or_else(|| "RadrootsOutboxError enum is missing".to_owned())?;
    if !matches!(error_enum.vis, syn::Visibility::Public(_))
        || !error_enum
            .attrs
            .iter()
            .any(|attribute| attribute.path().is_ident("non_exhaustive"))
    {
        return Err("outbox error authority must be non-exhaustive".to_owned());
    }
    let error_variants = error_enum
        .variants
        .iter()
        .map(|variant| variant.ident.to_string())
        .collect::<BTreeSet<_>>();
    for variant in ERROR_VARIANTS {
        if !error_variants.contains(*variant) {
            return Err(format!(
                "outbox error authority is missing typed variant `{variant}`"
            ));
        }
    }
    let public = read_utf8(workspace_root, "crates/outbox/src/lib.rs")?;
    for symbol in PUBLIC_SYMBOLS {
        if !public.contains(symbol) {
            return Err(format!("outbox public surface is missing `{symbol}`"));
        }
    }
    for removed in REMOVED_SYMBOLS {
        if public.contains(removed) {
            return Err(format!(
                "removed raw outbox migration symbol `{removed}` remains public"
            ));
        }
    }
    let generated = read_utf8(workspace_root, "crates/outbox/src/generated.rs")?;
    if !generated.contains("mod outbox_migration_manifest") {
        return Err("outbox generated descriptor is not registered".to_owned());
    }
    Ok(())
}

fn validate_vector(workspace_root: &Path) -> Result<(), String> {
    let bytes = read_regular_file(workspace_root, VECTOR_RELATIVE)?;
    let vector: Vector = serde_json::from_slice(&bytes)
        .map_err(|error| format!("parse {VECTOR_RELATIVE}: {error}"))?;
    if bytes != canonical_json_bytes(&vector)? {
        return Err(format!("{VECTOR_RELATIVE} must be canonical pretty JSON"));
    }
    if vector.schema_version != 1
        || vector.contract_id != CONTRACT_ID
        || vector.executor.id != "radroots_outbox.migration_authority_v1.result_vector_executor.v1"
        || vector.executor.path != VECTOR_EXECUTOR_RELATIVE
        || vector.executor.test != "migration_authority_v1_result_vector"
        || vector.delegated_suite.lane != "nix run .#contract"
        || vector.delegated_suite.package != "radroots_outbox"
    {
        return Err(format!(
            "{VECTOR_RELATIVE} has invalid executor or contract identity"
        ));
    }
    let direct = [
        "fresh_initialization",
        "exact_unledgered_adoption",
        "partial_unledgered_rejected",
        "ledger_checksum_tamper_rejected",
        "newer_history_rejected",
        "rollback_below_floor_rejected",
        "caller_state_preserved",
        "current_reopen_no_history_write",
    ];
    if vector.cases.len() != direct.len()
        || vector
            .cases
            .iter()
            .map(|case| case.id.as_str())
            .collect::<Vec<_>>()
            != direct
        || vector
            .cases
            .iter()
            .any(|case| case.execution != "direct_executor")
    {
        return Err(format!(
            "{VECTOR_RELATIVE} direct case inventory is not exact"
        ));
    }
    let mut authorities = BTreeSet::new();
    for authority in &vector.delegated_suite.authorities {
        if !authorities.insert(authority.authority.as_str()) {
            return Err(format!(
                "{VECTOR_RELATIVE} contains duplicate delegated authority"
            ));
        }
        let source = read_utf8(workspace_root, &authority.authority_path)?;
        if !source.contains(&authority.authority) {
            return Err(format!(
                "delegated authority `{}` is absent from {}",
                authority.authority, authority.authority_path
            ));
        }
    }
    if authorities.len() != 10 {
        return Err(format!(
            "{VECTOR_RELATIVE} must bind ten delegated authorities"
        ));
    }
    let executor = read_utf8(workspace_root, VECTOR_EXECUTOR_RELATIVE)?;
    for case in direct {
        if !executor.contains(case) {
            return Err(format!("vector executor does not execute `{case}`"));
        }
    }
    Ok(())
}

fn validate_release_authority(workspace_root: &Path) -> Result<(), String> {
    let release = read_utf8(workspace_root, RELEASE_RECORD_RELATIVE)?;
    if !release.contains(&format!("id = \"{RELEASE_CHANGE_ID}\"")) {
        return Err(format!(
            "{RELEASE_RECORD_RELATIVE} is missing `{RELEASE_CHANGE_ID}`"
        ));
    }
    let changelog = read_utf8(workspace_root, CHANGELOG_RELATIVE)?;
    if !changelog.contains(CHANGELOG_MARKER) {
        return Err(format!(
            "{CHANGELOG_RELATIVE} is missing `{CHANGELOG_MARKER}`"
        ));
    }
    Ok(())
}

fn discover_migrations(workspace_root: &Path) -> Result<Vec<DiscoveredMigration>, String> {
    discover_migrations_in(
        &workspace_root.join("crates/outbox/migrations"),
        workspace_root,
    )
}

fn discover_migrations_in(
    directory: &Path,
    relative_root: &Path,
) -> Result<Vec<DiscoveredMigration>, String> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| format!("read {}: {error}", directory.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("read migration directory entry: {error}"))?;
    entries.sort_by_key(|entry| entry.file_name());
    let mut pairs =
        std::collections::BTreeMap::<(u32, String), (Option<PathBuf>, Option<PathBuf>)>::new();
    for entry in entries {
        let metadata = fs::symlink_metadata(entry.path())
            .map_err(|error| format!("inspect {}: {error}", entry.path().display()))?;
        if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
            return Err(format!(
                "migration input must be a regular file: {}",
                entry.path().display()
            ));
        }
        let filename = entry
            .file_name()
            .into_string()
            .map_err(|_| "migration filename must be UTF-8".to_owned())?;
        let (stem, direction) = if let Some(stem) = filename.strip_suffix(".up.sql") {
            (stem, "up")
        } else if let Some(stem) = filename.strip_suffix(".down.sql") {
            (stem, "down")
        } else {
            return Err(format!("unknown migration file `{filename}`"));
        };
        let (version, name) = stem
            .split_once('_')
            .ok_or_else(|| format!("invalid migration filename `{filename}`"))?;
        if version.len() != 4
            || !version.bytes().all(|byte| byte.is_ascii_digit())
            || name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        {
            return Err(format!("invalid migration filename `{filename}`"));
        }
        let version = version.parse::<u32>().map_err(|error| error.to_string())?;
        let pair = pairs.entry((version, name.to_owned())).or_default();
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
            let relative = |path: PathBuf| {
                path.strip_prefix(relative_root)
                    .map(|path| path.to_string_lossy().replace('\\', "/"))
                    .map_err(|_| format!("migration is outside {}", relative_root.display()))
            };
            Ok(DiscoveredMigration {
                version,
                name,
                up_relative: relative(
                    up.ok_or_else(|| format!("migration {version} is missing up SQL"))?,
                )?,
                down_relative: relative(
                    down.ok_or_else(|| format!("migration {version} is missing down SQL"))?,
                )?,
            })
        })
        .collect()
}

fn validate_frozen_file(
    workspace_root: &Path,
    relative: &str,
    expected_length: usize,
    expected_sha256: &str,
) -> Result<(), String> {
    let bytes = read_regular_file(workspace_root, relative)?;
    if bytes.len() != expected_length || sha256_hex(&bytes) != expected_sha256 {
        return Err(format!(
            "frozen migration `{relative}` byte identity drifted"
        ));
    }
    Ok(())
}

fn parse_rust(workspace_root: &Path, relative: &str) -> Result<syn::File, String> {
    let source = read_utf8(workspace_root, relative)?;
    syn::parse_file(&source).map_err(|error| format!("parse {relative}: {error}"))
}

fn extract_string_const(source: &syn::File, name: &str) -> Result<String, String> {
    source
        .items
        .iter()
        .find_map(|item| {
            let Item::Const(item) = item else { return None };
            (item.ident == name).then_some(item.expr.as_ref())
        })
        .and_then(|expr| match expr {
            Expr::Lit(literal) => match &literal.lit {
                syn::Lit::Str(value) => Some(value.value()),
                _ => None,
            },
            _ => None,
        })
        .ok_or_else(|| format!("missing string const `{name}`"))
}

fn extract_string_array_const(source: &syn::File, name: &str) -> Result<Vec<String>, String> {
    let expression = source
        .items
        .iter()
        .find_map(|item| {
            let Item::Const(item) = item else { return None };
            (item.ident == name).then_some(item.expr.as_ref())
        })
        .ok_or_else(|| format!("missing array const `{name}`"))?;
    let Expr::Reference(reference) = expression else {
        return Err(format!("array const `{name}` must be a reference"));
    };
    let Expr::Array(array) = reference.expr.as_ref() else {
        return Err(format!("array const `{name}` must reference an array"));
    };
    array
        .elems
        .iter()
        .map(|element| {
            let Expr::Lit(literal) = element else {
                return Err(format!("array const `{name}` must contain string literals"));
            };
            let syn::Lit::Str(value) = &literal.lit else {
                return Err(format!("array const `{name}` must contain string literals"));
            };
            Ok(value.value())
        })
        .collect()
}

fn descriptor_for_file(workspace_root: &Path, relative: &str) -> Result<Value, String> {
    descriptor_for_bytes(relative, &read_regular_file(workspace_root, relative)?)
}

fn descriptor_for_bytes(relative: &str, bytes: &[u8]) -> Result<Value, String> {
    let byte_length =
        u64::try_from(bytes.len()).map_err(|_| format!("{relative} byte length is outside u64"))?;
    Ok(json!({
        "path": relative,
        "byte_length": byte_length,
        "sha256": sha256_hex(bytes),
        "hash_algorithm": HASH_ALGORITHM
    }))
}

fn read_utf8(workspace_root: &Path, relative: &str) -> Result<String, String> {
    let bytes = read_regular_file(workspace_root, relative)?;
    String::from_utf8(bytes).map_err(|error| format!("{relative} must be UTF-8: {error}"))
}

fn canonical_json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("serialize canonical JSON: {error}"))?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_discovery_rejects_unknown_missing_and_non_regular_inputs() {
        let root = tempfile::tempdir().expect("tempdir");
        let directory = root.path().join("migrations");
        fs::create_dir(&directory).expect("directory");
        fs::write(directory.join("0001_outbox.up.sql"), b"up").expect("up");
        fs::write(directory.join("0001_outbox.down.sql"), b"down").expect("down");
        assert_eq!(
            discover_migrations_in(&directory, root.path())
                .expect("discovery")
                .len(),
            1
        );
        fs::write(directory.join("README"), b"unknown").expect("unknown");
        assert!(
            discover_migrations_in(&directory, root.path())
                .expect_err("unknown")
                .contains("unknown migration file")
        );
        fs::remove_file(directory.join("README")).expect("remove unknown");
        fs::remove_file(directory.join("0001_outbox.down.sql")).expect("remove down");
        assert!(
            discover_migrations_in(&directory, root.path())
                .expect_err("missing down")
                .contains("missing down SQL")
        );
        fs::create_dir(directory.join("0002_future.up.sql")).expect("non-regular");
        assert!(
            discover_migrations_in(&directory, root.path())
                .expect_err("non-regular")
                .contains("regular file")
        );
    }

    #[test]
    fn canonical_workspace_authority_and_generated_artifacts_are_current() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("workspace root");
        validate_source_authority(root).expect("source authority");
        validate_vector(root).expect("vector");
        validate_release_authority(root).expect("release authority");
        validate_outbox_migration_manifest(root).expect("generated artifacts");
    }
}
