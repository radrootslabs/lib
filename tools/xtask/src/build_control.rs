use std::{
    collections::BTreeSet,
    fs,
    io::Write,
    path::{Component, Path, PathBuf},
    process::Command,
};

use fs2::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::build_output::is_build_output_directory;

const SOURCE_LOCK_NAME: &str = "radroots.lib.source-lock.v1.toml";
const CONSUMER_MARKER: &str = ".radroots-consumer-root";
const CATALOG_RELATIVE: &str = "contracts/crates/catalog.v2.toml";
const REPOSITORY: &str = "https://github.com/radrootslabs/lib";
const ARCHITECTURE: &str = "radroots.crates.release.v2";
const VERSION: &str = "0.1.0-alpha";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Operation {
    Check,
    Test,
    Clippy,
}

impl Operation {
    pub fn cargo_subcommand(self) -> &'static str {
        match self {
            Self::Check => "check",
            Self::Test => "test",
            Self::Clippy => "clippy",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    Check,
    Write,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PackageGroups {
    schema: String,
    catalog_sha256: String,
    group: Vec<PackageGroup>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PackageGroup {
    id: String,
    packages: Vec<String>,
    active_packages: Vec<String>,
    reserved_packages: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceLock {
    pub schema: String,
    pub repository: String,
    pub revision: String,
    pub architecture: String,
    pub workspace_catalog_sha256: String,
    pub version: String,
    pub source_archive_sha256: Option<String>,
    #[serde(default = "default_consumer_lockfile")]
    pub lockfile: String,
    pub lockfile_sha256: String,
}

fn default_consumer_lockfile() -> String {
    "Cargo.lock".to_owned()
}

#[derive(Clone, Debug)]
pub struct ConsumerRoot {
    path: PathBuf,
    product: String,
    source_lock: SourceLock,
}

impl ConsumerRoot {
    pub fn open(path: &Path) -> Result<Self, String> {
        require_absolute_real_directory(path, "consumer root")?;
        let canonical = fs::canonicalize(path)
            .map_err(|error| format!("canonicalize consumer root {}: {error}", path.display()))?;
        let marker = read_regular_no_follow(&canonical.join(CONSUMER_MARKER))?;
        let product = String::from_utf8(marker)
            .map_err(|error| format!("consumer marker is not UTF-8: {error}"))?
            .trim()
            .to_owned();
        if !matches!(product.as_str(), "sdk" | "mobile" | "myc" | "rhi") {
            return Err("consumer marker must contain sdk, mobile, myc, or rhi".to_owned());
        }
        let source_lock_path = canonical.join(SOURCE_LOCK_NAME);
        let source_lock = parse_source_lock(&source_lock_path)?;
        validate_source_lock(&source_lock)?;
        validate_consumer_files(&canonical, &source_lock)?;
        Ok(Self {
            path: canonical,
            product,
            source_lock,
        })
    }

    fn output(&self, relative: &Path) -> Result<PathBuf, String> {
        validate_relative_path(relative, "artifact output")?;
        let output = self.path.join(relative);
        ensure_no_symlink_components(&self.path, relative)?;
        Ok(output)
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CacheManifest {
    schema: String,
    repository: String,
    revision: String,
    workspace_catalog_sha256: String,
    source_archive_sha256: Option<String>,
    tree: String,
}

#[derive(Debug, Deserialize)]
struct CargoLock {
    #[serde(default)]
    package: Vec<CargoLockPackage>,
}

#[derive(Debug, Deserialize)]
struct CargoLockPackage {
    source: Option<String>,
}

#[derive(Debug, Serialize)]
struct ArtifactManifest<'a> {
    schema: &'static str,
    product: &'a str,
    target: &'a str,
    language: &'a str,
    external_names: Vec<&'a str>,
    files: Vec<ArtifactFile>,
    provenance: Provenance<'a>,
}

#[derive(Debug, Serialize)]
struct ArtifactFile {
    path: String,
    bytes: u64,
    sha256: String,
}

#[derive(Debug, Serialize)]
struct Provenance<'a> {
    repository: &'a str,
    revision: &'a str,
    architecture: &'a str,
    catalog_sha256: &'a str,
    lockfile_sha256: &'a str,
    source_archive_sha256: Option<&'a str>,
    source_date_epoch: u64,
    builder_id: &'a str,
    features: Vec<&'a str>,
}

pub fn group_plan(
    workspace_root: &Path,
    group: &str,
    operation: Operation,
    include_reserved: bool,
) -> Result<Vec<String>, String> {
    validate_identifier(group, "group")?;
    let projection_path = workspace_root.join("contracts/crates/generated/package_groups.v1.toml");
    let bytes = read_regular_no_follow(&projection_path)?;
    let raw = std::str::from_utf8(&bytes)
        .map_err(|error| format!("package group projection is not UTF-8: {error}"))?;
    let projection = toml::from_str::<PackageGroups>(raw)
        .map_err(|error| format!("parse package group projection: {error}"))?;
    if projection.schema != "radroots.workspace.package-groups.v1" {
        return Err("package group projection schema drifted".to_owned());
    }
    validate_sha256(&projection.catalog_sha256, "catalog digest")?;
    let catalog = read_regular_no_follow(&workspace_root.join(CATALOG_RELATIVE))?;
    if sha256(&catalog) != projection.catalog_sha256 {
        return Err("package group projection is stale".to_owned());
    }
    let selected = projection
        .group
        .iter()
        .find(|candidate| candidate.id == group)
        .ok_or_else(|| format!("unknown catalog group {group}"))?;
    let expected = selected
        .active_packages
        .iter()
        .chain(selected.reserved_packages.iter())
        .cloned()
        .collect::<BTreeSet<_>>();
    if selected.packages.iter().cloned().collect::<BTreeSet<_>>() != expected
        || selected.active_packages.is_empty()
    {
        return Err(format!("catalog group {group} is malformed or not active"));
    }
    let packages = if include_reserved {
        &selected.packages
    } else {
        &selected.active_packages
    };
    let mut plan = vec![
        operation.cargo_subcommand().to_owned(),
        "--locked".to_owned(),
    ];
    if operation != Operation::Test {
        plan.push("--all-targets".to_owned());
    }
    for package in packages {
        plan.push("-p".to_owned());
        plan.push(package.clone());
    }
    if operation == Operation::Clippy {
        plan.push("--".to_owned());
        plan.push("-D".to_owned());
        plan.push("warnings".to_owned());
    }
    Ok(plan)
}

pub fn execute_group_plan(workspace_root: &Path, plan: &[String]) -> Result<(), String> {
    let output = Command::new("cargo")
        .args(plan)
        .current_dir(workspace_root)
        .status()
        .map_err(|error| format!("run catalog group plan: {error}"))?;
    if output.success() {
        Ok(())
    } else {
        Err(format!("catalog group plan failed with {output}"))
    }
}

pub fn print_plan(plan: &[String]) {
    println!("cargo {}", plan.join(" "));
}

pub fn validate_consumer(path: &Path) -> Result<ConsumerRoot, String> {
    ConsumerRoot::open(path)
}

pub fn materialize(
    consumer_path: &Path,
    cache_root: &Path,
    offline: bool,
) -> Result<PathBuf, String> {
    let consumer = ConsumerRoot::open(consumer_path)?;
    materialize_from(
        &consumer,
        cache_root,
        offline,
        &consumer.source_lock.repository,
    )
}

fn materialize_from(
    consumer: &ConsumerRoot,
    cache_root: &Path,
    offline: bool,
    fetch_url: &str,
) -> Result<PathBuf, String> {
    require_absolute_real_directory(cache_root, "cache root")?;
    let key = format!(
        "{}-{}",
        consumer.source_lock.revision, consumer.source_lock.workspace_catalog_sha256
    );
    let destination = cache_root.join(key);
    let lock_path = cache_root.join(".radroots-source-cache.lock");
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|error| format!("open source cache lock: {error}"))?;
    lock.lock_exclusive()
        .map_err(|error| format!("lock source cache: {error}"))?;
    let result = if destination.exists() {
        verify_cache(&destination, &consumer.source_lock).map(|()| destination.clone())
    } else if offline {
        Err("offline source materialization requires a verified cache entry".to_owned())
    } else {
        prefetch_cache(cache_root, &destination, &consumer.source_lock, fetch_url)?;
        verify_cache(&destination, &consumer.source_lock)?;
        Ok(destination.clone())
    };
    FileExt::unlock(&lock).map_err(|error| format!("unlock source cache: {error}"))?;
    result
}

fn prefetch_cache(
    cache_root: &Path,
    destination: &Path,
    source_lock: &SourceLock,
    fetch_url: &str,
) -> Result<(), String> {
    let staging = tempfile::Builder::new()
        .prefix(".radroots-source-stage-")
        .tempdir_in(cache_root)
        .map_err(|error| format!("create source cache staging directory: {error}"))?;
    git(staging.path(), &["init", "--quiet"])?;
    git(staging.path(), &["remote", "add", "origin", fetch_url])?;
    git(
        staging.path(),
        &[
            "fetch",
            "--quiet",
            "--depth=1",
            "origin",
            &source_lock.revision,
        ],
    )?;
    git(
        staging.path(),
        &["checkout", "--quiet", "--detach", "FETCH_HEAD"],
    )?;
    let tree = git_stdout(staging.path(), &["rev-parse", "HEAD^{tree}"])?;
    let manifest = CacheManifest {
        schema: "radroots.source-cache.v1".to_owned(),
        repository: source_lock.repository.clone(),
        revision: source_lock.revision.clone(),
        workspace_catalog_sha256: source_lock.workspace_catalog_sha256.clone(),
        source_archive_sha256: source_lock.source_archive_sha256.clone(),
        tree: tree.trim().to_owned(),
    };
    let raw = toml::to_string(&manifest)
        .map_err(|error| format!("serialize source cache manifest: {error}"))?;
    atomic_write(
        &staging.path().join(".radroots-source-cache.v1.toml"),
        raw.as_bytes(),
    )?;
    let staging_path = staging.keep();
    fs::rename(&staging_path, destination).map_err(|error| {
        format!(
            "install source cache {} -> {}: {error}",
            staging_path.display(),
            destination.display()
        )
    })?;
    set_readonly_tree(destination)
}

fn verify_cache(path: &Path, source_lock: &SourceLock) -> Result<(), String> {
    require_absolute_real_directory(path, "cache entry")?;
    let head = git_stdout(path, &["rev-parse", "HEAD"])?;
    let tree = git_stdout(path, &["rev-parse", "HEAD^{tree}"])?;
    if head.trim() != source_lock.revision {
        return Err("source cache revision drifted".to_owned());
    }
    git(path, &["diff", "--quiet", "--no-ext-diff"])?;
    git(path, &["diff", "--cached", "--quiet", "--no-ext-diff"])?;
    verify_untracked_cache_paths(path)?;
    let manifest_path = path.join(".radroots-source-cache.v1.toml");
    let bytes = read_regular_no_follow(&manifest_path)?;
    let raw = std::str::from_utf8(&bytes)
        .map_err(|error| format!("source cache manifest is not UTF-8: {error}"))?;
    let manifest = toml::from_str::<CacheManifest>(raw)
        .map_err(|error| format!("parse source cache manifest: {error}"))?;
    if manifest.schema != "radroots.source-cache.v1"
        || manifest.repository != source_lock.repository
        || manifest.revision != source_lock.revision
        || manifest.workspace_catalog_sha256 != source_lock.workspace_catalog_sha256
        || manifest.source_archive_sha256 != source_lock.source_archive_sha256
        || manifest.tree != tree.trim()
    {
        return Err("source cache manifest drifted".to_owned());
    }
    let catalog = read_regular_no_follow(&path.join(CATALOG_RELATIVE))?;
    if sha256(&catalog) != source_lock.workspace_catalog_sha256 {
        return Err("source cache catalog digest drifted".to_owned());
    }
    Ok(())
}

pub fn verify_source_archive(path: &Path, expected_sha256: &str) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("source archive path must be absolute".to_owned());
    }
    validate_sha256(expected_sha256, "source archive digest")?;
    let bytes = read_regular_no_follow(path)?;
    if sha256(&bytes) != expected_sha256 {
        return Err("source archive digest drifted".to_owned());
    }
    verify_bundle(path)
}

pub fn create_source_archive(
    source_root: &Path,
    revision: &str,
    output_path: &Path,
) -> Result<String, String> {
    require_absolute_real_directory(source_root, "archive source root")?;
    validate_oid(revision, "archive revision")?;
    if !output_path.is_absolute() {
        return Err("source archive output must be absolute".to_owned());
    }
    git(
        source_root,
        &["cat-file", "-e", &format!("{revision}^{{commit}}")],
    )?;
    let parent = output_path
        .parent()
        .ok_or_else(|| "source archive output has no parent".to_owned())?;
    create_directories_no_follow(parent)?;
    if let Ok(metadata) = fs::symlink_metadata(output_path)
        && (metadata.file_type().is_symlink() || !metadata.is_file())
    {
        return Err("source archive output must be a regular file".to_owned());
    }
    let temporary = tempfile::Builder::new()
        .prefix(".radroots-source-archive-")
        .tempfile_in(parent)
        .map_err(|error| format!("stage source archive: {error}"))?;
    let temporary_path = temporary.path().to_path_buf();
    temporary
        .close()
        .map_err(|error| format!("prepare source archive staging path: {error}"))?;
    let archive_repo = tempfile::TempDir::new_in(parent)
        .map_err(|error| format!("create archive staging repository: {error}"))?;
    git(archive_repo.path(), &["init", "--bare", "--quiet"])?;
    let fetch = Command::new("git")
        .args(["fetch", "--quiet", "--no-tags"])
        .arg(source_root)
        .arg(format!("{revision}:refs/heads/archive"))
        .current_dir(archive_repo.path())
        .output()
        .map_err(|error| format!("stage archive revision: {error}"))?;
    if !fetch.status.success() {
        return Err(format!(
            "stage archive revision failed: {}",
            String::from_utf8_lossy(&fetch.stderr).trim()
        ));
    }
    let output = Command::new("git")
        .args(["bundle", "create"])
        .arg(&temporary_path)
        .arg("refs/heads/archive")
        .current_dir(archive_repo.path())
        .output()
        .map_err(|error| format!("create source archive: {error}"))?;
    if !output.status.success() {
        let _ = fs::remove_file(&temporary_path);
        return Err(format!(
            "create source archive failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let bytes = read_regular_no_follow(&temporary_path)?;
    let digest = sha256(&bytes);
    verify_bundle(&temporary_path)?;
    let listed = Command::new("git")
        .args(["bundle", "list-heads"])
        .arg(&temporary_path)
        .output()
        .map_err(|error| format!("list source archive heads: {error}"))?;
    if !listed.status.success()
        || String::from_utf8_lossy(&listed.stdout).trim()
            != format!("{revision} refs/heads/archive")
    {
        let _ = fs::remove_file(&temporary_path);
        return Err("source archive does not contain exactly the requested revision".to_owned());
    }
    if output_path.exists() {
        let existing = read_regular_no_follow(output_path)?;
        let _ = fs::remove_file(&temporary_path);
        if existing == bytes {
            return Ok(digest);
        }
        return Err(
            "immutable source archive output already exists with different bytes".to_owned(),
        );
    }
    fs::File::open(&temporary_path)
        .and_then(|file| file.sync_all())
        .map_err(|error| format!("sync source archive: {error}"))?;
    fs::rename(&temporary_path, output_path)
        .map_err(|error| format!("install source archive: {error}"))?;
    Ok(digest)
}

fn verify_bundle(path: &Path) -> Result<(), String> {
    let verification_repo = tempfile::TempDir::new()
        .map_err(|error| format!("create bundle verification repository: {error}"))?;
    git(verification_repo.path(), &["init", "--bare", "--quiet"])?;
    let output = Command::new("git")
        .args(["bundle", "verify"])
        .arg(path)
        .current_dir(verification_repo.path())
        .output()
        .map_err(|error| format!("run git bundle verify: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "source archive is not a valid Git bundle: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

#[allow(clippy::too_many_arguments)]
pub fn artifact(
    product: &str,
    target: &str,
    language: &str,
    mode: Mode,
    consumer_path: &Path,
    source_path: &Path,
    output_relative: &Path,
    source_date_epoch: u64,
    builder_id: &str,
    features: &[String],
) -> Result<(), String> {
    validate_identifier(product, "product")?;
    validate_identifier(target, "target")?;
    validate_identifier(language, "language")?;
    validate_identifier(builder_id, "builder id")?;
    validate_generation_roots(product, target, language, consumer_path, source_path)?;
    let consumer = ConsumerRoot::open(consumer_path)?;
    let mut sorted_features = features.iter().map(String::as_str).collect::<Vec<_>>();
    sorted_features.sort_unstable();
    sorted_features.dedup();
    for feature in &sorted_features {
        validate_identifier(feature, "feature")?;
    }
    let external_names = match product {
        "sdk" => vec!["radroots", "radroots_sdk"],
        "mobile" => vec!["RadrootsFFI", "RadrootsKitBindings"],
        _ => return Err("unsupported artifact product".to_owned()),
    };
    let manifest = ArtifactManifest {
        schema: "radroots.artifact-manifest.v1",
        product,
        target,
        language,
        external_names,
        files: Vec::new(),
        provenance: Provenance {
            repository: &consumer.source_lock.repository,
            revision: &consumer.source_lock.revision,
            architecture: &consumer.source_lock.architecture,
            catalog_sha256: &consumer.source_lock.workspace_catalog_sha256,
            lockfile_sha256: &consumer.source_lock.lockfile_sha256,
            source_archive_sha256: consumer.source_lock.source_archive_sha256.as_deref(),
            source_date_epoch,
            builder_id,
            features: sorted_features,
        },
    };
    let mut bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|error| format!("serialize artifact manifest: {error}"))?;
    bytes.push(b'\n');
    let output = consumer.output(output_relative)?;
    match mode {
        Mode::Check => {
            let current = read_regular_no_follow(&output)?;
            if current == bytes {
                Ok(())
            } else {
                Err(format!("artifact manifest {} is stale", output.display()))
            }
        }
        Mode::Write => atomic_write(&output, &bytes),
    }
}

pub fn validate_generation_roots(
    product: &str,
    target: &str,
    language: &str,
    consumer_path: &Path,
    source_path: &Path,
) -> Result<(), String> {
    validate_identifier(product, "product")?;
    validate_identifier(target, "target")?;
    validate_identifier(language, "language")?;
    validate_artifact_route(product, target, language)?;
    let consumer = ConsumerRoot::open(consumer_path)?;
    if consumer.product != product {
        return Err(format!(
            "consumer marker {} does not match artifact product {product}",
            consumer.product
        ));
    }
    verify_source_root(source_path, &consumer.source_lock)
}

fn validate_artifact_route(product: &str, target: &str, language: &str) -> Result<(), String> {
    let valid = match product {
        "sdk" => matches!(
            (target, language),
            ("typescript", "typescript")
                | ("wasm", "javascript")
                | ("ffi", "swift")
                | ("ffi", "kotlin")
        ),
        "mobile" => matches!(
            (target, language),
            ("ios", "swift") | ("android", "kotlin") | ("wasm", "javascript")
        ),
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(format!(
            "unsupported artifact route {product}/{target}/{language}"
        ))
    }
}

fn verify_source_root(path: &Path, source_lock: &SourceLock) -> Result<(), String> {
    require_absolute_real_directory(path, "source root")?;
    if git_stdout(path, &["rev-parse", "HEAD"])?.trim() != source_lock.revision {
        return Err("source root revision does not match source lock".to_owned());
    }
    let catalog = read_regular_no_follow(&path.join(CATALOG_RELATIVE))?;
    if sha256(&catalog) != source_lock.workspace_catalog_sha256 {
        return Err("source root catalog does not match source lock".to_owned());
    }
    git(path, &["diff", "--quiet", "--no-ext-diff"])?;
    git(path, &["diff", "--cached", "--quiet", "--no-ext-diff"])?;
    verify_untracked_cache_paths(path)
}

fn verify_untracked_cache_paths(path: &Path) -> Result<(), String> {
    let status = git_stdout(path, &["status", "--porcelain", "--untracked-files=all"])?;
    for line in status.lines() {
        if line != "?? .radroots-source-cache.v1.toml" {
            return Err(format!("source tree contains ungoverned change {line}"));
        }
    }
    Ok(())
}

fn parse_source_lock(path: &Path) -> Result<SourceLock, String> {
    let bytes = read_regular_no_follow(path)?;
    let raw = std::str::from_utf8(&bytes)
        .map_err(|error| format!("source lock is not UTF-8: {error}"))?;
    toml::from_str(raw).map_err(|error| format!("parse source lock {}: {error}", path.display()))
}

fn validate_source_lock(source_lock: &SourceLock) -> Result<(), String> {
    if source_lock.schema != "radroots.lib.source-lock.v1"
        || source_lock.repository != REPOSITORY
        || source_lock.architecture != ARCHITECTURE
        || source_lock.version != VERSION
    {
        return Err(
            "source lock identity, repository, architecture, or version drifted".to_owned(),
        );
    }
    validate_oid(&source_lock.revision, "source lock revision")?;
    validate_sha256(
        &source_lock.workspace_catalog_sha256,
        "source lock catalog digest",
    )?;
    validate_sha256(&source_lock.lockfile_sha256, "source lock lockfile digest")?;
    validate_relative_path(Path::new(&source_lock.lockfile), "source lock lockfile")?;
    if let Some(digest) = &source_lock.source_archive_sha256 {
        validate_sha256(digest, "source lock archive digest")?;
    }
    Ok(())
}

fn validate_consumer_files(root: &Path, source_lock: &SourceLock) -> Result<(), String> {
    let lockfile_relative = Path::new(&source_lock.lockfile);
    ensure_no_symlink_components(root, lockfile_relative)?;
    let lockfile = root.join(lockfile_relative);
    let lockfile_bytes = read_regular_no_follow(&lockfile)?;
    if sha256(&lockfile_bytes) != source_lock.lockfile_sha256 {
        return Err("consumer Cargo.lock digest drifted".to_owned());
    }
    let lockfile_raw = std::str::from_utf8(&lockfile_bytes)
        .map_err(|error| format!("consumer Cargo.lock is not UTF-8: {error}"))?;
    let cargo_lock = toml::from_str::<CargoLock>(lockfile_raw)
        .map_err(|error| format!("parse consumer Cargo.lock: {error}"))?;
    let expected_source = format!(
        "git+{}?rev={}#{}",
        source_lock.repository, source_lock.revision, source_lock.revision
    );
    let mut lock_source_count = 0_usize;
    for source in cargo_lock
        .package
        .iter()
        .filter_map(|package| package.source.as_deref())
        .filter(|source| source.contains("radrootslabs/lib"))
    {
        lock_source_count += 1;
        if source != expected_source {
            return Err("consumer Cargo.lock contains a mixed or floating lib source".to_owned());
        }
    }
    if lock_source_count == 0 {
        return Err("consumer Cargo.lock contains no canonical lib source".to_owned());
    }
    let mut manifests = Vec::new();
    collect_manifests(root, root, &mut manifests)?;
    if manifests.is_empty() {
        return Err("consumer root contains no Cargo manifest".to_owned());
    }
    let mut dependency_count = 0_usize;
    for manifest in manifests {
        let bytes = read_regular_no_follow(&manifest)?;
        let raw = std::str::from_utf8(&bytes)
            .map_err(|error| format!("consumer manifest is not UTF-8: {error}"))?;
        let value = toml::from_str::<toml::Value>(raw)
            .map_err(|error| format!("parse consumer manifest {}: {error}", manifest.display()))?;
        validate_manifest_value(&value, source_lock, &mut dependency_count)?;
    }
    if dependency_count == 0 {
        return Err("consumer manifests contain no canonical lib dependency".to_owned());
    }
    Ok(())
}

fn collect_manifests(root: &Path, current: &Path, output: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(current)
        .map_err(|error| format!("read consumer directory {}: {error}", current.display()))?
    {
        let entry = entry.map_err(|error| format!("read consumer directory entry: {error}"))?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("inspect {}: {error}", entry.path().display()))?;
        if file_type.is_symlink() {
            return Err(format!(
                "consumer tree contains symlink {}",
                entry.path().display()
            ));
        }
        let name = entry.file_name();
        if file_type.is_dir() {
            if matches!(name.to_str(), Some(".git" | ".radroots"))
                || is_build_output_directory(name.as_os_str())
            {
                continue;
            }
            collect_manifests(root, &entry.path(), output)?;
        } else if name == "Cargo.toml" {
            entry
                .path()
                .strip_prefix(root)
                .map_err(|_| "consumer manifest escaped root".to_owned())?;
            output.push(entry.path());
        }
    }
    output.sort();
    Ok(())
}

fn validate_manifest_value(
    value: &toml::Value,
    source_lock: &SourceLock,
    dependency_count: &mut usize,
) -> Result<(), String> {
    match value {
        toml::Value::Table(table) => {
            let structured_lib_url = table
                .get("git")
                .and_then(toml::Value::as_str)
                .filter(|git| git.contains("radrootslabs/lib"));
            if let Some(git) = structured_lib_url {
                *dependency_count += 1;
                if git != source_lock.repository
                    || table.get("rev").and_then(toml::Value::as_str)
                        != Some(source_lock.revision.as_str())
                    || table.get("version").and_then(toml::Value::as_str) != Some("=0.1.0-alpha")
                    || table.contains_key("branch")
                    || table.contains_key("tag")
                    || table.contains_key("path")
                {
                    return Err(
                        "consumer manifest contains a mixed or floating lib dependency".to_owned(),
                    );
                }
            }
            for (key, child) in table {
                if structured_lib_url.is_some() && key == "git" {
                    continue;
                }
                if key == "patch" && format!("{child:?}").contains("radrootslabs/lib") {
                    return Err("consumer manifest contains a lib patch override".to_owned());
                }
                validate_manifest_value(child, source_lock, dependency_count)?;
            }
        }
        toml::Value::Array(values) => {
            for child in values {
                validate_manifest_value(child, source_lock, dependency_count)?;
            }
        }
        toml::Value::String(value) if value.contains("radrootslabs/lib") => {
            return Err("consumer manifest contains an unstructured lib source".to_owned());
        }
        _ => {}
    }
    Ok(())
}

pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if bytes.contains(&b'\r') || !bytes.ends_with(b"\n") {
        return Err("generated text must use LF and end with one newline".to_owned());
    }
    atomic_write_bytes(path, bytes)
}

pub(crate) fn atomic_write_bytes(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("output {} has no parent", path.display()))?;
    create_directories_no_follow(parent)?;
    if let Ok(metadata) = fs::symlink_metadata(path) {
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(format!("output {} must be a regular file", path.display()));
        }
        if fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))? == bytes {
            return Ok(());
        }
    }
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| format!("stage output in {}: {error}", parent.display()))?;
    temporary
        .write_all(bytes)
        .map_err(|error| format!("stage output {}: {error}", path.display()))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| format!("sync staged output {}: {error}", path.display()))?;
    temporary
        .persist(path)
        .map_err(|error| format!("replace output {}: {}", path.display(), error.error))?;
    Ok(())
}

