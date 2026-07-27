use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output};

const POLICY_RELATIVE: &str = "contracts/releases/publish_policy.toml";
const PROFILES_RELATIVE: &str = "contracts/coverage-profiles.toml";
const POLICY_SCHEMA_VERSION: u32 = 1;
const CLOSURE_DIRECTORY: &str = "release-package-closure";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleasePolicy {
    schema: PolicySchema,
    release: ReleaseVersion,
    classification: ReleaseClassification,
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
    manifest_path: PathBuf,
}

struct PackageWorkspaceEntry {
    name: String,
    source_directory: PathBuf,
}

pub(crate) fn validate_release_packages(workspace_root: &Path) -> Result<(), String> {
    ensure_clean_git_worktree(workspace_root)?;
    let policy = load_toml::<ReleasePolicy>(&workspace_root.join(POLICY_RELATIVE))?;
    let public = validate_policy(&policy)?;
    let profiles = load_toml::<FeatureProfilesFile>(&workspace_root.join(PROFILES_RELATIVE))?;
    let selected_profiles = select_profiles(&policy, &profiles.profiles)?;
    let workspace_packages = load_workspace_packages(workspace_root)?;

    let cargo_target_root = cargo_target_root(workspace_root)?;
    let closure_root = cargo_target_root.join(CLOSURE_DIRECTORY);
    reset_owned_closure_directory(&cargo_target_root, &closure_root)?;
    let archive_root = closure_root.join("archives");
    let extracted_root = closure_root.join("extracted");
    let list_root = closure_root.join("package-lists");
    let check_root = closure_root.join("archive-workspace");
    for directory in [&archive_root, &extracted_root, &list_root, &check_root] {
        fs::create_dir_all(directory)
            .map_err(|error| format!("create {}: {error}", directory.display()))?;
    }
    let source_patch_config = write_source_patch_config(&closure_root, &workspace_packages)?;

    for package in &policy.publish_order.crates {
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
        let filename = format!("{package}-{}.crate", policy.release.version);
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
            &policy.release.version,
            &package_list,
        )?;
        validate_normalized_manifest(
            &package_extract_root.join("Cargo.toml"),
            package,
            &policy.release.version,
            &public,
        )?;
    }

    write_archive_workspace(&check_root, &extracted_root, &workspace_packages, &public)?;
    run_cargo(
        &check_root,
        &["generate-lockfile"],
        "lock archive workspace",
    )?;
    for (package, profile) in selected_profiles {
        let mut args = vec!["check", "--locked", "-p", package.as_str(), "--lib"];
        if profile.no_default_features {
            args.push("--no-default-features");
        }
        let feature_argument;
        if !profile.features.is_empty() {
            feature_argument = profile.features.join(",");
            args.push("--features");
            args.push(&feature_argument);
        }
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

fn validate_policy(policy: &ReleasePolicy) -> Result<BTreeSet<String>, String> {
    if policy.schema.version != POLICY_SCHEMA_VERSION {
        return Err(format!(
            "release package policy schema.version must be {POLICY_SCHEMA_VERSION}"
        ));
    }
    if policy.release.version.trim().is_empty() {
        return Err("release package policy version must not be empty".to_owned());
    }
    let classes = [
        ("classification.public", &policy.classification.public),
        ("classification.internal", &policy.classification.internal),
        ("classification.deferred", &policy.classification.deferred),
        ("classification.retired", &policy.classification.retired),
        ("classification.yank_only", &policy.classification.yank_only),
    ];
    let mut classified = BTreeSet::new();
    for (label, values) in classes {
        let unique = unique_nonempty(values, label)?;
        for value in unique {
            if !classified.insert(value.clone()) {
                return Err(format!(
                    "release package policy classifies {value} more than once"
                ));
            }
        }
    }
    let public = unique_nonempty(&policy.classification.public, "classification.public")?;
    let ordered = unique_nonempty(&policy.publish_order.crates, "publish_order.crates")?;
    if ordered != public {
        return Err(
            "release package publish order must contain every public package exactly once"
                .to_owned(),
        );
    }
    Ok(public)
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
    policy: &ReleasePolicy,
    profiles: &FeatureProfiles,
) -> Result<Vec<(String, FeatureProfile)>, String> {
    validate_profile("profiles.default", &profiles.default)?;
    let classified = policy
        .classification
        .public
        .iter()
        .chain(&policy.classification.internal)
        .chain(&policy.classification.deferred)
        .chain(&policy.classification.retired)
        .chain(&policy.classification.yank_only)
        .cloned()
        .collect::<BTreeSet<_>>();
    for (package, profile) in &profiles.crates {
        if !classified.contains(package) {
            return Err(format!(
                "release feature profile references unknown package {package}"
            ));
        }
        validate_profile(&format!("profiles.crates.{package}"), profile)?;
    }
    Ok(policy
        .publish_order
        .crates
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
    for required in ["Cargo.toml", "Cargo.toml.orig", "README"] {
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
    if publish.as_slice() != [toml::Value::String("crates-io".to_owned())] {
        return Err(format!(
            "normalized manifest for {expected_package} is not crates.io-only"
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
    packages: &[PackageWorkspaceEntry],
    public: &BTreeSet<String>,
) -> Result<(), String> {
    let mut manifest = String::from("[workspace]\nresolver = \"2\"\nmembers = [\n");
    for package in public {
        manifest.push_str(&format!("  {:?},\n", extracted_root.join(package)));
    }
    manifest.push_str("]\n\n[patch.crates-io]\n");
    for package in packages {
        let path = if public.contains(&package.name) {
            extracted_root.join(&package.name)
        } else {
            package.source_directory.clone()
        };
        manifest.push_str(&format!("{} = {{ path = {:?} }}\n", package.name, path));
    }
    fs::write(workspace_root.join("Cargo.toml"), manifest)
        .map_err(|error| format!("write archive workspace manifest: {error}"))
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
        validate_normalized_manifest(&manifest, "radroots_public", "1.0.0-alpha.1", &public)
            .expect("valid normalized manifest");

        let invalid = fs::read_to_string(&manifest)
            .expect("read manifest")
            .replace("radroots_dependency", "radroots_internal");
        fs::write(&manifest, invalid).expect("write invalid manifest");
        assert!(
            validate_normalized_manifest(&manifest, "radroots_public", "1.0.0-alpha.1", &public,)
                .expect_err("internal production dependency must fail")
                .contains("non-public package radroots_internal")
        );
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
        fs::write(package_root.join("README"), "readme").expect("write README");
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
            "README".to_owned(),
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
            fs::read_to_string(destination.join("README")).expect("read extracted README");
        assert_eq!(contents, "readme");
    }

    #[test]
    fn source_patch_config_maps_workspace_packages_without_touching_sources() {
        let root = tempfile::tempdir().expect("temporary closure root");
        let packages = [PackageWorkspaceEntry {
            name: "radroots_fixture".to_owned(),
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
}
