use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path},
};

const INVENTORY_RELATIVE: &str = "contracts/semantic_validator_inventory.toml";
const EXPECTED_VALIDATOR_IDS: [&str; 10] = [
    "artifact_transactions",
    "event_store_successors",
    "feature_support",
    "immutable_predecessors",
    "operations_and_boundaries",
    "outbox_successors",
    "phase1_media_successors",
    "registry_v7",
    "release_closure",
    "validator_inventory",
];
const EXPECTED_IMPLEMENTATIONS: [&str; 21] = [
    "tools/xtask/src/contract.rs",
    "tools/xtask/src/contract/admission_authority.rs",
    "tools/xtask/src/contract/artifact_bundle.rs",
    "tools/xtask/src/contract/blossom_publication_readiness.rs",
    "tools/xtask/src/contract/blossom_raster_decoder_security.rs",
    "tools/xtask/src/contract/comment_authority.rs",
    "tools/xtask/src/contract/deletion_authority.rs",
    "tools/xtask/src/contract/feature_support.rs",
    "tools/xtask/src/contract/food_availability_projection.rs",
    "tools/xtask/src/contract/nip09_reconciliation.rs",
    "tools/xtask/src/contract/outbox_migration.rs",
    "tools/xtask/src/contract/outbox_phase1_publication.rs",
    "tools/xtask/src/contract/phase1_publication_allowlist.rs",
    "tools/xtask/src/contract/phase1_publication_artifact.rs",
    "tools/xtask/src/contract/phase1_publication_media_readiness.rs",
    "tools/xtask/src/contract/raw_source_rebuild.rs",
    "tools/xtask/src/contract/registry_v7.rs",
    "tools/xtask/src/contract/release_package.rs",
    "tools/xtask/src/contract/release_provenance.rs",
    "tools/xtask/src/contract/source_maintenance.rs",
    "tools/xtask/src/contract/validator_inventory.rs",
];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ValidatorInventory {
    schema_version: u32,
    validators: Vec<ValidatorEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ValidatorEntry {
    id: String,
    semantic: bool,
    parsers: Vec<ParserKind>,
    implementation_paths: Vec<String>,
    governed_inputs: Vec<String>,
    #[serde(default)]
    byte_hash_scopes: Vec<ByteHashScope>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "snake_case")]
enum ParserKind {
    CargoMetadata,
    ExecutableVector,
    GitObject,
    GovernedBytes,
    Json,
    JsonSchema,
    MarkdownTable,
    RustAst,
    SqliteCatalog,
    TarArchive,
    Toml,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "snake_case")]
enum ByteHashScope {
    CanonicalVectorsAndMirrors,
    GeneratedContractArtifacts,
    ImmutablePredecessorArtifacts,
    PackageArchives,
    RawFuzzSeeds,
    ReleaseProvenanceArtifacts,
    SqlMigrations,
}

pub(super) fn validate_semantic_validator_inventory(workspace_root: &Path) -> Result<(), String> {
    let path = workspace_root.join(INVENTORY_RELATIVE);
    let source =
        fs::read_to_string(&path).map_err(|error| format!("read {INVENTORY_RELATIVE}: {error}"))?;
    let inventory = toml::from_str::<ValidatorInventory>(&source)
        .map_err(|error| format!("parse {INVENTORY_RELATIVE}: {error}"))?;
    if inventory.schema_version != 1 {
        return Err(format!("{INVENTORY_RELATIVE} schema_version must be 1"));
    }

    let expected_ids = EXPECTED_VALIDATOR_IDS.into_iter().collect::<BTreeSet<_>>();
    let mut entries = BTreeMap::new();
    let mut previous_id = None::<&str>;
    for entry in &inventory.validators {
        if previous_id.is_some_and(|previous| previous >= entry.id.as_str()) {
            return Err(format!(
                "{INVENTORY_RELATIVE} validator ids must be strictly sorted and unique"
            ));
        }
        previous_id = Some(entry.id.as_str());
        if entries.insert(entry.id.as_str(), entry).is_some() {
            return Err(format!("{INVENTORY_RELATIVE} duplicates {}", entry.id));
        }
    }
    if entries.keys().copied().collect::<BTreeSet<_>>() != expected_ids {
        return Err(format!(
            "{INVENTORY_RELATIVE} validator id inventory is incomplete"
        ));
    }

    let mut implementations = BTreeSet::new();
    for entry in entries.values() {
        validate_entry(workspace_root, entry)?;
        for relative in &entry.implementation_paths {
            if !implementations.insert(relative.as_str()) {
                return Err(format!(
                    "{INVENTORY_RELATIVE} implementation {relative} has multiple owners"
                ));
            }
        }
    }
    let expected_implementations = EXPECTED_IMPLEMENTATIONS
        .into_iter()
        .collect::<BTreeSet<_>>();
    if implementations != expected_implementations {
        let missing = expected_implementations
            .difference(&implementations)
            .copied()
            .collect::<Vec<_>>();
        let unexpected = implementations
            .difference(&expected_implementations)
            .copied()
            .collect::<Vec<_>>();
        return Err(format!(
            "{INVENTORY_RELATIVE} implementation inventory drifted; missing {missing:?}, unexpected {unexpected:?}"
        ));
    }
    super::raw_source_rebuild::validate_event_store_production_source_authority(workspace_root)?;
    Ok(())
}

