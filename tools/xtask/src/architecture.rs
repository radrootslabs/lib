use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path},
};

use serde::Deserialize;

mod api_leakage;
mod core_contract;
mod dependency_boundary;
mod retired_compatibility;

const DEVIATIONS_RELATIVE: &str = "contracts/architecture/deviations.toml";
const ARCHITECTURE_RELATIVE: &str = "contracts/crates/release_v1/radroots_crates_release_v1.toml";
const ARCHITECTURE_ID: &str = "radroots.crates.release.v1";
const PUBLIC_HOMEPAGE: &str = "https://radroots.org";
const PUBLIC_README: &str = "README.md";
const PUBLIC_AUTHORS: &[&str] = &["Tyson Lupul <tyson@radroots.org>"];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeviationLedger {
    schema_version: u16,
    architecture_id: String,
    deviation: Vec<DeviationRecord>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeviationRecord {
    id: String,
    date: String,
    status: String,
    approval: String,
    affected_steps: Vec<String>,
    spec_anchors: Vec<String>,
    source_evidence: Vec<String>,
    replacement_action: String,
    verification: Vec<String>,
    unresolved_risk: String,
    normative_architecture_change: bool,
    adr_required: bool,
    #[serde(default)]
    closure_evidence: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ArchitectureIdentity {
    spec_id: String,
    edition: String,
    resolver: String,
    rust_version: String,
    license: String,
    canonical_repositories: Vec<String>,
    repositories: BTreeMap<String, ArchitectureRepository>,
    package: Vec<ArchitecturePackage>,
}

#[derive(Debug, Deserialize)]
struct ArchitectureRepository {
    url: String,
    version: String,
    packages: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ArchitecturePackage {
    name: String,
}

#[derive(Debug, Deserialize)]
struct ConsolidationOwnership {
    canonical_rust_repository: String,
}

#[derive(Debug, Deserialize)]
struct WorkspaceManifest {
    workspace: WorkspaceMembers,
}

#[derive(Debug, Deserialize)]
struct WorkspaceMembershipManifest {
    workspace: WorkspaceMembership,
}

#[derive(Debug, Deserialize)]
struct WorkspaceMembership {
    members: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct WorkspaceMembers {
    members: Vec<String>,
    resolver: String,
    package: WorkspacePackage,
    metadata: WorkspaceMetadata,
    lints: WorkspaceLints,
}

#[derive(Debug, Deserialize)]
struct WorkspaceLints {
    rust: WorkspaceRustLints,
    rustdoc: WorkspaceRustdocLints,
    clippy: WorkspaceClippyLints,
}

#[derive(Debug, Deserialize)]
struct WorkspaceRustLints {
    unsafe_code: String,
}

#[derive(Debug, Deserialize)]
struct WorkspaceRustdocLints {
    broken_intra_doc_links: String,
}

#[derive(Debug, Deserialize)]
struct WorkspaceClippyLints {
    dbg_macro: String,
    todo: String,
    unimplemented: String,
}

#[derive(Debug, Deserialize)]
struct WorkspaceMetadata {
    radroots: RadrootsWorkspaceMetadata,
}

#[derive(Debug, Deserialize)]
struct RadrootsWorkspaceMetadata {
    #[serde(rename = "public-package")]
    public_package: PublicPackageMetadata,
}

#[derive(Debug, Deserialize)]
struct PublicPackageMetadata {
    version: String,
    authors: Vec<String>,
    readme: String,
}

#[derive(Debug, Deserialize)]
struct WorkspacePackage {
    version: String,
    edition: String,
    #[serde(rename = "rust-version")]
    rust_version: String,
    license: String,
    repository: String,
    homepage: String,
    readme: String,
    authors: Vec<String>,
}

pub fn validate(workspace_root: &Path) -> Result<(), String> {
    validate_workspace_members(workspace_root)?;
    let architecture_path = workspace_root.join(ARCHITECTURE_RELATIVE);
    let architecture_raw = fs::read_to_string(&architecture_path)
        .map_err(|error| format!("read {}: {error}", architecture_path.display()))?;
    let architecture = toml::from_str::<ArchitectureIdentity>(&architecture_raw)
        .map_err(|error| format!("parse {}: {error}", architecture_path.display()))?;

    let architecture_packages = architecture
        .package
        .iter()
        .map(|package| package.name.clone())
        .collect::<BTreeSet<_>>();
    core_contract::validate(workspace_root)?;
    api_leakage::validate_policy_catalog(workspace_root, &architecture_packages)?;
    dependency_boundary::validate_policy_catalog(workspace_root, &architecture_packages)?;
    retired_compatibility::validate(workspace_root)?;
    validate_workspace_toolchain(workspace_root, &architecture)?;
    validate_public_package_metadata(workspace_root, &architecture)?;
    validate_no_production_sibling_paths(workspace_root)?;
    validate_public_dependency_versions(workspace_root, &architecture)?;

    let ledger_path = workspace_root.join(DEVIATIONS_RELATIVE);
    let ledger_raw = fs::read_to_string(&ledger_path)
        .map_err(|error| format!("read {}: {error}", ledger_path.display()))?;
    validate_ledger(workspace_root, &architecture.spec_id, &ledger_raw)
}

pub fn validate_dependency_boundaries(workspace_root: &Path) -> Result<(), String> {
    validate(workspace_root)?;
    dependency_boundary::validate_resolved_boundaries(workspace_root)
}

pub fn validate_api_boundaries(workspace_root: &Path) -> Result<(), String> {
    validate(workspace_root)?;
    api_leakage::validate_public_api(workspace_root)
}

pub fn validate_ci(workspace_root: &Path) -> Result<(), String> {
    validate(workspace_root)?;
    dependency_boundary::validate_resolved_boundaries(workspace_root)?;
    api_leakage::validate_public_api(workspace_root)
}

fn validate_workspace_toolchain(
    workspace_root: &Path,
    architecture: &ArchitectureIdentity,
) -> Result<(), String> {
    let manifest_path = workspace_root.join("Cargo.toml");
    let manifest_raw = fs::read_to_string(&manifest_path)
        .map_err(|error| format!("read {}: {error}", manifest_path.display()))?;
    let manifest = toml::from_str::<WorkspaceManifest>(&manifest_raw)
        .map_err(|error| format!("parse {}: {error}", manifest_path.display()))?;
    if manifest.workspace.resolver != architecture.resolver || architecture.resolver != "3" {
        return Err(format!(
            "workspace resolver {} must match architecture resolver {}",
            manifest.workspace.resolver, architecture.resolver
        ));
    }
    if manifest.workspace.package.rust_version != architecture.rust_version
        || architecture.rust_version != "1.97.1"
    {
        return Err(format!(
            "workspace rust-version {} must match architecture rust_version {}",
            manifest.workspace.package.rust_version, architecture.rust_version
        ));
    }
    let workspace_package = &manifest.workspace.package;
    let public_metadata = &manifest.workspace.metadata.radroots.public_package;
    let repository = architecture
        .repositories
        .values()
        .find(|repository| repository.url == workspace_package.repository)
        .ok_or_else(|| "workspace repository has no architecture allocation".to_owned())?;
    if public_metadata.version != repository.version {
        return Err(format!(
            "public package version source {} must match repository version {}",
            public_metadata.version, repository.version
        ));
    }
    if public_metadata.authors != PUBLIC_AUTHORS {
        return Err("public package authors source must match the workspace convention".to_owned());
    }
    if public_metadata.readme != PUBLIC_README {
        return Err(format!(
            "public package readme source must be {PUBLIC_README}"
        ));
    }
    let lints = &manifest.workspace.lints;
    if lints.rust.unsafe_code != "deny"
        || lints.rustdoc.broken_intra_doc_links != "deny"
        || lints.clippy.dbg_macro != "deny"
        || lints.clippy.todo != "deny"
        || lints.clippy.unimplemented != "deny"
    {
        return Err(
            "workspace lints must deny unsafe code and deny the approved rustdoc/Clippy baseline"
                .to_owned(),
        );
    }
    if workspace_package.edition != architecture.edition || architecture.edition != "2024" {
        return Err(format!(
            "workspace edition {} must match architecture edition {}",
            workspace_package.edition, architecture.edition
        ));
    }
    if workspace_package.license != architecture.license {
        return Err(format!(
            "workspace license {} must match architecture license {}",
            workspace_package.license, architecture.license
        ));
    }
    if !architecture
        .canonical_repositories
        .contains(&workspace_package.repository)
    {
        return Err(format!(
            "workspace repository {} is not canonical",
            workspace_package.repository
        ));
    }
    if workspace_package.homepage != PUBLIC_HOMEPAGE {
        return Err(format!(
            "workspace homepage {} must be {PUBLIC_HOMEPAGE}",
            workspace_package.homepage
        ));
    }
    if workspace_package.authors != PUBLIC_AUTHORS {
        return Err("workspace authors must match the public package convention".to_owned());
    }
    if !workspace_root.join(&workspace_package.readme).is_file() {
        return Err(format!(
            "workspace readme {} does not exist",
            workspace_package.readme
        ));
    }
    for license in ["LICENSE-MIT", "LICENSE-APACHE"] {
        if !workspace_root.join(license).is_file() {
            return Err(format!("workspace is missing {license}"));
        }
    }
    let toolchain_path = workspace_root.join("rust-toolchain.toml");
    let toolchain_raw = fs::read_to_string(&toolchain_path)
        .map_err(|error| format!("read {}: {error}", toolchain_path.display()))?;
    let toolchain = toolchain_raw
        .parse::<toml::Value>()
        .map_err(|error| format!("parse {}: {error}", toolchain_path.display()))?;
    let channel = toolchain
        .get("toolchain")
        .and_then(|value| value.get("channel"))
        .and_then(toml::Value::as_str);
    if channel != Some(architecture.rust_version.as_str()) {
        return Err(format!(
            "rust-toolchain.toml channel must match architecture rust_version {}",
            architecture.rust_version
        ));
    }
    Ok(())
}

fn validate_public_package_metadata(
    workspace_root: &Path,
    architecture: &ArchitectureIdentity,
) -> Result<(), String> {
    let workspace_raw = fs::read_to_string(workspace_root.join("Cargo.toml"))
        .map_err(|error| format!("read workspace Cargo.toml: {error}"))?;
    let workspace = toml::from_str::<WorkspaceManifest>(&workspace_raw)
        .map_err(|error| format!("parse workspace Cargo.toml: {error}"))?;
    let repository = architecture
        .repositories
        .values()
        .find(|repository| repository.url == workspace.workspace.package.repository)
        .ok_or_else(|| "workspace repository has no architecture allocation".to_owned())?;
    let all_packages = architecture
        .package
        .iter()
        .map(|package| package.name.as_str())
        .collect::<BTreeSet<_>>();
    let consolidation_path = workspace_root.join("contracts/consolidation/architecture.v1.toml");
    let consolidated_owner = if consolidation_path.is_file() {
        let consolidation = fs::read_to_string(&consolidation_path)
            .map_err(|error| format!("read {}: {error}", consolidation_path.display()))?;
        Some(
            toml::from_str::<ConsolidationOwnership>(&consolidation)
                .map_err(|error| format!("parse {}: {error}", consolidation_path.display()))?
                .canonical_rust_repository,
        )
    } else {
        None
    };
    let local_packages =
        if consolidated_owner.as_deref() == Some(workspace.workspace.package.repository.as_str()) {
            all_packages.iter().map(|name| (*name).to_owned()).collect()
        } else {
            repository.packages.iter().cloned().collect::<BTreeSet<_>>()
        };
    let mut found_local_packages = BTreeSet::new();

    for member in &workspace.workspace.members {
        let manifest_path = workspace_root.join(member).join("Cargo.toml");
        let raw = fs::read_to_string(&manifest_path)
            .map_err(|error| format!("read {}: {error}", manifest_path.display()))?;
        let manifest = raw
            .parse::<toml::Value>()
            .map_err(|error| format!("parse {}: {error}", manifest_path.display()))?;
        let package = manifest
            .get("package")
            .and_then(toml::Value::as_table)
            .ok_or_else(|| format!("{} is missing [package]", manifest_path.display()))?;
        let name = package
            .get("name")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| format!("{} is missing package.name", manifest_path.display()))?;
        if !all_packages.contains(name) {
            continue;
        }
        if !local_packages.contains(name) {
            return Err(format!(
                "public package {name} belongs to a different canonical repository"
            ));
        }
        found_local_packages.insert(name.to_owned());
        validate_public_manifest_field(
            package,
            "version",
            &workspace.workspace.package.version,
            &repository.version,
            name,
        )?;
        validate_public_manifest_field(
            package,
            "edition",
            &workspace.workspace.package.edition,
            &architecture.edition,
            name,
        )?;
        validate_public_manifest_field(
            package,
            "rust-version",
            &workspace.workspace.package.rust_version,
            &architecture.rust_version,
            name,
        )?;
        validate_public_manifest_field(
            package,
            "license",
            &workspace.workspace.package.license,
            &architecture.license,
            name,
        )?;
        validate_public_manifest_field(
            package,
            "repository",
            &workspace.workspace.package.repository,
            &repository.url,
            name,
        )?;
        validate_public_manifest_field(
            package,
            "homepage",
            &workspace.workspace.package.homepage,
            PUBLIC_HOMEPAGE,
            name,
        )?;
        let authors = resolve_public_authors(package, &workspace.workspace.package.authors)
            .ok_or_else(|| format!("public package {name} must declare authors"))?;
        if authors != PUBLIC_AUTHORS {
            return Err(format!(
                "public package {name} authors must match the workspace convention"
            ));
        }
        let readme = package.get("readme").and_then(toml::Value::as_str);
        if readme != Some(PUBLIC_README)
            || !workspace_root.join(member).join(PUBLIC_README).is_file()
        {
            return Err(format!(
                "public package {name} must use an existing crate-local {PUBLIC_README}"
            ));
        }
        let publish_registries = package
            .get("publish")
            .and_then(toml::Value::as_array)
            .ok_or_else(|| format!("public package {name} must set publish = [\"crates-io\"]"))?;
        if publish_registries.len() != 1 || publish_registries[0].as_str() != Some("crates-io") {
            return Err(format!(
                "public package {name} must set publish = [\"crates-io\"]"
            ));
        }
        let inherits_lints = manifest
            .get("lints")
            .and_then(toml::Value::as_table)
            .is_some_and(|lints| {
                lints.len() == 1
                    && lints.get("workspace").and_then(toml::Value::as_bool) == Some(true)
            });
        if !inherits_lints {
            return Err(format!(
                "public package {name} must inherit the workspace lint policy"
            ));
        }
    }
    if found_local_packages != local_packages {
        let missing = local_packages
            .difference(&found_local_packages)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        let extra = found_local_packages
            .difference(&local_packages)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "workspace public package inventory is missing: {missing}; workspace public package inventory has unallocated packages: {extra}"
        ));
    }
    Ok(())
}

fn validate_public_dependency_versions(
    workspace_root: &Path,
    architecture: &ArchitectureIdentity,
) -> Result<(), String> {
    let workspace_path = workspace_root.join("Cargo.toml");
    let workspace_raw = fs::read_to_string(&workspace_path)
        .map_err(|error| format!("read {}: {error}", workspace_path.display()))?;
    let workspace = toml::from_str::<WorkspaceManifest>(&workspace_raw)
        .map_err(|error| format!("parse {}: {error}", workspace_path.display()))?;
    let workspace_value = workspace_raw
        .parse::<toml::Value>()
        .map_err(|error| format!("parse {}: {error}", workspace_path.display()))?;
    let workspace_dependencies = workspace_value
        .get("workspace")
        .and_then(toml::Value::as_table)
        .and_then(|workspace| workspace.get("dependencies"))
        .and_then(toml::Value::as_table);
    let repository = architecture
        .repositories
        .values()
        .find(|repository| repository.url == workspace.workspace.package.repository)
        .ok_or_else(|| "workspace repository has no architecture allocation".to_owned())?;
    let local_packages = repository
        .packages
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let public_packages = architecture
        .package
        .iter()
        .map(|package| package.name.as_str())
        .collect::<BTreeSet<_>>();
    let package_versions = architecture
        .repositories
        .values()
        .flat_map(|repository| {
            repository
                .packages
                .iter()
                .map(|package| (package.as_str(), repository.version.as_str()))
        })
        .collect::<BTreeMap<_, _>>();

    for member in &workspace.workspace.members {
        let manifest_path = workspace_root.join(member).join("Cargo.toml");
        let raw = fs::read_to_string(&manifest_path)
            .map_err(|error| format!("read {}: {error}", manifest_path.display()))?;
        let manifest = raw
            .parse::<toml::Value>()
            .map_err(|error| format!("parse {}: {error}", manifest_path.display()))?;
        let package_name = manifest
            .get("package")
            .and_then(toml::Value::as_table)
            .and_then(|package| package.get("name"))
            .and_then(toml::Value::as_str)
            .ok_or_else(|| format!("{} is missing package.name", manifest_path.display()))?;
        if !local_packages.contains(package_name) {
            continue;
        }
        let policy = PublicDependencyPolicy {
            workspace_root,
            workspace_dependencies,
            public_packages: &public_packages,
            local_packages: &local_packages,
            package_versions: &package_versions,
        };
        validate_public_dependency_sections(member, package_name, &manifest, &policy)?;
    }
    Ok(())
}

struct PublicDependencyPolicy<'a> {
    workspace_root: &'a Path,
    workspace_dependencies: Option<&'a toml::value::Table>,
    public_packages: &'a BTreeSet<&'a str>,
    local_packages: &'a BTreeSet<&'a str>,
    package_versions: &'a BTreeMap<&'a str, &'a str>,
}

