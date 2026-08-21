use std::{
    collections::BTreeSet,
    ffi::OsStr,
    fmt, fs,
    io::{Read as _, Seek as _, SeekFrom, Write as _},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use serde::Deserialize;
use sha2::{Digest as _, Sha256};
use tempfile::{NamedTempFile, TempDir};
use walkdir::WalkDir;

use crate::service_source_lock::{
    ContractVersions, LIB_REPOSITORY, LOCK_FILENAME, ServiceSourceLockParts, ServiceSourceLockV1,
};

const CATALOG_RELATIVE: &str = "contracts/crates/catalog.v2.toml";
const CARGO_MANIFEST: &str = "Cargo.toml";
const CARGO_LOCK: &str = "Cargo.lock";
const FLAKE_LOCK: &str = "flake.lock";
const RUST_TOOLCHAIN: &str = "rust-toolchain.toml";
const LIB_VERSION_REQUIREMENT: &str = "=0.1.0-alpha";
const HOST_PACKAGE: &str = "radroots_service_host";
const HOST_FEATURE_PROFILE: &str = "service-host";
const RUST_VERSION: &str = "1.97.1";
const MAX_MANIFEST_BYTES: usize = 1_048_576;
const MAX_CARGO_LOCK_BYTES: usize = 16_777_216;
const MAX_FLAKE_LOCK_BYTES: usize = 4_194_304;
const MAX_TOOLCHAIN_BYTES: usize = 65_536;
const MAX_CATALOG_BYTES: usize = 4_194_304;
const MAX_GIT_OUTPUT_BYTES: usize = 65_536;
const MAX_ARCHIVE_BYTES: u64 = 1_073_741_824;
const MAX_TREE_ENTRIES: usize = 16_384;
const MAX_MANIFESTS: usize = 512;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommandMode {
    Check,
    Write,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CommandError {
    InvalidServiceRoot,
    DirtyServiceSource,
    InvalidServiceMetadata,
    InvalidCargoManifest,
    InvalidCargoLock,
    InvalidFlakeLock,
    InvalidToolchain,
    InvalidSourceArchive,
    UnreachableRevision,
    InvalidSourceLock,
    StaleSourceLock,
    WriteFailure,
}

impl fmt::Display for CommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidServiceRoot => "service source-lock root is invalid",
            Self::DirtyServiceSource => "service source contains an ungoverned change",
            Self::InvalidServiceMetadata => "service source-lock metadata is invalid",
            Self::InvalidCargoManifest => "service Cargo manifest dependency is invalid",
            Self::InvalidCargoLock => "service Cargo lock is invalid",
            Self::InvalidFlakeLock => "service flake lock is invalid",
            Self::InvalidToolchain => "service Rust toolchain is invalid",
            Self::InvalidSourceArchive => "Lib source archive is invalid",
            Self::UnreachableRevision => "Lib revision is not remotely reachable",
            Self::InvalidSourceLock => "service source lock is invalid",
            Self::StaleSourceLock => "service source lock is stale",
            Self::WriteFailure => "service source lock could not be updated",
        })
    }
}

impl std::error::Error for CommandError {}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ServiceMetadata {
    service: String,
    contract_versions: ContractVersions,
}

#[derive(Debug, Deserialize)]
struct CargoLockDocument {
    #[serde(default)]
    package: Vec<CargoLockPackage>,
}

#[derive(Debug, Deserialize)]
struct CargoLockPackage {
    name: String,
    version: String,
    source: Option<String>,
}

struct ArchiveEvidence {
    revision: String,
    archive_sha256: String,
    catalog_sha256: String,
    package_names: BTreeSet<String>,
}

trait RevisionReachability {
    fn verify(&self, revision: &str) -> Result<(), CommandError>;
}

struct PublicLibRemote;

impl RevisionReachability for PublicLibRemote {
    fn verify(&self, revision: &str) -> Result<(), CommandError> {
        let repository = TempDir::new().map_err(|_| CommandError::UnreachableRevision)?;
        git_status(repository.path(), ["init", "--bare", "--quiet"])
            .map_err(|_| CommandError::UnreachableRevision)?;
        let status = Command::new("git")
            .args(["fetch", "--quiet", "--no-tags", "--depth=1", LIB_REPOSITORY])
            .arg(revision)
            .current_dir(repository.path())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|_| CommandError::UnreachableRevision)?;
        if !status.success() {
            return Err(CommandError::UnreachableRevision);
        }
        git_status(repository.path(), ["cat-file", "-e", revision])
            .map_err(|_| CommandError::UnreachableRevision)
    }
}

pub(crate) fn run(
    mode: CommandMode,
    service_root: &Path,
    source_archive: &Path,
) -> Result<(), String> {
    run_with(mode, service_root, source_archive, &PublicLibRemote)
        .map_err(|error| error.to_string())
}

fn run_with(
    mode: CommandMode,
    service_root: &Path,
    source_archive: &Path,
    reachability: &dyn RevisionReachability,
) -> Result<(), CommandError> {
    let service_root = validate_service_root(service_root)?;
    let initial_head = service_head(&service_root)?;
    validate_service_cleanliness(&service_root)?;

    let root_manifest = read_bounded_regular(
        &service_root.join(CARGO_MANIFEST),
        MAX_MANIFEST_BYTES,
        CommandError::InvalidCargoManifest,
    )?;
    let root_manifest = parse_toml(&root_manifest, CommandError::InvalidCargoManifest)?;
    let metadata = parse_service_metadata(&root_manifest)?;
    let revision = validate_cargo_manifests(&service_root, None)?;

    let flake_lock = read_bounded_regular(
        &service_root.join(FLAKE_LOCK),
        MAX_FLAKE_LOCK_BYTES,
        CommandError::InvalidFlakeLock,
    )?;
    validate_flake_lock(&flake_lock, &revision)?;
    let flake_lock_sha256 = sha256(&flake_lock);

    validate_toolchain(&service_root)?;
    let archive = validate_archive(source_archive, &revision)?;
    let confirmed_revision = validate_cargo_manifests(&service_root, Some(&archive.package_names))?;
    if confirmed_revision != revision {
        return Err(CommandError::InvalidCargoManifest);
    }
    let cargo_lock = read_bounded_regular(
        &service_root.join(CARGO_LOCK),
        MAX_CARGO_LOCK_BYTES,
        CommandError::InvalidCargoLock,
    )?;
    validate_cargo_lock(&cargo_lock, &revision, &archive.package_names)?;
    let cargo_lock_sha256 = sha256(&cargo_lock);
    reachability.verify(&archive.revision)?;
    validate_service_cleanliness(&service_root)?;
    if service_head(&service_root)? != initial_head {
        return Err(CommandError::DirtyServiceSource);
    }

    let desired = ServiceSourceLockV1::new(ServiceSourceLockParts {
        service: &metadata.service,
        revision: &archive.revision,
        workspace_catalog_sha256: &archive.catalog_sha256,
        source_archive_sha256: &archive.archive_sha256,
        cargo_lock_sha256: &cargo_lock_sha256,
        flake_lock_sha256: &flake_lock_sha256,
        contract_versions: metadata.contract_versions,
    })
    .map_err(|_| CommandError::InvalidServiceMetadata)?;

    let lock_path = service_root.join(LOCK_FILENAME);
    let result = match mode {
        CommandMode::Check => {
            let current = read_bounded_regular(&lock_path, 4096, CommandError::InvalidSourceLock)?;
            let current = ServiceSourceLockV1::from_canonical_bytes(&current)
                .map_err(|_| CommandError::InvalidSourceLock)?;
            if current == desired {
                Ok(())
            } else {
                Err(CommandError::StaleSourceLock)
            }
        }
        CommandMode::Write => {
            atomic_write_lock(&service_root, &lock_path, desired.canonical_bytes())?;
            let current = read_bounded_regular(&lock_path, 4096, CommandError::WriteFailure)?;
            let current = ServiceSourceLockV1::from_canonical_bytes(&current)
                .map_err(|_| CommandError::WriteFailure)?;
            if current == desired {
                Ok(())
            } else {
                Err(CommandError::WriteFailure)
            }
        }
    };
    result?;
    validate_service_cleanliness(&service_root)?;
    if service_head(&service_root)? == initial_head {
        Ok(())
    } else {
        Err(CommandError::DirtyServiceSource)
    }
}