fn create_directories_no_follow(path: &Path) -> Result<(), String> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(format!(
                    "output parent {} is not a real directory",
                    current.display()
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&current).map_err(|error| {
                    format!("create output directory {}: {error}", current.display())
                })?;
            }
            Err(error) => {
                return Err(format!(
                    "inspect output directory {}: {error}",
                    current.display()
                ));
            }
        }
    }
    Ok(())
}

fn ensure_no_symlink_components(root: &Path, relative: &Path) -> Result<(), String> {
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(format!(
                    "artifact path contains symlink {}",
                    current.display()
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(error) => {
                return Err(format!(
                    "inspect artifact path {}: {error}",
                    current.display()
                ));
            }
        }
    }
    Ok(())
}

fn read_regular_no_follow(path: &Path) -> Result<Vec<u8>, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("inspect {}: {error}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!(
            "{} must be a regular non-symlink file",
            path.display()
        ));
    }
    fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))
}

fn require_absolute_real_directory(path: &Path, label: &str) -> Result<(), String> {
    if !path.is_absolute() {
        return Err(format!("{label} must be absolute"));
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("inspect {label} {}: {error}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!("{label} must be a real directory"));
    }
    Ok(())
}

fn validate_relative_path(path: &Path, label: &str) -> Result<(), String> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!("{label} must be a safe relative path"));
    }
    Ok(())
}

