use super::artifact_bundle::{
    GeneratedArtifact, read_regular_file, with_artifact_bundle_transaction,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;

const CONTRACT_ID: &str = "radroots_outbox.migration_authority.v1";
const AUTHORITY_ID: &str = "versioned_outbox_migration_authority_v1";
const HASH_ALGORITHM: &str = "sha256_bytes_v1";
const WRITE_COMMAND: &str = "cargo xtask contract outbox-migration-manifest --write";
const REGISTRY_RELATIVE: &str = "crates/outbox/contracts/migration_registry.v1.json";
const MIGRATION_DIRECTORY_RELATIVE: &str = "crates/outbox/migrations";
const FEATURE_MATRIX_RELATIVE: &str = "contracts/outbox_feature_matrix.toml";
const OUTBOX_CARGO_RELATIVE: &str = "crates/outbox/Cargo.toml";
const GENERATED_RUNTIME_RELATIVE: &str = "crates/outbox/src/generated/outbox_migration_registry.rs";
const MANIFEST_RELATIVE: &str = "crates/outbox/contracts/migration_authority_v1.manifest.json";
const MANIFEST_SCHEMA_RELATIVE: &str =
    "crates/outbox/contracts/migration_authority_v1.manifest.schema.json";
const MANIFEST_SHA256_RELATIVE: &str =
    "crates/outbox/contracts/migration_authority_v1.manifest.sha256";
const VECTOR_RELATIVE: &str = "contracts/conformance/vectors/outbox/migration_authority.v1.json";
const VECTOR_MIRROR_RELATIVE: &str = "crates/outbox/tests/fixtures/migration_authority.v1.json";
const VECTOR_EXECUTOR_RELATIVE: &str =
    "crates/outbox/tests/migration_authority_v1_result_vector.rs";
const VECTOR_EXECUTOR_ID: &str = "radroots_outbox.migration_authority_v1.result_vector_executor.v1";
const VECTOR_EXECUTOR_TEST: &str = "migration_authority_v1_result_vector";
const RELEASE_RELATIVE: &str = "contracts/releases/1.0.0-alpha.1.toml";
const CHANGELOG_RELATIVE: &str = "CHANGELOG.md";
const RELEASE_CHANGE_ID: &str = "outbox-versioned-migration-authority";

const FROZEN_UP_LENGTH: usize = 5_470;
const FROZEN_DOWN_LENGTH: usize = 159;
const FROZEN_UP_SHA256: &str = "a7ee775d32c2b9f845961425362e1b1e558ce0d025f7d22dd58f118ba4dab4fa";
const FROZEN_DOWN_SHA256: &str = "5d56f978f9172dc5ecbc5043a6c286c75926974d8a2a9e44fffa7c134829af61";
const FROZEN_SCHEMA_SHA256: &str =
    "e7eeba00de78ec6d990c620e7c056018166e8a00bb703e472ef6f67a00870293";
const FROZEN_UP_PATH: &str = "crates/outbox/migrations/0001_outbox.up.sql";
const FROZEN_DOWN_PATH: &str = "crates/outbox/migrations/0001_outbox.down.sql";
const LEDGER_NAME: &str = "radroots_outbox_schema_migrations";
const RESERVED_PREFIX: &str = "outbox_";

const FROZEN_OBJECTS: &[&str] = &[
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
const FROZEN_TABLES: &[&str] = &[
    "outbox_delivery_attempt",
    "outbox_delivery_plan",
    "outbox_delivery_target",
    "outbox_event",
    "outbox_operations",
];

const SOURCE_FILES: &[(&str, &str)] = &[
    ("outbox_package_manifest", "crates/outbox/Cargo.toml"),
    ("outbox_public_surface", "crates/outbox/src/lib.rs"),
    ("outbox_error_surface", "crates/outbox/src/error.rs"),
    ("generated_module", "crates/outbox/src/generated.rs"),
    (
        "migration_registry_runtime",
        "crates/outbox/src/migrations.rs",
    ),
    ("schema_runtime", "crates/outbox/src/schema.rs"),
    ("store_integration", "crates/outbox/src/store.rs"),
    ("vector_executor", VECTOR_EXECUTOR_RELATIVE),
    (
        "contract_governance",
        "tools/xtask/src/contract/outbox_migration.rs",
    ),
    ("contract_dispatch", "tools/xtask/src/contract.rs"),
    ("xtask_dispatch", "tools/xtask/src/main.rs"),
    ("nix_feature_executor", "build/nix/common.nix"),
    ("nix_feature_check", "build/nix/checks.nix"),
    ("outbox_readme", "crates/outbox/README"),
    ("release_record", RELEASE_RELATIVE),
    ("release_notes", CHANGELOG_RELATIVE),
];

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Registry {
    schema_version: u32,
    contract_id: String,
    migrations: Vec<RegistryMigration>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RegistryMigration {
    version: u32,
    name: String,
    up_path: String,
    down_path: String,
    up_byte_length: usize,
    down_byte_length: usize,
    up_sha256: String,
    down_sha256: String,
    schema_sha256: String,
    owned_objects: Vec<String>,
    owned_tables: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FeatureMatrix {
    schema_version: u32,
    package: String,
    feature_edges: BTreeMap<String, Vec<String>>,
    profiles: Vec<FeatureProfile>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FeatureProfile {
    id: String,
    cargo_args: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Vector {
    schema_version: u32,
    contract_id: String,
    executor: VectorExecutor,
    delegated_suite: DelegatedSuite,
    cases: Vec<VectorCase>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct VectorExecutor {
    id: String,
    path: String,
    test: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DelegatedSuite {
    lane: String,
    package: String,
    authorities: Vec<DelegatedAuthority>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DelegatedAuthority {
    authority: String,
    authority_path: String,
}

#[derive(Debug, Deserialize)]
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
    Ok(())
}

fn expected_artifacts(workspace_root: &Path) -> Result<Vec<GeneratedArtifact>, String> {
    let registry = load_and_validate_registry(workspace_root)?;
    let matrix = load_and_validate_feature_matrix(workspace_root)?;
    validate_vector(workspace_root)?;
    validate_release_authority(workspace_root)?;
    let generated_runtime = generated_runtime_registry(&registry)?;
    let schema_bytes = canonical_json_bytes(&manifest_schema())?;
    let manifest = expected_manifest(
        workspace_root,
        &registry,
        &matrix,
        &schema_bytes,
        generated_runtime.as_bytes(),
    )?;
    let manifest_bytes = canonical_json_bytes(&manifest)?;
    let manifest_sha256 = sha256_hex(&manifest_bytes);
    let vector = read_regular_file(workspace_root, VECTOR_RELATIVE)?;
    Ok(vec![
        GeneratedArtifact {
            relative: GENERATED_RUNTIME_RELATIVE,
            contents: generated_runtime.into_bytes(),
        },
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
            relative: VECTOR_MIRROR_RELATIVE,
            contents: vector,
        },
    ])
}

fn load_and_validate_registry(workspace_root: &Path) -> Result<Registry, String> {
    let bytes = read_regular_file(workspace_root, REGISTRY_RELATIVE)?;
    let registry: Registry = serde_json::from_slice(&bytes)
        .map_err(|error| format!("parse {REGISTRY_RELATIVE}: {error}"))?;
    validate_registry_shape(&registry)?;
    let discovered = discover_migrations(workspace_root)?;
    if discovered.len() != registry.migrations.len() {
        return Err(format!(
            "outbox migration discovery found {} pairs but registry declares {}",
            discovered.len(),
            registry.migrations.len()
        ));
    }
    for (discovered, migration) in discovered.iter().zip(&registry.migrations) {
        if discovered.version != migration.version
            || discovered.name != migration.name
            || discovered.up_relative != migration.up_path
            || discovered.down_relative != migration.down_path
        {
            return Err(format!(
                "outbox migration discovery does not match registry version {}",
                migration.version
            ));
        }
        validate_declared_file(
            workspace_root,
            migration.version,
            "up",
            &migration.up_path,
            migration.up_byte_length,
            &migration.up_sha256,
        )?;
        validate_declared_file(
            workspace_root,
            migration.version,
            "down",
            &migration.down_path,
            migration.down_byte_length,
            &migration.down_sha256,
        )?;
    }
    Ok(registry)
}

fn validate_registry_shape(registry: &Registry) -> Result<(), String> {
    if registry.schema_version != 1 || registry.contract_id != CONTRACT_ID {
        return Err("outbox migration registry identity must remain v1".to_owned());
    }
    if registry.migrations.is_empty() {
        return Err("outbox migration registry must not be empty".to_owned());
    }
    let mut names = BTreeSet::new();
    let mut objects = BTreeSet::new();
    let mut tables = BTreeSet::new();
    for (index, migration) in registry.migrations.iter().enumerate() {
        let expected_version = u32::try_from(index)
            .map_err(|_| "outbox migration registry is too large".to_owned())?
            .checked_add(1)
            .ok_or_else(|| "outbox migration version overflow".to_owned())?;
        if migration.version != expected_version {
            return Err(format!(
                "outbox migration registry gap: expected {expected_version}, found {}",
                migration.version
            ));
        }
        validate_migration_name(&migration.name)?;
        if !names.insert(migration.name.as_str()) {
            return Err(format!(
                "outbox migration name `{}` is duplicated",
                migration.name
            ));
        }
        validate_sha256(&migration.up_sha256, migration.version, "up")?;
        validate_sha256(&migration.down_sha256, migration.version, "down")?;
        validate_sha256(&migration.schema_sha256, migration.version, "schema")?;
        if migration.owned_objects.is_empty() || migration.owned_tables.is_empty() {
            return Err(format!(
                "outbox migration {} must own objects and tables",
                migration.version
            ));
        }
        validate_sorted_unique_names(
            migration.version,
            "object",
            &migration.owned_objects,
            &mut objects,
        )?;
        validate_sorted_unique_names(
            migration.version,
            "table",
            &migration.owned_tables,
            &mut tables,
        )?;
        if migration
            .owned_tables
            .iter()
            .any(|table| !migration.owned_objects.contains(table))
        {
            return Err(format!(
                "outbox migration {} table inventory is not contained in its object inventory",
                migration.version
            ));
        }
    }
    validate_frozen_baseline(&registry.migrations[0])
}

fn validate_frozen_baseline(migration: &RegistryMigration) -> Result<(), String> {
    let objects = migration
        .owned_objects
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let tables = migration
        .owned_tables
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    if migration.version != 1
        || migration.name != "outbox"
        || migration.up_path != FROZEN_UP_PATH
        || migration.down_path != FROZEN_DOWN_PATH
        || migration.up_byte_length != FROZEN_UP_LENGTH
        || migration.down_byte_length != FROZEN_DOWN_LENGTH
        || migration.up_sha256 != FROZEN_UP_SHA256
        || migration.down_sha256 != FROZEN_DOWN_SHA256
        || migration.schema_sha256 != FROZEN_SCHEMA_SHA256
        || objects != FROZEN_OBJECTS
        || tables != FROZEN_TABLES
    {
        return Err("frozen outbox migration 0001 authority changed".to_owned());
    }
    Ok(())
}

fn validate_migration_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(format!("invalid outbox migration name `{name}`"));
    }
    Ok(())
}

fn validate_sorted_unique_names(
    version: u32,
    kind: &str,
    names: &[String],
    aggregate: &mut BTreeSet<String>,
) -> Result<(), String> {
    if names.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(format!(
            "outbox migration {version} {kind} inventory must be sorted and unique"
        ));
    }
    for name in names {
        if !name.starts_with(RESERVED_PREFIX)
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
            || !aggregate.insert(name.clone())
        {
            return Err(format!(
                "outbox migration {version} has invalid or repeated {kind} `{name}`"
            ));
        }
    }
    Ok(())
}

fn validate_declared_file(
    workspace_root: &Path,
    version: u32,
    direction: &str,
    relative: &str,
    expected_length: usize,
    expected_sha256: &str,
) -> Result<(), String> {
    let bytes = read_regular_file(workspace_root, relative)?;
    if bytes.len() != expected_length || sha256_hex(&bytes) != expected_sha256 {
        return Err(format!(
            "outbox migration {version} {direction} bytes do not match the registry"
        ));
    }
    Ok(())
}

fn validate_sha256(value: &str, version: u32, field: &str) -> Result<(), String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!(
            "outbox migration {version} has invalid {field} SHA-256"
        ));
    }
    Ok(())
}

fn discover_migrations(workspace_root: &Path) -> Result<Vec<DiscoveredMigration>, String> {
    let directory = workspace_root.join(MIGRATION_DIRECTORY_RELATIVE);
    let mut entries = fs::read_dir(&directory)
        .map_err(|error| format!("read {}: {error}", directory.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("read outbox migration entry: {error}"))?;
    entries.sort_by_key(fs::DirEntry::file_name);
    let mut pairs = BTreeMap::<(u32, String), (Option<String>, Option<String>)>::new();
    for entry in entries {
        let file_type = entry
            .file_type()
            .map_err(|error| format!("inspect {}: {error}", entry.path().display()))?;
        if !file_type.is_file() || file_type.is_symlink() {
            return Err(format!(
                "outbox migration input must be a regular file: {}",
                entry.path().display()
            ));
        }
        let filename = entry
            .file_name()
            .into_string()
            .map_err(|_| "outbox migration filename is not UTF-8".to_owned())?;
        let (stem, direction) = if let Some(stem) = filename.strip_suffix(".up.sql") {
            (stem, "up")
        } else if let Some(stem) = filename.strip_suffix(".down.sql") {
            (stem, "down")
        } else {
            return Err(format!("unknown outbox migration file `{filename}`"));
        };
        let (version, name) = stem
            .split_once('_')
            .ok_or_else(|| format!("invalid outbox migration filename `{filename}`"))?;
        if version.len() != 4 || !version.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(format!("invalid outbox migration filename `{filename}`"));
        }
        validate_migration_name(name)?;
        let version = version
            .parse::<u32>()
            .map_err(|error| format!("parse outbox migration version: {error}"))?;
        let relative = format!("{MIGRATION_DIRECTORY_RELATIVE}/{filename}");
        let pair = pairs.entry((version, name.to_owned())).or_default();
        let slot = if direction == "up" {
            &mut pair.0
        } else {
            &mut pair.1
        };
        if slot.replace(relative).is_some() {
            return Err(format!(
                "duplicate outbox migration {version} {direction} file"
            ));
        }
    }
    pairs
        .into_iter()
        .map(|((version, name), (up, down))| {
            Ok(DiscoveredMigration {
                version,
                name,
                up_relative: up
                    .ok_or_else(|| format!("outbox migration {version} is missing up SQL"))?,
                down_relative: down
                    .ok_or_else(|| format!("outbox migration {version} is missing down SQL"))?,
            })
        })
        .collect()
}

fn load_and_validate_feature_matrix(workspace_root: &Path) -> Result<FeatureMatrix, String> {
    let bytes = read_regular_file(workspace_root, FEATURE_MATRIX_RELATIVE)?;
    let source = std::str::from_utf8(&bytes)
        .map_err(|error| format!("decode {FEATURE_MATRIX_RELATIVE}: {error}"))?;
    let matrix: FeatureMatrix = toml::from_str(source)
        .map_err(|error| format!("parse {FEATURE_MATRIX_RELATIVE}: {error}"))?;
    if matrix.schema_version != 1 || matrix.package != "radroots_outbox" {
        return Err("outbox feature matrix identity must remain v1".to_owned());
    }
    let expected_profiles = [
        (
            "no-default",
            ["--no-default-features", "--all-targets"].as_slice(),
        ),
        (
            "sqlite",
            [
                "--no-default-features",
                "--features",
                "sqlite",
                "--all-targets",
            ]
            .as_slice(),
        ),
        (
            "sqlite-runtime-tokio",
            [
                "--no-default-features",
                "--features",
                "sqlite,runtime-tokio",
                "--all-targets",
            ]
            .as_slice(),
        ),
        (
            "event-store-adapter",
            [
                "--no-default-features",
                "--features",
                "event-store-adapter",
                "--all-targets",
            ]
            .as_slice(),
        ),
        (
            "all-features",
            ["--all-features", "--all-targets"].as_slice(),
        ),
    ];
    if matrix.profiles.len() != expected_profiles.len() {
        return Err("outbox feature matrix must declare exactly five profiles".to_owned());
    }
    for (profile, (expected_id, expected_args)) in matrix.profiles.iter().zip(expected_profiles) {
        if profile.id != expected_id
            || profile
                .cargo_args
                .iter()
                .map(String::as_str)
                .ne(expected_args.iter().copied())
        {
            return Err(format!(
                "outbox feature profile `{}` does not match governed arguments",
                profile.id
            ));
        }
    }

    let cargo_bytes = read_regular_file(workspace_root, OUTBOX_CARGO_RELATIVE)?;
    let cargo_source = std::str::from_utf8(&cargo_bytes)
        .map_err(|error| format!("decode {OUTBOX_CARGO_RELATIVE}: {error}"))?;
    let cargo: toml::Value = toml::from_str(cargo_source)
        .map_err(|error| format!("parse {OUTBOX_CARGO_RELATIVE}: {error}"))?;
    let cargo_features = cargo
        .get("features")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| "outbox Cargo manifest is missing [features]".to_owned())?;
    let mut actual_edges = BTreeMap::new();
    for (feature, value) in cargo_features {
        let edges = value
            .as_array()
            .ok_or_else(|| format!("outbox Cargo feature `{feature}` must be an array"))?
            .iter()
            .map(|edge| {
                edge.as_str().map(str::to_owned).ok_or_else(|| {
                    format!("outbox Cargo feature `{feature}` has a non-string edge")
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        actual_edges.insert(feature.clone(), edges);
    }
    if actual_edges != matrix.feature_edges {
        return Err(
            "outbox Cargo feature graph contains an undeclared, missing, or reordered edge"
                .to_owned(),
        );
    }
    Ok(matrix)
}

fn validate_vector(workspace_root: &Path) -> Result<(), String> {
    let bytes = read_regular_file(workspace_root, VECTOR_RELATIVE)?;
    let vector: Vector = serde_json::from_slice(&bytes)
        .map_err(|error| format!("parse {VECTOR_RELATIVE}: {error}"))?;
    if vector.schema_version != 1
        || vector.contract_id != CONTRACT_ID
        || vector.executor.id != VECTOR_EXECUTOR_ID
        || vector.executor.path != VECTOR_EXECUTOR_RELATIVE
        || vector.executor.test != VECTOR_EXECUTOR_TEST
        || vector.delegated_suite.lane != "nix run .#contract"
        || vector.delegated_suite.package != "radroots_outbox"
    {
        return Err("outbox migration vector identity is invalid".to_owned());
    }
    let expected_cases = BTreeSet::from([
        "fresh_initialization",
        "exact_unledgered_adoption",
        "partial_unledgered_rejected",
        "ledger_checksum_tamper_rejected",
        "newer_history_rejected",
        "caller_state_preserved",
        "current_reopen_no_history_write",
    ]);
    let mut actual_cases = BTreeSet::new();
    for case in &vector.cases {
        if case.execution != "direct_executor"
            || case.expected_outcome.is_empty()
            || !actual_cases.insert(case.id.as_str())
            || case.expected_error.is_some() != case.expected_outcome.starts_with("rejected")
        {
            return Err(format!(
                "invalid outbox migration vector case `{}`",
                case.id
            ));
        }
    }
    if actual_cases != expected_cases {
        return Err("outbox migration vector case inventory is incomplete".to_owned());
    }
    let mut authorities = BTreeSet::new();
    for authority in &vector.delegated_suite.authorities {
        if authority.authority.is_empty() || !authorities.insert(authority.authority.as_str()) {
            return Err("outbox migration delegated authorities must be unique".to_owned());
        }
        let source = read_regular_file(workspace_root, &authority.authority_path)?;
        let source = std::str::from_utf8(&source)
            .map_err(|error| format!("decode {}: {error}", authority.authority_path))?;
        if !source.contains(&authority.authority) {
            return Err(format!(
                "outbox migration delegated authority `{}` is not present in {}",
                authority.authority, authority.authority_path
            ));
        }
    }
    if authorities.len() != 12 {
        return Err("outbox migration delegated authority inventory is incomplete".to_owned());
    }
    Ok(())
}

fn validate_release_authority(workspace_root: &Path) -> Result<(), String> {
    let release_bytes = read_regular_file(workspace_root, RELEASE_RELATIVE)?;
    let release_source = std::str::from_utf8(&release_bytes)
        .map_err(|error| format!("decode {RELEASE_RELATIVE}: {error}"))?;
    let release: toml::Value = toml::from_str(release_source)
        .map_err(|error| format!("parse {RELEASE_RELATIVE}: {error}"))?;
    let changes = release
        .get("changes")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("{RELEASE_RELATIVE} must define changes"))?;
    let matching = changes
        .iter()
        .filter(|change| change.get("id").and_then(toml::Value::as_str) == Some(RELEASE_CHANGE_ID))
        .collect::<Vec<_>>();
    let [change] = matching.as_slice() else {
        return Err(format!(
            "{RELEASE_RELATIVE} must define exactly one `{RELEASE_CHANGE_ID}` change"
        ));
    };
    let impacts = change
        .get("semver_impacts")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("release change `{RELEASE_CHANGE_ID}` has no semver impacts"))?
        .iter()
        .map(|impact| {
            impact
                .as_str()
                .ok_or_else(|| "release semver impacts must be strings".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let expected_impacts = [
        "add_exported_type",
        "add_exported_function",
        "add_exported_constant",
        "add_enum_variant",
        "add_conformance_vector",
        "remove_exported_constant",
        "remove_exported_function",
        "change_exported_algorithm_behavior",
    ];
    if change.get("classification").and_then(toml::Value::as_str) != Some("breaking")
        || impacts != expected_impacts
        || change
            .get("summary")
            .and_then(toml::Value::as_str)
            .is_none_or(str::is_empty)
    {
        return Err(format!(
            "release change `{RELEASE_CHANGE_ID}` has invalid classification, impacts, or summary"
        ));
    }
    let changelog = read_regular_file(workspace_root, CHANGELOG_RELATIVE)?;
    let changelog = std::str::from_utf8(&changelog)
        .map_err(|error| format!("decode {CHANGELOG_RELATIVE}: {error}"))?;
    let marker = format!("<!-- release-change: {RELEASE_CHANGE_ID} -->");
    if changelog.matches(&marker).count() != 1 {
        return Err(format!(
            "{CHANGELOG_RELATIVE} must contain exactly one `{marker}`"
        ));
    }
    Ok(())
}

fn generated_runtime_registry(registry: &Registry) -> Result<String, String> {
    let mut generated = String::from(
        "// @generated by `cargo xtask contract outbox-migration-manifest --write`; do not edit.\n\nuse crate::migrations::OutboxMigration;\n\n",
    );
    for migration in &registry.migrations {
        let up_filename = migration_filename(&migration.up_path)?;
        let down_filename = migration_filename(&migration.down_path)?;
        generated.push_str(&format!(
            "const OUTBOX_MIGRATION_{:04}: OutboxMigration = OutboxMigration {{\n",
            migration.version
        ));
        generated.push_str(&format!("    version: {},\n", migration.version));
        generated.push_str(&format!("    name: {:?},\n", migration.name));
        generated.push_str(&format!(
            "    up_sql: include_str!(\"../../migrations/{up_filename}\"),\n"
        ));
        generated.push_str(&format!(
            "    down_sql: include_str!(\"../../migrations/{down_filename}\"),\n"
        ));
        generated.push_str(&format!(
            "    up_len: {},\n    down_len: {},\n",
            migration.up_byte_length, migration.down_byte_length
        ));
        generated.push_str(&format!(
            "    up_sha256: {:?},\n    down_sha256: {:?},\n    schema_sha256: {:?},\n",
            migration.up_sha256, migration.down_sha256, migration.schema_sha256
        ));
        generated.push_str("    owned_object_names: &[\n");
        for name in &migration.owned_objects {
            generated.push_str(&format!("        {name:?},\n"));
        }
        generated.push_str("    ],\n    owned_table_names: &[\n");
        for name in &migration.owned_tables {
            generated.push_str(&format!("        {name:?},\n"));
        }
        generated.push_str("    ],\n};\n\n");
    }
    if registry.migrations.len() == 1 {
        generated.push_str(&format!(
            "pub(crate) const OUTBOX_MIGRATIONS: &[OutboxMigration] = &[OUTBOX_MIGRATION_{:04}];\n",
            registry.migrations[0].version
        ));
    } else {
        generated.push_str("pub(crate) const OUTBOX_MIGRATIONS: &[OutboxMigration] = &[\n");
        for migration in &registry.migrations {
            generated.push_str(&format!("    OUTBOX_MIGRATION_{:04},\n", migration.version));
        }
        generated.push_str("];\n");
    }
    Ok(generated)
}

fn migration_filename(relative: &str) -> Result<&str, String> {
    let prefix = format!("{MIGRATION_DIRECTORY_RELATIVE}/");
    let filename = relative
        .strip_prefix(&prefix)
        .ok_or_else(|| format!("migration path `{relative}` is outside the governed directory"))?;
    if filename.is_empty() || filename.contains('/') || filename.contains('\\') {
        return Err(format!("invalid migration path `{relative}`"));
    }
    Ok(filename)
}

fn expected_manifest(
    workspace_root: &Path,
    registry: &Registry,
    matrix: &FeatureMatrix,
    schema_bytes: &[u8],
    generated_runtime: &[u8],
) -> Result<Value, String> {
    let minimum = registry
        .migrations
        .first()
        .ok_or_else(|| "outbox migration registry must not be empty".to_owned())?
        .version;
    let current = registry
        .migrations
        .last()
        .ok_or_else(|| "outbox migration registry must not be empty".to_owned())?
        .version;
    let migrations = registry
        .migrations
        .iter()
        .map(|migration| {
            Ok(json!({
                "version": migration.version,
                "name": migration.name,
                "up": descriptor_for_file(workspace_root, &migration.up_path)?,
                "down": descriptor_for_file(workspace_root, &migration.down_path)?,
                "schema_sha256": migration.schema_sha256,
                "owned_objects": migration.owned_objects,
                "owned_tables": migration.owned_tables,
            }))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let feature_edges = matrix
        .feature_edges
        .iter()
        .map(|(feature, enables)| json!({ "feature": feature, "enables": enables }))
        .collect::<Vec<_>>();
    let profiles = matrix
        .profiles
        .iter()
        .map(|profile| json!({ "id": profile.id, "cargo_args": profile.cargo_args }))
        .collect::<Vec<_>>();
    let sources = SOURCE_FILES
        .iter()
        .map(|(role, relative)| {
            Ok(json!({
                "role": role,
                "file": descriptor_for_file(workspace_root, relative)?,
            }))
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(json!({
        "schema_version": 1,
        "contract_id": CONTRACT_ID,
        "authority_id": AUTHORITY_ID,
        "manifest_schema": descriptor_for_bytes(MANIFEST_SCHEMA_RELATIVE, schema_bytes),
        "registry_source": descriptor_for_file(workspace_root, REGISTRY_RELATIVE)?,
        "version_bounds": {
            "minimum": minimum,
            "current": current,
            "derivation": "first_and_last_ordered_registry_entries_v1",
        },
        "ledger": {
            "name": LEDGER_NAME,
            "reserved_prefix": RESERVED_PREFIX,
            "catalog_fingerprint": "sha256_type_nul_name_nul_table_name_nul_sql_nul_sorted_v1",
            "migration_transaction": "begin_immediate_v1",
            "rollback_transaction": "begin_exclusive_test_executor_v1",
            "adoption": "exact_unledgered_0001_catalog_only_v1",
        },
        "migrations": migrations,
        "feature_matrix": {
            "source": descriptor_for_file(workspace_root, FEATURE_MATRIX_RELATIVE)?,
            "package": matrix.package,
            "feature_edges": feature_edges,
            "profiles": profiles,
        },
        "generated_runtime": descriptor_for_bytes(GENERATED_RUNTIME_RELATIVE, generated_runtime),
        "result_vector": {
            "canonical": descriptor_for_file(workspace_root, VECTOR_RELATIVE)?,
            "mirror_path": VECTOR_MIRROR_RELATIVE,
            "executor": descriptor_for_file(workspace_root, VECTOR_EXECUTOR_RELATIVE)?,
            "executor_id": VECTOR_EXECUTOR_ID,
            "executor_test": VECTOR_EXECUTOR_TEST,
        },
        "source_files": sources,
        "release": {
            "change_id": RELEASE_CHANGE_ID,
            "record": RELEASE_RELATIVE,
            "changelog": CHANGELOG_RELATIVE,
        },
    }))
}

fn manifest_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://radroots.org/contracts/outbox/migration_authority_v1.manifest.schema.json",
        "type": "object",
        "additionalProperties": false,
        "required": [
            "schema_version", "contract_id", "authority_id", "manifest_schema",
            "registry_source", "version_bounds", "ledger", "migrations",
            "feature_matrix", "generated_runtime", "result_vector", "source_files", "release"
        ],
        "properties": {
            "schema_version": { "const": 1 },
            "contract_id": { "const": CONTRACT_ID },
            "authority_id": { "const": AUTHORITY_ID },
            "manifest_schema": { "$ref": "#/$defs/file" },
            "registry_source": { "$ref": "#/$defs/file" },
            "version_bounds": {
                "type": "object",
                "additionalProperties": false,
                "required": ["minimum", "current", "derivation"],
                "properties": {
                    "minimum": { "type": "integer", "minimum": 1 },
                    "current": { "type": "integer", "minimum": 1 },
                    "derivation": { "const": "first_and_last_ordered_registry_entries_v1" }
                }
            },
            "ledger": {
                "type": "object",
                "additionalProperties": false,
                "required": [
                    "name", "reserved_prefix", "catalog_fingerprint", "migration_transaction",
                    "rollback_transaction", "adoption"
                ],
                "properties": {
                    "name": { "const": LEDGER_NAME },
                    "reserved_prefix": { "const": RESERVED_PREFIX },
                    "catalog_fingerprint": { "const": "sha256_type_nul_name_nul_table_name_nul_sql_nul_sorted_v1" },
                    "migration_transaction": { "const": "begin_immediate_v1" },
                    "rollback_transaction": { "const": "begin_exclusive_test_executor_v1" },
                    "adoption": { "const": "exact_unledgered_0001_catalog_only_v1" }
                }
            },
            "migrations": {
                "type": "array",
                "minItems": 1,
                "maxItems": 9999,
                "items": { "$ref": "#/$defs/migration" }
            },
            "feature_matrix": {
                "type": "object",
                "additionalProperties": false,
                "required": ["source", "package", "feature_edges", "profiles"],
                "properties": {
                    "source": { "$ref": "#/$defs/file" },
                    "package": { "const": "radroots_outbox" },
                    "feature_edges": {
                        "type": "array", "minItems": 1, "uniqueItems": true,
                        "items": { "$ref": "#/$defs/feature_edge" }
                    },
                    "profiles": {
                        "type": "array", "minItems": 5, "maxItems": 5,
                        "items": { "$ref": "#/$defs/profile" }
                    }
                }
            },
            "generated_runtime": { "$ref": "#/$defs/file" },
            "result_vector": {
                "type": "object",
                "additionalProperties": false,
                "required": ["canonical", "mirror_path", "executor", "executor_id", "executor_test"],
                "properties": {
                    "canonical": { "$ref": "#/$defs/file" },
                    "mirror_path": { "const": VECTOR_MIRROR_RELATIVE },
                    "executor": { "$ref": "#/$defs/file" },
                    "executor_id": { "const": VECTOR_EXECUTOR_ID },
                    "executor_test": { "const": VECTOR_EXECUTOR_TEST }
                }
            },
            "source_files": {
                "type": "array", "minItems": 16, "maxItems": 16,
                "items": { "$ref": "#/$defs/source_file" }
            },
            "release": {
                "type": "object",
                "additionalProperties": false,
                "required": ["change_id", "record", "changelog"],
                "properties": {
                    "change_id": { "const": RELEASE_CHANGE_ID },
                    "record": { "const": RELEASE_RELATIVE },
                    "changelog": { "const": CHANGELOG_RELATIVE }
                }
            }
        },
        "$defs": {
            "sha256": { "type": "string", "pattern": "^[0-9a-f]{64}$" },
            "file": {
                "type": "object",
                "additionalProperties": false,
                "required": ["path", "byte_length", "sha256", "hash_algorithm"],
                "properties": {
                    "path": { "type": "string", "minLength": 1 },
                    "byte_length": { "type": "integer", "minimum": 1 },
                    "sha256": { "$ref": "#/$defs/sha256" },
                    "hash_algorithm": { "const": HASH_ALGORITHM }
                }
            },
            "migration": {
                "type": "object",
                "additionalProperties": false,
                "required": ["version", "name", "up", "down", "schema_sha256", "owned_objects", "owned_tables"],
                "properties": {
                    "version": { "type": "integer", "minimum": 1, "maximum": 9999 },
                    "name": { "type": "string", "pattern": "^[a-z0-9_]+$" },
                    "up": { "$ref": "#/$defs/file" },
                    "down": { "$ref": "#/$defs/file" },
                    "schema_sha256": { "$ref": "#/$defs/sha256" },
                    "owned_objects": {
                        "type": "array", "minItems": 1, "uniqueItems": true,
                        "items": { "type": "string", "pattern": "^outbox_[a-z0-9_]+$" }
                    },
                    "owned_tables": {
                        "type": "array", "minItems": 1, "uniqueItems": true,
                        "items": { "type": "string", "pattern": "^outbox_[a-z0-9_]+$" }
                    }
                }
            },
            "feature_edge": {
                "type": "object",
                "additionalProperties": false,
                "required": ["feature", "enables"],
                "properties": {
                    "feature": { "type": "string", "minLength": 1 },
                    "enables": {
                        "type": "array", "uniqueItems": true,
                        "items": { "type": "string", "minLength": 1 }
                    }
                }
            },
            "profile": {
                "type": "object",
                "additionalProperties": false,
                "required": ["id", "cargo_args"],
                "properties": {
                    "id": { "type": "string", "minLength": 1 },
                    "cargo_args": {
                        "type": "array", "minItems": 1,
                        "items": { "type": "string", "minLength": 1 }
                    }
                }
            },
            "source_file": {
                "type": "object",
                "additionalProperties": false,
                "required": ["role", "file"],
                "properties": {
                    "role": { "type": "string", "minLength": 1 },
                    "file": { "$ref": "#/$defs/file" }
                }
            }
        }
    })
}

fn descriptor_for_file(workspace_root: &Path, relative: &str) -> Result<Value, String> {
    let bytes = read_regular_file(workspace_root, relative)?;
    Ok(descriptor_for_bytes(relative, &bytes))
}

fn descriptor_for_bytes(relative: &str, bytes: &[u8]) -> Value {
    json!({
        "path": relative,
        "byte_length": bytes.len(),
        "sha256": sha256_hex(bytes),
        "hash_algorithm": HASH_ALGORITHM,
    })
}

fn canonical_json_bytes(value: &Value) -> Result<Vec<u8>, String> {
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("serialize outbox migration artifact: {error}"))?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("xtask workspace root")
            .to_path_buf()
    }

    #[test]
    fn outbox_registry_accepts_an_appended_successor_and_rejects_a_gap() {
        let mut registry = load_and_validate_registry(&workspace_root()).expect("live registry");
        let mut successor = registry.migrations[0].clone();
        successor.version = 2;
        successor.name = "future".to_owned();
        successor.up_path = "crates/outbox/migrations/0002_future.up.sql".to_owned();
        successor.down_path = "crates/outbox/migrations/0002_future.down.sql".to_owned();
        successor.owned_objects = vec!["outbox_future".to_owned()];
        successor.owned_tables = vec!["outbox_future".to_owned()];
        registry.migrations.push(successor);
        validate_registry_shape(&registry).expect("contiguous successor");
        registry.migrations[1].version = 3;
        assert!(
            validate_registry_shape(&registry)
                .expect_err("gap")
                .contains("expected 2")
        );
    }

    #[test]
    fn outbox_registry_rejects_any_frozen_baseline_mutation() {
        let mut registry = load_and_validate_registry(&workspace_root()).expect("live registry");
        registry.migrations[0].up_byte_length += 1;
        assert_eq!(
            validate_registry_shape(&registry).expect_err("frozen mutation"),
            "frozen outbox migration 0001 authority changed"
        );
    }

    #[test]
    fn outbox_feature_matrix_matches_declared_edges() {
        let matrix = load_and_validate_feature_matrix(&workspace_root()).expect("feature matrix");
        assert_eq!(matrix.profiles.len(), 5);
        assert_eq!(matrix.feature_edges.len(), 4);
    }

    #[test]
    fn outbox_manifest_schema_closes_every_object() {
        fn visit(value: &Value) {
            if value.get("type").and_then(Value::as_str) == Some("object") {
                assert_eq!(
                    value.get("additionalProperties"),
                    Some(&Value::Bool(false)),
                    "object schema must be closed: {value}"
                );
            }
            match value {
                Value::Array(values) => values.iter().for_each(visit),
                Value::Object(values) => values.values().for_each(visit),
                _ => {}
            }
        }
        visit(&manifest_schema());
    }

    #[test]
    fn outbox_generated_runtime_derives_every_registry_entry() {
        let registry = load_and_validate_registry(&workspace_root()).expect("live registry");
        let generated = generated_runtime_registry(&registry).expect("generated runtime");
        for migration in &registry.migrations {
            assert!(generated.contains(&format!("version: {}", migration.version)));
            assert!(generated.contains(&migration.up_sha256));
            assert!(generated.contains(&migration.down_sha256));
            assert!(generated.contains(&migration.schema_sha256));
        }
    }
}