fn service_head(root: &Path) -> Result<String, CommandError> {
    let bytes = git_stdout(root, ["rev-parse", "HEAD"], 128)
        .map_err(|_| CommandError::InvalidServiceRoot)?;
    let revision = std::str::from_utf8(&bytes)
        .map_err(|_| CommandError::InvalidServiceRoot)?
        .trim();
    if valid_lower_hex(revision, 40) {
        Ok(revision.to_owned())
    } else {
        Err(CommandError::InvalidServiceRoot)
    }
}

fn validate_service_root(path: &Path) -> Result<PathBuf, CommandError> {
    if !path.is_absolute() {
        return Err(CommandError::InvalidServiceRoot);
    }
    let metadata = fs::symlink_metadata(path).map_err(|_| CommandError::InvalidServiceRoot)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(CommandError::InvalidServiceRoot);
    }
    let canonical = fs::canonicalize(path).map_err(|_| CommandError::InvalidServiceRoot)?;
    let top = git_stdout(
        &canonical,
        ["rev-parse", "--show-toplevel"],
        MAX_GIT_OUTPUT_BYTES,
    )
    .map_err(|_| CommandError::InvalidServiceRoot)?;
    let top = std::str::from_utf8(&top).map_err(|_| CommandError::InvalidServiceRoot)?;
    let top = fs::canonicalize(top.trim()).map_err(|_| CommandError::InvalidServiceRoot)?;
    if top == canonical {
        Ok(canonical)
    } else {
        Err(CommandError::InvalidServiceRoot)
    }
}

fn validate_service_cleanliness(root: &Path) -> Result<(), CommandError> {
    git_status(
        root,
        [
            "diff",
            "--quiet",
            "--",
            ".",
            ":(exclude)radroots.service.source-lock.v1.toml",
        ],
    )
    .map_err(|_| CommandError::DirtyServiceSource)?;
    git_status(
        root,
        [
            "diff",
            "--cached",
            "--quiet",
            "--",
            ".",
            ":(exclude)radroots.service.source-lock.v1.toml",
        ],
    )
    .map_err(|_| CommandError::DirtyServiceSource)?;
    let untracked = git_stdout(
        root,
        ["ls-files", "--others", "--exclude-standard", "-z"],
        MAX_GIT_OUTPUT_BYTES,
    )
    .map_err(|_| CommandError::DirtyServiceSource)?;
    let mut paths = untracked
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty());
    if paths.all(|path| path == LOCK_FILENAME.as_bytes()) {
        Ok(())
    } else {
        Err(CommandError::DirtyServiceSource)
    }
}

fn parse_service_metadata(root: &toml::Value) -> Result<ServiceMetadata, CommandError> {
    let table = root
        .get("workspace")
        .and_then(|value| value.get("metadata"))
        .and_then(|value| value.get("radroots"))
        .and_then(|value| value.get("service_source_lock"))
        .and_then(toml::Value::as_table)
        .ok_or(CommandError::InvalidServiceMetadata)?;
    let expected = BTreeSet::from([
        "admin_contract_version",
        "config_contract_version",
        "host_feature_profile",
        "provider_contract_version",
        "service",
        "state_contract_version",
        "status_contract_version",
    ]);
    if table.keys().map(String::as_str).collect::<BTreeSet<_>>() != expected {
        return Err(CommandError::InvalidServiceMetadata);
    }
    let text = |key: &str| {
        table
            .get(key)
            .and_then(toml::Value::as_str)
            .ok_or(CommandError::InvalidServiceMetadata)
    };
    let version = |key: &str| {
        table
            .get(key)
            .and_then(toml::Value::as_integer)
            .and_then(|value| u32::try_from(value).ok())
            .filter(|value| *value != 0)
            .ok_or(CommandError::InvalidServiceMetadata)
    };
    if text("host_feature_profile")? != HOST_FEATURE_PROFILE {
        return Err(CommandError::InvalidServiceMetadata);
    }
    Ok(ServiceMetadata {
        service: text("service")?.to_owned(),
        contract_versions: ContractVersions::new(
            version("config_contract_version")?,
            version("state_contract_version")?,
            version("admin_contract_version")?,
            version("status_contract_version")?,
            version("provider_contract_version")?,
        ),
    })
}

fn validate_cargo_manifests(
    root: &Path,
    lib_packages: Option<&BTreeSet<String>>,
) -> Result<String, CommandError> {
    let mut manifests = Vec::new();
    let mut entries = 0_usize;
    let walker = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            !matches!(
                entry.file_name().to_str(),
                Some(".git" | ".gradle" | ".kotlin" | "build" | "node_modules" | "out" | "target")
            )
        });
    for entry in walker {
        entries = entries
            .checked_add(1)
            .ok_or(CommandError::InvalidCargoManifest)?;
        if entries > MAX_TREE_ENTRIES {
            return Err(CommandError::InvalidCargoManifest);
        }
        let entry = entry.map_err(|_| CommandError::InvalidCargoManifest)?;
        if entry.file_name() == OsStr::new(CARGO_MANIFEST) {
            if entry.file_type().is_symlink() || !entry.file_type().is_file() {
                return Err(CommandError::InvalidCargoManifest);
            }
            manifests.push(entry.into_path());
            if manifests.len() > MAX_MANIFESTS {
                return Err(CommandError::InvalidCargoManifest);
            }
        }
    }
    manifests.sort();
    if manifests.is_empty() {
        return Err(CommandError::InvalidCargoManifest);
    }
    let mut state = ManifestState::default();
    for manifest in manifests {
        let bytes = read_bounded_regular(
            &manifest,
            MAX_MANIFEST_BYTES,
            CommandError::InvalidCargoManifest,
        )?;
        let value = parse_toml(&bytes, CommandError::InvalidCargoManifest)?;
        validate_manifest_node(&value, None, false, false, lib_packages, &mut state)?;
    }
    if state.dependencies == 0 || !state.host_dependency {
        return Err(CommandError::InvalidCargoManifest);
    }
    state.revision.ok_or(CommandError::InvalidCargoManifest)
}