fn validate_identifier(value: &str, label: &str) -> Result<(), String> {
    if value.is_empty()
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-' | b'/')
        })
    {
        return Err(format!("{label} contains unsupported characters"));
    }
    Ok(())
}

fn validate_oid(value: &str, label: &str) -> Result<(), String> {
    if value.len() != 40
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!("{label} must be lowercase full 40-hex"));
    }
    Ok(())
}

fn validate_sha256(value: &str, label: &str) -> Result<(), String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!("{label} must be lowercase 64-hex"));
    }
    Ok(())
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn git(root: &Path, args: &[&str]) -> Result<(), String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|error| format!("run git {}: {error}", args.join(" ")))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

fn git_stdout(root: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
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
        .map_err(|error| format!("git {} output is not UTF-8: {error}", args.join(" ")))
}

fn set_readonly_tree(root: &Path) -> Result<(), String> {
    for entry in walk(root)? {
        let metadata = fs::symlink_metadata(&entry)
            .map_err(|error| format!("inspect cache path {}: {error}", entry.display()))?;
        if metadata.file_type().is_symlink() {
            return Err(format!("source cache contains symlink {}", entry.display()));
        }
        let mut permissions = metadata.permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&entry, permissions)
            .map_err(|error| format!("protect cache path {}: {error}", entry.display()))?;
    }
    Ok(())
}