fn validate_public_dependency_sections(
    member: &str,
    owner: &str,
    manifest: &toml::Value,
    policy: &PublicDependencyPolicy<'_>,
) -> Result<(), String> {
    let manifest_table = manifest
        .as_table()
        .ok_or_else(|| format!("{member}/Cargo.toml must be a TOML table"))?;
    for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
        if let Some(dependencies) = manifest_table.get(section).and_then(toml::Value::as_table) {
            validate_public_dependency_table(member, owner, section, dependencies, policy)?;
        }
    }
    if let Some(targets) = manifest_table.get("target").and_then(toml::Value::as_table) {
        for (target, target_value) in targets {
            let Some(target_table) = target_value.as_table() else {
                continue;
            };
            for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
                if let Some(dependencies) =
                    target_table.get(section).and_then(toml::Value::as_table)
                {
                    validate_public_dependency_table(
                        member,
                        owner,
                        &format!("target.{target}.{section}"),
                        dependencies,
                        policy,
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn validate_public_dependency_table(
    member: &str,
    owner: &str,
    section: &str,
    dependencies: &toml::value::Table,
    policy: &PublicDependencyPolicy<'_>,
) -> Result<(), String> {
    for (dependency_key, declaration) in dependencies {
        let inherits_workspace = declaration
            .as_table()
            .and_then(|table| table.get("workspace"))
            .and_then(toml::Value::as_bool)
            == Some(true);
        let resolved = if inherits_workspace {
            policy
                .workspace_dependencies
                .and_then(|dependencies| dependencies.get(dependency_key))
                .ok_or_else(|| {
                    format!(
                        "public package {owner} {section}.{dependency_key} inherits a missing workspace dependency"
                    )
                })?
        } else {
            declaration
        };
        let resolved_table = resolved.as_table();
        let dependency_path = resolved_table
            .and_then(|table| table.get("path"))
            .and_then(toml::Value::as_str);
        let declared_package = resolved_table
            .and_then(|table| table.get("package"))
            .and_then(toml::Value::as_str);
        let path_base = if inherits_workspace {
            policy.workspace_root.to_path_buf()
        } else {
            policy.workspace_root.join(member)
        };
        let path_package = if declared_package.is_none() {
            dependency_path
                .map(|path| dependency_package_name(&path_base.join(path)))
                .transpose()?
        } else {
            None
        };
        let dependency_name = declared_package
            .or(path_package.as_deref())
            .unwrap_or(dependency_key.as_str());
        let first_party =
            dependency_key.starts_with("radroots_") || dependency_name.starts_with("radroots_");
        if first_party && !policy.public_packages.contains(dependency_name) {
            return Err(format!(
                "public package {owner} {section}.{dependency_key} must not depend on private first-party package {dependency_name}"
            ));
        }
        if !policy.public_packages.contains(dependency_name) {
            continue;
        }
        let version = match resolved {
            toml::Value::String(version) => Some(version.as_str()),
            toml::Value::Table(table) => table.get("version").and_then(toml::Value::as_str),
            _ => None,
        };
        let governed_version = policy
            .package_versions
            .get(dependency_name)
            .ok_or_else(|| {
                format!("public dependency {dependency_name} has no repository version authority")
            })?;
        let exact_version = format!("={governed_version}");
        if policy.local_packages.contains(dependency_name)
            && (dependency_path.is_none() || version != Some(exact_version.as_str()))
        {
            return Err(format!(
                "public package {owner} {section}.{dependency_key} dependency on {dependency_name} must declare path and exact version {exact_version}"
            ));
        }
        if !policy.local_packages.contains(dependency_name)
            && (dependency_path.is_some() || version != Some(exact_version.as_str()))
        {
            return Err(format!(
                "public package {owner} {section}.{dependency_key} cross-repository dependency on {dependency_name} must declare exact registry version {exact_version} without a path"
            ));
        }
    }
    Ok(())
}

fn validate_no_production_sibling_paths(workspace_root: &Path) -> Result<(), String> {
    let canonical_root = fs::canonicalize(workspace_root)
        .map_err(|error| format!("canonicalize {}: {error}", workspace_root.display()))?;
    let workspace_path = workspace_root.join("Cargo.toml");
    let workspace_raw = fs::read_to_string(&workspace_path)
        .map_err(|error| format!("read {}: {error}", workspace_path.display()))?;
    let workspace = toml::from_str::<WorkspaceManifest>(&workspace_raw)
        .map_err(|error| format!("parse {}: {error}", workspace_path.display()))?;
    let workspace_value = workspace_raw
        .parse::<toml::Value>()
        .map_err(|error| format!("parse {}: {error}", workspace_path.display()))?;
    if let Some(dependencies) = workspace_value
        .get("workspace")
        .and_then(toml::Value::as_table)
        .and_then(|workspace| workspace.get("dependencies"))
        .and_then(toml::Value::as_table)
    {
        validate_dependency_path_table(
            &canonical_root,
            &canonical_root,
            "workspace",
            "workspace.dependencies",
            dependencies,
        )?;
    }
    for member in &workspace.workspace.members {
        let manifest_path = workspace_root.join(member).join("Cargo.toml");
        let raw = fs::read_to_string(&manifest_path)
            .map_err(|error| format!("read {}: {error}", manifest_path.display()))?;
        let manifest = raw
            .parse::<toml::Value>()
            .map_err(|error| format!("parse {}: {error}", manifest_path.display()))?;
        validate_manifest_dependency_paths(
            &canonical_root,
            &canonical_root.join(member),
            member,
            &manifest,
        )?;
    }
    Ok(())
}

fn validate_manifest_dependency_paths(
    workspace_root: &Path,
    package_root: &Path,
    owner: &str,
    manifest: &toml::Value,
) -> Result<(), String> {
    let Some(manifest_table) = manifest.as_table() else {
        return Err(format!("{owner}/Cargo.toml must be a TOML table"));
    };
    for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
        if let Some(dependencies) = manifest_table.get(section).and_then(toml::Value::as_table) {
            validate_dependency_path_table(
                workspace_root,
                package_root,
                owner,
                section,
                dependencies,
            )?;
        }
    }
    if let Some(targets) = manifest_table.get("target").and_then(toml::Value::as_table) {
        for (target, target_value) in targets {
            let Some(target_table) = target_value.as_table() else {
                continue;
            };
            for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
                if let Some(dependencies) =
                    target_table.get(section).and_then(toml::Value::as_table)
                {
                    validate_dependency_path_table(
                        workspace_root,
                        package_root,
                        owner,
                        &format!("target.{target}.{section}"),
                        dependencies,
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn validate_dependency_path_table(
    workspace_root: &Path,
    path_base: &Path,
    owner: &str,
    section: &str,
    dependencies: &toml::value::Table,
) -> Result<(), String> {
    for (dependency, declaration) in dependencies {
        let Some(path) = declaration
            .as_table()
            .and_then(|table| table.get("path"))
            .and_then(toml::Value::as_str)
        else {
            continue;
        };
        let resolved = fs::canonicalize(path_base.join(path)).map_err(|error| {
            format!(
                "resolve production dependency {owner} {section}.{dependency} path {path}: {error}"
            )
        })?;
        if !resolved.starts_with(workspace_root) {
            return Err(format!(
                "production dependency {owner} {section}.{dependency} path {path} escapes the repository; use a registry version or an external local-development override"
            ));
        }
    }
    Ok(())
}

fn dependency_package_name(package_root: &Path) -> Result<String, String> {
    let manifest_path = package_root.join("Cargo.toml");
    let raw = fs::read_to_string(&manifest_path).map_err(|error| {
        format!(
            "read dependency manifest {}: {error}",
            manifest_path.display()
        )
    })?;
    let manifest = raw.parse::<toml::Value>().map_err(|error| {
        format!(
            "parse dependency manifest {}: {error}",
            manifest_path.display()
        )
    })?;
    manifest
        .get("package")
        .and_then(toml::Value::as_table)
        .and_then(|package| package.get("name"))
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("{} is missing package.name", manifest_path.display()))
}

fn validate_public_manifest_field(
    package: &toml::value::Table,
    key: &str,
    workspace_value: &str,
    expected: &str,
    package_name: &str,
) -> Result<(), String> {
    let value = resolve_public_string(package, key, workspace_value)
        .ok_or_else(|| format!("public package {package_name} must declare {key}"))?;
    if value != expected {
        return Err(format!(
            "public package {package_name} {key} {value} must be {expected}"
        ));
    }
    Ok(())
}

fn resolve_public_string<'a>(
    package: &'a toml::value::Table,
    key: &str,
    workspace_value: &'a str,
) -> Option<&'a str> {
    match package.get(key)? {
        toml::Value::String(value) => Some(value),
        toml::Value::Table(value)
            if value.get("workspace").and_then(toml::Value::as_bool) == Some(true) =>
        {
            Some(workspace_value)
        }
        _ => None,
    }
}

fn resolve_public_authors<'a>(
    package: &'a toml::value::Table,
    workspace_authors: &'a [String],
) -> Option<Vec<&'a str>> {
    match package.get("authors")? {
        toml::Value::Array(values) => values.iter().map(toml::Value::as_str).collect(),
        toml::Value::Table(value)
            if value.get("workspace").and_then(toml::Value::as_bool) == Some(true) =>
        {
            Some(workspace_authors.iter().map(String::as_str).collect())
        }
        _ => None,
    }
}

fn validate_workspace_members(workspace_root: &Path) -> Result<(), String> {
    let manifest_path = workspace_root.join("Cargo.toml");
    let manifest_raw = fs::read_to_string(&manifest_path)
        .map_err(|error| format!("read {}: {error}", manifest_path.display()))?;
    let manifest = toml::from_str::<WorkspaceMembershipManifest>(&manifest_raw)
        .map_err(|error| format!("parse {}: {error}", manifest_path.display()))?;
    let manifest_value = manifest_raw
        .parse::<toml::Value>()
        .map_err(|error| format!("parse {}: {error}", manifest_path.display()))?;
    validate_manifest_dependency_keys(&manifest_value, "Cargo.toml")?;
    let declared = manifest
        .workspace
        .members
        .into_iter()
        .collect::<BTreeSet<_>>();
    let mut discovered = BTreeSet::new();
    for root in ["crates", "tools"] {
        let directory = workspace_root.join(root);
        for entry in fs::read_dir(&directory)
            .map_err(|error| format!("read {}: {error}", directory.display()))?
        {
            let entry =
                entry.map_err(|error| format!("read {} entry: {error}", directory.display()))?;
            if entry
                .file_type()
                .map_err(|error| format!("read {} type: {error}", entry.path().display()))?
                .is_dir()
                && entry.path().join("Cargo.toml").is_file()
            {
                let directory_name = entry.file_name().to_string_lossy().into_owned();
                require_lower_snake_case(
                    &directory_name,
                    &format!("{root}/{directory_name} crate directory"),
                )?;
                let package_manifest_path = entry.path().join("Cargo.toml");
                let package_manifest_raw =
                    fs::read_to_string(&package_manifest_path).map_err(|error| {
                        format!("read {}: {error}", package_manifest_path.display())
                    })?;
                let package_manifest =
                    package_manifest_raw
                        .parse::<toml::Value>()
                        .map_err(|error| {
                            format!("parse {}: {error}", package_manifest_path.display())
                        })?;
                let package_name = package_manifest
                    .get("package")
                    .and_then(toml::Value::as_table)
                    .and_then(|package| package.get("name"))
                    .and_then(toml::Value::as_str)
                    .ok_or_else(|| {
                        format!(
                            "{} is missing package.name",
                            package_manifest_path.display()
                        )
                    })?;
                require_lower_snake_case(
                    package_name,
                    &format!("{} package.name", package_manifest_path.display()),
                )?;
                validate_manifest_dependency_keys(
                    &package_manifest,
                    &package_manifest_path.display().to_string(),
                )?;
                discovered.insert(format!("{root}/{directory_name}"));
            }
        }
    }
    if declared != discovered {
        let missing = discovered
            .difference(&declared)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        let unknown = declared
            .difference(&discovered)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "workspace membership is missing local package roots: {missing}; workspace membership has unknown package roots: {unknown}"
        ));
    }
    Ok(())
}

fn validate_manifest_dependency_keys(manifest: &toml::Value, label: &str) -> Result<(), String> {
    let Some(manifest) = manifest.as_table() else {
        return Err(format!("{label} must be a TOML table"));
    };
    for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
        if let Some(dependencies) = manifest.get(section).and_then(toml::Value::as_table) {
            validate_dependency_keys(dependencies, label, section)?;
        }
    }
    if let Some(workspace_dependencies) = manifest
        .get("workspace")
        .and_then(toml::Value::as_table)
        .and_then(|workspace| workspace.get("dependencies"))
        .and_then(toml::Value::as_table)
    {
        validate_dependency_keys(workspace_dependencies, label, "workspace.dependencies")?;
    }
    if let Some(targets) = manifest.get("target").and_then(toml::Value::as_table) {
        for (target, target_value) in targets {
            let Some(target_table) = target_value.as_table() else {
                continue;
            };
            for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
                if let Some(dependencies) =
                    target_table.get(section).and_then(toml::Value::as_table)
                {
                    validate_dependency_keys(
                        dependencies,
                        label,
                        &format!("target.{target}.{section}"),
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn validate_dependency_keys(
    dependencies: &toml::value::Table,
    label: &str,
    section: &str,
) -> Result<(), String> {
    for key in dependencies
        .keys()
        .filter(|key| key.starts_with("radroots"))
    {
        require_lower_snake_case(key, &format!("{label} {section} dependency key"))?;
    }
    Ok(())
}

fn require_lower_snake_case(value: &str, label: &str) -> Result<(), String> {
    if is_lower_snake_case(value) {
        Ok(())
    } else {
        Err(format!("{label} `{value}` must use lowercase snake case"))
    }
}

fn is_lower_snake_case(value: &str) -> bool {
    !value.is_empty()
        && value.split('_').all(|segment| {
            segment
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_lowercase)
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
}

fn validate_ledger(
    workspace_root: &Path,
    expected_architecture_id: &str,
    raw: &str,
) -> Result<(), String> {
    let ledger = toml::from_str::<DeviationLedger>(raw)
        .map_err(|error| format!("parse {DEVIATIONS_RELATIVE}: {error}"))?;
    if ledger.schema_version != 1 {
        return Err("deviation ledger schema_version must be 1".to_owned());
    }
    if ledger.architecture_id != expected_architecture_id
        || ledger.architecture_id != ARCHITECTURE_ID
    {
        return Err(format!(
            "deviation ledger architecture_id {} must match {}",
            ledger.architecture_id, expected_architecture_id
        ));
    }

    let mut ids = BTreeSet::new();
    for record in &ledger.deviation {
        validate_record(workspace_root, record)?;
        if !ids.insert(record.id.as_str()) {
            return Err(format!("duplicate deviation id {}", record.id));
        }
    }
    Ok(())
}

fn validate_record(workspace_root: &Path, record: &DeviationRecord) -> Result<(), String> {
    if !is_deviation_id(&record.id) {
        return Err(format!("deviation id {} must use RCRV1-DEV-NNN", record.id));
    }
    if !is_iso_date(&record.date) {
        return Err(format!("deviation {} date must use YYYY-MM-DD", record.id));
    }
    if !matches!(record.status.as_str(), "active" | "closed" | "superseded") {
        return Err(format!(
            "deviation {} status must be active, closed, or superseded",
            record.id
        ));
    }
    require_text(&record.id, "approval", &record.approval)?;
    require_text(&record.id, "replacement_action", &record.replacement_action)?;
    require_text(&record.id, "unresolved_risk", &record.unresolved_risk)?;
    require_nonempty_list(&record.id, "affected_steps", &record.affected_steps)?;
    require_nonempty_list(&record.id, "spec_anchors", &record.spec_anchors)?;
    require_nonempty_list(&record.id, "source_evidence", &record.source_evidence)?;
    require_nonempty_list(&record.id, "verification", &record.verification)?;

    for step in &record.affected_steps {
        let valid = step.len() == 3
            && step.bytes().all(|byte| byte.is_ascii_digit())
            && step
                .parse::<u16>()
                .is_ok_and(|value| (1..=315).contains(&value));
        if !valid {
            return Err(format!(
                "deviation {} affected step {} must be in 001..315",
                record.id, step
            ));
        }
    }
    for anchor in &record.spec_anchors {
        validate_spec_anchor(workspace_root, &record.id, anchor)?;
    }
    if record.status == "active" && !record.closure_evidence.is_empty() {
        return Err(format!(
            "active deviation {} must not carry closure_evidence",
            record.id
        ));
    }
    if record.status != "active" {
        require_nonempty_list(&record.id, "closure_evidence", &record.closure_evidence)?;
    }

    let _ = (record.normative_architecture_change, record.adr_required);
    Ok(())
}

fn validate_spec_anchor(
    workspace_root: &Path,
    deviation_id: &str,
    anchor: &str,
) -> Result<(), String> {
    let (relative, fragment) = anchor
        .split_once('#')
        .map_or((anchor, None), |(path, fragment)| (path, Some(fragment)));
    if relative.trim().is_empty() || fragment.is_none_or(|value| value.trim().is_empty()) {
        return Err(format!(
            "deviation {deviation_id} has invalid spec anchor {anchor}"
        ));
    }
    let path = Path::new(relative);
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(format!(
            "deviation {deviation_id} spec anchor must be repository-relative: {anchor}"
        ));
    }
    if relative != ARCHITECTURE_RELATIVE {
        return Err(format!(
            "deviation {deviation_id} spec anchor must target {ARCHITECTURE_RELATIVE}: {anchor}"
        ));
    }
    let machine_path = workspace_root.join(path);
    let raw = fs::read_to_string(&machine_path)
        .map_err(|error| format!("read {}: {error}", machine_path.display()))?;
    let machine = toml::from_str::<toml::Value>(&raw)
        .map_err(|error| format!("parse {}: {error}", machine_path.display()))?;
    let selector = fragment.expect("fragment is required above");
    if !machine_selector_exists(&machine, selector) {
        return Err(format!(
            "deviation {deviation_id} spec anchor has unknown machine selector {selector}"
        ));
    }
    Ok(())
}

fn machine_selector_exists(machine: &toml::Value, selector: &str) -> bool {
    let segments = selector.split('.').collect::<Vec<_>>();
    if segments.iter().any(|segment| segment.is_empty()) {
        return false;
    }
    let root = match machine.as_table() {
        Some(root) => root,
        None => return false,
    };
    match segments.as_slice() {
        ["repositories", repository] => root
            .get("repositories")
            .and_then(toml::Value::as_table)
            .and_then(|repositories| repositories.get(*repository))
            .is_some_and(toml::Value::is_table),
        ["repository_policy"] | ["release_policy"] => {
            root.get(segments[0]).is_some_and(toml::Value::is_table)
        }
        ["quality_policy", "coverage"] => root
            .get("quality_policy")
            .and_then(toml::Value::as_table)
            .and_then(|quality| quality.get("coverage"))
            .is_some_and(toml::Value::is_table),
        ["package", package] => root
            .get("package")
            .and_then(toml::Value::as_array)
            .is_some_and(|packages| {
                packages.iter().any(|candidate| {
                    candidate
                        .as_table()
                        .and_then(|table| table.get("name"))
                        .and_then(toml::Value::as_str)
                        == Some(*package)
                })
            }),
        _ => false,
    }
}

fn require_text(deviation_id: &str, field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!(
            "deviation {deviation_id} field {field} must not be empty"
        ));
    }
    Ok(())
}

fn require_nonempty_list(deviation_id: &str, field: &str, values: &[String]) -> Result<(), String> {
    if values.is_empty() || values.iter().any(|value| value.trim().is_empty()) {
        return Err(format!(
            "deviation {deviation_id} field {field} must contain non-empty values"
        ));
    }
    Ok(())
}

fn is_deviation_id(value: &str) -> bool {
    value
        .strip_prefix("RCRV1-DEV-")
        .is_some_and(|suffix| suffix.len() == 3 && suffix.bytes().all(|byte| byte.is_ascii_digit()))
}

fn is_iso_date(value: &str) -> bool {
    value.len() == 10
        && value.as_bytes()[4] == b'-'
        && value.as_bytes()[7] == b'-'
        && value
            .bytes()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{
        ArchitectureIdentity, ArchitecturePackage, ArchitectureRepository, validate_ledger,
        validate_no_production_sibling_paths, validate_public_dependency_versions,
        validate_public_package_metadata, validate_workspace_members, validate_workspace_toolchain,
    };

    fn test_root(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("radroots_architecture_{label}_{nonce}"));
        fs::create_dir_all(root.join("contracts/crates/release_v1")).expect("create spec root");
        fs::write(
            root.join("contracts/crates/release_v1/radroots_crates_release_v1.toml"),
            "spec_id = \"radroots.crates.release.v1\"\n\n[repositories.sdk]\nurl = \"https://github.com/radrootslabs/sdk\"\n",
        )
        .expect("write spec");
        root
    }

    fn complete_ledger() -> &'static str {
        r#"schema_version = 1
architecture_id = "radroots.crates.release.v1"

[[deviation]]
id = "RCRV1-DEV-001"
date = "2026-07-27"
status = "active"
approval = "Explicit user correction dated 2026-07-27."
affected_steps = ["015", "016"]
spec_anchors = ["contracts/crates/release_v1/radroots_crates_release_v1.toml#repositories.sdk"]
source_evidence = ["The approved architecture assigns packages to the existing lib and sdk repositories."]
replacement_action = "Keep both standalone repositories and verify them independently."
verification = ["Repository-local architecture validation passes."]
unresolved_risk = "Remote publication remains separately authorized."
normative_architecture_change = false
adr_required = false
"#
    }

    fn architecture() -> ArchitectureIdentity {
        ArchitectureIdentity {
            spec_id: "radroots.crates.release.v1".to_string(),
            edition: "2024".to_string(),
            resolver: "3".to_string(),
            rust_version: "1.97.1".to_string(),
            license: "MIT OR Apache-2.0".to_string(),
            canonical_repositories: vec!["https://github.com/radrootslabs/sdk".to_string()],
            repositories: BTreeMap::from([(
                "sdk".to_string(),
                ArchitectureRepository {
                    url: "https://github.com/radrootslabs/sdk".to_string(),
                    version: "0.1.0".to_string(),
                    packages: vec!["radroots".to_string()],
                },
            )]),
            package: vec![ArchitecturePackage {
                name: "radroots".to_string(),
            }],
        }
    }

    fn complete_workspace_manifest(members: &str) -> String {
        format!(
            "[workspace]\nmembers = [{members}]\nresolver = \"3\"\n\n[workspace.package]\nversion = \"0.1.0\"\nedition = \"2024\"\nrust-version = \"1.97.1\"\nlicense = \"MIT OR Apache-2.0\"\nrepository = \"https://github.com/radrootslabs/sdk\"\nhomepage = \"https://radroots.org\"\nreadme = \"README\"\nauthors = [\"Tyson Lupul <tyson@radroots.org>\"]\n\n[workspace.metadata.radroots.public-package]\nversion = \"0.1.0\"\nauthors = [\"Tyson Lupul <tyson@radroots.org>\"]\nreadme = \"README.md\"\n\n[workspace.lints.rust]\nunsafe_code = \"deny\"\n\n[workspace.lints.rustdoc]\nbroken_intra_doc_links = \"deny\"\n\n[workspace.lints.clippy]\ndbg_macro = \"deny\"\ntodo = \"deny\"\nunimplemented = \"deny\"\n"
        )
    }

    #[test]
    fn accepts_complete_active_deviation() {
        let root = test_root("complete");
        validate_ledger(&root, "radroots.crates.release.v1", complete_ledger())
            .expect("complete deviation");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_incomplete_active_deviation() {
        let root = test_root("incomplete");
        let incomplete = complete_ledger().replace(
            "spec_anchors = [\"contracts/crates/release_v1/radroots_crates_release_v1.toml#repositories.sdk\"]",
            "spec_anchors = []",
        );
        let error = validate_ledger(&root, "radroots.crates.release.v1", &incomplete)
            .expect_err("missing anchor must fail");
        assert!(error.contains("field spec_anchors must contain non-empty values"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn deviation_anchors_require_resolving_machine_selectors() {
        let root = test_root("machine_selector");
        let invalid = complete_ledger().replace("#repositories.sdk", "#garbage");
        let error = validate_ledger(&root, "radroots.crates.release.v1", &invalid)
            .expect_err("arbitrary fragment must fail");
        assert!(error.contains("unknown machine selector garbage"));

        let markdown_slug =
            complete_ledger().replace("#repositories.sdk", "#20-current-to-target-migration-map");
        let error = validate_ledger(&root, "radroots.crates.release.v1", &markdown_slug)
            .expect_err("Markdown heading slug must fail");
        assert!(error.contains("unknown machine selector"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn workspace_membership_requires_every_local_package_root() {
        let root = test_root("workspace_members");
        for path in ["crates/a", "tools/xtask"] {
            fs::create_dir_all(root.join(path)).expect("create package root");
            fs::write(
                root.join(path).join("Cargo.toml"),
                "[package]\nname = \"fixture\"\n",
            )
            .expect("write package manifest");
        }
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/a\", \"tools/xtask\"]\nresolver = \"3\"\n\n[workspace.package]\nrust-version = \"1.97.1\"\n",
        )
        .expect("write complete workspace");
        validate_workspace_members(&root).expect("complete membership");

        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"tools/xtask\"]\nresolver = \"3\"\n\n[workspace.package]\nrust-version = \"1.97.1\"\n",
        )
        .expect("write incomplete workspace");
        let error = validate_workspace_members(&root).expect_err("missing member must fail");
        assert!(error.contains("missing local package roots: crates/a"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn workspace_identity_rejects_kebab_case_crate_names_and_dependency_keys() {
        let root = test_root("workspace_snake_case_identity");
        fs::create_dir_all(root.join("crates/radroots_probe")).expect("create package root");
        fs::create_dir_all(root.join("tools/xtask")).expect("create xtask root");
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/radroots_probe\", \"tools/xtask\"]\nresolver = \"3\"\n",
        )
        .expect("write workspace manifest");
        fs::write(
            root.join("crates/radroots_probe/Cargo.toml"),
            "[package]\nname = \"radroots-probe\"\n",
        )
        .expect("write kebab package manifest");
        fs::write(
            root.join("tools/xtask/Cargo.toml"),
            "[package]\nname = \"xtask\"\n",
        )
        .expect("write xtask manifest");

        let package_error = validate_workspace_members(&root)
            .expect_err("kebab-case Cargo package identity must fail");
        assert!(package_error.contains("package.name"));
        assert!(package_error.contains("lowercase snake case"));

        fs::write(
            root.join("crates/radroots_probe/Cargo.toml"),
            "[package]\nname = \"radroots_probe\"\n",
        )
        .expect("write snake package manifest");
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/radroots_probe\", \"tools/xtask\"]\nresolver = \"3\"\n\n[workspace.dependencies]\nradroots-probe = \"0.1.0\"\n",
        )
        .expect("write kebab dependency key");

        let dependency_error = validate_workspace_members(&root)
            .expect_err("kebab-case Radroots dependency key must fail");
        assert!(dependency_error.contains("workspace.dependencies dependency key"));
        assert!(dependency_error.contains("lowercase snake case"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn workspace_toolchain_requires_exact_resolver_and_rust_version() {
        let root = test_root("workspace_toolchain");
        fs::write(root.join("Cargo.toml"), complete_workspace_manifest(""))
            .expect("write workspace manifest");
        for path in ["README", "LICENSE-MIT", "LICENSE-APACHE"] {
            fs::write(root.join(path), "fixture\n").expect("write workspace metadata file");
        }
        fs::write(
            root.join("rust-toolchain.toml"),
            "[toolchain]\nchannel = \"1.97.1\"\n",
        )
        .expect("write toolchain");
        let architecture = architecture();
        validate_workspace_toolchain(&root, &architecture).expect("exact toolchain policy");

        fs::write(
            root.join("rust-toolchain.toml"),
            "[toolchain]\nchannel = \"1.97.0\"\n",
        )
        .expect("write mismatched toolchain");
        let error = validate_workspace_toolchain(&root, &architecture)
            .expect_err("mismatched toolchain must fail");
        assert!(error.contains("channel must match"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn public_package_metadata_requires_canonical_inheritance() {
        let root = test_root("public_package_metadata");
        fs::create_dir_all(root.join("crates/radroots")).expect("create public package");
        fs::write(
            root.join("Cargo.toml"),
            complete_workspace_manifest("\"crates/radroots\""),
        )
        .expect("write workspace manifest");
        let manifest = "[package]\nname = \"radroots\"\nversion.workspace = true\nedition.workspace = true\nrust-version.workspace = true\nlicense.workspace = true\nrepository.workspace = true\nhomepage.workspace = true\nauthors.workspace = true\nreadme = \"README.md\"\npublish = [\"crates-io\"]\n\n[lints]\nworkspace = true\n";
        fs::write(root.join("crates/radroots/Cargo.toml"), manifest)
            .expect("write public manifest");
        fs::write(root.join("crates/radroots/README.md"), "fixture\n")
            .expect("write public readme");
        validate_public_package_metadata(&root, &architecture()).expect("canonical metadata");

        fs::write(
            root.join("crates/radroots/Cargo.toml"),
            manifest.replace("[lints]\nworkspace = true\n", ""),
        )
        .expect("write incomplete public manifest");
        let error = validate_public_package_metadata(&root, &architecture())
            .expect_err("missing lint inheritance must fail");
        assert!(error.contains("must inherit the workspace lint policy"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn public_package_inventory_requires_every_local_package() {
        let root = test_root("public_package_inventory");
        fs::write(root.join("Cargo.toml"), complete_workspace_manifest(""))
            .expect("write workspace manifest");
        let error = validate_public_package_metadata(&root, &architecture())
            .expect_err("missing local public package must fail");
        assert!(error.contains("workspace public package inventory is missing: radroots"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn public_dependencies_require_path_and_exact_version() {
        let root = test_root("public_dependency_version");
        fs::create_dir_all(root.join("crates/radroots")).expect("create public package");
        fs::create_dir_all(root.join("crates/dependency")).expect("create dependency package");
        fs::write(
            root.join("Cargo.toml"),
            format!(
                "{}\n[workspace.dependencies]\nradroots_core = {{ package = \"radroots_core\", path = \"crates/dependency\", version = \"=0.1.0\" }}\n",
                complete_workspace_manifest("\"crates/radroots\"")
            ),
        )
        .expect("write workspace manifest");
        fs::write(
            root.join("crates/radroots/Cargo.toml"),
            "[package]\nname = \"radroots\"\nversion = \"0.1.0\"\n\n[dependencies]\nradroots_core = { workspace = true }\n",
        )
        .expect("write public manifest");
        fs::write(
            root.join("crates/dependency/Cargo.toml"),
            "[package]\nname = \"radroots_core\"\nversion = \"0.1.0\"\n",
        )
        .expect("write dependency manifest");
        let mut architecture = architecture();
        architecture.package.push(ArchitecturePackage {
            name: "radroots_core".to_owned(),
        });
        architecture
            .repositories
            .get_mut("sdk")
            .expect("sdk repository")
            .packages
            .push("radroots_core".to_owned());
        validate_public_dependency_versions(&root, &architecture)
            .expect("path plus exact version public dependency");

        let workspace_path = root.join("Cargo.toml");
        let workspace = fs::read_to_string(&workspace_path).expect("read workspace manifest");
        fs::write(
            &workspace_path,
            workspace.replace(", version = \"=0.1.0\"", ""),
        )
        .expect("write path-only dependency");
        let error = validate_public_dependency_versions(&root, &architecture)
            .expect_err("path-only public dependency must fail");
        assert!(error.contains("must declare path and exact version =0.1.0"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn production_dependencies_reject_sibling_paths() {
        let root = test_root("production_sibling_path");
        let sibling = root.with_extension("sibling");
        fs::create_dir_all(root.join("crates/radroots")).expect("create public package");
        fs::create_dir_all(root.join("crates/dependency")).expect("create local dependency");
        fs::create_dir_all(&sibling).expect("create sibling dependency");
        fs::write(
            root.join("Cargo.toml"),
            complete_workspace_manifest("\"crates/radroots\""),
        )
        .expect("write workspace manifest");
        let sibling_path = sibling.to_string_lossy();
        fs::write(
            root.join("crates/radroots/Cargo.toml"),
            format!(
                "[package]\nname = \"radroots\"\nversion = \"0.1.0\"\n\n[dependencies]\nprobe = {{ path = \"{sibling_path}\" }}\n"
            ),
        )
        .expect("write sibling dependency");
        let error = validate_no_production_sibling_paths(&root)
            .expect_err("sibling production path must fail");
        assert!(error.contains("escapes the repository"));

        fs::write(
            root.join("crates/radroots/Cargo.toml"),
            "[package]\nname = \"radroots\"\nversion = \"0.1.0\"\n\n[dependencies]\nprobe = { path = \"../dependency\" }\n",
        )
        .expect("write local dependency");
        fs::create_dir_all(root.join(".cargo")).expect("create cargo config directory");
        fs::write(
            root.join(".cargo/config.toml"),
            format!("[patch.crates-io]\nprobe = {{ path = \"{sibling_path}\" }}\n"),
        )
        .expect("write development override");
        validate_no_production_sibling_paths(&root).expect("in-repository path");
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(sibling);
    }
}