#[derive(Default)]
struct ManifestState {
    revision: Option<String>,
    dependencies: usize,
    host_dependency: bool,
}

fn validate_manifest_node(
    value: &toml::Value,
    key: Option<&str>,
    in_patch: bool,
    in_dependencies: bool,
    lib_packages: Option<&BTreeSet<String>>,
    state: &mut ManifestState,
) -> Result<(), CommandError> {
    match value {
        toml::Value::Table(table) => {
            let git = table.get("git").and_then(toml::Value::as_str);
            let package = table.get("package").and_then(toml::Value::as_str).or(key);
            let known_lib_dependency = (in_dependencies || in_patch)
                && package.is_some_and(|package| {
                    lib_packages.map_or(package == HOST_PACKAGE, |packages| {
                        packages.contains(package)
                    })
                });
            let canonical_lib_dependency =
                (in_dependencies || in_patch) && git.is_some_and(|git| git == LIB_REPOSITORY);
            if known_lib_dependency || canonical_lib_dependency {
                let git = git.ok_or(CommandError::InvalidCargoManifest)?;
                if in_patch
                    || git != LIB_REPOSITORY
                    || table.get("version").and_then(toml::Value::as_str)
                        != Some(LIB_VERSION_REQUIREMENT)
                    || table.contains_key("branch")
                    || table.contains_key("tag")
                    || table.contains_key("path")
                {
                    return Err(CommandError::InvalidCargoManifest);
                }
                let revision = table
                    .get("rev")
                    .and_then(toml::Value::as_str)
                    .filter(|value| valid_lower_hex(value, 40))
                    .ok_or(CommandError::InvalidCargoManifest)?;
                match &state.revision {
                    Some(expected) if expected != revision => {
                        return Err(CommandError::InvalidCargoManifest);
                    }
                    None => state.revision = Some(revision.to_owned()),
                    _ => {}
                }
                state.dependencies += 1;
                state.host_dependency |= key == Some(HOST_PACKAGE)
                    || table.get("package").and_then(toml::Value::as_str) == Some(HOST_PACKAGE);
            }
            for (child_key, child) in table {
                if (known_lib_dependency || canonical_lib_dependency) && child_key == "git" {
                    continue;
                }
                let child_dependency_scope = in_dependencies
                    || matches!(
                        child_key.as_str(),
                        "dependencies" | "dev-dependencies" | "build-dependencies"
                    );
                validate_manifest_node(
                    child,
                    Some(child_key),
                    in_patch || child_key == "patch",
                    child_dependency_scope,
                    lib_packages,
                    state,
                )?;
            }
        }
        toml::Value::Array(values) => {
            for child in values {
                validate_manifest_node(child, key, in_patch, in_dependencies, lib_packages, state)?;
            }
        }
        toml::Value::String(_)
            if in_dependencies
                && key.is_some_and(|key| {
                    lib_packages.map_or(key == HOST_PACKAGE, |packages| packages.contains(key))
                }) =>
        {
            return Err(CommandError::InvalidCargoManifest);
        }
        _ => {}
    }
    Ok(())
}

fn validate_cargo_lock(
    bytes: &[u8],
    revision: &str,
    lib_packages: &BTreeSet<String>,
) -> Result<(), CommandError> {
    let text = std::str::from_utf8(bytes).map_err(|_| CommandError::InvalidCargoLock)?;
    let document =
        toml::from_str::<CargoLockDocument>(text).map_err(|_| CommandError::InvalidCargoLock)?;
    let expected = format!("git+{LIB_REPOSITORY}?rev={revision}#{revision}");
    let mut count = 0_usize;
    let mut host = false;
    for package in document.package {
        if lib_packages.contains(&package.name) {
            let source = package.source.ok_or(CommandError::InvalidCargoLock)?;
            if source != expected || package.version != "0.1.0-alpha" {
                return Err(CommandError::InvalidCargoLock);
            }
            count += 1;
            host |= package.name == HOST_PACKAGE;
        }
    }
    if count != 0 && host {
        Ok(())
    } else {
        Err(CommandError::InvalidCargoLock)
    }
}

fn validate_flake_lock(bytes: &[u8], revision: &str) -> Result<(), CommandError> {
    let value = serde_json::from_slice::<serde_json::Value>(bytes)
        .map_err(|_| CommandError::InvalidFlakeLock)?;
    let version = value.get("version").and_then(serde_json::Value::as_u64);
    let root_name = value.get("root").and_then(serde_json::Value::as_str);
    let nodes = value.get("nodes").and_then(serde_json::Value::as_object);
    if version != Some(7) || root_name.is_none() || nodes.is_none() {
        return Err(CommandError::InvalidFlakeLock);
    }
    let nodes = nodes.ok_or(CommandError::InvalidFlakeLock)?;
    let root = nodes
        .get(root_name.ok_or(CommandError::InvalidFlakeLock)?)
        .and_then(|node| node.get("inputs"))
        .and_then(serde_json::Value::as_object)
        .ok_or(CommandError::InvalidFlakeLock)?;
    let direct = root
        .values()
        .filter_map(serde_json::Value::as_str)
        .collect::<Vec<_>>();
    let mut lib_nodes = 0_usize;
    let mut exact_direct = 0_usize;
    for (name, node) in nodes {
        let locked = node.get("locked").and_then(serde_json::Value::as_object);
        let original = node.get("original").and_then(serde_json::Value::as_object);
        let is_lib = locked.is_some_and(|locked| {
            locked.get("owner").and_then(serde_json::Value::as_str) == Some("radrootslabs")
                && locked.get("repo").and_then(serde_json::Value::as_str) == Some("lib")
        }) || original.is_some_and(|original| {
            original.get("owner").and_then(serde_json::Value::as_str) == Some("radrootslabs")
                && original.get("repo").and_then(serde_json::Value::as_str) == Some("lib")
        });
        if !is_lib {
            continue;
        }
        lib_nodes += 1;
        let locked = locked.ok_or(CommandError::InvalidFlakeLock)?;
        let original = original.ok_or(CommandError::InvalidFlakeLock)?;
        let locked_keys = locked.keys().map(String::as_str).collect::<BTreeSet<_>>();
        let original_keys = original.keys().map(String::as_str).collect::<BTreeSet<_>>();
        let exact = locked_keys
            == BTreeSet::from(["lastModified", "narHash", "owner", "repo", "rev", "type"])
            && original_keys == BTreeSet::from(["owner", "repo", "rev", "type"])
            && locked.get("type").and_then(serde_json::Value::as_str) == Some("github")
            && locked.get("owner").and_then(serde_json::Value::as_str) == Some("radrootslabs")
            && locked.get("repo").and_then(serde_json::Value::as_str) == Some("lib")
            && locked.get("rev").and_then(serde_json::Value::as_str) == Some(revision)
            && locked
                .get("narHash")
                .and_then(serde_json::Value::as_str)
                .is_some_and(valid_nix_sha256)
            && original.get("type").and_then(serde_json::Value::as_str) == Some("github")
            && original.get("owner").and_then(serde_json::Value::as_str) == Some("radrootslabs")
            && original.get("repo").and_then(serde_json::Value::as_str) == Some("lib")
            && original.get("rev").and_then(serde_json::Value::as_str) == Some(revision)
            && original.get("ref").is_none();
        if direct.iter().filter(|direct| **direct == name).count() == 1 && exact {
            exact_direct += 1;
        }
    }
    if lib_nodes == 1 && exact_direct == 1 {
        Ok(())
    } else {
        Err(CommandError::InvalidFlakeLock)
    }
}