fn walk(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut pending = vec![root.to_path_buf()];
    let mut output = Vec::new();
    while let Some(path) = pending.pop() {
        output.push(path.clone());
        if fs::symlink_metadata(&path)
            .map_err(|error| format!("inspect {}: {error}", path.display()))?
            .is_dir()
        {
            for entry in
                fs::read_dir(&path).map_err(|error| format!("read {}: {error}", path.display()))?
            {
                pending.push(
                    entry
                        .map_err(|error| format!("read directory entry: {error}"))?
                        .path(),
                );
            }
        }
    }
    output.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture {
        _root: tempfile::TempDir,
        source: PathBuf,
        consumer: PathBuf,
        cache: PathBuf,
        source_lock: SourceLock,
    }

    impl Fixture {
        fn new(product: &str) -> Self {
            let root = tempfile::TempDir::new().expect("fixture root");
            let source = root.path().join("source");
            let consumer = root.path().join("consumer");
            let cache = root.path().join("cache");
            fs::create_dir(&source).expect("source");
            fs::create_dir(&consumer).expect("consumer");
            fs::create_dir(&cache).expect("cache");
            git(&source, &["init", "--initial-branch=master"]).expect("init");
            git(&source, &["config", "user.name", "Build Control Fixture"]).expect("name");
            git(
                &source,
                &["config", "user.email", "build-control@radroots.org"],
            )
            .expect("email");
            fs::create_dir_all(source.join("contracts/crates")).expect("contracts");
            fs::write(
                source.join(CATALOG_RELATIVE),
                "schema = \"radroots.workspace.catalog.v2\"\n",
            )
            .expect("catalog");
            fs::create_dir_all(source.join("src")).expect("source crate");
            fs::write(
                source.join("Cargo.toml"),
                "[package]\nname = \"source_fixture\"\nversion = \"0.1.0-alpha\"\nedition = \"2024\"\n",
            )
            .expect("source manifest");
            fs::write(source.join("src/lib.rs"), "pub const READY: bool = true;\n")
                .expect("source library");
            fs::write(
                source.join("Cargo.lock"),
                "# This file is automatically @generated by Cargo.\n# It is not intended for manual editing.\nversion = 4\n\n[[package]]\nname = \"source_fixture\"\nversion = \"0.1.0-alpha\"\n",
            )
            .expect("source lockfile");
            git(&source, &["add", "--all"]).expect("add");
            git(&source, &["commit", "-m", "seed source fixture"]).expect("commit");
            let revision = git_stdout(&source, &["rev-parse", "HEAD"])
                .expect("revision")
                .trim()
                .to_owned();
            let catalog_sha256 =
                sha256(&fs::read(source.join(CATALOG_RELATIVE)).expect("read source catalog"));
            fs::write(consumer.join(CONSUMER_MARKER), format!("{product}\n")).expect("marker");
            fs::write(
                consumer.join("Cargo.toml"),
                format!(
                    "[package]\nname = \"consumer\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nradroots_core = {{ git = \"{REPOSITORY}\", rev = \"{revision}\", version = \"=0.1.0-alpha\" }}\n"
                ),
            )
            .expect("manifest");
            let consumer_lock = format!(
                "version = 4\n\n[[package]]\nname = \"radroots_core\"\nversion = \"0.1.0-alpha\"\nsource = \"git+{REPOSITORY}?rev={revision}#{revision}\"\n"
            );
            fs::write(consumer.join("Cargo.lock"), &consumer_lock).expect("consumer lockfile");
            let source_lock = SourceLock {
                schema: "radroots.lib.source-lock.v1".to_owned(),
                repository: REPOSITORY.to_owned(),
                revision,
                architecture: ARCHITECTURE.to_owned(),
                workspace_catalog_sha256: catalog_sha256,
                version: VERSION.to_owned(),
                source_archive_sha256: None,
                lockfile: default_consumer_lockfile(),
                lockfile_sha256: sha256(consumer_lock.as_bytes()),
            };
            fs::write(
                consumer.join(SOURCE_LOCK_NAME),
                toml::to_string(&source_lock).expect("serialize source lock"),
            )
            .expect("source lock");
            Self {
                _root: root,
                source,
                consumer,
                cache,
                source_lock,
            }
        }
    }

    #[test]
    fn source_lock_and_consumer_root_fail_closed() {
        let fixture = Fixture::new("sdk");
        ConsumerRoot::open(&fixture.consumer).expect("valid consumer");
        let mut invalid = fixture.source_lock.clone();
        invalid.revision = "short".to_owned();
        assert!(validate_source_lock(&invalid).is_err());

        fs::write(
            fixture.consumer.join("Cargo.toml"),
            "[package]\nname = \"consumer\"\nversion = \"0.1.0\"\n\n[dependencies]\nradroots_core = { git = \"https://github.com/radrootslabs/lib\", branch = \"master\", version = \"*\" }\n",
        )
        .expect("floating manifest");
        assert!(ConsumerRoot::open(&fixture.consumer).is_err());
    }

    #[test]
    fn source_lock_accepts_services_without_creating_artifact_routes() {
        for product in ["myc", "rhi"] {
            let fixture = Fixture::new(product);
            let consumer = ConsumerRoot::open(&fixture.consumer).expect("valid service consumer");
            assert_eq!(consumer.product, product);
            assert!(validate_artifact_route(product, "linux", "rust").is_err());
        }
    }

    #[test]
    fn source_lock_supports_a_contained_nested_lockfile() {
        let mut fixture = Fixture::new("sdk");
        let core = fixture.consumer.join("core");
        fs::create_dir(&core).expect("create nested capsule");
        fs::rename(fixture.consumer.join("Cargo.lock"), core.join("Cargo.lock"))
            .expect("move lockfile");
        fixture.source_lock.lockfile = "core/Cargo.lock".to_owned();
        fs::write(
            fixture.consumer.join(SOURCE_LOCK_NAME),
            toml::to_string(&fixture.source_lock).expect("serialize nested source lock"),
        )
        .expect("write nested source lock");

        ConsumerRoot::open(&fixture.consumer).expect("nested lockfile is valid");

        fixture.source_lock.lockfile = "../Cargo.lock".to_owned();
        fs::write(
            fixture.consumer.join(SOURCE_LOCK_NAME),
            toml::to_string(&fixture.source_lock).expect("serialize escaping source lock"),
        )
        .expect("write escaping source lock");
        assert!(ConsumerRoot::open(&fixture.consumer).is_err());
    }

    #[test]
    fn consumer_manifest_discovery_skips_swiftpm_build_output_only() {
        let fixture = Fixture::new("mobile");
        let swiftpm_checkout = fixture.consumer.join(".build/checkouts/dependency");
        fs::create_dir_all(&swiftpm_checkout).expect("SwiftPM checkout directory");
        fs::write(swiftpm_checkout.join("Cargo.toml"), "not valid TOML")
            .expect("SwiftPM build manifest");
        ConsumerRoot::open(&fixture.consumer).expect("SwiftPM build output is excluded");

        let source_lookalike = fixture.consumer.join(".builder");
        fs::create_dir(&source_lookalike).expect("source lookalike directory");
        fs::write(source_lookalike.join("Cargo.toml"), "not valid TOML")
            .expect("source lookalike manifest");
        assert!(ConsumerRoot::open(&fixture.consumer).is_err());
    }

    #[test]
    fn materialization_reuses_verified_cache_and_rejects_tampering() {
        let fixture = Fixture::new("sdk");
        let consumer = ConsumerRoot::open(&fixture.consumer).expect("consumer");
        let cached = materialize_from(
            &consumer,
            &fixture.cache,
            false,
            fixture.source.to_str().expect("source path"),
        )
        .expect("prefetch");
        assert_eq!(
            materialize_from(
                &consumer,
                &fixture.cache,
                true,
                fixture.source.to_str().expect("source path"),
            )
            .expect("offline reuse"),
            cached
        );
        let frozen_target = fixture._root.path().join("frozen-target");
        let frozen = Command::new("cargo")
            .args(["check", "--offline", "--frozen", "--locked"])
            .env("CARGO_TARGET_DIR", &frozen_target)
            .current_dir(&cached)
            .output()
            .expect("run frozen offline source smoke");
        assert!(
            frozen.status.success(),
            "frozen offline source smoke failed: {}",
            String::from_utf8_lossy(&frozen.stderr)
        );
        let catalog = cached.join(CATALOG_RELATIVE);
        make_path_writable(&catalog).expect("make catalog writable");
        fs::write(&catalog, "tampered\n").expect("tamper");
        assert!(verify_cache(&cached, &fixture.source_lock).is_err());
        make_writable(&cached);
    }

    #[test]
    fn artifact_manifests_are_deterministic_and_route_checked() {
        let fixture = Fixture::new("sdk");
        let output = Path::new("generated/artifact-manifest.json");
        artifact(
            "sdk",
            "typescript",
            "typescript",
            Mode::Write,
            &fixture.consumer,
            &fixture.source,
            output,
            1_700_000_000,
            "fixture_builder",
            &["zeta".to_owned(), "alpha".to_owned(), "alpha".to_owned()],
        )
        .expect("write artifact manifest");
        artifact(
            "sdk",
            "typescript",
            "typescript",
            Mode::Check,
            &fixture.consumer,
            &fixture.source,
            output,
            1_700_000_000,
            "fixture_builder",
            &["alpha".to_owned(), "zeta".to_owned()],
        )
        .expect("check artifact manifest");
        assert!(
            validate_artifact_route("mobile", "ios", "kotlin").is_err(),
            "unsupported target/language must fail"
        );
        assert!(validate_artifact_route("sdk", "wasm", "javascript").is_ok());
        assert!(validate_artifact_route("sdk", "ffi", "swift").is_ok());
        assert!(validate_artifact_route("sdk", "ffi", "kotlin").is_ok());
        assert!(validate_artifact_route("sdk", "wasm", "typescript").is_err());
        assert!(
            artifact(
                "sdk",
                "typescript",
                "typescript",
                Mode::Write,
                &fixture.consumer,
                &fixture.source,
                Path::new("../escape"),
                1,
                "fixture_builder",
                &[],
            )
            .is_err()
        );
    }

    #[test]
    fn atomic_outputs_reject_unsafe_text_and_path_collisions() {
        let root = tempfile::TempDir::new().expect("output fixture");
        let output = root.path().join("generated/output.json");
        atomic_write(&output, b"prior\n").expect("initial output");
        #[cfg(unix)]
        let initial_inode = {
            use std::os::unix::fs::MetadataExt;
            fs::metadata(&output).expect("initial metadata").ino()
        };
        atomic_write(&output, b"prior\n").expect("identical no-op");
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            assert_eq!(
                fs::metadata(&output).expect("no-op metadata").ino(),
                initial_inode
            );
        }
        assert!(atomic_write(&output, b"missing newline").is_err());
        assert!(atomic_write(&output, b"windows\r\n").is_err());
        assert_eq!(
            fs::read(&output).expect("prior output retained"),
            b"prior\n"
        );
        let directory_output = root.path().join("directory-output");
        fs::create_dir_all(&directory_output).expect("directory collision");
        assert!(atomic_write(&directory_output, b"{}\n").is_err());

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;

            let symlink_root = tempfile::TempDir::new().expect("symlink fixture");
            let outside = tempfile::TempDir::new().expect("outside fixture");
            symlink(outside.path(), symlink_root.path().join("generated"))
                .expect("output parent symlink");
            assert!(
                atomic_write(&symlink_root.path().join("generated/output.json"), b"{}\n").is_err()
            );
        }
    }

    #[test]
    fn source_archive_round_trip_is_digest_bound_and_immutable() {
        let fixture = Fixture::new("sdk");
        let archive = fixture._root.path().join("archives/source.bundle");
        let digest =
            create_source_archive(&fixture.source, &fixture.source_lock.revision, &archive)
                .expect("create archive");
        verify_source_archive(&archive, &digest).expect("verify archive");
        assert_eq!(
            create_source_archive(&fixture.source, &fixture.source_lock.revision, &archive,)
                .expect("identical archive no-op"),
            digest
        );
        assert!(verify_source_archive(&archive, &"0".repeat(64)).is_err());
        assert!(verify_source_archive(Path::new("relative.bundle"), &digest).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn consumer_and_source_roots_reject_symlinks() {
        use std::os::unix::fs::symlink;

        let fixture = Fixture::new("sdk");
        let consumer_link = fixture._root.path().join("consumer-link");
        symlink(&fixture.consumer, &consumer_link).expect("consumer symlink");
        assert!(ConsumerRoot::open(&consumer_link).is_err());

        let source_link = fixture._root.path().join("source-link");
        symlink(&fixture.source, &source_link).expect("source symlink");
        assert!(verify_source_root(&source_link, &fixture.source_lock).is_err());
    }

    #[test]
    fn checked_in_group_plan_is_explicit_and_active_only() {
        let plan = group_plan(
            &crate::workspace_root(),
            "public_native",
            Operation::Check,
            false,
        )
        .expect("public native plan");
        assert!(plan.iter().any(|arg| arg == "radroots_core"));
        assert!(plan.iter().any(|arg| arg == "radroots_sdk"));
        assert!(!plan.iter().any(|arg| arg == "--workspace"));
        assert!(group_plan(&crate::workspace_root(), "missing", Operation::Check, false).is_err());
    }

    fn make_writable(root: &Path) {
        if let Ok(paths) = walk(root) {
            for path in paths {
                let _ = make_path_writable(&path);
            }
        }
    }

    fn make_path_writable(path: &Path) -> Result<(), std::io::Error> {
        let mut permissions = fs::metadata(path)?.permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            permissions.set_mode(permissions.mode() | 0o200);
        }
        #[cfg(not(unix))]
        permissions.set_readonly(false);
        fs::set_permissions(path, permissions)
    }
}
