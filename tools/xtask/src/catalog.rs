use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path},
    process::Command,
};

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::contract::artifact_bundle::{
    GeneratedArtifact, read_regular_file, with_artifact_bundle_transaction,
};

const CATALOG_RELATIVE: &str = "contracts/crates/catalog.v1.toml";
const RELEASE_RELATIVE: &str = "contracts/crates/release.v2.toml";
const CONSOLIDATION_RELATIVE: &str = "contracts/consolidation/architecture.v1.toml";
const GROUPS_RELATIVE: &str = "contracts/crates/generated/package_groups.v1.toml";
const PLATFORMS_RELATIVE: &str = "contracts/crates/generated/platform_inventory.v1.toml";
const RELEASE_INVENTORY_RELATIVE: &str = "contracts/crates/generated/release_inventory.v2.toml";
const CATALOG_SCHEMA: &str = "radroots.workspace.catalog.v1";
const RELEASE_ID: &str = "radroots.crates.release.v2";
const CONSOLIDATION_ID: &str = "radroots.rust.consolidation.v1";
const VERSION: &str = "0.1.0-alpha";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    schema_version: u16,
    schema: String,
    architecture: String,
    consolidation: String,
    version: String,
    rust_version: String,
    edition: String,
    resolver: String,
    public_package_count: usize,
    package_count: usize,
    digest_algorithm: String,
    source_tree_digest_algorithm: String,
    source_provenance_policy: String,
    provenance_correction_contract: String,
    retired_packages: Vec<String>,
    package: Vec<CatalogPackage>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogPackage {
    name: String,
    path: String,
    state: String,
    tier: String,
    visibility: String,
    publish: bool,
    version: String,
    license: String,
    platforms: Vec<String>,
    groups: Vec<String>,
    owners: Vec<String>,
    permitted_dependency_tiers: Vec<String>,
    source_repository: String,
    source_revision: String,
    source_path: String,
    source_tree_sha256: String,
    compatibility: Vec<String>,
    removal_gate: Option<String>,
    replaces: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleaseV2 {
    schema_version: u16,
    spec_id: String,
    status: String,
    supersedes_without_mutation: String,
    canonical_repository: String,
    version: String,
    public_package_count: usize,
    publication_authorized: bool,
    public_packages: Vec<String>,
    v1_artifact: Vec<V1Artifact>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct V1Artifact {
    path: String,
    sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConsolidationV1 {
    schema_version: u16,
    consolidation_id: String,
    architecture: String,
    canonical_rust_repository: String,
    donor_repositories: Vec<String>,
    consumer_repositories: Vec<String>,
    source_retirement_requires: Vec<String>,
    repository_archival_requires: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CargoMetadata {
    packages: Vec<CargoPackage>,
    workspace_members: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CargoPackage {
    id: String,
    name: String,
    version: String,
    manifest_path: String,
    license: Option<String>,
    publish: Option<Vec<String>>,
    dependencies: Vec<CargoDependency>,
}

#[derive(Debug, Deserialize)]
struct CargoDependency {
    name: String,
    req: String,
    path: Option<String>,
    kind: Option<String>,
    target: Option<String>,
}

pub fn run(args: &[String], workspace_root: &Path) -> Result<(), String> {
    match args {
        [command] if command == "check" => check(workspace_root),
        [command] if command == "write" => write(workspace_root),
        _ => Err("catalog accepts check or write".to_owned()),
    }
}

pub(crate) fn check(workspace_root: &Path) -> Result<(), String> {
    let loaded = load_and_validate(workspace_root)?;
    for artifact in render_projections(&loaded.catalog, &loaded.catalog_digest) {
        let current = read_regular_file(workspace_root, artifact.relative)?;
        if current != artifact.contents {
            return Err(format!(
                "generated catalog projection {} is stale; run catalog write",
                artifact.relative
            ));
        }
    }
    Ok(())
}

fn write(workspace_root: &Path) -> Result<(), String> {
    let loaded = load_and_validate(workspace_root)?;
    let artifacts = render_projections(&loaded.catalog, &loaded.catalog_digest);
    with_artifact_bundle_transaction(workspace_root, |transaction| transaction.write(artifacts))
}

struct LoadedCatalog {
    catalog: Catalog,
    catalog_digest: String,
}

fn load_and_validate(workspace_root: &Path) -> Result<LoadedCatalog, String> {
    let catalog_bytes = read_regular_file(workspace_root, CATALOG_RELATIVE)?;
    let catalog = parse_toml::<Catalog>(CATALOG_RELATIVE, &catalog_bytes)?;
    let release = parse_file::<ReleaseV2>(workspace_root, RELEASE_RELATIVE)?;
    let consolidation = parse_file::<ConsolidationV1>(workspace_root, CONSOLIDATION_RELATIVE)?;
    validate_catalog(&catalog)?;
    validate_release(&release, &catalog, workspace_root)?;
    validate_consolidation(&consolidation)?;
    validate_workspace_manifest(&catalog, workspace_root)?;
    let metadata = cargo_metadata(workspace_root)?;
    validate_metadata(&catalog, &metadata, workspace_root)?;
    validate_active_source_provenance(&catalog, workspace_root)?;
    Ok(LoadedCatalog {
        catalog,
        catalog_digest: sha256(&catalog_bytes),
    })
}

fn parse_file<T: for<'de> Deserialize<'de>>(
    workspace_root: &Path,
    relative: &str,
) -> Result<T, String> {
    let bytes = read_regular_file(workspace_root, relative)?;
    parse_toml(relative, &bytes)
}

fn parse_toml<T: for<'de> Deserialize<'de>>(relative: &str, bytes: &[u8]) -> Result<T, String> {
    let raw =
        std::str::from_utf8(bytes).map_err(|error| format!("{relative} is not UTF-8: {error}"))?;
    toml::from_str(raw).map_err(|error| format!("parse {relative}: {error}"))
}

fn validate_catalog(catalog: &Catalog) -> Result<(), String> {
    if catalog.schema_version != 1
        || catalog.schema != CATALOG_SCHEMA
        || catalog.architecture != RELEASE_ID
        || catalog.consolidation != CONSOLIDATION_ID
        || catalog.version != VERSION
        || catalog.rust_version != "1.97.1"
        || catalog.edition != "2024"
        || catalog.resolver != "3"
        || catalog.public_package_count != 19
        || catalog.package_count != catalog.package.len()
        || catalog.digest_algorithm != "sha256-raw-bytes-v1"
        || catalog.source_tree_digest_algorithm != "sha256-git-ls-tree-r-v1"
        || catalog.source_provenance_policy != "immutable_after_source_retirement"
        || catalog.provenance_correction_contract != "approved_correction_record_required"
    {
        return Err("catalog identity, toolchain, or cardinality drifted".to_owned());
    }

    let expected_public = expected_public_packages();
    let allowed_states = BTreeSet::from(["active", "reserved_import", "reserved_refactor"]);
    let allowed_visibilities = BTreeSet::from([
        "public_release",
        "private_runtime",
        "private_adapter",
        "private_boundary",
        "private_codegen",
        "private_fixture",
        "private_tool",
    ]);
    let allowed_licenses =
        BTreeSet::from(["MIT OR Apache-2.0", "GPL-3.0-only", "GPL-3.0-or-later"]);
    let mut names = BTreeSet::new();
    let mut paths = BTreeSet::new();
    let mut public = BTreeSet::new();
    let mut package_by_name = BTreeMap::new();
    let mut groups = BTreeSet::new();
    for package in &catalog.package {
        validate_identifier(&package.name, "package name")?;
        validate_relative_path(&package.path)?;
        validate_identifier(&package.tier, "package tier")?;
        let expected_path = match package.name.as_str() {
            "radroots" => "crates/radroots".to_owned(),
            "xtask" => "tools/xtask".to_owned(),
            name => format!(
                "crates/{}",
                name.strip_prefix("radroots_")
                    .ok_or_else(|| format!("package {name} lacks the radroots_ prefix"))?
            ),
        };
        if package.path != expected_path {
            return Err(format!(
                "package {} must use canonical path {expected_path}",
                package.name
            ));
        }
        if !names.insert(package.name.as_str()) || !paths.insert(package.path.as_str()) {
            return Err(format!(
                "duplicate package name or path for {}",
                package.name
            ));
        }
        if !allowed_states.contains(package.state.as_str())
            || !allowed_visibilities.contains(package.visibility.as_str())
            || !allowed_licenses.contains(package.license.as_str())
            || package.version != VERSION
            || package.platforms.is_empty()
            || package.groups.is_empty()
            || package.owners.is_empty()
            || package.permitted_dependency_tiers.is_empty()
            || package.compatibility.is_empty()
        {
            return Err(format!("catalog package {} is incomplete", package.name));
        }
        validate_unique_identifiers(&package.platforms, "platform")?;
        validate_unique_identifiers(&package.groups, "group")?;
        validate_unique_identifiers(&package.owners, "owner")?;
        validate_unique_identifiers(
            &package.permitted_dependency_tiers,
            "permitted dependency tier",
        )?;
        validate_unique_identifiers(&package.compatibility, "compatibility authority")?;
        for group in &package.groups {
            groups.insert(group.as_str());
        }
        if package.visibility == "public_release" {
            public.insert(package.name.as_str());
            if !package.publish || package.license != "MIT OR Apache-2.0" {
                return Err(format!(
                    "public package {} must be permissive and publication-eligible",
                    package.name
                ));
            }
        } else if package.publish {
            return Err(format!(
                "private package {} must set publish=false",
                package.name
            ));
        }
        if package.license.starts_with("GPL-")
            && !matches!(
                package.visibility.as_str(),
                "private_runtime" | "private_adapter" | "private_boundary" | "private_codegen"
            )
        {
            return Err(format!("GPL package {} has an invalid class", package.name));
        }
        if package.source_repository
            != format!(
                "https://github.com/radrootslabs/{}",
                source_repository_name(&package.source_repository)?
            )
        {
            return Err(format!(
                "package {} source repository is noncanonical",
                package.name
            ));
        }
        validate_oid(&package.source_revision, "source revision")?;
        validate_relative_path(&package.source_path)?;
        validate_sha256(&package.source_tree_sha256, "source tree digest")?;
        for replaced in &package.replaces {
            validate_package_identity(replaced, "replaced package")?;
        }
        if package.state == "active" && package.removal_gate.is_some() {
            return Err(format!(
                "active package {} has a removal gate",
                package.name
            ));
        }
        if package.state != "active"
            && package
                .removal_gate
                .as_deref()
                .is_none_or(|gate| gate.trim().is_empty())
        {
            return Err(format!(
                "reserved package {} lacks a removal gate",
                package.name
            ));
        }
        package_by_name.insert(package.name.as_str(), package);
    }
    if public != expected_public {
        return Err("catalog must contain exactly the 19 approved public packages".to_owned());
    }
    let required_groups = BTreeSet::from([
        "boundaries",
        "mobile",
        "portable",
        "preview",
        "public_native",
        "sdk",
        "studio",
        "tools",
        "wasm",
    ]);
    if !required_groups.is_subset(&groups) {
        return Err("catalog is missing one or more required package groups".to_owned());
    }
    let portable = catalog
        .package
        .iter()
        .filter(|package| package.groups.iter().any(|group| group == "portable"))
        .map(|package| package.name.as_str())
        .collect::<BTreeSet<_>>();
    if portable != expected_public {
        return Err("portable group must contain exactly the 19 public packages".to_owned());
    }
    for package in &catalog.package {
        for tier in &package.permitted_dependency_tiers {
            if !catalog
                .package
                .iter()
                .any(|candidate| candidate.tier == *tier)
            {
                return Err(format!(
                    "package {} permits unknown tier {tier}",
                    package.name
                ));
            }
        }
    }
    let retired = catalog
        .retired_packages
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if retired.len() != catalog.retired_packages.len()
        || retired
            .iter()
            .any(|name| package_by_name.contains_key(name))
        || !expected_retired_packages().is_subset(&retired)
    {
        return Err("retired package identities are incomplete or reused".to_owned());
    }
    let xtasks = catalog
        .package
        .iter()
        .filter(|package| package.name == "xtask")
        .collect::<Vec<_>>();
    if xtasks.len() != 1
        || xtasks[0].path != "tools/xtask"
        || xtasks[0].visibility != "private_tool"
        || xtasks[0].state != "active"
    {
        return Err("catalog must contain exactly one canonical xtask".to_owned());
    }
    Ok(())
}

fn validate_release(
    release: &ReleaseV2,
    catalog: &Catalog,
    workspace_root: &Path,
) -> Result<(), String> {
    if release.schema_version != 1
        || release.spec_id != RELEASE_ID
        || release.status != "approved_not_published"
        || release.supersedes_without_mutation != "radroots.crates.release.v1"
        || release.canonical_repository != "https://github.com/radrootslabs/lib"
        || release.version != VERSION
        || release.public_package_count != 19
        || release.publication_authorized
        || release
            .public_packages
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>()
            != expected_public_packages()
    {
        return Err("release v2 identity or public allocation drifted".to_owned());
    }
    if release.public_packages.len() != release.public_package_count
        || catalog.public_package_count != release.public_package_count
    {
        return Err("release v2 package cardinality drifted".to_owned());
    }
    let mut paths = BTreeSet::new();
    for artifact in &release.v1_artifact {
        validate_relative_path(&artifact.path)?;
        validate_sha256(&artifact.sha256, "v1 artifact digest")?;
        if !paths.insert(artifact.path.as_str()) {
            return Err("release v2 repeats a v1 artifact".to_owned());
        }
        let bytes = read_regular_file(workspace_root, &artifact.path)?;
        if sha256(&bytes) != artifact.sha256 {
            return Err(format!("historical v1 artifact {} drifted", artifact.path));
        }
    }
    let expected = BTreeSet::from([
        "docs/specs/radroots_crates_release_v1.dot",
        "docs/specs/radroots_crates_release_v1.md",
        "docs/specs/radroots_crates_release_v1.sha256",
        "docs/specs/radroots_crates_release_v1.toml",
        "docs/specs/radroots_crates_release_v1_inventory.csv",
    ]);
    if paths != expected {
        return Err("release v2 must pin every historical v1 authority artifact".to_owned());
    }
    Ok(())
}

fn validate_consolidation(contract: &ConsolidationV1) -> Result<(), String> {
    if contract.schema_version != 1
        || contract.consolidation_id != CONSOLIDATION_ID
        || contract.architecture != RELEASE_ID
        || contract.canonical_rust_repository != "https://github.com/radrootslabs/lib"
        || contract
            .donor_repositories
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>()
            != BTreeSet::from([
                "https://github.com/radrootslabs/app_rt",
                "https://github.com/radrootslabs/sdk",
                "https://github.com/radrootslabs/studio_app",
            ])
        || contract
            .consumer_repositories
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>()
            != BTreeSet::from([
                "https://github.com/radrootslabs/sdk",
                "https://github.com/radrootslabs/studio_app",
            ])
        || contract.source_retirement_requires.is_empty()
        || contract.repository_archival_requires.is_empty()
    {
        return Err("consolidation v1 repository authority drifted".to_owned());
    }
    validate_unique_identifiers(
        &contract.source_retirement_requires,
        "source retirement requirement",
    )?;
    validate_unique_identifiers(
        &contract.repository_archival_requires,
        "repository archival requirement",
    )
}

fn cargo_metadata(workspace_root: &Path) -> Result<CargoMetadata, String> {
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--locked", "--no-deps"])
        .current_dir(workspace_root)
        .output()
        .map_err(|error| format!("run cargo metadata: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("parse cargo metadata output: {error}"))
}

fn validate_workspace_manifest(catalog: &Catalog, workspace_root: &Path) -> Result<(), String> {
    let bytes = read_regular_file(workspace_root, "Cargo.toml")?;
    let raw =
        std::str::from_utf8(&bytes).map_err(|error| format!("Cargo.toml is not UTF-8: {error}"))?;
    let manifest =
        toml::from_str::<toml::Value>(raw).map_err(|error| format!("parse Cargo.toml: {error}"))?;
    let workspace = manifest
        .get("workspace")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| "Cargo.toml lacks [workspace]".to_owned())?;
    let members = workspace
        .get("members")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| "Cargo.toml lacks explicit workspace members".to_owned())?
        .iter()
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| "workspace member must be a string".to_owned())
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    let active_paths = catalog
        .package
        .iter()
        .filter(|package| package.state == "active")
        .map(|package| package.path.as_str())
        .collect::<BTreeSet<_>>();
    if members != active_paths
        || members
            .iter()
            .any(|member| member.contains('*') || member.contains('?') || member.contains('['))
    {
        return Err(
            "Cargo workspace members must exactly match explicit active catalog paths".to_owned(),
        );
    }
    if workspace.get("resolver").and_then(toml::Value::as_str) != Some("3") {
        return Err("Cargo workspace resolver must remain 3".to_owned());
    }
    let dependencies = manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("dependencies"))
        .and_then(toml::Value::as_table)
        .ok_or_else(|| "Cargo.toml lacks [workspace.dependencies]".to_owned())?;
    for package in catalog
        .package
        .iter()
        .filter(|package| package.state == "active" && package.name != "xtask")
    {
        let dependency = dependencies
            .get(&package.name)
            .and_then(toml::Value::as_table)
            .ok_or_else(|| format!("workspace dependency {} is missing", package.name))?;
        if dependency.get("path").and_then(toml::Value::as_str) != Some(package.path.as_str())
            || dependency.get("version").and_then(toml::Value::as_str)
                != Some(format!("={VERSION}").as_str())
            || ["git", "rev", "branch", "tag"]
                .iter()
                .any(|key| dependency.contains_key(*key))
        {
            return Err(format!(
                "workspace dependency {} must use its catalog path and exact version only",
                package.name
            ));
        }
    }
    Ok(())
}

fn validate_metadata(
    catalog: &Catalog,
    metadata: &CargoMetadata,
    workspace_root: &Path,
) -> Result<(), String> {
    let workspace_ids = metadata
        .workspace_members
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let workspace_packages = metadata
        .packages
        .iter()
        .filter(|package| workspace_ids.contains(package.id.as_str()))
        .map(|package| (package.name.as_str(), package))
        .collect::<BTreeMap<_, _>>();
    let catalog_by_name = catalog
        .package
        .iter()
        .map(|package| (package.name.as_str(), package))
        .collect::<BTreeMap<_, _>>();
    let active = catalog
        .package
        .iter()
        .filter(|package| package.state == "active")
        .map(|package| package.name.as_str())
        .collect::<BTreeSet<_>>();
    if workspace_packages.keys().copied().collect::<BTreeSet<_>>() != active {
        return Err("active catalog packages and Cargo workspace members differ".to_owned());
    }
    for package in catalog
        .package
        .iter()
        .filter(|package| package.state != "active")
    {
        if workspace_root.join(&package.path).exists() {
            return Err(format!(
                "reserved package path {} exists before its catalog activation",
                package.path
            ));
        }
    }
    let canonical_root = fs::canonicalize(workspace_root)
        .map_err(|error| format!("canonicalize {}: {error}", workspace_root.display()))?;
    for (name, cargo) in &workspace_packages {
        let package = catalog_by_name[name];
        let manifest = Path::new(&cargo.manifest_path);
        let relative_manifest = manifest
            .strip_prefix(&canonical_root)
            .map_err(|_| format!("manifest {} escapes workspace", manifest.display()))?;
        let actual_path = relative_manifest
            .parent()
            .ok_or_else(|| format!("manifest {} has no package root", manifest.display()))?
            .to_string_lossy()
            .replace('\\', "/");
        let cargo_publish = cargo
            .publish
            .as_ref()
            .is_some_and(|registries| !registries.is_empty());
        if package.path != actual_path
            || package.version != cargo.version
            || package.license != cargo.license.as_deref().unwrap_or("")
            || package.publish != cargo_publish
        {
            return Err(format!("catalog and Cargo metadata drifted for {name}"));
        }
        for dependency in &cargo.dependencies {
            let Some(target) = catalog_by_name.get(dependency.name.as_str()) else {
                continue;
            };
            if dependency.path.is_none() || dependency.req != format!("={VERSION}") {
                return Err(format!(
                    "first-party dependency {name} -> {} must use path plus ={VERSION}",
                    dependency.name
                ));
            }
            if !package
                .permitted_dependency_tiers
                .iter()
                .any(|tier| tier == &target.tier)
            {
                return Err(format!(
                    "catalog tier policy forbids {name} -> {} ({:?}, {:?})",
                    dependency.name, dependency.kind, dependency.target
                ));
            }
            if package.visibility == "public_release" && target.visibility != "public_release" {
                return Err(format!(
                    "public package {name} depends on private package {}",
                    dependency.name
                ));
            }
            if package.license == "MIT OR Apache-2.0" && target.license.starts_with("GPL-") {
                return Err(format!(
                    "permissive package {name} depends on GPL package {}",
                    dependency.name
                ));
            }
        }
    }
    Ok(())
}

fn validate_active_source_provenance(
    catalog: &Catalog,
    workspace_root: &Path,
) -> Result<(), String> {
    for package in catalog.package.iter().filter(|package| {
        package.state == "active"
            && package.source_repository == "https://github.com/radrootslabs/lib"
    }) {
        let output = Command::new("git")
            .args([
                "ls-tree",
                "-r",
                &package.source_revision,
                "--",
                &package.source_path,
            ])
            .current_dir(workspace_root)
            .output()
            .map_err(|error| format!("run git ls-tree for {}: {error}", package.name))?;
        if !output.status.success() {
            return Err(format!(
                "source revision for {} is unavailable: {}",
                package.name,
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        if sha256(&output.stdout) != package.source_tree_sha256 {
            return Err(format!(
                "source tree provenance drifted for {}",
                package.name
            ));
        }
    }
    Ok(())
}

fn render_projections(catalog: &Catalog, digest: &str) -> Vec<GeneratedArtifact> {
    let mut groups = BTreeMap::<&str, Vec<&str>>::new();
    let mut platforms = BTreeMap::<&str, Vec<&str>>::new();
    let mut public = Vec::new();
    let mut private = Vec::new();
    let mut reserved = Vec::new();
    for package in &catalog.package {
        for group in &package.groups {
            groups.entry(group).or_default().push(&package.name);
        }
        for platform in &package.platforms {
            platforms.entry(platform).or_default().push(&package.name);
        }
        if package.visibility == "public_release" {
            public.push(package.name.as_str());
        } else {
            private.push(package.name.as_str());
        }
        if package.state != "active" {
            reserved.push(package.name.as_str());
        }
    }
    sort_map_values(&mut groups);
    sort_map_values(&mut platforms);
    public.sort_unstable();
    private.sort_unstable();
    reserved.sort_unstable();
    vec![
        GeneratedArtifact {
            relative: GROUPS_RELATIVE,
            contents: render_map_projection(
                "radroots.workspace.package-groups.v1",
                digest,
                "group",
                &groups,
            )
            .into_bytes(),
        },
        GeneratedArtifact {
            relative: PLATFORMS_RELATIVE,
            contents: render_map_projection(
                "radroots.workspace.platform-inventory.v1",
                digest,
                "platform",
                &platforms,
            )
            .into_bytes(),
        },
        GeneratedArtifact {
            relative: RELEASE_INVENTORY_RELATIVE,
            contents: format!(
                "schema = \"radroots.workspace.release-inventory.v2\"\ncatalog_sha256 = \"{digest}\"\narchitecture = \"{RELEASE_ID}\"\nversion = \"{VERSION}\"\npublic_packages = {}\nprivate_packages = {}\nreserved_packages = {}\n",
                toml_array(&public),
                toml_array(&private),
                toml_array(&reserved)
            )
            .into_bytes(),
        },
    ]
}

fn render_map_projection(
    schema: &str,
    digest: &str,
    table: &str,
    values: &BTreeMap<&str, Vec<&str>>,
) -> String {
    let mut output = format!("schema = \"{schema}\"\ncatalog_sha256 = \"{digest}\"\n");
    for (id, packages) in values {
        output.push_str(&format!(
            "\n[[{table}]]\nid = \"{id}\"\npackages = {}\n",
            toml_array(packages)
        ));
    }
    output
}

fn toml_array(values: &[&str]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| format!("\"{value}\""))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn sort_map_values(values: &mut BTreeMap<&str, Vec<&str>>) {
    for packages in values.values_mut() {
        packages.sort_unstable();
        packages.dedup();
    }
}

fn expected_public_packages() -> BTreeSet<&'static str> {
    BTreeSet::from([
        "radroots",
        "radroots_blossom",
        "radroots_core",
        "radroots_event",
        "radroots_event_codec",
        "radroots_geonames",
        "radroots_identity",
        "radroots_nostr",
        "radroots_nostr_connect",
        "radroots_protocol",
        "radroots_sdk",
        "radroots_secrets",
        "radroots_signing",
        "radroots_storage",
        "radroots_storage_sqlite",
        "radroots_sync",
        "radroots_trade",
        "radroots_transport",
        "radroots_transport_nostr",
    ])
}

fn expected_retired_packages() -> BTreeSet<&'static str> {
    BTreeSet::from([
        "radroots-studio-application",
        "radroots-studio-domain",
        "radroots-studio-ffi",
        "radroots-studio-nostr",
        "radroots-studio-storage",
        "radroots-studio-uniffi-bindgen",
        "radroots_app_bindgen",
        "radroots_app_core",
        "radroots_app_ffi",
        "radroots_app_wasm",
        "radroots_sdk_xtask",
    ])
}

fn validate_unique_identifiers(values: &[String], context: &str) -> Result<(), String> {
    let mut unique = BTreeSet::new();
    for value in values {
        validate_identifier(value, context)?;
        if !unique.insert(value.as_str()) {
            return Err(format!("duplicate {context} {value}"));
        }
    }
    Ok(())
}

fn validate_identifier(value: &str, context: &str) -> Result<(), String> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(format!("{context} must use lowercase snake case"));
    }
    Ok(())
}

fn validate_package_identity(value: &str, context: &str) -> Result<(), String> {
    if value.is_empty()
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
    {
        return Err(format!("{context} is invalid"));
    }
    Ok(())
}

fn validate_relative_path(value: &str) -> Result<(), String> {
    let path = Path::new(value);
    if value.is_empty()
        || path.is_absolute()
        || value.contains('\\')
        || value
            .split('/')
            .any(|segment| segment.is_empty() || matches!(segment, "." | ".."))
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!("catalog path {value:?} is unsafe"));
    }
    Ok(())
}