fn validate_toolchain(root: &Path) -> Result<(), CommandError> {
    let bytes = read_bounded_regular(
        &root.join(RUST_TOOLCHAIN),
        MAX_TOOLCHAIN_BYTES,
        CommandError::InvalidToolchain,
    )?;
    let value = parse_toml(&bytes, CommandError::InvalidToolchain)?;
    if value
        .get("toolchain")
        .and_then(|value| value.get("channel"))
        .and_then(toml::Value::as_str)
        == Some(RUST_VERSION)
    {
        Ok(())
    } else {
        Err(CommandError::InvalidToolchain)
    }
}

fn validate_archive(path: &Path, revision: &str) -> Result<ArchiveEvidence, CommandError> {
    if !path.is_absolute() {
        return Err(CommandError::InvalidSourceArchive);
    }
    let metadata = fs::symlink_metadata(path).map_err(|_| CommandError::InvalidSourceArchive)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() == 0
        || metadata.len() > MAX_ARCHIVE_BYTES
    {
        return Err(CommandError::InvalidSourceArchive);
    }
    let mut source = fs::File::open(path).map_err(|_| CommandError::InvalidSourceArchive)?;
    let before = source
        .metadata()
        .map_err(|_| CommandError::InvalidSourceArchive)?;
    let mut stable = NamedTempFile::new().map_err(|_| CommandError::InvalidSourceArchive)?;
    let mut hasher = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 65_536];
    loop {
        let read = source
            .read(&mut buffer)
            .map_err(|_| CommandError::InvalidSourceArchive)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(u64::try_from(read).map_err(|_| CommandError::InvalidSourceArchive)?)
            .filter(|total| *total <= MAX_ARCHIVE_BYTES)
            .ok_or(CommandError::InvalidSourceArchive)?;
        hasher.update(&buffer[..read]);
        stable
            .write_all(&buffer[..read])
            .map_err(|_| CommandError::InvalidSourceArchive)?;
    }
    let after = source
        .metadata()
        .map_err(|_| CommandError::InvalidSourceArchive)?;
    if total == 0 || before.len() != total || after.len() != total {
        return Err(CommandError::InvalidSourceArchive);
    }
    stable
        .flush()
        .and_then(|()| stable.as_file().sync_all())
        .map_err(|_| CommandError::InvalidSourceArchive)?;
    stable
        .as_file_mut()
        .seek(SeekFrom::Start(0))
        .map_err(|_| CommandError::InvalidSourceArchive)?;

    let verification = TempDir::new().map_err(|_| CommandError::InvalidSourceArchive)?;
    git_status(verification.path(), ["init", "--bare", "--quiet"])
        .map_err(|_| CommandError::InvalidSourceArchive)?;
    let status = Command::new("git")
        .args(["bundle", "verify"])
        .arg(stable.path())
        .current_dir(verification.path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|_| CommandError::InvalidSourceArchive)?;
    if !status.success() {
        return Err(CommandError::InvalidSourceArchive);
    }
    let heads = command_stdout(
        Command::new("git")
            .args(["bundle", "list-heads"])
            .arg(stable.path()),
        MAX_GIT_OUTPUT_BYTES,
    )
    .map_err(|_| CommandError::InvalidSourceArchive)?;
    let expected_head = format!("{revision} refs/heads/archive\n");
    if heads != expected_head.as_bytes() {
        return Err(CommandError::InvalidSourceArchive);
    }
    let fetch = Command::new("git")
        .args(["fetch", "--quiet", "--no-tags"])
        .arg(stable.path())
        .arg(format!("{revision}:refs/heads/archive"))
        .current_dir(verification.path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|_| CommandError::InvalidSourceArchive)?;
    if !fetch.success() {
        return Err(CommandError::InvalidSourceArchive);
    }
    let catalog_spec = format!("{revision}:{CATALOG_RELATIVE}");
    let catalog = git_stdout(
        verification.path(),
        ["show", catalog_spec.as_str()],
        MAX_CATALOG_BYTES,
    )
    .map_err(|_| CommandError::InvalidSourceArchive)?;
    if catalog.is_empty() {
        return Err(CommandError::InvalidSourceArchive);
    }
    let package_names = catalog_package_names(&catalog)?;
    Ok(ArchiveEvidence {
        revision: revision.to_owned(),
        archive_sha256: hex::encode(hasher.finalize()),
        catalog_sha256: sha256(&catalog),
        package_names,
    })
}

fn catalog_package_names(bytes: &[u8]) -> Result<BTreeSet<String>, CommandError> {
    let value = parse_toml(bytes, CommandError::InvalidSourceArchive)?;
    if value.get("schema").and_then(toml::Value::as_str) != Some("radroots.workspace.catalog.v2")
        || value.get("architecture").and_then(toml::Value::as_str)
            != Some("radroots.crates.release.v2")
        || value.get("version").and_then(toml::Value::as_str) != Some("0.1.0-alpha")
    {
        return Err(CommandError::InvalidSourceArchive);
    }
    let packages = value
        .get("package")
        .and_then(toml::Value::as_array)
        .ok_or(CommandError::InvalidSourceArchive)?;
    let expected_count = value
        .get("package_count")
        .and_then(toml::Value::as_integer)
        .and_then(|count| usize::try_from(count).ok())
        .ok_or(CommandError::InvalidSourceArchive)?;
    if packages.is_empty() || packages.len() != expected_count || packages.len() > MAX_MANIFESTS {
        return Err(CommandError::InvalidSourceArchive);
    }
    let mut names = BTreeSet::new();
    for package in packages {
        let name = package
            .get("name")
            .and_then(toml::Value::as_str)
            .ok_or(CommandError::InvalidSourceArchive)?;
        if !names.insert(name.to_owned()) {
            return Err(CommandError::InvalidSourceArchive);
        }
    }
    if names.contains(HOST_PACKAGE) {
        Ok(names)
    } else {
        Err(CommandError::InvalidSourceArchive)
    }
}

fn atomic_write_lock(root: &Path, path: &Path, bytes: &[u8]) -> Result<(), CommandError> {
    if let Ok(metadata) = fs::symlink_metadata(path)
        && (metadata.file_type().is_symlink() || !metadata.is_file())
    {
        return Err(CommandError::WriteFailure);
    }
    let mut temporary = NamedTempFile::new_in(root).map_err(|_| CommandError::WriteFailure)?;
    temporary
        .write_all(bytes)
        .and_then(|()| temporary.as_file().sync_all())
        .map_err(|_| CommandError::WriteFailure)?;
    temporary
        .persist(path)
        .map_err(|_| CommandError::WriteFailure)?;
    fs::File::open(root)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| CommandError::WriteFailure)
}

