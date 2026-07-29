use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output};

const POLICY_RELATIVE: &str = "contracts/releases/publish_policy.toml";
const PROFILES_RELATIVE: &str = "contracts/coverage-profiles.toml";
const ARCHITECTURE_RELATIVE: &str = "docs/specs/radroots_crates_release_v1.toml";
const POLICY_SCHEMA_VERSION: u32 = 1;
const CLOSURE_DIRECTORY: &str = "release-package-closure";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleasePolicy {
    schema: PolicySchema,
    release: ReleaseVersion,
    publication: PublicationControl,
    workspace_classification: WorkspaceReleaseClassification,
    publish_order: PublishOrder,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PolicySchema {
    version: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleaseVersion {
    version: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PublicationControl {
    frozen: bool,
    registry: String,
    final_enablement_step: u16,
    spec_id: String,
    approved_packages: Vec<String>,
    local_packages: Vec<String>,
    external_packages: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkspaceReleaseClassification {
    private: Vec<String>,
    build_codegen: Vec<String>,
    test_support: Vec<String>,
    preview: Vec<String>,
    retired: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PublishOrder {
    crates: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FeatureProfile {
    no_default_features: bool,
    features: Vec<String>,
    test_threads: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FeatureProfilesFile {
    profiles: FeatureProfiles,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FeatureProfiles {
    default: FeatureProfile,
    #[serde(default)]
    crates: BTreeMap<String, FeatureProfile>,
}

#[derive(Debug, Deserialize)]
struct CargoMetadata {
    packages: Vec<CargoMetadataPackage>,
    workspace_members: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CargoMetadataPackage {
    id: String,
    name: String,
    version: String,
    manifest_path: PathBuf,
}

struct PackageWorkspaceEntry {
    name: String,
    version: String,
    source_directory: PathBuf,
}

#[derive(Debug, Deserialize)]
struct CratesReleaseArchitecture {
    repositories: CratesReleaseRepositories,
}

#[derive(Debug, Deserialize)]
struct CratesReleaseRepositories {
    lib: CratesReleaseRepository,
}

#[derive(Debug, Deserialize)]
struct CratesReleaseRepository {
    version: String,
}

#[derive(Debug)]
struct ValidatedReleasePolicy {
    package_order: Vec<String>,
    local_packages: BTreeSet<String>,
    workspace_packages: BTreeSet<String>,
    frozen: bool,
}

pub(crate) fn validate_release_packages(workspace_root: &Path) -> Result<(), String> {
    ensure_clean_git_worktree(workspace_root)?;
    let policy = load_toml::<ReleasePolicy>(&workspace_root.join(POLICY_RELATIVE))?;
    let validated = validate_policy(&policy)?;
    let package_version = load_package_version(workspace_root)?;
    let profiles = load_toml::<FeatureProfilesFile>(&workspace_root.join(PROFILES_RELATIVE))?;
    let selected_profiles = select_profiles(&validated, &profiles.profiles)?;
    let workspace_packages = load_workspace_packages(workspace_root)?;
    validate_local_package_versions(
        &workspace_packages,
        &validated.local_packages,
        &package_version,
    )?;

    let cargo_target_root = cargo_target_root(workspace_root)?;
    let closure_root = cargo_target_root.join(CLOSURE_DIRECTORY);
    reset_owned_closure_directory(&cargo_target_root, &closure_root)?;
    let archive_root = closure_root.join("archives");
    let list_root = closure_root.join("package-lists");
    let check_root = closure_root.join("archive-workspace");
    let extracted_root = check_root.join("members");
    for directory in [&archive_root, &extracted_root, &list_root, &check_root] {
        fs::create_dir_all(directory)
            .map_err(|error| format!("create {}: {error}", directory.display()))?;
    }
    let source_patch_config = write_source_patch_config(&closure_root, &workspace_packages)?;

    for package in &validated.package_order {
        let package_list = cargo_package_list(workspace_root, package)?;
        fs::write(
            list_root.join(format!("{package}.txt")),
            format!(
                "{}\n",
                package_list.iter().cloned().collect::<Vec<_>>().join("\n")
            ),
        )
        .map_err(|error| format!("write package list for {package}: {error}"))?;

        run_cargo_with_config(
            workspace_root,
            &source_patch_config,
            &["package", "--locked", "--no-verify", "-p", package],
            &format!("package {package}"),
        )?;
        let filename = format!("{package}-{package_version}.crate");
        let cargo_archive = cargo_target_root.join("package").join(&filename);
        require_regular_file(&cargo_archive, &format!("Cargo archive for {package}"))?;
        let governed_archive = archive_root.join(&filename);
        fs::copy(&cargo_archive, &governed_archive).map_err(|error| {
            format!(
                "copy package archive {} to {}: {error}",
                cargo_archive.display(),
                governed_archive.display()
            )
        })?;

        let package_extract_root = extracted_root.join(package);
        extract_and_validate_archive(
            &governed_archive,
            &package_extract_root,
            package,
            &package_version,
            &package_list,
        )?;
        validate_normalized_manifest(
            &package_extract_root.join("Cargo.toml"),
            package,
            &package_version,
            &validated.local_packages,
            validated.frozen,
        )?;
    }

    write_archive_workspace(
        &check_root,
        &extracted_root,
        &selected_profiles,
        &package_version,
    )?;
    run_cargo(
        &check_root,
        &["generate-lockfile"],
        "lock archive workspace",
    )?;
    for (package, _) in selected_profiles {
        let probe = archive_probe_package_name(&package);
        let args = ["check", "--locked", "-p", probe.as_str(), "--lib"];
        run_cargo(
            &check_root,
            &args,
            &format!("check extracted package {package}"),
        )?;
    }

    super::write_release_provenance(
        workspace_root,
        &archive_root,
        &closure_root.join("release-provenance.json"),
    )?;
    eprintln!(
        "release package closure verified under {}",
        closure_root.display()
    );
    Ok(())
}

fn load_toml<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, String> {
    let raw =
        fs::read_to_string(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    toml::from_str(&raw).map_err(|error| format!("parse {}: {error}", path.display()))
}

fn load_package_version(workspace_root: &Path) -> Result<String, String> {
    let architecture =
        load_toml::<CratesReleaseArchitecture>(&workspace_root.join(ARCHITECTURE_RELATIVE))?;
    if architecture.repositories.lib.version.trim().is_empty() {
        return Err(
            "crates release architecture repositories.lib.version must not be empty".to_owned(),
        );
    }
    Ok(architecture.repositories.lib.version)
}

fn validate_local_package_versions(
    workspace_packages: &[PackageWorkspaceEntry],
    local_packages: &BTreeSet<String>,
    package_version: &str,
) -> Result<(), String> {
    let workspace_versions = workspace_packages
        .iter()
        .map(|package| (package.name.as_str(), package.version.as_str()))
        .collect::<BTreeMap<_, _>>();
    for package in local_packages {
        let actual = workspace_versions
            .get(package.as_str())
            .ok_or_else(|| format!("approved local package {package} is not a workspace member"))?;
        if *actual != package_version {
            return Err(format!(
                "approved local package {package} version {actual} must match crates release architecture version {package_version}"
            ));
        }
    }
    Ok(())
}

fn validate_policy(policy: &ReleasePolicy) -> Result<ValidatedReleasePolicy, String> {
    if policy.schema.version != POLICY_SCHEMA_VERSION {
        return Err(format!(
            "release package policy schema.version must be {POLICY_SCHEMA_VERSION}"
        ));
    }
    if policy.release.version.trim().is_empty() {
        return Err("release package policy version must not be empty".to_owned());
    }
    if policy.publication.registry != "crates-io" {
        return Err("publication.registry must be crates-io".to_owned());
    }
    if policy.publication.final_enablement_step != 305 {
        return Err("publication.final_enablement_step must be 305".to_owned());
    }
    if policy.publication.spec_id != "radroots.crates.release.v1" {
        return Err("publication.spec_id must be radroots.crates.release.v1".to_owned());
    }
    let approved = unique_nonempty(
        &policy.publication.approved_packages,
        "publication.approved_packages",
    )?;
    let local = unique_nonempty(
        &policy.publication.local_packages,
        "publication.local_packages",
    )?;
    let external = unique_nonempty(
        &policy.publication.external_packages,
        "publication.external_packages",
    )?;
    if local.is_empty() {
        return Err("publication.local_packages must not be empty".to_owned());
    }
    if !local.is_disjoint(&external) {
        return Err("publication local and external package ownership must not overlap".to_owned());
    }
    let owned = local.union(&external).cloned().collect::<BTreeSet<_>>();
    if owned != approved {
        return Err(
            "publication local and external package ownership must partition approved_packages"
                .to_owned(),
        );
    }

    let classes = [
        (
            "workspace_classification.private",
            &policy.workspace_classification.private,
        ),
        (
            "workspace_classification.build_codegen",
            &policy.workspace_classification.build_codegen,
        ),
        (
            "workspace_classification.test_support",
            &policy.workspace_classification.test_support,
        ),
        (
            "workspace_classification.preview",
            &policy.workspace_classification.preview,
        ),
        (
            "workspace_classification.retired",
            &policy.workspace_classification.retired,
        ),
    ];
    let mut workspace_packages = local.clone();
    for (label, values) in classes {
        let unique = unique_nonempty(values, label)?;
        for value in unique {
            if approved.contains(&value) || !workspace_packages.insert(value.clone()) {
                return Err(format!(
                    "release package policy classifies {value} more than once"
                ));
            }
        }
    }
    let ordered = unique_nonempty(&policy.publish_order.crates, "publish_order.crates")?;
    let package_order = if policy.publication.frozen {
        if !ordered.is_empty() {
            return Err(
                "publish_order.crates must remain empty while publication is frozen".to_owned(),
            );
        }
        policy.publication.local_packages.clone()
    } else {
        if ordered != local {
            return Err(
                "publish_order.crates must contain every approved local package exactly once when publication is enabled"
                    .to_owned(),
            );
        }
        policy.publish_order.crates.clone()
    };
    Ok(ValidatedReleasePolicy {
        package_order,
        local_packages: local,
        workspace_packages,
        frozen: policy.publication.frozen,
    })
}

fn unique_nonempty(values: &[String], label: &str) -> Result<BTreeSet<String>, String> {
    let mut unique = BTreeSet::new();
    for value in values {
        if value.trim().is_empty() {
            return Err(format!("{label} contains an empty package name"));
        }
        if !unique.insert(value.clone()) {
            return Err(format!("{label} contains duplicate package {value}"));
        }
    }
    Ok(unique)
}

fn select_profiles(
    policy: &ValidatedReleasePolicy,
    profiles: &FeatureProfiles,
) -> Result<Vec<(String, FeatureProfile)>, String> {
    validate_profile("profiles.default", &profiles.default)?;
    for (package, profile) in &profiles.crates {
        if !policy.workspace_packages.contains(package) {
            return Err(format!(
                "release feature profile references unknown package {package}"
            ));
        }
        validate_profile(&format!("profiles.crates.{package}"), profile)?;
    }
    Ok(policy
        .package_order
        .iter()
        .map(|package| {
            (
                package.clone(),
                profiles
                    .crates
                    .get(package)
                    .unwrap_or(&profiles.default)
                    .clone(),
            )
        })
        .collect())
}

fn validate_profile(label: &str, profile: &FeatureProfile) -> Result<(), String> {
    if profile.test_threads == 0 {
        return Err(format!("{label}.test_threads must be positive"));
    }
    unique_nonempty(&profile.features, &format!("{label}.features"))?;
    Ok(())
}

fn cargo_target_root(workspace_root: &Path) -> Result<PathBuf, String> {
    let raw = env::var_os("CARGO_TARGET_DIR")
        .ok_or_else(|| "release package preflight requires CARGO_TARGET_DIR".to_owned())?;
    let target = PathBuf::from(raw);
    if !target.is_absolute() {
        return Err("release package preflight requires an absolute CARGO_TARGET_DIR".to_owned());
    }
    fs::create_dir_all(&target)
        .map_err(|error| format!("create CARGO_TARGET_DIR {}: {error}", target.display()))?;
    let target = fs::canonicalize(&target).map_err(|error| {
        format!(
            "canonicalize CARGO_TARGET_DIR {}: {error}",
            target.display()
        )
    })?;
    let workspace = fs::canonicalize(workspace_root)
        .map_err(|error| format!("canonicalize {}: {error}", workspace_root.display()))?;
    if target.starts_with(&workspace) || workspace.starts_with(&target) {
        return Err(
            "release package preflight CARGO_TARGET_DIR must be outside the source worktree"
                .to_owned(),
        );
    }
    Ok(target)
}

fn reset_owned_closure_directory(target_root: &Path, closure_root: &Path) -> Result<(), String> {
    if closure_root.parent() != Some(target_root)
        || closure_root.file_name().and_then(|name| name.to_str()) != Some(CLOSURE_DIRECTORY)
    {
        return Err("refusing to reset an unowned release package directory".to_owned());
    }
    match fs::symlink_metadata(closure_root) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => Err(format!(
            "release package closure path {} must be a real directory",
            closure_root.display()
        )),
        Ok(_) => fs::remove_dir_all(closure_root)
            .map_err(|error| format!("reset {}: {error}", closure_root.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("inspect {}: {error}", closure_root.display())),
    }
}

fn cargo_package_list(workspace_root: &Path, package: &str) -> Result<BTreeSet<String>, String> {
    let output = cargo_output(
        workspace_root,
        &["package", "--locked", "--list", "-p", package],
        &format!("list package {package}"),
    )?;
    let stdout = String::from_utf8(output.stdout)
        .map_err(|error| format!("package list for {package} must be UTF-8: {error}"))?;
    let mut paths = BTreeSet::new();
    for raw in stdout.lines() {
        let path = validated_relative_path(raw, &format!("package list for {package}"))?;
        if !paths.insert(path) {
            return Err(format!(
                "package list for {package} contains a duplicate path"
            ));
        }
    }
    if paths.is_empty() {
        return Err(format!("package list for {package} must not be empty"));
    }
    Ok(paths)
}

fn run_cargo(directory: &Path, args: &[&str], purpose: &str) -> Result<(), String> {
    cargo_output(directory, args, purpose).map(|_| ())
}

fn run_cargo_with_config(
    directory: &Path,
    config_path: &Path,
    args: &[&str],
    purpose: &str,
) -> Result<(), String> {
    let output = Command::new("cargo")
        .args(["--config"])
        .arg(config_path)
        .args(args)
        .current_dir(directory)
        .output()
        .map_err(|error| format!("{purpose}: run cargo {}: {error}", args.join(" ")))?;
    if !output.status.success() {
        return Err(format!(
            "{purpose}: cargo {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

fn cargo_output(directory: &Path, args: &[&str], purpose: &str) -> Result<Output, String> {
    let output = Command::new("cargo")
        .args(args)
        .current_dir(directory)
        .output()
        .map_err(|error| format!("{purpose}: run cargo {}: {error}", args.join(" ")))?;
    if !output.status.success() {
        return Err(format!(
            "{purpose}: cargo {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(output)
}

fn require_regular_file(path: &Path, label: &str) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("inspect {label} {}: {error}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() == 0 {
        return Err(format!(
            "{label} {} must be a nonempty regular file",
            path.display()
        ));
    }
    Ok(())
}

fn extract_and_validate_archive(
    archive_path: &Path,
    destination: &Path,
    package: &str,
    version: &str,
    expected_files: &BTreeSet<String>,
) -> Result<(), String> {
    if destination.exists() {
        return Err(format!(
            "archive extraction destination {} already exists",
            destination.display()
        ));
    }
    fs::create_dir_all(destination)
        .map_err(|error| format!("create {}: {error}", destination.display()))?;
    let expected_root = format!("{package}-{version}");
    let mut archived_files = BTreeSet::new();
    let archive_entries = tar_archive_entries(archive_path)?;
    for (archive_entry, entry_type) in archive_entries {
        let relative = archive_relative_path(&archive_entry, &expected_root)?;
        if entry_type == 'd' {
            continue;
        }
        if relative.as_os_str().is_empty() {
            return Err("package archive root entry is not a package file".to_owned());
        }
        if entry_type != '-' {
            return Err(format!(
                "package archive {} contains a non-file entry {}",
                archive_path.display(),
                archive_entry.display()
            ));
        }
        let relative_string = path_to_slash_string(&relative)?;
        reject_disposable_package_path(&relative_string)?;
        if !archived_files.insert(relative_string.clone()) {
            return Err(format!(
                "package archive {} contains duplicate path {relative_string}",
                archive_path.display()
            ));
        }
    }
    if &archived_files != expected_files {
        let missing = expected_files
            .difference(&archived_files)
            .cloned()
            .collect::<Vec<_>>();
        let extra = archived_files
            .difference(expected_files)
            .cloned()
            .collect::<Vec<_>>();
        return Err(format!(
            "package list/archive mismatch for {package}; missing: {}; extra: {}",
            missing.join(", "),
            extra.join(", ")
        ));
    }
    for required in ["Cargo.toml", "Cargo.toml.orig", "README.md"] {
        if !archived_files.contains(required) {
            return Err(format!(
                "package archive for {package} is missing {required}"
            ));
        }
    }
    if !archived_files.iter().any(|path| path.starts_with("src/")) {
        return Err(format!("package archive for {package} has no src files"));
    }
    let extraction = Command::new("tar")
        .args(["-xf"])
        .arg(archive_path)
        .args(["-C"])
        .arg(destination)
        .arg("--strip-components=1")
        .output()
        .map_err(|error| format!("extract {} with tar: {error}", archive_path.display()))?;
    if !extraction.status.success() {
        return Err(format!(
            "extract {} with tar failed: {}",
            archive_path.display(),
            String::from_utf8_lossy(&extraction.stderr).trim()
        ));
    }
    let extracted_files = collect_extracted_files(destination)?;
    if extracted_files != archived_files {
        return Err(format!(
            "extracted package inventory for {package} differs from its archive"
        ));
    }
    Ok(())
}

fn tar_archive_entries(archive_path: &Path) -> Result<Vec<(PathBuf, char)>, String> {
    let paths = Command::new("tar")
        .args(["-tf"])
        .arg(archive_path)
        .output()
        .map_err(|error| format!("list {} with tar: {error}", archive_path.display()))?;
    if !paths.status.success() {
        return Err(format!(
            "list {} with tar failed: {}",
            archive_path.display(),
            String::from_utf8_lossy(&paths.stderr).trim()
        ));
    }
    let verbose = Command::new("tar")
        .args(["-tvf"])
        .arg(archive_path)
        .output()
        .map_err(|error| format!("inspect {} with tar: {error}", archive_path.display()))?;
    if !verbose.status.success() {
        return Err(format!(
            "inspect {} with tar failed: {}",
            archive_path.display(),
            String::from_utf8_lossy(&verbose.stderr).trim()
        ));
    }
    let path_output = String::from_utf8(paths.stdout)
        .map_err(|error| format!("archive paths must be UTF-8: {error}"))?;
    let verbose_output = String::from_utf8(verbose.stdout)
        .map_err(|error| format!("archive metadata must be UTF-8: {error}"))?;
    let path_lines = path_output.lines().collect::<Vec<_>>();
    let type_lines = verbose_output.lines().collect::<Vec<_>>();
    if path_lines.len() != type_lines.len() || path_lines.is_empty() {
        return Err(format!(
            "archive {} path and type inventories differ or are empty",
            archive_path.display()
        ));
    }
    path_lines
        .into_iter()
        .zip(type_lines)
        .map(|(path, metadata)| {
            let entry_type = metadata
                .chars()
                .next()
                .ok_or_else(|| "tar emitted empty archive metadata".to_owned())?;
            Ok((PathBuf::from(path), entry_type))
        })
        .collect()
}

fn archive_relative_path(path: &Path, expected_root: &str) -> Result<PathBuf, String> {
    let mut components = path.components();
    match components.next() {
        Some(Component::Normal(root)) if root == expected_root => {}
        _ => {
            return Err(format!(
                "package archive entry {} is outside root {expected_root}",
                path.display()
            ));
        }
    }
    let mut relative = PathBuf::new();
    for component in components {
        match component {
            Component::Normal(value) => relative.push(value),
            _ => {
                return Err(format!(
                    "package archive entry {} contains an unsafe component",
                    path.display()
                ));
            }
        }
    }
    Ok(relative)
}

fn validated_relative_path(raw: &str, label: &str) -> Result<String, String> {
    if raw.is_empty() || raw.contains('\\') {
        return Err(format!("{label} contains an invalid path {raw:?}"));
    }
    let path = Path::new(raw);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!("{label} contains an unsafe path {raw:?}"));
    }
    reject_disposable_package_path(raw)?;
    Ok(raw.to_owned())
}

fn reject_disposable_package_path(path: &str) -> Result<(), String> {
    let first = path.split('/').next().unwrap_or(path);
    if matches!(first, ".git" | ".beads" | "target" | "result" | ".local")
        || matches!(
            path,
            "status_extbuild_resume_usage.txt"
                | "status_radroots_ios_phase_1_foundational_events_update_6_2026-07-26.md"
                | "handoff-radroots-ios-phase-1-foundational-events-2026-07-26.txt"
                | "prompt-resume-radroots-ios-phase-1-foundational-events.txt"
        )
    {
        return Err(format!("package inventory contains disposable path {path}"));
    }
    Ok(())
}

fn path_to_slash_string(path: &Path) -> Result<String, String> {
    path.components()
        .map(|component| match component {
            Component::Normal(value) => value
                .to_str()
                .map(str::to_owned)
                .ok_or_else(|| "package archive paths must be UTF-8".to_owned()),
            _ => Err("package archive path must be relative".to_owned()),
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|parts| parts.join("/"))
}

fn collect_extracted_files(root: &Path) -> Result<BTreeSet<String>, String> {
    fn visit(root: &Path, directory: &Path, files: &mut BTreeSet<String>) -> Result<(), String> {
        for entry in fs::read_dir(directory)
            .map_err(|error| format!("read {}: {error}", directory.display()))?
        {
            let entry = entry.map_err(|error| format!("read extracted entry: {error}"))?;
            let metadata = fs::symlink_metadata(entry.path())
                .map_err(|error| format!("inspect {}: {error}", entry.path().display()))?;
            if metadata.file_type().is_symlink() {
                return Err(format!(
                    "extracted package contains symlink {}",
                    entry.path().display()
                ));
            }
            if metadata.is_dir() {
                visit(root, &entry.path(), files)?;
            } else if metadata.is_file() {
                let relative = entry
                    .path()
                    .strip_prefix(root)
                    .map_err(|error| format!("resolve extracted path: {error}"))?
                    .to_path_buf();
                files.insert(path_to_slash_string(&relative)?);
            } else {
                return Err(format!(
                    "extracted package contains non-file {}",
                    entry.path().display()
                ));
            }
        }
        Ok(())
    }
    let mut files = BTreeSet::new();
    visit(root, root, &mut files)?;
    Ok(files)
}

fn validate_normalized_manifest(
    manifest_path: &Path,
    expected_package: &str,
    release_version: &str,
    public: &BTreeSet<String>,
    publication_frozen: bool,
) -> Result<(), String> {
    let raw = fs::read_to_string(manifest_path)
        .map_err(|error| format!("read {}: {error}", manifest_path.display()))?;
    let manifest = toml::from_str::<toml::Value>(&raw)
        .map_err(|error| format!("parse {}: {error}", manifest_path.display()))?;
    let package = manifest
        .get("package")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{} has no package table", manifest_path.display()))?;
    if package.get("name").and_then(toml::Value::as_str) != Some(expected_package)
        || package.get("version").and_then(toml::Value::as_str) != Some(release_version)
    {
        return Err(format!(
            "normalized manifest for {expected_package} has wrong package identity"
        ));
    }
    let publish = package
        .get("publish")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("normalized manifest for {expected_package} has no publish list"))?;
    let expected_publish = if publication_frozen {
        Vec::new()
    } else {
        vec![toml::Value::String("crates-io".to_owned())]
    };
    if publish != &expected_publish {
        return Err(format!(
            "normalized manifest for {expected_package} has publish authority inconsistent with the publication freeze"
        ));
    }

    for section in ["dependencies", "build-dependencies"] {
        validate_dependency_table(
            manifest.get(section),
            expected_package,
            section,
            release_version,
            public,
            true,
        )?;
    }
    validate_dependency_table(
        manifest.get("dev-dependencies"),
        expected_package,
        "dev-dependencies",
        release_version,
        public,
        false,
    )?;
    if let Some(targets) = manifest.get("target").and_then(toml::Value::as_table) {
        for (target, target_value) in targets {
            let target_table = target_value
                .as_table()
                .ok_or_else(|| format!("normalized manifest target {target} must be a table"))?;
            for section in ["dependencies", "build-dependencies"] {
                validate_dependency_table(
                    target_table.get(section),
                    expected_package,
                    &format!("target.{target}.{section}"),
                    release_version,
                    public,
                    true,
                )?;
            }
            validate_dependency_table(
                target_table.get("dev-dependencies"),
                expected_package,
                &format!("target.{target}.dev-dependencies"),
                release_version,
                public,
                false,
            )?;
        }
    }
    Ok(())
}

fn validate_dependency_table(
    value: Option<&toml::Value>,
    owner: &str,
    section: &str,
    release_version: &str,
    public: &BTreeSet<String>,
    require_public_radroots: bool,
) -> Result<(), String> {
    let Some(value) = value else {
        return Ok(());
    };
    let table = value.as_table().ok_or_else(|| {
        format!("normalized manifest {owner} {section} must be a dependency table")
    })?;
    for (key, dependency) in table {
        let (package, version) = match dependency {
            toml::Value::String(version) => (key.as_str(), version.as_str()),
            toml::Value::Table(configuration) => {
                for forbidden in ["path", "git", "branch", "rev", "tag"] {
                    if configuration.contains_key(forbidden) {
                        return Err(format!(
                            "normalized manifest {owner} {section}.{key} contains forbidden {forbidden} source authority"
                        ));
                    }
                }
                let package = configuration
                    .get("package")
                    .and_then(toml::Value::as_str)
                    .unwrap_or(key);
                let version = configuration
                    .get("version")
                    .and_then(toml::Value::as_str)
                    .ok_or_else(|| {
                        format!(
                            "normalized manifest {owner} {section}.{key} has no registry version"
                        )
                    })?;
                (package, version)
            }
            _ => {
                return Err(format!(
                    "normalized manifest {owner} {section}.{key} has invalid dependency configuration"
                ));
            }
        };
        if package.starts_with("radroots_") {
            if require_public_radroots && !public.contains(package) {
                return Err(format!(
                    "normalized manifest {owner} {section}.{key} references non-public package {package}"
                ));
            }
            if version != format!("={release_version}") {
                return Err(format!(
                    "normalized manifest {owner} {section}.{key} must pin {package} to ={release_version}"
                ));
            }
        }
    }
    Ok(())
}

fn load_workspace_packages(workspace_root: &Path) -> Result<Vec<PackageWorkspaceEntry>, String> {
    let output = cargo_output(
        workspace_root,
        &["metadata", "--locked", "--format-version", "1"],
        "load release package workspace metadata",
    )?;
    let metadata = serde_json::from_slice::<CargoMetadata>(&output.stdout)
        .map_err(|error| format!("parse cargo metadata for release packages: {error}"))?;
    let members = metadata
        .workspace_members
        .into_iter()
        .collect::<BTreeSet<_>>();
    let mut packages = Vec::new();
    let mut found = BTreeSet::new();
    for package in metadata.packages {
        if !members.contains(&package.id) {
            continue;
        }
        found.insert(package.id);
        let source_directory = package
            .manifest_path
            .parent()
            .ok_or_else(|| format!("package {} manifest has no parent", package.name))?
            .to_path_buf();
        packages.push(PackageWorkspaceEntry {
            name: package.name,
            version: package.version,
            source_directory,
        });
    }
    if found != members {
        return Err("cargo metadata omitted one or more workspace package records".to_owned());
    }
    packages.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(packages)
}

fn write_source_patch_config(
    closure_root: &Path,
    packages: &[PackageWorkspaceEntry],
) -> Result<PathBuf, String> {
    let config_path = closure_root.join("source-patches.toml");
    let mut config = String::from("[patch.crates-io]\n");
    for package in packages {
        config.push_str(&format!(
            "{} = {{ path = {:?} }}\n",
            package.name, package.source_directory
        ));
    }
    fs::write(&config_path, config)
        .map_err(|error| format!("write source patch config: {error}"))?;
    Ok(config_path)
}

fn write_archive_workspace(
    workspace_root: &Path,
    extracted_root: &Path,
    profiles: &[(String, FeatureProfile)],
    release_version: &str,
) -> Result<(), String> {
    let probes_root = workspace_root.join("probes");
    fs::create_dir_all(&probes_root)
        .map_err(|error| format!("create {}: {error}", probes_root.display()))?;
    let mut manifest = String::from("[workspace]\nresolver = \"2\"\nmembers = [\n");
    for (package, profile) in profiles {
        let probe = archive_probe_package_name(package);
        let probe_root = probes_root.join(&probe);
        fs::create_dir_all(probe_root.join("src"))
            .map_err(|error| format!("create {}: {error}", probe_root.display()))?;
        let features = profile
            .features
            .iter()
            .map(|feature| format!("{feature:?}"))
            .collect::<Vec<_>>()
            .join(", ");
        let probe_manifest = format!(
            "[package]\nname = {probe:?}\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n[dependencies.candidate]\npackage = {package:?}\nversion = \"={release_version}\"\ndefault-features = {}\nfeatures = [{features}]\n",
            !profile.no_default_features
        );
        fs::write(probe_root.join("Cargo.toml"), probe_manifest)
            .map_err(|error| format!("write archive probe manifest for {package}: {error}"))?;
        fs::write(
            probe_root.join("src/lib.rs"),
            "#![forbid(unsafe_code)]\n\npub fn archive_profile_probe() {}\n",
        )
        .map_err(|error| format!("write archive probe source for {package}: {error}"))?;
        manifest.push_str(&format!("  {:?},\n", PathBuf::from("probes").join(probe)));
    }
    manifest.push_str("]\n\n[patch.crates-io]\n");
    for (package, _) in profiles {
        manifest.push_str(&format!(
            "{package} = {{ path = {:?} }}\n",
            extracted_root.join(package)
        ));
    }
    fs::write(workspace_root.join("Cargo.toml"), manifest)
        .map_err(|error| format!("write archive workspace manifest: {error}"))
}

fn archive_probe_package_name(package: &str) -> String {
    format!("release_probe_{package}")
}

fn ensure_clean_git_worktree(workspace_root: &Path) -> Result<(), String> {
    let output = Command::new("git")
        .args(["status", "--porcelain=v1", "--untracked-files=all"])
        .current_dir(workspace_root)
        .output()
        .map_err(|error| format!("inspect release package Git worktree: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "inspect release package Git worktree failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    if !output.stdout.is_empty() {
        return Err("release package preflight requires an exact clean Git worktree".to_owned());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_package_paths_reject_escapes_and_disposable_files() {
        assert_eq!(
            validated_relative_path("src/lib.rs", "fixture").expect("valid path"),
            "src/lib.rs"
        );
        for invalid in [
            "",
            "/absolute",
            "../escape",
            "src/../escape",
            "target/output",
            ".beads/state",
            "status_extbuild_resume_usage.txt",
        ] {
            assert!(validated_relative_path(invalid, "fixture").is_err());
        }
    }

    #[test]
    fn normalized_manifest_requires_public_exact_registry_dependencies() {
        let root = tempfile::tempdir().expect("temporary manifest root");
        let manifest = root.path().join("Cargo.toml");
        fs::write(
            &manifest,
            r#"[package]
name = "radroots_public"
version = "1.0.0-alpha.1"
publish = ["crates-io"]

[dependencies.public_alias]
package = "radroots_dependency"
version = "=1.0.0-alpha.1"
optional = true

[target.'cfg(unix)'.build-dependencies.serde]
version = "1"

[dev-dependencies.radroots_internal]
version = "=1.0.0-alpha.1"
"#,
        )
        .expect("write manifest");
        let public = BTreeSet::from([
            "radroots_dependency".to_owned(),
            "radroots_public".to_owned(),
        ]);
        validate_normalized_manifest(
            &manifest,
            "radroots_public",
            "1.0.0-alpha.1",
            &public,
            false,
        )
        .expect("valid normalized manifest");

        let invalid = fs::read_to_string(&manifest)
            .expect("read manifest")
            .replace("radroots_dependency", "radroots_internal");
        fs::write(&manifest, invalid).expect("write invalid manifest");
        assert!(
            validate_normalized_manifest(
                &manifest,
                "radroots_public",
                "1.0.0-alpha.1",
                &public,
                false,
            )
            .expect_err("internal production dependency must fail")
            .contains("non-public package radroots_internal")
        );
    }

    fn frozen_policy() -> ReleasePolicy {
        ReleasePolicy {
            schema: PolicySchema {
                version: POLICY_SCHEMA_VERSION,
            },
            release: ReleaseVersion {
                version: "1.0.0-alpha.1".to_owned(),
            },
            publication: PublicationControl {
                frozen: true,
                registry: "crates-io".to_owned(),
                final_enablement_step: 305,
                spec_id: "radroots.crates.release.v1".to_owned(),
                approved_packages: vec![
                    "radroots_public".to_owned(),
                    "radroots_external".to_owned(),
                ],
                local_packages: vec!["radroots_public".to_owned()],
                external_packages: vec!["radroots_external".to_owned()],
            },
            workspace_classification: WorkspaceReleaseClassification {
                private: vec!["radroots_internal".to_owned()],
                build_codegen: vec!["xtask".to_owned()],
                test_support: Vec::new(),
                preview: Vec::new(),
                retired: Vec::new(),
            },
            publish_order: PublishOrder { crates: Vec::new() },
        }
    }

    #[test]
    fn frozen_policy_packages_local_approved_crates_without_enabling_publication() {
        let validated = validate_policy(&frozen_policy()).expect("valid frozen policy");
        assert_eq!(validated.package_order, ["radroots_public"]);
        assert!(validated.frozen);

        let mut invalid = frozen_policy();
        invalid
            .publish_order
            .crates
            .push("radroots_public".to_owned());
        assert!(
            validate_policy(&invalid)
                .unwrap_err()
                .contains("must remain empty while publication is frozen")
        );
    }

    #[test]
    fn approved_local_packages_use_the_crate_architecture_version() {
        let packages = [PackageWorkspaceEntry {
            name: "radroots_public".to_owned(),
            version: "0.1.0-alpha".to_owned(),
            source_directory: PathBuf::from("crates/public"),
        }];
        let local = BTreeSet::from(["radroots_public".to_owned()]);
        validate_local_package_versions(&packages, &local, "0.1.0-alpha")
            .expect("matching architecture version");
        assert!(
            validate_local_package_versions(&packages, &local, "1.0.0-alpha.1")
                .unwrap_err()
                .contains("must match crates release architecture version")
        );
    }

    #[test]
    fn frozen_normalized_manifest_requires_an_empty_publish_list() {
        let root = tempfile::tempdir().expect("temporary manifest root");
        let manifest = root.path().join("Cargo.toml");
        fs::write(
            &manifest,
            "[package]\nname = 'radroots_public'\nversion = '0.1.0-alpha'\npublish = []\n",
        )
        .expect("write frozen manifest");
        let public = BTreeSet::from(["radroots_public".to_owned()]);
        validate_normalized_manifest(&manifest, "radroots_public", "0.1.0-alpha", &public, true)
            .expect("frozen package manifest");
    }

    #[test]
    fn archive_extraction_matches_the_exact_package_list() {
        let root = tempfile::tempdir().expect("temporary archive root");
        let package_root = root.path().join("radroots_fixture-1.0.0");
        fs::create_dir_all(package_root.join("src")).expect("create package fixture");
        fs::write(
            package_root.join("Cargo.toml"),
            "[package]\nname='radroots_fixture'\nversion='1.0.0'\npublish=['crates-io']\n",
        )
        .expect("write Cargo manifest");
        fs::write(package_root.join("Cargo.toml.orig"), "original")
            .expect("write original manifest");
        fs::write(package_root.join("README.md"), "readme").expect("write README");
        fs::write(package_root.join("src/lib.rs"), "pub fn fixture() {}").expect("write source");
        let archive_path = root.path().join("radroots_fixture-1.0.0.crate");
        let status = Command::new("tar")
            .args(["-czf"])
            .arg(&archive_path)
            .args(["-C"])
            .arg(root.path())
            .arg("radroots_fixture-1.0.0")
            .status()
            .expect("create fixture archive with tar");
        assert!(status.success());

        let expected = BTreeSet::from([
            "Cargo.toml".to_owned(),
            "Cargo.toml.orig".to_owned(),
            "README.md".to_owned(),
            "src/lib.rs".to_owned(),
        ]);
        let destination = root.path().join("extracted");
        extract_and_validate_archive(
            &archive_path,
            &destination,
            "radroots_fixture",
            "1.0.0",
            &expected,
        )
        .expect("extract exact archive");
        let contents =
            fs::read_to_string(destination.join("README.md")).expect("read extracted README");
        assert_eq!(contents, "readme");
    }

    #[test]
    fn source_patch_config_maps_workspace_packages_without_touching_sources() {
        let root = tempfile::tempdir().expect("temporary closure root");
        let packages = [PackageWorkspaceEntry {
            name: "radroots_fixture".to_owned(),
            version: "0.1.0-alpha".to_owned(),
            source_directory: root.path().join("source"),
        }];
        let config_path =
            write_source_patch_config(root.path(), &packages).expect("write source patches");
        let config = load_toml::<toml::Value>(&config_path).expect("parse source patches");
        assert_eq!(
            config["patch"]["crates-io"]["radroots_fixture"]["path"].as_str(),
            packages[0].source_directory.to_str()
        );
    }

    #[test]
    fn archive_workspace_profiles_extracted_crates_through_dependency_probes() {
        let root = tempfile::tempdir().expect("temporary archive workspace root");
        let workspace = root.path().join("workspace");
        let extracted = workspace.join("members");
        fs::create_dir_all(&extracted).expect("create extracted root");
        let profiles = [(
            "radroots_fixture".to_owned(),
            FeatureProfile {
                no_default_features: true,
                features: vec!["serde".to_owned()],
                test_threads: 1,
            },
        )];
        write_archive_workspace(&workspace, &extracted, &profiles, "1.0.0-alpha.1")
            .expect("write archive workspace");

        let probe = workspace
            .join("probes")
            .join("release_probe_radroots_fixture");
        let manifest = fs::read_to_string(probe.join("Cargo.toml")).expect("read probe manifest");
        assert!(manifest.contains("package = \"radroots_fixture\""));
        assert!(manifest.contains("default-features = false"));
        assert!(manifest.contains("features = [\"serde\"]"));
        let workspace_manifest =
            fs::read_to_string(workspace.join("Cargo.toml")).expect("read workspace manifest");
        assert!(workspace_manifest.contains("probes/release_probe_radroots_fixture"));
        assert!(
            workspace_manifest.contains(
                extracted
                    .join("radroots_fixture")
                    .to_str()
                    .expect("UTF-8 fixture path")
            )
        );
    }
}
