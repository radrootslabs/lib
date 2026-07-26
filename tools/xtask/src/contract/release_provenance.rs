use super::artifact_bundle::{
    GeneratedArtifact, read_regular_file, with_artifact_bundle_transaction,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

const SCHEMA_VERSION: u32 = 1;
const CONTRACT_ID: &str = "radroots.release.phase1_publication_provenance.v1";
const AUTHORITY: &str = "candidate_release_evidence_not_protocol_authority_v1";
const HASH_ALGORITHM: &str = "sha256_bytes_v1";
const SCHEMA_RELATIVE: &str =
    "contracts/releases/provenance/phase1_publication_release_provenance_v1.schema.json";
const SEMANTIC_CONTRACT_RELATIVE: &str =
    "crates/event_codec/contracts/phase1_publication_allowlist_v1.manifest.json";
const TOOLCHAIN_RELATIVE: &str = "rust-toolchain.toml";
const LOCKFILE_RELATIVE: &str = "Cargo.lock";
const FEATURE_PROFILES_RELATIVE: &str = "contracts/coverage-profiles.toml";
const PUBLISH_POLICY_RELATIVE: &str = "contracts/releases/publish_policy.toml";
const WRITE_SCHEMA_COMMAND: &str = "cargo xtask contract release-provenance-schema --write";
const METADATA_COMMAND: &str = "cargo metadata --locked --format-version 1";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct FileDescriptor {
    path: String,
    byte_length: u64,
    sha256: String,
    hash_algorithm: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct GitCandidate {
    commit_oid: String,
    tree_oid: String,
    object_format: String,
    clean_worktree: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ToolchainEvidence {
    channel: String,
    components: Vec<String>,
    targets: Vec<String>,
    contract: FileDescriptor,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct DependencyGraphEvidence {
    command: String,
    normalization: String,
    package_count: u64,
    node_count: u64,
    normalized_byte_length: u64,
    normalized_sha256: String,
    hash_algorithm: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct FeatureProfile {
    package: String,
    no_default_features: bool,
    features: Vec<String>,
    test_threads: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct FeatureProfileEvidence {
    contract: FileDescriptor,
    selected: Vec<FeatureProfile>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PackageArchive {
    package: String,
    version: String,
    filename: String,
    byte_length: u64,
    sha256: String,
    hash_algorithm: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PackageEvidence {
    release_version: String,
    publish_policy: FileDescriptor,
    archives: Vec<PackageArchive>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CommandEvidence {
    purpose: String,
    command: String,
    required_result: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ReleaseProvenanceManifest {
    schema_version: u32,
    contract_id: String,
    authority: String,
    manifest_schema: FileDescriptor,
    semantic_contract: FileDescriptor,
    candidate: GitCandidate,
    source_digest: GitTreeDigest,
    toolchain: ToolchainEvidence,
    lockfile: FileDescriptor,
    dependency_graph: DependencyGraphEvidence,
    feature_profiles: FeatureProfileEvidence,
    packages: PackageEvidence,
    commands: Vec<CommandEvidence>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct GitTreeDigest {
    algorithm: String,
    oid: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RustToolchainFile {
    toolchain: RustToolchain,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RustToolchain {
    channel: String,
    #[serde(default)]
    components: Vec<String>,
    #[serde(default)]
    targets: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TestProfile {
    no_default_features: bool,
    features: Vec<String>,
    test_threads: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TestProfilesFile {
    profiles: TestProfiles,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TestProfiles {
    default: TestProfile,
    #[serde(default)]
    crates: BTreeMap<String, TestProfile>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PublishPolicy {
    release: ReleaseVersion,
    classification: ReleaseClassification,
    publish_order: PublishOrder,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleaseVersion {
    version: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleaseClassification {
    public: Vec<String>,
    internal: Vec<String>,
    deferred: Vec<String>,
    retired: Vec<String>,
    yank_only: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PublishOrder {
    crates: Vec<String>,
}

pub(crate) fn write_release_provenance_schema(workspace_root: &Path) -> Result<(), String> {
    with_artifact_bundle_transaction(workspace_root, |transaction| {
        transaction.write(vec![GeneratedArtifact {
            relative: SCHEMA_RELATIVE,
            contents: canonical_json_bytes(&release_provenance_schema())?,
        }])?;
        validate_release_provenance_schema_under_lock(workspace_root)
    })
}

pub(crate) fn validate_release_provenance_schema(workspace_root: &Path) -> Result<(), String> {
    with_artifact_bundle_transaction(workspace_root, |_| {
        validate_release_provenance_schema_under_lock(workspace_root)
    })
}

fn validate_release_provenance_schema_under_lock(workspace_root: &Path) -> Result<(), String> {
    let expected = canonical_json_bytes(&release_provenance_schema())?;
    let actual = read_regular_file(workspace_root, SCHEMA_RELATIVE)?;
    if actual != expected {
        return Err(format!(
            "generated release provenance schema {SCHEMA_RELATIVE} is stale; run {WRITE_SCHEMA_COMMAND}"
        ));
    }
    let schema: Value = serde_json::from_slice(&actual)
        .map_err(|error| format!("parse {SCHEMA_RELATIVE}: {error}"))?;
    jsonschema::validator_for(&schema)
        .map_err(|error| format!("compile {SCHEMA_RELATIVE}: {error}"))?;
    Ok(())
}

pub(crate) fn write_release_provenance(
    workspace_root: &Path,
    package_directory: &Path,
    output: &Path,
) -> Result<(), String> {
    validate_release_provenance_schema(workspace_root)?;
    require_output_outside_workspace(workspace_root, output)?;
    let manifest = describe_release_provenance(workspace_root, package_directory)?;
    validate_manifest(&manifest)?;
    write_external_atomic(output, &canonical_json_bytes(&manifest)?)
}

fn describe_release_provenance(
    workspace_root: &Path,
    package_directory: &Path,
) -> Result<ReleaseProvenanceManifest, String> {
    ensure_clean_git_worktree(workspace_root)?;
    let commit_oid = git_stdout(workspace_root, &["rev-parse", "HEAD"])?;
    let tree_oid = git_stdout(workspace_root, &["rev-parse", "HEAD^{tree}"])?;
    let git_object_format = object_format(&commit_oid)?;
    if object_format(&tree_oid)? != git_object_format {
        return Err("Git commit and tree object formats differ".to_owned());
    }

    let schema = descriptor_for_file(workspace_root, SCHEMA_RELATIVE)?;
    let semantic_contract = descriptor_for_file(workspace_root, SEMANTIC_CONTRACT_RELATIVE)?;
    let lockfile = descriptor_for_file(workspace_root, LOCKFILE_RELATIVE)?;
    let toolchain = load_toolchain(workspace_root)?;
    let policy = load_toml::<PublishPolicy>(workspace_root, PUBLISH_POLICY_RELATIVE)?;
    validate_publish_policy(&policy)?;
    let feature_profiles = selected_feature_profiles(workspace_root, &policy)?;
    let dependency_graph = dependency_graph_evidence(workspace_root)?;
    let packages = package_evidence(workspace_root, package_directory, &policy)?;

    Ok(ReleaseProvenanceManifest {
        schema_version: SCHEMA_VERSION,
        contract_id: CONTRACT_ID.to_owned(),
        authority: AUTHORITY.to_owned(),
        manifest_schema: schema,
        semantic_contract,
        candidate: GitCandidate {
            commit_oid: commit_oid.clone(),
            tree_oid: tree_oid.clone(),
            object_format: git_object_format.to_owned(),
            clean_worktree: true,
        },
        source_digest: GitTreeDigest {
            algorithm: format!("git_tree_{git_object_format}_v1"),
            oid: tree_oid,
        },
        toolchain,
        lockfile,
        dependency_graph,
        feature_profiles,
        packages,
        commands: required_commands(),
    })
}

fn validate_manifest(manifest: &ReleaseProvenanceManifest) -> Result<(), String> {
    let commit_format = object_format(&manifest.candidate.commit_oid)?;
    let tree_format = object_format(&manifest.candidate.tree_oid)?;
    if manifest.schema_version != SCHEMA_VERSION
        || manifest.contract_id != CONTRACT_ID
        || manifest.authority != AUTHORITY
        || !manifest.candidate.clean_worktree
        || commit_format != manifest.candidate.object_format
        || tree_format != manifest.candidate.object_format
        || manifest.source_digest.oid != manifest.candidate.tree_oid
        || manifest.source_digest.algorithm
            != format!("git_tree_{}_v1", manifest.candidate.object_format)
        || manifest.manifest_schema.path != SCHEMA_RELATIVE
        || manifest.semantic_contract.path != SEMANTIC_CONTRACT_RELATIVE
        || manifest.toolchain.contract.path != TOOLCHAIN_RELATIVE
        || manifest.lockfile.path != LOCKFILE_RELATIVE
        || manifest.feature_profiles.contract.path != FEATURE_PROFILES_RELATIVE
        || manifest.packages.publish_policy.path != PUBLISH_POLICY_RELATIVE
        || manifest.commands != required_commands()
    {
        return Err("release provenance manifest authority or Git identity drifted".to_owned());
    }
    let profiles = manifest
        .feature_profiles
        .selected
        .iter()
        .map(|profile| profile.package.as_str())
        .collect::<Vec<_>>();
    let archives = manifest
        .packages
        .archives
        .iter()
        .map(|archive| archive.package.as_str())
        .collect::<Vec<_>>();
    if profiles != archives
        || manifest.packages.archives.iter().any(|archive| {
            archive.version != manifest.packages.release_version
                || archive.filename != format!("{}-{}.crate", archive.package, archive.version)
        })
    {
        return Err(
            "release provenance feature profiles and package archives must cover the same ordered public package set"
                .to_owned(),
        );
    }
    let schema = release_provenance_schema();
    let instance = serde_json::to_value(manifest)
        .map_err(|error| format!("serialize release provenance manifest: {error}"))?;
    validate_json_schema(&schema, &instance)
}

fn load_toolchain(workspace_root: &Path) -> Result<ToolchainEvidence, String> {
    let toolchain = load_toml::<RustToolchainFile>(workspace_root, TOOLCHAIN_RELATIVE)?.toolchain;
    require_unique_nonempty(&toolchain.components, "toolchain.components")?;
    require_unique_nonempty(&toolchain.targets, "toolchain.targets")?;
    if toolchain.channel.trim().is_empty() {
        return Err("toolchain.channel must not be empty".to_owned());
    }
    Ok(ToolchainEvidence {
        channel: toolchain.channel,
        components: toolchain.components,
        targets: toolchain.targets,
        contract: descriptor_for_file(workspace_root, TOOLCHAIN_RELATIVE)?,
    })
}

fn selected_feature_profiles(
    workspace_root: &Path,
    policy: &PublishPolicy,
) -> Result<FeatureProfileEvidence, String> {
    let profiles =
        load_toml::<TestProfilesFile>(workspace_root, FEATURE_PROFILES_RELATIVE)?.profiles;
    validate_test_profile("profiles.default", &profiles.default)?;
    for (package, profile) in &profiles.crates {
        validate_test_profile(&format!("profiles.crates.{package}"), profile)?;
    }
    let classified = all_classified_packages(policy);
    let public = policy
        .classification
        .public
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let unknown_overrides = profiles
        .crates
        .keys()
        .filter(|package| !classified.contains(*package))
        .cloned()
        .collect::<Vec<_>>();
    if !unknown_overrides.is_empty() {
        return Err(format!(
            "feature profile overrides reference unknown packages: {}",
            unknown_overrides.join(", ")
        ));
    }
    let selected = policy
        .publish_order
        .crates
        .iter()
        .filter(|package| public.contains(*package))
        .map(|package| {
            let profile = profiles.crates.get(package).unwrap_or(&profiles.default);
            FeatureProfile {
                package: package.clone(),
                no_default_features: profile.no_default_features,
                features: profile.features.clone(),
                test_threads: profile.test_threads,
            }
        })
        .collect();
    Ok(FeatureProfileEvidence {
        contract: descriptor_for_file(workspace_root, FEATURE_PROFILES_RELATIVE)?,
        selected,
    })
}

fn validate_test_profile(label: &str, profile: &TestProfile) -> Result<(), String> {
    if profile.test_threads == 0 {
        return Err(format!("{label}.test_threads must be positive"));
    }
    require_unique_nonempty(&profile.features, &format!("{label}.features"))
}

fn validate_publish_policy(policy: &PublishPolicy) -> Result<(), String> {
    if policy.release.version.trim().is_empty() {
        return Err("release.version must not be empty".to_owned());
    }
    for (label, packages) in [
        ("classification.public", &policy.classification.public),
        ("classification.internal", &policy.classification.internal),
        ("classification.deferred", &policy.classification.deferred),
        ("classification.retired", &policy.classification.retired),
        ("classification.yank_only", &policy.classification.yank_only),
    ] {
        require_unique_nonempty(packages, label)?;
    }
    let all = all_classified_packages(policy);
    let classified_count = policy.classification.public.len()
        + policy.classification.internal.len()
        + policy.classification.deferred.len()
        + policy.classification.retired.len()
        + policy.classification.yank_only.len();
    if all.len() != classified_count {
        return Err("release package classifications must be pairwise unique".to_owned());
    }
    require_unique_nonempty(&policy.publish_order.crates, "publish_order.crates")?;
    let public = policy
        .classification
        .public
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let ordered = policy
        .publish_order
        .crates
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if ordered != public {
        return Err(
            "publish_order.crates must contain every public package exactly once".to_owned(),
        );
    }
    Ok(())
}

fn all_classified_packages(policy: &PublishPolicy) -> BTreeSet<String> {
    policy
        .classification
        .public
        .iter()
        .chain(&policy.classification.internal)
        .chain(&policy.classification.deferred)
        .chain(&policy.classification.retired)
        .chain(&policy.classification.yank_only)
        .cloned()
        .collect()
}

fn package_evidence(
    workspace_root: &Path,
    package_directory: &Path,
    policy: &PublishPolicy,
) -> Result<PackageEvidence, String> {
    let archives = describe_package_archives(package_directory, policy)?;
    Ok(PackageEvidence {
        release_version: policy.release.version.clone(),
        publish_policy: descriptor_for_file(workspace_root, PUBLISH_POLICY_RELATIVE)?,
        archives,
    })
}

fn describe_package_archives(
    package_directory: &Path,
    policy: &PublishPolicy,
) -> Result<Vec<PackageArchive>, String> {
    let expected = policy
        .publish_order
        .crates
        .iter()
        .map(|package| {
            (
                format!("{package}-{}.crate", policy.release.version),
                package,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut found = BTreeMap::new();
    let entries = fs::read_dir(package_directory).map_err(|error| {
        format!(
            "read package archive directory {}: {error}",
            package_directory.display()
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "read package archive entry in {}: {error}",
                package_directory.display()
            )
        })?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("inspect {}: {error}", entry.path().display()))?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "package archive filename must be UTF-8".to_owned())?;
        if !name.ends_with(".crate") {
            continue;
        }
        if !file_type.is_file() || file_type.is_symlink() {
            return Err(format!("package archive {name} must be a regular file"));
        }
        let package = expected
            .get(&name)
            .ok_or_else(|| format!("unexpected package archive {name}"))?;
        let bytes = fs::read(entry.path())
            .map_err(|error| format!("read package archive {name}: {error}"))?;
        if bytes.is_empty() {
            return Err(format!("package archive {name} must not be empty"));
        }
        if found
            .insert(
                (*package).clone(),
                PackageArchive {
                    package: (*package).clone(),
                    version: policy.release.version.clone(),
                    filename: name,
                    byte_length: bytes.len() as u64,
                    sha256: sha256_hex(&bytes),
                    hash_algorithm: HASH_ALGORITHM.to_owned(),
                },
            )
            .is_some()
        {
            return Err(format!("duplicate package archive for {package}"));
        }
    }
    let missing = policy
        .publish_order
        .crates
        .iter()
        .filter(|package| !found.contains_key(*package))
        .cloned()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(format!("missing package archives: {}", missing.join(", ")));
    }
    policy
        .publish_order
        .crates
        .iter()
        .map(|package| {
            found
                .remove(package)
                .ok_or_else(|| format!("missing package archive for {package}"))
        })
        .collect()
}

fn dependency_graph_evidence(workspace_root: &Path) -> Result<DependencyGraphEvidence, String> {
    let output = Command::new("cargo")
        .args(["metadata", "--locked", "--format-version", "1"])
        .current_dir(workspace_root)
        .output()
        .map_err(|error| format!("run {METADATA_COMMAND}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "{METADATA_COMMAND} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let metadata: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("parse cargo metadata: {error}"))?;
    validate_metadata_source_roots(workspace_root, &metadata)?;
    let normalized = normalize_metadata(&metadata)?;
    let bytes = canonical_json_bytes(&normalized)?;
    let package_count = normalized["packages"]
        .as_array()
        .map_or(0, |packages| packages.len()) as u64;
    let node_count = normalized["resolve"]["nodes"]
        .as_array()
        .map_or(0, |nodes| nodes.len()) as u64;
    Ok(DependencyGraphEvidence {
        command: METADATA_COMMAND.to_owned(),
        normalization: "cargo_metadata_semantic_graph_v1".to_owned(),
        package_count,
        node_count,
        normalized_byte_length: bytes.len() as u64,
        normalized_sha256: sha256_hex(&bytes),
        hash_algorithm: HASH_ALGORITHM.to_owned(),
    })
}

fn validate_metadata_source_roots(workspace_root: &Path, metadata: &Value) -> Result<(), String> {
    let workspace = fs::canonicalize(workspace_root)
        .map_err(|error| format!("canonicalize {}: {error}", workspace_root.display()))?;
    for package in required_array(metadata, "packages")? {
        if package
            .get("source")
            .is_some_and(|source| !source.is_null())
        {
            continue;
        }
        let manifest_path = required_string(package, "manifest_path")?;
        let canonical = fs::canonicalize(manifest_path)
            .map_err(|error| format!("canonicalize package manifest {manifest_path}: {error}"))?;
        if !canonical.starts_with(&workspace) {
            return Err(format!(
                "unversioned path package {} is outside the candidate source tree: {manifest_path}",
                required_string(package, "name")?
            ));
        }
    }
    Ok(())
}

fn normalize_metadata(metadata: &Value) -> Result<Value, String> {
    let packages = required_array(metadata, "packages")?;
    let mut stable_ids = BTreeMap::new();
    let mut unique_stable_ids = BTreeSet::new();
    for package in packages {
        let id = required_string(package, "id")?;
        let name = required_string(package, "name")?;
        let version = required_string(package, "version")?;
        let source = package
            .get("source")
            .and_then(Value::as_str)
            .unwrap_or("workspace");
        let stable = format!("{name}@{version}|{source}");
        if !unique_stable_ids.insert(stable.clone()) {
            return Err(format!(
                "cargo metadata package identities collide after path-independent normalization: {stable}"
            ));
        }
        if stable_ids.insert(id.to_owned(), stable).is_some() {
            return Err(format!("duplicate cargo metadata package id {id}"));
        }
    }

    let mut normalized_packages = packages
        .iter()
        .map(|package| normalize_package(package, &stable_ids))
        .collect::<Result<Vec<_>, _>>()?;
    sort_values(&mut normalized_packages)?;

    let resolve = metadata
        .get("resolve")
        .and_then(Value::as_object)
        .ok_or_else(|| "cargo metadata resolve must be an object".to_owned())?;
    let mut nodes = required_array(&Value::Object(resolve.clone()), "nodes")?
        .iter()
        .map(|node| normalize_node(node, &stable_ids))
        .collect::<Result<Vec<_>, _>>()?;
    sort_values(&mut nodes)?;

    Ok(json!({
        "format_version": metadata.get("version").cloned().unwrap_or_else(|| json!(1)),
        "packages": normalized_packages,
        "resolve": {
            "root": normalize_optional_id(resolve.get("root"), &stable_ids)?,
            "nodes": nodes,
        },
        "workspace_members": normalize_id_array(metadata.get("workspace_members"), &stable_ids)?,
        "workspace_default_members": normalize_id_array(metadata.get("workspace_default_members"), &stable_ids)?,
    }))
}

fn normalize_package(
    package: &Value,
    stable_ids: &BTreeMap<String, String>,
) -> Result<Value, String> {
    let original_id = required_string(package, "id")?;
    let mut dependencies = required_array(package, "dependencies")?
        .iter()
        .map(normalize_dependency)
        .collect::<Result<Vec<_>, _>>()?;
    sort_values(&mut dependencies)?;
    let mut targets = required_array(package, "targets")?
        .iter()
        .map(normalize_target)
        .collect::<Result<Vec<_>, _>>()?;
    sort_values(&mut targets)?;
    let features = package
        .get("features")
        .and_then(Value::as_object)
        .ok_or_else(|| "cargo metadata package features must be an object".to_owned())?;
    let normalized_features = features
        .iter()
        .map(|(name, values)| {
            Ok((
                name.clone(),
                Value::Array(sorted_string_values(values, "package feature")?),
            ))
        })
        .collect::<Result<Map<_, _>, String>>()?;
    Ok(json!({
        "id": stable_id(original_id, stable_ids)?,
        "name": required_string(package, "name")?,
        "version": required_string(package, "version")?,
        "source": package.get("source").cloned().unwrap_or(Value::Null),
        "checksum": package.get("checksum").cloned().unwrap_or(Value::Null),
        "edition": package.get("edition").cloned().unwrap_or(Value::Null),
        "rust_version": package.get("rust_version").cloned().unwrap_or(Value::Null),
        "features": normalized_features,
        "dependencies": dependencies,
        "targets": targets,
    }))
}

fn normalize_dependency(dependency: &Value) -> Result<Value, String> {
    Ok(json!({
        "name": required_string(dependency, "name")?,
        "source": dependency.get("source").cloned().unwrap_or(Value::Null),
        "req": dependency.get("req").cloned().unwrap_or(Value::Null),
        "kind": dependency.get("kind").cloned().unwrap_or(Value::Null),
        "rename": dependency.get("rename").cloned().unwrap_or(Value::Null),
        "optional": dependency.get("optional").cloned().unwrap_or(Value::Bool(false)),
        "uses_default_features": dependency.get("uses_default_features").cloned().unwrap_or(Value::Bool(true)),
        "features": sorted_string_values(dependency.get("features").unwrap_or(&Value::Null), "dependency features")?,
        "target": dependency.get("target").cloned().unwrap_or(Value::Null),
        "registry": dependency.get("registry").cloned().unwrap_or(Value::Null),
    }))
}

fn normalize_target(target: &Value) -> Result<Value, String> {
    Ok(json!({
        "name": required_string(target, "name")?,
        "kind": sorted_string_values(target.get("kind").unwrap_or(&Value::Null), "target kind")?,
        "crate_types": sorted_string_values(target.get("crate_types").unwrap_or(&Value::Null), "target crate_types")?,
        "edition": target.get("edition").cloned().unwrap_or(Value::Null),
        "doctest": target.get("doctest").cloned().unwrap_or(Value::Null),
        "test": target.get("test").cloned().unwrap_or(Value::Null),
        "doc": target.get("doc").cloned().unwrap_or(Value::Null),
    }))
}

fn normalize_node(node: &Value, stable_ids: &BTreeMap<String, String>) -> Result<Value, String> {
    let mut dependencies = node
        .get("deps")
        .and_then(Value::as_array)
        .ok_or_else(|| "cargo metadata resolve node deps must be an array".to_owned())?
        .iter()
        .map(|dependency| {
            let mut dep_kinds = required_array(dependency, "dep_kinds")?
                .iter()
                .map(|kind| {
                    Ok(json!({
                        "kind": kind.get("kind").cloned().unwrap_or(Value::Null),
                        "target": kind.get("target").cloned().unwrap_or(Value::Null),
                    }))
                })
                .collect::<Result<Vec<_>, String>>()?;
            sort_values(&mut dep_kinds)?;
            Ok(json!({
                "name": required_string(dependency, "name")?,
                "pkg": stable_id(required_string(dependency, "pkg")?, stable_ids)?,
                "dep_kinds": dep_kinds,
            }))
        })
        .collect::<Result<Vec<_>, String>>()?;
    sort_values(&mut dependencies)?;
    Ok(json!({
        "id": stable_id(required_string(node, "id")?, stable_ids)?,
        "dependencies": dependencies,
        "features": sorted_string_values(node.get("features").unwrap_or(&Value::Null), "node features")?,
    }))
}

fn normalize_optional_id(
    value: Option<&Value>,
    stable_ids: &BTreeMap<String, String>,
) -> Result<Value, String> {
    match value {
        None | Some(Value::Null) => Ok(Value::Null),
        Some(Value::String(id)) => Ok(Value::String(stable_id(id, stable_ids)?.to_owned())),
        Some(_) => Err("cargo metadata resolve root must be a string or null".to_owned()),
    }
}

fn normalize_id_array(
    value: Option<&Value>,
    stable_ids: &BTreeMap<String, String>,
) -> Result<Vec<String>, String> {
    let values = value
        .and_then(Value::as_array)
        .ok_or_else(|| "cargo metadata workspace member list must be an array".to_owned())?;
    let mut normalized = values
        .iter()
        .map(|value| {
            let id = value
                .as_str()
                .ok_or_else(|| "cargo metadata package id must be a string".to_owned())?;
            stable_id(id, stable_ids).map(str::to_owned)
        })
        .collect::<Result<Vec<_>, _>>()?;
    normalized.sort();
    Ok(normalized)
}

fn stable_id<'a>(id: &str, stable_ids: &'a BTreeMap<String, String>) -> Result<&'a str, String> {
    stable_ids
        .get(id)
        .map(String::as_str)
        .ok_or_else(|| format!("cargo metadata references unknown package id {id}"))
}

fn required_array<'a>(value: &'a Value, key: &str) -> Result<&'a Vec<Value>, String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("cargo metadata {key} must be an array"))
}

fn required_string<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("cargo metadata {key} must be a string"))
}

fn sorted_string_values(value: &Value, label: &str) -> Result<Vec<Value>, String> {
    let values = value
        .as_array()
        .ok_or_else(|| format!("{label} must be an array"))?;
    let mut strings = values
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("{label} entries must be strings"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    strings.sort();
    Ok(strings.into_iter().map(Value::String).collect())
}

fn sort_values(values: &mut [Value]) -> Result<(), String> {
    let mut keyed = values
        .iter()
        .map(|value| canonical_json_bytes(value).map(|key| (key, value)))
        .collect::<Result<Vec<_>, _>>()?;
    keyed.sort_by(|left, right| left.0.cmp(&right.0));
    let ordered = keyed
        .into_iter()
        .map(|(_, value)| value.clone())
        .collect::<Vec<_>>();
    values.clone_from_slice(&ordered);
    Ok(())
}

fn required_commands() -> Vec<CommandEvidence> {
    [
        (
            "dependency_graph",
            METADATA_COMMAND,
            "exit_zero_and_normalized_digest_match",
        ),
        (
            "package_archive_verification",
            "cargo xtask release preflight",
            "exit_zero_for_exact_clean_candidate",
        ),
        (
            "canonical_release_lane",
            "nix run .#release-preflight",
            "exit_zero_for_exact_clean_candidate",
        ),
        (
            "provenance_collection",
            "cargo xtask release provenance --package-dir <PACKAGE_DIR> --out <OUTPUT>",
            "canonical_manifest_written_outside_source_worktree",
        ),
    ]
    .into_iter()
    .map(|(purpose, command, required_result)| CommandEvidence {
        purpose: purpose.to_owned(),
        command: command.to_owned(),
        required_result: required_result.to_owned(),
    })
    .collect()
}

fn ensure_clean_git_worktree(workspace_root: &Path) -> Result<(), String> {
    let output = Command::new("git")
        .args(["status", "--porcelain=v1", "--untracked-files=all"])
        .current_dir(workspace_root)
        .output()
        .map_err(|error| format!("inspect Git worktree: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "inspect Git worktree failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    if !output.stdout.is_empty() {
        return Err("release provenance requires an exact clean Git worktree".to_owned());
    }
    Ok(())
}

fn git_stdout(workspace_root: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(workspace_root)
        .output()
        .map_err(|error| format!("run git {}: {error}", args.join(" ")))?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_owned())
        .map_err(|error| format!("git {} output must be UTF-8: {error}", args.join(" ")))
}

fn object_format(oid: &str) -> Result<&'static str, String> {
    if !oid
        .bytes()
        .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err("Git object id must be lowercase hexadecimal".to_owned());
    }
    match oid.len() {
        40 => Ok("sha1"),
        64 => Ok("sha256"),
        length => Err(format!("unsupported Git object id length {length}")),
    }
}

fn require_output_outside_workspace(workspace_root: &Path, output: &Path) -> Result<(), String> {
    let workspace = fs::canonicalize(workspace_root)
        .map_err(|error| format!("canonicalize {}: {error}", workspace_root.display()))?;
    let absolute_output = absolute_lexical(output)?;
    let resolved_output = resolve_existing_ancestor(&absolute_output)?;
    if absolute_output.starts_with(&workspace) || resolved_output.starts_with(&workspace) {
        return Err("release provenance output must be outside the source worktree".to_owned());
    }
    Ok(())
}

fn resolve_existing_ancestor(path: &Path) -> Result<PathBuf, String> {
    let mut ancestor = path;
    let mut suffix = Vec::new();
    while !ancestor.exists() {
        let name = ancestor
            .file_name()
            .ok_or_else(|| format!("output path has no existing ancestor: {}", path.display()))?;
        suffix.push(name.to_owned());
        ancestor = ancestor
            .parent()
            .ok_or_else(|| format!("output path has no parent: {}", path.display()))?;
    }
    let mut resolved = fs::canonicalize(ancestor)
        .map_err(|error| format!("canonicalize {}: {error}", ancestor.display()))?;
    for component in suffix.into_iter().rev() {
        resolved.push(component);
    }
    Ok(resolved)
}

fn absolute_lexical(path: &Path) -> Result<PathBuf, String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| format!("resolve current directory: {error}"))?
            .join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(format!("cannot normalize output path {}", path.display()));
                }
            }
        }
    }
    Ok(normalized)
}

fn write_external_atomic(output: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = output
        .parent()
        .ok_or_else(|| format!("output path has no parent: {}", output.display()))?;
    fs::create_dir_all(parent).map_err(|error| format!("create {}: {error}", parent.display()))?;
    if fs::symlink_metadata(output).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Err(format!(
            "release provenance output cannot be a symlink: {}",
            output.display()
        ));
    }
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| format!("create temporary provenance output: {error}"))?;
    temporary
        .write_all(bytes)
        .map_err(|error| format!("write temporary provenance output: {error}"))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| format!("sync temporary provenance output: {error}"))?;
    temporary
        .persist(output)
        .map_err(|error| format!("persist {}: {}", output.display(), error.error))?;
    Ok(())
}

fn descriptor_for_file(workspace_root: &Path, relative: &str) -> Result<FileDescriptor, String> {
    descriptor_for_bytes(relative, &read_regular_file(workspace_root, relative)?)
}

fn descriptor_for_bytes(path: &str, bytes: &[u8]) -> Result<FileDescriptor, String> {
    Ok(FileDescriptor {
        path: path.to_owned(),
        byte_length: bytes.len() as u64,
        sha256: sha256_hex(bytes),
        hash_algorithm: HASH_ALGORITHM.to_owned(),
    })
}

fn load_toml<T: for<'de> Deserialize<'de>>(
    workspace_root: &Path,
    relative: &str,
) -> Result<T, String> {
    let bytes = read_regular_file(workspace_root, relative)?;
    let source = std::str::from_utf8(&bytes)
        .map_err(|error| format!("{relative} must be UTF-8: {error}"))?;
    toml::from_str(source).map_err(|error| format!("parse {relative}: {error}"))
}

fn require_unique_nonempty(values: &[String], label: &str) -> Result<(), String> {
    let mut unique = BTreeSet::new();
    for value in values {
        if value.trim().is_empty() {
            return Err(format!("{label} entries must not be empty"));
        }
        if !unique.insert(value) {
            return Err(format!("{label} contains duplicate entry {value}"));
        }
    }
    Ok(())
}

fn canonical_json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    let mut bytes =
        serde_json::to_vec_pretty(value).map_err(|error| format!("serialize JSON: {error}"))?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn validate_json_schema(schema: &Value, instance: &Value) -> Result<(), String> {
    let validator = jsonschema::validator_for(schema)
        .map_err(|error| format!("compile {SCHEMA_RELATIVE}: {error}"))?;
    let errors = validator
        .iter_errors(instance)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "release provenance violates {SCHEMA_RELATIVE}: {}",
            errors.join("; ")
        ))
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn release_provenance_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://radroots.org/contracts/releases/phase1-publication-release-provenance-v1.schema.json",
        "title": "Radroots Phase 1 Publication Release Provenance",
        "type": "object",
        "additionalProperties": false,
        "required": ["schema_version", "contract_id", "authority", "manifest_schema", "semantic_contract", "candidate", "source_digest", "toolchain", "lockfile", "dependency_graph", "feature_profiles", "packages", "commands"],
        "properties": {
            "schema_version": {"const": SCHEMA_VERSION},
            "contract_id": {"const": CONTRACT_ID},
            "authority": {"const": AUTHORITY},
            "manifest_schema": {"$ref": "#/$defs/file"},
            "semantic_contract": {"$ref": "#/$defs/file"},
            "candidate": {
                "type": "object", "additionalProperties": false,
                "required": ["commit_oid", "tree_oid", "object_format", "clean_worktree"],
                "properties": {
                    "commit_oid": {"$ref": "#/$defs/git_oid"},
                    "tree_oid": {"$ref": "#/$defs/git_oid"},
                    "object_format": {"enum": ["sha1", "sha256"]},
                    "clean_worktree": {"const": true}
                }
            },
            "source_digest": {
                "type": "object", "additionalProperties": false,
                "required": ["algorithm", "oid"],
                "properties": {
                    "algorithm": {"enum": ["git_tree_sha1_v1", "git_tree_sha256_v1"]},
                    "oid": {"$ref": "#/$defs/git_oid"}
                }
            },
            "toolchain": {
                "type": "object", "additionalProperties": false,
                "required": ["channel", "components", "targets", "contract"],
                "properties": {
                    "channel": {"type": "string", "minLength": 1},
                    "components": {"$ref": "#/$defs/nonempty_unique_strings"},
                    "targets": {"$ref": "#/$defs/nonempty_unique_strings"},
                    "contract": {"$ref": "#/$defs/file"}
                }
            },
            "lockfile": {"$ref": "#/$defs/file"},
            "dependency_graph": {
                "type": "object", "additionalProperties": false,
                "required": ["command", "normalization", "package_count", "node_count", "normalized_byte_length", "normalized_sha256", "hash_algorithm"],
                "properties": {
                    "command": {"const": METADATA_COMMAND},
                    "normalization": {"const": "cargo_metadata_semantic_graph_v1"},
                    "package_count": {"type": "integer", "minimum": 1},
                    "node_count": {"type": "integer", "minimum": 1},
                    "normalized_byte_length": {"type": "integer", "minimum": 1},
                    "normalized_sha256": {"$ref": "#/$defs/sha256"},
                    "hash_algorithm": {"const": HASH_ALGORITHM}
                }
            },
            "feature_profiles": {
                "type": "object", "additionalProperties": false,
                "required": ["contract", "selected"],
                "properties": {
                    "contract": {"$ref": "#/$defs/file"},
                    "selected": {"type": "array", "minItems": 1, "items": {"$ref": "#/$defs/feature_profile"}}
                }
            },
            "packages": {
                "type": "object", "additionalProperties": false,
                "required": ["release_version", "publish_policy", "archives"],
                "properties": {
                    "release_version": {"type": "string", "minLength": 1},
                    "publish_policy": {"$ref": "#/$defs/file"},
                    "archives": {"type": "array", "minItems": 1, "items": {"$ref": "#/$defs/archive"}}
                }
            },
            "commands": {"type": "array", "minItems": 4, "maxItems": 4, "items": {"$ref": "#/$defs/command"}}
        },
        "$defs": {
            "sha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
            "git_oid": {"type": "string", "pattern": "^(?:[0-9a-f]{40}|[0-9a-f]{64})$"},
            "nonempty_unique_strings": {"type": "array", "minItems": 1, "uniqueItems": true, "items": {"type": "string", "minLength": 1}},
            "file": {
                "type": "object", "additionalProperties": false,
                "required": ["path", "byte_length", "sha256", "hash_algorithm"],
                "properties": {
                    "path": {"type": "string", "minLength": 1},
                    "byte_length": {"type": "integer", "minimum": 1},
                    "sha256": {"$ref": "#/$defs/sha256"},
                    "hash_algorithm": {"const": HASH_ALGORITHM}
                }
            },
            "feature_profile": {
                "type": "object", "additionalProperties": false,
                "required": ["package", "no_default_features", "features", "test_threads"],
                "properties": {
                    "package": {"type": "string", "minLength": 1},
                    "no_default_features": {"type": "boolean"},
                    "features": {"type": "array", "uniqueItems": true, "items": {"type": "string", "minLength": 1}},
                    "test_threads": {"type": "integer", "minimum": 1}
                }
            },
            "archive": {
                "type": "object", "additionalProperties": false,
                "required": ["package", "version", "filename", "byte_length", "sha256", "hash_algorithm"],
                "properties": {
                    "package": {"type": "string", "minLength": 1},
                    "version": {"type": "string", "minLength": 1},
                    "filename": {"type": "string", "pattern": "^[A-Za-z0-9_.-]+\\.crate$"},
                    "byte_length": {"type": "integer", "minimum": 1},
                    "sha256": {"$ref": "#/$defs/sha256"},
                    "hash_algorithm": {"const": HASH_ALGORITHM}
                }
            },
            "command": {
                "type": "object", "additionalProperties": false,
                "required": ["purpose", "command", "required_result"],
                "properties": {
                    "purpose": {"type": "string", "minLength": 1},
                    "command": {"type": "string", "minLength": 1},
                    "required_result": {"type": "string", "minLength": 1}
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str) -> FileDescriptor {
        FileDescriptor {
            path: path.to_owned(),
            byte_length: 1,
            sha256: "0".repeat(64),
            hash_algorithm: HASH_ALGORITHM.to_owned(),
        }
    }

    fn sample_manifest() -> ReleaseProvenanceManifest {
        ReleaseProvenanceManifest {
            schema_version: SCHEMA_VERSION,
            contract_id: CONTRACT_ID.to_owned(),
            authority: AUTHORITY.to_owned(),
            manifest_schema: file(SCHEMA_RELATIVE),
            semantic_contract: file(SEMANTIC_CONTRACT_RELATIVE),
            candidate: GitCandidate {
                commit_oid: "0".repeat(40),
                tree_oid: "1".repeat(40),
                object_format: "sha1".to_owned(),
                clean_worktree: true,
            },
            source_digest: GitTreeDigest {
                algorithm: "git_tree_sha1_v1".to_owned(),
                oid: "1".repeat(40),
            },
            toolchain: ToolchainEvidence {
                channel: "1.97.0".to_owned(),
                components: vec!["rustfmt".to_owned()],
                targets: vec!["wasm32-unknown-unknown".to_owned()],
                contract: file(TOOLCHAIN_RELATIVE),
            },
            lockfile: file(LOCKFILE_RELATIVE),
            dependency_graph: DependencyGraphEvidence {
                command: METADATA_COMMAND.to_owned(),
                normalization: "cargo_metadata_semantic_graph_v1".to_owned(),
                package_count: 1,
                node_count: 1,
                normalized_byte_length: 1,
                normalized_sha256: "2".repeat(64),
                hash_algorithm: HASH_ALGORITHM.to_owned(),
            },
            feature_profiles: FeatureProfileEvidence {
                contract: file(FEATURE_PROFILES_RELATIVE),
                selected: vec![FeatureProfile {
                    package: "alpha".to_owned(),
                    no_default_features: false,
                    features: Vec::new(),
                    test_threads: 1,
                }],
            },
            packages: PackageEvidence {
                release_version: "1.2.3".to_owned(),
                publish_policy: file(PUBLISH_POLICY_RELATIVE),
                archives: vec![PackageArchive {
                    package: "alpha".to_owned(),
                    version: "1.2.3".to_owned(),
                    filename: "alpha-1.2.3.crate".to_owned(),
                    byte_length: 1,
                    sha256: "3".repeat(64),
                    hash_algorithm: HASH_ALGORITHM.to_owned(),
                }],
            },
            commands: required_commands(),
        }
    }

    fn test_policy() -> PublishPolicy {
        PublishPolicy {
            release: ReleaseVersion {
                version: "1.2.3".to_owned(),
            },
            classification: ReleaseClassification {
                public: vec!["alpha".to_owned(), "beta".to_owned()],
                internal: vec!["internal".to_owned()],
                deferred: Vec::new(),
                retired: Vec::new(),
                yank_only: Vec::new(),
            },
            publish_order: PublishOrder {
                crates: vec!["alpha".to_owned(), "beta".to_owned()],
            },
        }
    }

    fn metadata(root: &str, dependency_package: &str) -> Value {
        let alpha_id = format!("path+file://{root}/alpha#alpha@1.0.0");
        let beta_id = format!("path+file://{root}/beta#beta@1.0.0");
        json!({
            "version": 1,
            "packages": [
                {
                    "name": "alpha", "version": "1.0.0", "id": alpha_id,
                    "source": null, "checksum": null, "edition": "2024", "rust_version": "1.97.0",
                    "features": {"default": []}, "dependencies": [],
                    "targets": [{"name": "alpha", "kind": ["lib"], "crate_types": ["lib"], "src_path": format!("{root}/alpha/src/lib.rs"), "edition": "2024", "doctest": true, "test": true, "doc": true}]
                },
                {
                    "name": "beta", "version": "1.0.0", "id": beta_id,
                    "source": null, "checksum": null, "edition": "2024", "rust_version": "1.97.0",
                    "features": {},
                    "dependencies": [{"name": "alpha", "source": null, "req": "*", "kind": null, "rename": null, "optional": false, "uses_default_features": true, "features": [], "target": null, "registry": null}],
                    "targets": [{"name": "beta", "kind": ["lib"], "crate_types": ["lib"], "src_path": format!("{root}/beta/src/lib.rs"), "edition": "2024", "doctest": true, "test": true, "doc": true}]
                }
            ],
            "workspace_members": [format!("path+file://{root}/alpha#alpha@1.0.0"), format!("path+file://{root}/beta#beta@1.0.0")],
            "workspace_default_members": [format!("path+file://{root}/alpha#alpha@1.0.0"), format!("path+file://{root}/beta#beta@1.0.0")],
            "resolve": {
                "root": null,
                "nodes": [
                    {"id": format!("path+file://{root}/alpha#alpha@1.0.0"), "deps": [], "features": ["default"]},
                    {"id": format!("path+file://{root}/beta#beta@1.0.0"), "deps": [{"name": "alpha", "pkg": dependency_package, "dep_kinds": [{"kind": null, "target": null}]}], "features": []}
                ]
            }
        })
    }

    #[test]
    fn release_provenance_metadata_normalization_ignores_workspace_paths() {
        let first = metadata(
            "/first/worktree",
            "path+file:///first/worktree/alpha#alpha@1.0.0",
        );
        let second = metadata(
            "/second/worktree",
            "path+file:///second/worktree/alpha#alpha@1.0.0",
        );
        assert_eq!(
            normalize_metadata(&first).unwrap(),
            normalize_metadata(&second).unwrap()
        );

        let mut changed = second;
        changed["resolve"]["nodes"][1]["deps"] = json!([]);
        assert_ne!(
            normalize_metadata(&first).unwrap(),
            normalize_metadata(&changed).unwrap()
        );
    }

    #[test]
    fn release_provenance_requires_complete_exact_package_archives() {
        let temp = tempfile::TempDir::new().unwrap();
        fs::write(temp.path().join("alpha-1.2.3.crate"), b"alpha").unwrap();
        let missing = describe_package_archives(temp.path(), &test_policy()).unwrap_err();
        assert!(missing.contains("missing package archives: beta"));

        fs::write(temp.path().join("beta-1.2.3.crate"), b"beta").unwrap();
        let archives = describe_package_archives(temp.path(), &test_policy()).unwrap();
        assert_eq!(archives.len(), 2);
        assert_eq!(archives[0].package, "alpha");

        fs::write(temp.path().join("unknown-1.2.3.crate"), b"unknown").unwrap();
        let extra = describe_package_archives(temp.path(), &test_policy()).unwrap_err();
        assert!(extra.contains("unexpected package archive"));
    }

    #[test]
    fn release_provenance_rejects_dirty_git_candidate() {
        let temp = tempfile::TempDir::new().unwrap();
        let git = |args: &[&str]| {
            let status = Command::new("git")
                .args(args)
                .current_dir(temp.path())
                .status()
                .unwrap();
            assert!(status.success());
        };
        git(&["init", "-q"]);
        fs::write(temp.path().join("tracked"), b"clean\n").unwrap();
        git(&["add", "tracked"]);
        git(&[
            "-c",
            "user.name=Radroots Test",
            "-c",
            "user.email=test@radroots.invalid",
            "commit",
            "-q",
            "-m",
            "initial",
        ]);
        ensure_clean_git_worktree(temp.path()).unwrap();
        fs::write(temp.path().join("tracked"), b"dirty\n").unwrap();
        assert!(
            ensure_clean_git_worktree(temp.path())
                .unwrap_err()
                .contains("clean Git worktree")
        );
    }

    #[test]
    fn release_provenance_schema_rejects_unknown_fields() {
        let schema = release_provenance_schema();
        jsonschema::validator_for(&schema).expect("schema compiles");
        let mut invalid = json!({"unexpected": true});
        let error = validate_json_schema(&schema, &invalid).unwrap_err();
        assert!(error.contains("release provenance violates"));
        invalid.as_object_mut().unwrap().remove("unexpected");
    }

    #[test]
    fn release_provenance_manifest_cross_fields_are_validated() {
        let mut manifest = sample_manifest();
        validate_manifest(&manifest).expect("sample release provenance");

        manifest.source_digest.oid = "4".repeat(40);
        assert!(
            validate_manifest(&manifest)
                .unwrap_err()
                .contains("Git identity")
        );

        manifest = sample_manifest();
        manifest.packages.archives[0].filename = "wrong.crate".to_owned();
        assert!(
            validate_manifest(&manifest)
                .unwrap_err()
                .contains("same ordered public package set")
        );
    }

    #[test]
    fn release_provenance_rejects_unversioned_sources_outside_candidate_tree() {
        let base = tempfile::TempDir::new().unwrap();
        let workspace = base.path().join("workspace");
        let external = base.path().join("external");
        fs::create_dir_all(&workspace).unwrap();
        fs::create_dir_all(&external).unwrap();
        fs::write(workspace.join("Cargo.toml"), b"[package]\nname='inside'\n").unwrap();
        fs::write(external.join("Cargo.toml"), b"[package]\nname='outside'\n").unwrap();
        let metadata = json!({
            "packages": [{
                "name": "outside",
                "source": null,
                "manifest_path": external.join("Cargo.toml")
            }]
        });
        let error = validate_metadata_source_roots(&workspace, &metadata).unwrap_err();
        assert!(error.contains("outside the candidate source tree"));
    }

    #[cfg(unix)]
    #[test]
    fn release_provenance_resolves_output_parent_symlinks() {
        use std::os::unix::fs::symlink;

        let base = tempfile::TempDir::new().unwrap();
        let workspace = base.path().join("workspace");
        fs::create_dir_all(&workspace).unwrap();
        let link = base.path().join("external-looking-link");
        symlink(&workspace, &link).unwrap();
        let error = require_output_outside_workspace(&workspace, &link.join("provenance.json"))
            .unwrap_err();
        assert!(error.contains("outside the source worktree"));

        require_output_outside_workspace(
            &workspace,
            &base.path().join("actual-external/provenance.json"),
        )
        .expect("real external output is accepted");
    }
}