fn read_bounded_regular(
    path: &Path,
    maximum: usize,
    error: CommandError,
) -> Result<Vec<u8>, CommandError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| error)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > maximum as u64 {
        return Err(error);
    }
    let file = fs::File::open(path).map_err(|_| error)?;
    let mut bytes = Vec::with_capacity(usize::try_from(metadata.len()).map_err(|_| error)?);
    file.take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| error)?;
    if bytes.len() > maximum {
        Err(error)
    } else {
        Ok(bytes)
    }
}

fn parse_toml(bytes: &[u8], error: CommandError) -> Result<toml::Value, CommandError> {
    let text = std::str::from_utf8(bytes).map_err(|_| error)?;
    toml::from_str(text).map_err(|_| error)
}

fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn valid_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_nix_sha256(value: &str) -> bool {
    let Some(encoded) = value.strip_prefix("sha256-") else {
        return false;
    };
    let bytes = encoded.as_bytes();
    bytes.len() == 44
        && bytes[43] == b'='
        && bytes[..43]
            .iter()
            .copied()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/'))
}

fn git_status<const N: usize>(root: &Path, args: [&str; N]) -> Result<(), ()> {
    let status = Command::new("git")
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|_| ())?;
    if status.success() { Ok(()) } else { Err(()) }
}

fn git_stdout<const N: usize>(root: &Path, args: [&str; N], maximum: usize) -> Result<Vec<u8>, ()> {
    let mut command = Command::new("git");
    command.args(args).current_dir(root);
    command_stdout(&mut command, maximum)
}

fn command_stdout(command: &mut Command, maximum: usize) -> Result<Vec<u8>, ()> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| ())?;
    let mut stdout = child.stdout.take().ok_or(())?;
    let mut bytes = Vec::new();
    if stdout
        .by_ref()
        .take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .is_err()
    {
        let _ = child.kill();
        let _ = child.wait();
        return Err(());
    }
    if bytes.len() > maximum {
        let _ = child.kill();
        let _ = child.wait();
        return Err(());
    }
    let status = child.wait().map_err(|_| ())?;
    if status.success() { Ok(bytes) } else { Err(()) }
}

#[cfg(test)]
mod tests {
    use std::{
        error::Error as _,
        sync::atomic::{AtomicBool, Ordering},
    };

    use super::*;

    struct Reachability {
        reachable: AtomicBool,
    }

    impl RevisionReachability for Reachability {
        fn verify(&self, _revision: &str) -> Result<(), CommandError> {
            if self.reachable.load(Ordering::SeqCst) {
                Ok(())
            } else {
                Err(CommandError::UnreachableRevision)
            }
        }
    }

    struct CommitOnVerify {
        service: PathBuf,
    }

    impl RevisionReachability for CommitOnVerify {
        fn verify(&self, _revision: &str) -> Result<(), CommandError> {
            fs::write(self.service.join("concurrent"), b"changed").expect("concurrent file");
            git(&self.service, &["add", "."]);
            git(
                &self.service,
                &["commit", "--quiet", "-m", "concurrent commit"],
            );
            Ok(())
        }
    }

    struct Fixture {
        root: TempDir,
        service: PathBuf,
        archive: PathBuf,
        revision: String,
    }

    impl Fixture {
        fn new() -> Self {
            let root = TempDir::new().expect("temp root");
            let lib = root.path().join("lib");
            fs::create_dir(&lib).expect("lib root");
            git(&lib, &["init", "--quiet"]);
            git(&lib, &["config", "user.email", "fixture@example.invalid"]);
            git(&lib, &["config", "user.name", "fixture"]);
            let catalog = lib.join(CATALOG_RELATIVE);
            fs::create_dir_all(catalog.parent().expect("catalog parent")).expect("catalog parent");
            fs::write(
                &catalog,
                br#"schema_version = 2
schema = "radroots.workspace.catalog.v2"
architecture = "radroots.crates.release.v2"
version = "0.1.0-alpha"
package_count = 2

[[package]]
name = "radroots_core"

[[package]]
name = "radroots_service_host"
"#,
            )
            .expect("catalog");
            git(&lib, &["add", "."]);
            git(&lib, &["commit", "--quiet", "-m", "fixture"]);
            git(&lib, &["branch", "-M", "archive"]);
            let revision =
                String::from_utf8(git_stdout(&lib, ["rev-parse", "HEAD"], 128).expect("revision"))
                    .expect("UTF-8")
                    .trim()
                    .to_owned();
            let archive = root.path().join("lib.bundle");
            let status = Command::new("git")
                .args(["bundle", "create"])
                .arg(&archive)
                .arg("refs/heads/archive")
                .current_dir(&lib)
                .status()
                .expect("bundle");
            assert!(status.success());

            let service = root.path().join("service");
            fs::create_dir(&service).expect("service root");
            fs::write(
                service.join(CARGO_MANIFEST),
                format!(
                    r#"[workspace]
resolver = "3"

[workspace.metadata.radroots.service_source_lock]
service = "fixture_service"
host_feature_profile = "service-host"
config_contract_version = 1
state_contract_version = 2
admin_contract_version = 3
status_contract_version = 4
provider_contract_version = 5

[workspace.dependencies]
radroots_service_host = {{ git = "{LIB_REPOSITORY}", rev = "{revision}", version = "=0.1.0-alpha" }}
"#,
                ),
            )
            .expect("manifest");
            fs::write(
                service.join(CARGO_LOCK),
                format!(
                    "version = 4\n\n[[package]]\nname = \"radroots_service_host\"\nversion = \"0.1.0-alpha\"\nsource = \"git+{LIB_REPOSITORY}?rev={revision}#{revision}\"\n"
                ),
            )
            .expect("Cargo.lock");
            fs::write(
                service.join(FLAKE_LOCK),
                format!(
                    r#"{{"nodes":{{"root":{{"inputs":{{"lib":"lib"}}}},"lib":{{"locked":{{"lastModified":1,"narHash":"sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=","owner":"radrootslabs","repo":"lib","rev":"{revision}","type":"github"}},"original":{{"owner":"radrootslabs","repo":"lib","rev":"{revision}","type":"github"}}}}}},"root":"root","version":7}}
"#,
                ),
            )
            .expect("flake.lock");
            fs::write(
                service.join(RUST_TOOLCHAIN),
                "[toolchain]\nchannel = \"1.97.1\"\ncomponents = [\"clippy\", \"rustfmt\"]\n",
            )
            .expect("toolchain");
            git(&service, &["init", "--quiet"]);
            git(
                &service,
                &["config", "user.email", "fixture@example.invalid"],
            );
            git(&service, &["config", "user.name", "fixture"]);
            git(&service, &["add", "."]);
            git(&service, &["commit", "--quiet", "-m", "fixture"]);
            Self {
                root,
                service,
                archive,
                revision,
            }
        }

        fn reachable(&self) -> Reachability {
            Reachability {
                reachable: AtomicBool::new(true),
            }
        }
    }

    fn git(root: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(root)
            .status()
            .expect("git");
        assert!(status.success(), "git {args:?}");
    }