fn validate_oid(value: &str, context: &str) -> Result<(), String> {
    if value.len() != 40
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!("{context} must be lowercase full 40-hex"));
    }
    Ok(())
}

fn validate_sha256(value: &str, context: &str) -> Result<(), String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!("{context} must be lowercase 64-hex"));
    }
    Ok(())
}

fn source_repository_name(url: &str) -> Result<&str, String> {
    url.strip_prefix("https://github.com/radrootslabs/")
        .filter(|name| {
            !name.is_empty()
                && name
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        })
        .ok_or_else(|| format!("invalid canonical source repository {url}"))
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checked_in_catalog() -> Catalog {
        parse_file(&crate::workspace_root(), CATALOG_RELATIVE).expect("checked-in catalog")
    }

    #[test]
    fn public_inventory_is_exact() {
        assert_eq!(expected_public_packages().len(), 19);
        assert!(expected_public_packages().contains("radroots"));
        assert!(expected_public_packages().contains("radroots_sdk"));
    }

    #[test]
    fn paths_and_digests_fail_closed() {
        assert!(validate_relative_path("crates/sdk").is_ok());
        assert!(validate_relative_path("../sdk").is_err());
        assert!(validate_relative_path("crates/./sdk").is_err());
        assert!(validate_sha256(&"a".repeat(64), "digest").is_ok());
        assert!(validate_sha256(&"A".repeat(64), "digest").is_err());
    }

    #[test]
    fn generated_maps_are_sorted_and_digest_bound() {
        let mut values = BTreeMap::from([("sdk", vec!["z", "a", "a"])]);
        sort_map_values(&mut values);
        let rendered = render_map_projection("fixture.v1", &"a".repeat(64), "group", &values);
        assert!(rendered.contains("catalog_sha256"));
        assert!(rendered.contains("packages = [\"a\", \"z\"]"));
    }

    #[test]
    fn checked_in_catalog_structure_is_valid() {
        validate_catalog(&checked_in_catalog()).expect("catalog structure");
    }

    #[test]
    fn catalog_rejects_version_visibility_license_and_retirement_drift() {
        let mut catalog = checked_in_catalog();
        catalog
            .package
            .iter_mut()
            .find(|package| package.name == "xtask")
            .expect("xtask")
            .publish = true;
        assert!(validate_catalog(&catalog).is_err());

        let mut catalog = checked_in_catalog();
        catalog
            .package
            .iter_mut()
            .find(|package| package.name == "radroots_core")
            .expect("core")
            .license = "GPL-3.0-only".to_owned();
        assert!(validate_catalog(&catalog).is_err());

        let mut catalog = checked_in_catalog();
        catalog
            .package
            .iter_mut()
            .find(|package| package.name == "radroots_mobile_core")
            .expect("mobile")
            .version = "0.1.0-alpha.1".to_owned();
        assert!(validate_catalog(&catalog).is_err());

        let mut catalog = checked_in_catalog();
        catalog.retired_packages.push("radroots_core".to_owned());
        assert!(validate_catalog(&catalog).is_err());
    }

    #[test]
    fn catalog_rejects_naming_path_and_package_count_drift() {
        let mut catalog = checked_in_catalog();
        catalog.package_count -= 1;
        assert!(validate_catalog(&catalog).is_err());

        let mut catalog = checked_in_catalog();
        catalog.package[0].name = "radroots-bad-name".to_owned();
        assert!(validate_catalog(&catalog).is_err());

        let mut catalog = checked_in_catalog();
        catalog.package[0].path = "../escape".to_owned();
        assert!(validate_catalog(&catalog).is_err());
    }

    #[test]
    fn metadata_rejects_floating_sources_and_tier_edges() {
        let workspace_root = crate::workspace_root();
        let catalog = checked_in_catalog();
        let mut metadata = cargo_metadata(&workspace_root).expect("cargo metadata");
        let dependency = metadata
            .packages
            .iter_mut()
            .find(|package| package.name == "radroots_event")
            .and_then(|package| {
                package
                    .dependencies
                    .iter_mut()
                    .find(|dependency| dependency.name == "radroots_core")
            })
            .expect("event to core dependency");
        dependency.path = None;
        dependency.req = "*".to_owned();
        assert!(validate_metadata(&catalog, &metadata, &workspace_root).is_err());

        let mut catalog = checked_in_catalog();
        catalog
            .package
            .iter_mut()
            .find(|package| package.name == "radroots_event")
            .expect("event")
            .permitted_dependency_tiers
            .retain(|tier| tier != "foundation");
        let metadata = cargo_metadata(&workspace_root).expect("cargo metadata");
        assert!(validate_metadata(&catalog, &metadata, &workspace_root).is_err());
    }
}