fn validate_entry(workspace_root: &Path, entry: &ValidatorEntry) -> Result<(), String> {
    if entry.id.trim().is_empty()
        || entry.parsers.is_empty()
        || entry.implementation_paths.is_empty()
        || entry.governed_inputs.is_empty()
    {
        return Err(format!(
            "{INVENTORY_RELATIVE} validator {} has an empty authority field",
            entry.id
        ));
    }
    validate_sorted_unique(&entry.id, "parser", &entry.parsers)?;
    validate_sorted_unique(
        &entry.id,
        "implementation path",
        &entry.implementation_paths,
    )?;
    validate_sorted_unique(&entry.id, "governed input", &entry.governed_inputs)?;
    validate_sorted_unique(&entry.id, "byte hash scope", &entry.byte_hash_scopes)?;
    if entry.semantic && entry.parsers == [ParserKind::GovernedBytes] {
        return Err(format!(
            "{INVENTORY_RELATIVE} semantic validator {} cannot use byte identity as semantic authority",
            entry.id
        ));
    }
    if !entry.byte_hash_scopes.is_empty() && !entry.parsers.contains(&ParserKind::GovernedBytes) {
        return Err(format!(
            "{INVENTORY_RELATIVE} validator {} declares byte hash scope without governed_bytes parsing",
            entry.id
        ));
    }
    for relative in entry
        .implementation_paths
        .iter()
        .chain(entry.governed_inputs.iter())
    {
        validate_relative_path(relative)?;
        if !workspace_root.join(relative).exists() {
            return Err(format!(
                "{INVENTORY_RELATIVE} validator {} references missing path {relative}",
                entry.id
            ));
        }
    }
    for relative in &entry.implementation_paths {
        if !relative.starts_with("tools/xtask/src/contract") || !relative.ends_with(".rs") {
            return Err(format!(
                "{INVENTORY_RELATIVE} validator {} has invalid implementation {relative}",
                entry.id
            ));
        }
        let source = fs::read_to_string(workspace_root.join(relative))
            .map_err(|error| format!("read validator implementation {relative}: {error}"))?;
        syn::parse_file(&source)
            .map_err(|error| format!("parse validator implementation {relative}: {error}"))?;
    }
    Ok(())
}

fn validate_relative_path(relative: &str) -> Result<(), String> {
    let path = Path::new(relative);
    if relative.is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(format!(
            "{INVENTORY_RELATIVE} contains invalid relative path {relative}"
        ));
    }
    Ok(())
}

fn validate_sorted_unique<T: Ord>(
    validator: &str,
    label: &str,
    values: &[T],
) -> Result<(), String> {
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(format!(
            "{INVENTORY_RELATIVE} validator {validator} {label}s must be strictly sorted and unique"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("xtask workspace root")
            .to_path_buf()
    }

    #[test]
    fn semantic_validator_inventory_is_complete_and_structured() {
        validate_semantic_validator_inventory(&workspace_root()).expect("validator inventory");
    }

    #[test]
    fn semantic_validator_cannot_delegate_meaning_to_byte_identity() {
        let entry = ValidatorEntry {
            id: "synthetic".to_owned(),
            semantic: true,
            parsers: vec![ParserKind::GovernedBytes],
            implementation_paths: vec![
                "tools/xtask/src/contract/validator_inventory.rs".to_owned(),
            ],
            governed_inputs: vec!["contracts/semantic_validator_inventory.toml".to_owned()],
            byte_hash_scopes: vec![ByteHashScope::GeneratedContractArtifacts],
        };
        assert!(
            validate_entry(&workspace_root(), &entry)
                .expect_err("semantic byte-only authority must fail")
                .contains("cannot use byte identity as semantic authority")
        );
    }
}