    #[test]
    fn update_and_check_bind_complete_local_evidence() {
        let fixture = Fixture::new();
        let reachability = fixture.reachable();
        run_with(
            CommandMode::Write,
            &fixture.service,
            &fixture.archive,
            &reachability,
        )
        .expect("write lock");
        run_with(
            CommandMode::Check,
            &fixture.service,
            &fixture.archive,
            &reachability,
        )
        .expect("check lock");

        let bytes = fs::read(fixture.service.join(LOCK_FILENAME)).expect("lock");
        let text = std::str::from_utf8(&bytes).expect("UTF-8");
        assert!(text.contains("service = \"fixture_service\""));
        assert!(text.contains(&format!("revision = \"{}\"", fixture.revision)));
        assert!(text.contains("config = 1\nstate = 2\nadmin = 3\nstatus = 4\nprovider = 5"));
        assert!(fixture.root.path().exists());
    }

    #[test]
    fn check_rejects_stale_lock_and_update_repairs_it() {
        let fixture = Fixture::new();
        let reachability = fixture.reachable();
        run_with(
            CommandMode::Write,
            &fixture.service,
            &fixture.archive,
            &reachability,
        )
        .expect("write lock");
        let lock = fixture.service.join(LOCK_FILENAME);
        let stale =
            fs::read_to_string(&lock)
                .expect("read lock")
                .replacen("config = 1", "config = 9", 1);
        fs::write(&lock, stale).expect("stale lock");
        assert_eq!(
            run_with(
                CommandMode::Check,
                &fixture.service,
                &fixture.archive,
                &reachability,
            ),
            Err(CommandError::StaleSourceLock)
        );
        run_with(
            CommandMode::Write,
            &fixture.service,
            &fixture.archive,
            &reachability,
        )
        .expect("repair lock");
    }

    #[test]
    fn dirty_source_and_unreachable_revision_fail_closed() {
        let fixture = Fixture::new();
        fs::write(fixture.service.join("untracked-secret"), b"sensitive").expect("dirty file");
        assert_eq!(
            run_with(
                CommandMode::Write,
                &fixture.service,
                &fixture.archive,
                &fixture.reachable(),
            ),
            Err(CommandError::DirtyServiceSource)
        );
        fs::remove_file(fixture.service.join("untracked-secret")).expect("remove dirty file");
        let unreachable = Reachability {
            reachable: AtomicBool::new(false),
        };
        assert_eq!(
            run_with(
                CommandMode::Write,
                &fixture.service,
                &fixture.archive,
                &unreachable,
            ),
            Err(CommandError::UnreachableRevision)
        );
        assert!(!fixture.service.join(LOCK_FILENAME).exists());

        let fixture = Fixture::new();
        let commit_on_verify = CommitOnVerify {
            service: fixture.service.clone(),
        };
        assert_eq!(
            run_with(
                CommandMode::Write,
                &fixture.service,
                &fixture.archive,
                &commit_on_verify,
            ),
            Err(CommandError::DirtyServiceSource)
        );
        assert!(!fixture.service.join(LOCK_FILENAME).exists());
    }

    #[test]
    fn cargo_flake_toolchain_and_archive_drift_are_distinct() {
        let cases = [
            (
                CARGO_MANIFEST,
                "radroots_service_host",
                "other",
                CommandError::InvalidCargoManifest,
            ),
            (
                CARGO_LOCK,
                "radroots_service_host",
                "other",
                CommandError::InvalidCargoLock,
            ),
            (
                FLAKE_LOCK,
                "radrootslabs",
                "other",
                CommandError::InvalidFlakeLock,
            ),
            (
                RUST_TOOLCHAIN,
                "1.97.1",
                "1.97.0",
                CommandError::InvalidToolchain,
            ),
        ];
        for (path, from, to, expected) in cases {
            let fixture = Fixture::new();
            let path = fixture.service.join(path);
            let mutated = fs::read_to_string(&path)
                .expect("read fixture")
                .replacen(from, to, 1);
            fs::write(&path, mutated).expect("mutate fixture");
            git(&fixture.service, &["add", "."]);
            git(&fixture.service, &["commit", "--quiet", "-m", "mutate"]);
            assert_eq!(
                run_with(
                    CommandMode::Write,
                    &fixture.service,
                    &fixture.archive,
                    &fixture.reachable(),
                ),
                Err(expected)
            );
        }

        let fixture = Fixture::new();
        let other = fixture.root.path().join("other");
        fs::create_dir(&other).expect("other repo");
        git(&other, &["init", "--quiet"]);
        git(&other, &["config", "user.email", "fixture@example.invalid"]);
        git(&other, &["config", "user.name", "fixture"]);
        fs::write(other.join("other"), b"other").expect("other file");
        git(&other, &["add", "."]);
        git(&other, &["commit", "--quiet", "-m", "other"]);
        git(&other, &["branch", "-M", "archive"]);
        let bad_archive = fixture.root.path().join("other.bundle");
        let status = Command::new("git")
            .args(["bundle", "create"])
            .arg(&bad_archive)
            .arg("refs/heads/archive")
            .current_dir(&other)
            .status()
            .expect("other bundle");
        assert!(status.success());
        assert_eq!(
            run_with(
                CommandMode::Write,
                &fixture.service,
                &bad_archive,
                &fixture.reachable(),
            ),
            Err(CommandError::InvalidSourceArchive)
        );

        let fixture = Fixture::new();
        let oversized = fixture.root.path().join("oversized.bundle");
        fs::File::create(&oversized)
            .and_then(|file| file.set_len(MAX_ARCHIVE_BYTES + 1))
            .expect("sparse oversized archive");
        assert_eq!(
            run_with(
                CommandMode::Write,
                &fixture.service,
                &oversized,
                &fixture.reachable(),
            ),
            Err(CommandError::InvalidSourceArchive)
        );
    }

    #[test]
    fn mixed_local_or_unlocked_lib_dependencies_fail_closed() {
        let fixture = Fixture::new();
        let manifest = fixture.service.join(CARGO_MANIFEST);
        let mut text = fs::read_to_string(&manifest).expect("manifest");
        text.push_str("radroots_core = { path = \"../lib/crates/core\" }\n");
        fs::write(&manifest, text).expect("local Lib dependency");
        git(&fixture.service, &["add", "."]);
        git(
            &fixture.service,
            &["commit", "--quiet", "-m", "local dependency"],
        );
        assert_eq!(
            run_with(
                CommandMode::Write,
                &fixture.service,
                &fixture.archive,
                &fixture.reachable(),
            ),
            Err(CommandError::InvalidCargoManifest)
        );

        let fixture = Fixture::new();
        let cargo_lock = fixture.service.join(CARGO_LOCK);
        let mut text = fs::read_to_string(&cargo_lock).expect("Cargo.lock");
        text.push_str("\n[[package]]\nname = \"radroots_core\"\nversion = \"0.1.0-alpha\"\n");
        fs::write(&cargo_lock, text).expect("unlocked Lib package");
        git(&fixture.service, &["add", "."]);
        git(
            &fixture.service,
            &["commit", "--quiet", "-m", "unlocked dependency"],
        );
        assert_eq!(
            run_with(
                CommandMode::Write,
                &fixture.service,
                &fixture.archive,
                &fixture.reachable(),
            ),
            Err(CommandError::InvalidCargoLock)
        );

        let fixture = Fixture::new();
        let manifest = fixture.service.join(CARGO_MANIFEST);
        let mut text = fs::read_to_string(&manifest).expect("manifest");
        text.push_str(&format!(
            "\n[patch.crates-io]\nradroots_core = {{ git = \"{LIB_REPOSITORY}\", rev = \"{}\", version = \"=0.1.0-alpha\" }}\n",
            fixture.revision
        ));
        fs::write(&manifest, text).expect("Lib patch");
        git(&fixture.service, &["add", "."]);
        git(&fixture.service, &["commit", "--quiet", "-m", "Lib patch"]);
        assert_eq!(
            run_with(
                CommandMode::Write,
                &fixture.service,
                &fixture.archive,
                &fixture.reachable(),
            ),
            Err(CommandError::InvalidCargoManifest)
        );
    }

    #[test]
    fn floating_or_unbounded_nix_input_fails_closed() {
        for mutation in [
            ("\"rev\":\"", "\"ref\":\"master\",\"rev\":\""),
            (
                "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
                "sha256-invalid",
            ),
            ("\"lastModified\":1,", "\"dir\":\"sub\",\"lastModified\":1,"),
        ] {
            let fixture = Fixture::new();
            let path = fixture.service.join(FLAKE_LOCK);
            let current = fs::read_to_string(&path).expect("flake lock");
            let mutated = if mutation.0 == "\"rev\":\"" {
                let second = current.rfind(mutation.0).expect("original revision field");
                format!(
                    "{}{}{}",
                    &current[..second],
                    mutation.1,
                    &current[second + mutation.0.len()..]
                )
            } else {
                current.replacen(mutation.0, mutation.1, 1)
            };
            fs::write(&path, mutated).expect("mutate flake lock");
            git(&fixture.service, &["add", "."]);
            git(&fixture.service, &["commit", "--quiet", "-m", "mutate"]);
            assert_eq!(
                run_with(
                    CommandMode::Write,
                    &fixture.service,
                    &fixture.archive,
                    &fixture.reachable(),
                ),
                Err(CommandError::InvalidFlakeLock)
            );
        }
    }

    #[test]
    fn flake_lock_rejects_each_independent_fixed_field_drift() {
        let fixture = Fixture::new();
        let bytes = fs::read(fixture.service.join(FLAKE_LOCK)).expect("flake lock");
        let canonical = serde_json::from_slice::<serde_json::Value>(&bytes).expect("flake json");
        for (pointer, replacement) in [
            ("/version", serde_json::json!(6)),
            ("/nodes/root/inputs/lib", serde_json::json!("other")),
            ("/nodes/lib/locked/type", serde_json::json!("git")),
            ("/nodes/lib/locked/owner", serde_json::json!("other")),
            ("/nodes/lib/locked/repo", serde_json::json!("other")),
            ("/nodes/lib/locked/rev", serde_json::json!("other")),
            (
                "/nodes/lib/locked/narHash",
                serde_json::json!("sha256-invalid"),
            ),
            ("/nodes/lib/original/type", serde_json::json!("git")),
            ("/nodes/lib/original/owner", serde_json::json!("other")),
            ("/nodes/lib/original/repo", serde_json::json!("other")),
            ("/nodes/lib/original/rev", serde_json::json!("other")),
        ] {
            let mut drifted = canonical.clone();
            *drifted.pointer_mut(pointer).expect("governed field") = replacement;
            assert_eq!(
                validate_flake_lock(
                    &serde_json::to_vec(&drifted).expect("flake json"),
                    &fixture.revision
                ),
                Err(CommandError::InvalidFlakeLock),
                "accepted drift at {pointer}"
            );
        }

        for (section, field) in [
            ("locked", "extra"),
            ("original", "extra"),
            ("original", "ref"),
        ] {
            let mut drifted = canonical.clone();
            drifted["nodes"]["lib"][section]
                .as_object_mut()
                .expect("flake section")
                .insert(field.into(), serde_json::json!("unexpected"));
            assert_eq!(
                validate_flake_lock(
                    &serde_json::to_vec(&drifted).expect("flake json"),
                    &fixture.revision
                ),
                Err(CommandError::InvalidFlakeLock),
                "accepted {section}.{field}"
            );
        }

        let mut duplicate = canonical.clone();
        duplicate["nodes"]["lib2"] = duplicate["nodes"]["lib"].clone();
        assert_eq!(
            validate_flake_lock(
                &serde_json::to_vec(&duplicate).expect("flake json"),
                &fixture.revision
            ),
            Err(CommandError::InvalidFlakeLock)
        );
    }

    #[test]
    fn service_metadata_and_catalog_reject_independent_field_drift() {
        let fixture = Fixture::new();
        let bytes = fs::read(fixture.service.join(CARGO_MANIFEST)).expect("manifest");
        let canonical = parse_toml(&bytes, CommandError::InvalidCargoManifest).expect("manifest");
        for (field, replacement) in [
            ("service", toml::Value::Integer(1)),
            ("host_feature_profile", toml::Value::String("other".into())),
            ("config_contract_version", toml::Value::Integer(0)),
            ("state_contract_version", toml::Value::Integer(0)),
            ("admin_contract_version", toml::Value::Integer(0)),
            ("status_contract_version", toml::Value::Integer(0)),
            ("provider_contract_version", toml::Value::Integer(0)),
        ] {
            let mut drifted = canonical.clone();
            drifted["workspace"]["metadata"]["radroots"]["service_source_lock"][field] =
                replacement;
            assert_eq!(
                parse_service_metadata(&drifted),
                Err(CommandError::InvalidServiceMetadata),
                "accepted metadata drift at {field}"
            );
        }
        let mut extra = canonical;
        extra["workspace"]["metadata"]["radroots"]["service_source_lock"]
            .as_table_mut()
            .expect("metadata table")
            .insert("extra".into(), toml::Value::Integer(1));
        assert_eq!(
            parse_service_metadata(&extra),
            Err(CommandError::InvalidServiceMetadata)
        );

        let catalog = br#"schema = "radroots.workspace.catalog.v2"
architecture = "radroots.crates.release.v2"
version = "0.1.0-alpha"
package_count = 2

[[package]]
name = "radroots_core"

[[package]]
name = "radroots_service_host"
"#;
        let canonical = parse_toml(catalog, CommandError::InvalidSourceArchive).expect("catalog");
        for (field, replacement) in [
            ("schema", toml::Value::String("other".into())),
            ("architecture", toml::Value::String("other".into())),
            ("version", toml::Value::String("other".into())),
            ("package_count", toml::Value::Integer(1)),
        ] {
            let mut drifted = canonical.clone();
            drifted[field] = replacement;
            assert_eq!(
                catalog_package_names(toml::to_string(&drifted).expect("catalog").as_bytes()),
                Err(CommandError::InvalidSourceArchive),
                "accepted catalog drift at {field}"
            );
        }
        let mut duplicate = canonical.clone();
        duplicate["package"][1]["name"] = toml::Value::String("radroots_core".into());
        assert_eq!(
            catalog_package_names(toml::to_string(&duplicate).expect("catalog").as_bytes()),
            Err(CommandError::InvalidSourceArchive)
        );
        let mut missing_host = canonical;
        missing_host["package"][1]["name"] = toml::Value::String("radroots_other".into());
        assert_eq!(
            catalog_package_names(toml::to_string(&missing_host).expect("catalog").as_bytes()),
            Err(CommandError::InvalidSourceArchive)
        );
    }

    #[test]
    fn manifest_and_cargo_lock_reject_each_independent_dependency_drift() {
        let revision = "a".repeat(40);
        let canonical = format!(
            "[dependencies]\nradroots_service_host = {{ git = \"{LIB_REPOSITORY}\", rev = \"{revision}\", version = \"{LIB_VERSION_REQUIREMENT}\" }}\n"
        );
        for (from, to) in [
            (LIB_REPOSITORY, "https://example.invalid/lib"),
            (LIB_VERSION_REQUIREMENT, "=0.2.0"),
            (revision.as_str(), "invalid"),
        ] {
            let value =
                toml::from_str::<toml::Value>(&canonical.replacen(from, to, 1)).expect("manifest");
            assert_eq!(
                validate_manifest_node(
                    &value,
                    None,
                    false,
                    false,
                    None,
                    &mut ManifestState::default()
                ),
                Err(CommandError::InvalidCargoManifest)
            );
        }
        for forbidden in ["branch", "tag", "path"] {
            let mutated = canonical.replacen(
                "version = \"=0.1.0-alpha\"",
                &format!("version = \"=0.1.0-alpha\", {forbidden} = \"forbidden\""),
                1,
            );
            let value = toml::from_str::<toml::Value>(&mutated).expect("manifest");
            assert_eq!(
                validate_manifest_node(
                    &value,
                    None,
                    false,
                    false,
                    None,
                    &mut ManifestState::default()
                ),
                Err(CommandError::InvalidCargoManifest),
                "accepted {forbidden}"
            );
        }
        let value = toml::from_str::<toml::Value>(&canonical).expect("manifest");
        assert_eq!(
            validate_manifest_node(
                &value,
                None,
                true,
                false,
                None,
                &mut ManifestState::default()
            ),
            Err(CommandError::InvalidCargoManifest)
        );
        let mut state = ManifestState {
            revision: Some("b".repeat(40)),
            ..ManifestState::default()
        };
        assert_eq!(
            validate_manifest_node(&value, None, false, false, None, &mut state),
            Err(CommandError::InvalidCargoManifest)
        );

        let packages = BTreeSet::from([HOST_PACKAGE.to_owned(), "radroots_core".to_owned()]);
        let expected = format!("git+{LIB_REPOSITORY}?rev={revision}#{revision}");
        for document in [
            format!(
                "version = 4\n\n[[package]]\nname = \"{HOST_PACKAGE}\"\nversion = \"0.1.0-alpha\"\n"
            ),
            format!(
                "version = 4\n\n[[package]]\nname = \"{HOST_PACKAGE}\"\nversion = \"0.1.0-alpha\"\nsource = \"other\"\n"
            ),
            format!(
                "version = 4\n\n[[package]]\nname = \"{HOST_PACKAGE}\"\nversion = \"0.2.0\"\nsource = \"{expected}\"\n"
            ),
            format!(
                "version = 4\n\n[[package]]\nname = \"radroots_core\"\nversion = \"0.1.0-alpha\"\nsource = \"{expected}\"\n"
            ),
            "version = 4\n".to_owned(),
        ] {
            assert_eq!(
                validate_cargo_lock(document.as_bytes(), &revision, &packages),
                Err(CommandError::InvalidCargoLock)
            );
        }
    }

    #[test]
    fn archive_and_manifest_filesystem_admission_is_fail_closed() {
        let root = TempDir::new().expect("filesystem fixture");
        let empty = root.path().join("empty");
        fs::write(&empty, b"").expect("empty file");
        assert!(matches!(
            validate_archive(Path::new("relative"), &"a".repeat(40)),
            Err(CommandError::InvalidSourceArchive)
        ));
        assert!(matches!(
            validate_archive(root.path(), &"a".repeat(40)),
            Err(CommandError::InvalidSourceArchive)
        ));
        assert!(matches!(
            validate_archive(&empty, &"a".repeat(40)),
            Err(CommandError::InvalidSourceArchive)
        ));

        let no_manifest = root.path().join("no-manifest");
        fs::create_dir(&no_manifest).expect("empty tree");
        assert_eq!(
            validate_cargo_manifests(&no_manifest, None),
            Err(CommandError::InvalidCargoManifest)
        );
        let no_dependency = root.path().join("no-dependency");
        fs::create_dir(&no_dependency).expect("manifest tree");
        fs::write(
            no_dependency.join(CARGO_MANIFEST),
            "[workspace]\nresolver = \"3\"\n",
        )
        .expect("manifest");
        assert_eq!(
            validate_cargo_manifests(&no_dependency, None),
            Err(CommandError::InvalidCargoManifest)
        );

        #[cfg(unix)]
        {
            let archive_link = root.path().join("archive-link");
            std::os::unix::fs::symlink(&empty, &archive_link).expect("archive symlink");
            assert!(matches!(
                validate_archive(&archive_link, &"a".repeat(40)),
                Err(CommandError::InvalidSourceArchive)
            ));
            let manifest_link_root = root.path().join("manifest-link");
            fs::create_dir(&manifest_link_root).expect("manifest symlink tree");
            std::os::unix::fs::symlink(&empty, manifest_link_root.join(CARGO_MANIFEST))
                .expect("manifest symlink");
            assert_eq!(
                validate_cargo_manifests(&manifest_link_root, None),
                Err(CommandError::InvalidCargoManifest)
            );
        }
    }

    #[test]
    fn source_lock_output_rejects_symlink_replacement() {
        let fixture = Fixture::new();
        let target = fixture.root.path().join("foreign");
        fs::write(&target, b"foreign").expect("foreign");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, fixture.service.join(LOCK_FILENAME)).expect("symlink");
        #[cfg(unix)]
        assert_eq!(
            run_with(
                CommandMode::Write,
                &fixture.service,
                &fixture.archive,
                &fixture.reachable(),
            ),
            Err(CommandError::WriteFailure)
        );
        #[cfg(unix)]
        assert_eq!(fs::read(&target).expect("foreign remains"), b"foreign");
    }

    #[test]
    fn operational_diagnostics_are_fixed_and_source_free() {
        let errors = [
            CommandError::InvalidServiceRoot,
            CommandError::DirtyServiceSource,
            CommandError::InvalidServiceMetadata,
            CommandError::InvalidCargoManifest,
            CommandError::InvalidCargoLock,
            CommandError::InvalidFlakeLock,
            CommandError::InvalidToolchain,
            CommandError::InvalidSourceArchive,
            CommandError::UnreachableRevision,
            CommandError::InvalidSourceLock,
            CommandError::StaleSourceLock,
            CommandError::WriteFailure,
        ];
        for error in errors {
            assert!(error.source().is_none());
            let rendered = format!("{error:?} {error}");
            assert!(!rendered.contains("fixture_service"));
            assert!(!rendered.contains("radrootslabs"));
            assert!(!rendered.contains("/tmp"));
        }
    }
}
