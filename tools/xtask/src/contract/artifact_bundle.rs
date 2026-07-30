use fs2::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, OpenOptions},
    io::{self, ErrorKind, Write},
    path::{Component, Path, PathBuf},
};
use tempfile::NamedTempFile;

const TRANSACTION_SCHEMA_VERSION: u32 = 1;
const TRANSACTION_DIRECTORY: &str = ".radroots-contract-artifact-transaction-v1";
const TRANSACTION_JOURNAL: &str = "journal.json";
const LOCK_DIRECTORY: &str = "radroots-xtask-contract-artifact-locks-v1";

pub(super) struct GeneratedArtifact {
    pub(super) relative: &'static str,
    pub(super) contents: Vec<u8>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ArtifactTransactionJournal {
    schema_version: u32,
    artifacts: Vec<ArtifactTransactionEntry>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ArtifactTransactionEntry {
    relative: String,
    original_byte_length: Option<u64>,
    original_sha256: Option<String>,
}

struct OriginalArtifact {
    path: PathBuf,
    relative: &'static str,
    contents: Option<Vec<u8>>,
    permissions: Option<fs::Permissions>,
}

struct PendingArtifact {
    original: OriginalArtifact,
    contents: Vec<u8>,
}

struct StagedArtifact {
    original: OriginalArtifact,
    temporary: NamedTempFile,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum SimulatedInterruption {
    AfterStaging,
    AfterCommits(usize),
}

pub(super) struct ArtifactBundleTransaction<'a> {
    workspace_root: &'a Path,
}

impl ArtifactBundleTransaction<'_> {
    pub(super) fn write(&self, artifacts: Vec<GeneratedArtifact>) -> Result<(), String> {
        write_artifact_bundle_impl(self.workspace_root, artifacts, None)
    }
}

pub(super) fn read_regular_file(workspace_root: &Path, relative: &str) -> Result<Vec<u8>, String> {
    let path = validate_workspace_path(workspace_root, relative, false)?;
    fs::read(path).map_err(|error| format!("read {relative}: {error}"))
}

pub(super) fn validate_canonical_json_artifact(relative: &str, bytes: &[u8]) -> Result<(), String> {
    if bytes.contains(&b'\r') {
        return Err(format!("{relative} must use LF line endings"));
    }
    if !bytes.ends_with(b"\n") || bytes.ends_with(b"\n\n") {
        return Err(format!("{relative} must end with exactly one LF"));
    }
    Ok(())
}

pub(super) fn validate_sha256_artifact(relative: &str, bytes: &[u8]) -> Result<(), String> {
    if bytes.len() != 65 || bytes[64] != b'\n' {
        return Err(format!(
            "{relative} must contain 64 lowercase hexadecimal bytes and one LF"
        ));
    }
    if !bytes[..64]
        .iter()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(format!(
            "{relative} must contain a lowercase SHA-256 digest"
        ));
    }
    Ok(())
}

pub(super) fn with_artifact_bundle_transaction<T>(
    workspace_root: &Path,
    operation: impl FnOnce(&ArtifactBundleTransaction<'_>) -> Result<T, String>,
) -> Result<T, String> {
    let lock = acquire_workspace_lock(workspace_root)?;
    let result = recover_pending_transaction(workspace_root)
        .and_then(|()| operation(&ArtifactBundleTransaction { workspace_root }));
    let unlock = FileExt::unlock(&lock).map_err(|error| {
        format!(
            "unlock generated artifact transaction for {}: {error}",
            workspace_root.display()
        )
    });
    match (result, unlock) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(error)) => Err(error),
        (Err(error), Err(unlock)) => Err(format!("{error}; {unlock}")),
    }
}

fn write_artifact_bundle_impl(
    workspace_root: &Path,
    artifacts: Vec<GeneratedArtifact>,
    simulated_interruption: Option<SimulatedInterruption>,
) -> Result<(), String> {
    let mut relative_paths = BTreeSet::new();
    for artifact in &artifacts {
        if artifact.relative == TRANSACTION_DIRECTORY
            || artifact
                .relative
                .starts_with(&format!("{TRANSACTION_DIRECTORY}/"))
        {
            return Err(format!(
                "generated artifact path conflicts with transaction authority: {}",
                artifact.relative
            ));
        }
        if !relative_paths.insert(artifact.relative) {
            return Err(format!(
                "generated artifact bundle contains duplicate path: {}",
                artifact.relative
            ));
        }
    }

    let mut pending = Vec::new();
    for artifact in artifacts {
        let path = validate_workspace_path(workspace_root, artifact.relative, true)?;
        let parent = path
            .parent()
            .ok_or_else(|| format!("artifact path has no parent: {}", artifact.relative))?;
        fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
        validate_workspace_path(workspace_root, artifact.relative, true)?;

        let (contents, permissions) = match fs::read(&path) {
            Ok(current) if current == artifact.contents => continue,
            Ok(current) => (
                Some(current),
                Some(
                    fs::metadata(&path)
                        .map_err(|error| format!("inspect {}: {error}", artifact.relative))?
                        .permissions(),
                ),
            ),
            Err(error) if error.kind() == ErrorKind::NotFound => (None, default_permissions()),
            Err(error) => return Err(format!("read {}: {error}", artifact.relative)),
        };

        pending.push(PendingArtifact {
            original: OriginalArtifact {
                path,
                relative: artifact.relative,
                contents,
                permissions,
            },
            contents: artifact.contents,
        });
    }

    if pending.is_empty() {
        return Ok(());
    }
    let transaction_directory = create_transaction_directory(workspace_root)?;
    let staged = match stage_artifacts(&transaction_directory, pending) {
        Ok(staged) => staged,
        Err(error) => {
            let cleanup = discard_transaction_directory(workspace_root);
            return Err(match cleanup {
                Ok(()) => error,
                Err(cleanup) => format!("{error}; discard unprepared transaction: {cleanup}"),
            });
        }
    };
    if simulated_interruption == Some(SimulatedInterruption::AfterStaging) {
        retain_staged_files(staged)?;
        return Err("simulated interruption after staging artifact transaction".to_owned());
    }
    if let Err(error) = prepare_transaction(workspace_root, &staged) {
        let cleanup = discard_transaction_directory(workspace_root);
        return Err(match cleanup {
            Ok(()) => error,
            Err(cleanup) => format!("{error}; discard unprepared transaction: {cleanup}"),
        });
    }

    let mut committed = Vec::new();
    if simulated_interruption == Some(SimulatedInterruption::AfterCommits(0)) {
        return Err("simulated interruption after preparing artifact transaction".to_owned());
    }
    for staged_artifact in staged {
        let StagedArtifact {
            original,
            temporary,
        } = staged_artifact;
        if let Err(error) = temporary.persist(&original.path) {
            return fail_and_rollback(
                workspace_root,
                &committed,
                format!("persist {}: {}", original.relative, error.error),
            );
        }
        let relative = original.relative;
        let path = original.path.clone();
        committed.push(original);
        if let Err(error) = sync_parent(&path) {
            return fail_and_rollback(
                workspace_root,
                &committed,
                format!("sync parent for {relative}: {error}"),
            );
        }
        if simulated_interruption == Some(SimulatedInterruption::AfterCommits(committed.len())) {
            return Err(format!(
                "simulated interruption after committing {} artifact(s)",
                committed.len()
            ));
        }
    }
    finish_transaction(workspace_root)
}

fn create_transaction_directory(workspace_root: &Path) -> Result<PathBuf, String> {
    let transaction_directory = transaction_directory(workspace_root);
    fs::create_dir(&transaction_directory).map_err(|error| {
        format!(
            "create artifact transaction directory {}: {error}",
            transaction_directory.display()
        )
    })?;
    sync_directory(workspace_root).map_err(|error| {
        format!("sync workspace after preparing artifact transaction directory: {error}")
    })?;
    Ok(transaction_directory)
}

fn stage_artifacts(
    transaction_directory: &Path,
    pending: Vec<PendingArtifact>,
) -> Result<Vec<StagedArtifact>, String> {
    let mut staged = Vec::with_capacity(pending.len());
    for pending_artifact in pending {
        let PendingArtifact { original, contents } = pending_artifact;
        let mut temporary = NamedTempFile::new_in(transaction_directory)
            .map_err(|error| format!("stage {}: {error}", original.relative))?;
        temporary
            .write_all(&contents)
            .and_then(|_| temporary.flush())
            .map_err(|error| format!("stage {}: {error}", original.relative))?;
        if let Some(permissions) = original.permissions.as_ref() {
            fs::set_permissions(temporary.path(), permissions.clone())
                .map_err(|error| format!("set permissions for {}: {error}", original.relative))?;
        }
        temporary
            .as_file()
            .sync_all()
            .map_err(|error| format!("sync staged {}: {error}", original.relative))?;
        let staged_bytes = fs::read(temporary.path())
            .map_err(|error| format!("verify staged {}: {error}", original.relative))?;
        if staged_bytes != contents {
            return Err(format!(
                "staged bytes do not match generated artifact {}",
                original.relative
            ));
        }
        staged.push(StagedArtifact {
            original,
            temporary,
        });
    }
    sync_directory(transaction_directory).map_err(|error| {
        format!(
            "sync staged artifact transaction {}: {error}",
            transaction_directory.display()
        )
    })?;
    Ok(staged)
}

fn retain_staged_files(staged: Vec<StagedArtifact>) -> Result<(), String> {
    for staged_artifact in staged {
        let relative = staged_artifact.original.relative;
        staged_artifact.temporary.keep().map_err(|error| {
            format!(
                "retain simulated staged artifact {relative}: {}",
                error.error
            )
        })?;
    }
    Ok(())
}

fn acquire_workspace_lock(workspace_root: &Path) -> Result<fs::File, String> {
    let lock_path = workspace_lock_path(workspace_root)?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|error| {
            format!(
                "open generated artifact lock {}: {error}",
                lock_path.display()
            )
        })?;
    lock.lock_exclusive().map_err(|error| {
        format!(
            "lock generated artifact bundle {}: {error}",
            lock_path.display()
        )
    })?;
    Ok(lock)
}

fn workspace_lock_path(workspace_root: &Path) -> Result<PathBuf, String> {
    validate_workspace_root(workspace_root)?;
    let canonical_root = fs::canonicalize(workspace_root).map_err(|error| {
        format!(
            "canonicalize workspace root {}: {error}",
            workspace_root.display()
        )
    })?;
    let digest = Sha256::digest(canonical_root.as_os_str().as_encoded_bytes());
    let lock_directory = std::env::temp_dir().join(LOCK_DIRECTORY);
    fs::create_dir_all(&lock_directory).map_err(|error| {
        format!(
            "create lock directory {}: {error}",
            lock_directory.display()
        )
    })?;
    let metadata = lock_directory.symlink_metadata().map_err(|error| {
        format!(
            "inspect lock directory {}: {error}",
            lock_directory.display()
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!(
            "generated artifact lock path must be a non-symlink directory: {}",
            lock_directory.display()
        ));
    }
    let lock_path = lock_directory.join(format!("{}.lock", hex::encode(digest)));
    if matches!(
        lock_path.symlink_metadata(),
        Ok(metadata) if metadata.file_type().is_symlink()
    ) {
        return Err(format!(
            "generated artifact lock must not be a symlink: {}",
            lock_path.display()
        ));
    }
    Ok(lock_path)
}

fn prepare_transaction(workspace_root: &Path, staged: &[StagedArtifact]) -> Result<(), String> {
    let transaction_directory = transaction_directory(workspace_root);

    let mut artifacts = Vec::with_capacity(staged.len());
    for (index, staged_artifact) in staged.iter().enumerate() {
        let original = &staged_artifact.original;
        let (original_byte_length, original_sha256) =
            if let Some(contents) = original.contents.as_deref() {
                let backup_path = transaction_directory.join(backup_name(index));
                let mut backup = OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .open(&backup_path)
                    .map_err(|error| {
                        format!(
                            "create transaction backup {}: {error}",
                            backup_path.display()
                        )
                    })?;
                backup
                    .write_all(contents)
                    .and_then(|()| backup.flush())
                    .map_err(|error| {
                        format!(
                            "write transaction backup {}: {error}",
                            backup_path.display()
                        )
                    })?;
                if let Some(permissions) = original.permissions.as_ref() {
                    fs::set_permissions(&backup_path, permissions.clone()).map_err(|error| {
                        format!(
                            "set transaction backup permissions {}: {error}",
                            backup_path.display()
                        )
                    })?;
                }
                backup.sync_all().map_err(|error| {
                    format!("sync transaction backup {}: {error}", backup_path.display())
                })?;
                (
                    Some(u64::try_from(contents.len()).map_err(|_| {
                        format!("{} byte length does not fit in u64", original.relative)
                    })?),
                    Some(sha256_hex(contents)),
                )
            } else {
                (None, None)
            };
        artifacts.push(ArtifactTransactionEntry {
            relative: original.relative.to_owned(),
            original_byte_length,
            original_sha256,
        });
    }
    sync_directory(&transaction_directory).map_err(|error| {
        format!(
            "sync artifact transaction backups {}: {error}",
            transaction_directory.display()
        )
    })?;

    let journal = ArtifactTransactionJournal {
        schema_version: TRANSACTION_SCHEMA_VERSION,
        artifacts,
    };
    let mut journal_bytes = serde_json::to_vec_pretty(&journal)
        .map_err(|error| format!("serialize artifact transaction journal: {error}"))?;
    journal_bytes.push(b'\n');
    let journal_path = transaction_directory.join(TRANSACTION_JOURNAL);
    let mut temporary = NamedTempFile::new_in(&transaction_directory).map_err(|error| {
        format!(
            "stage artifact transaction journal {}: {error}",
            journal_path.display()
        )
    })?;
    temporary
        .write_all(&journal_bytes)
        .and_then(|()| temporary.flush())
        .and_then(|()| temporary.as_file().sync_all())
        .map_err(|error| {
            format!(
                "write artifact transaction journal {}: {error}",
                journal_path.display()
            )
        })?;
    temporary
        .persist_noclobber(&journal_path)
        .map_err(|error| {
            format!(
                "persist artifact transaction journal {}: {}",
                journal_path.display(),
                error.error
            )
        })?;
    sync_directory(&transaction_directory).map_err(|error| {
        format!(
            "sync prepared artifact transaction {}: {error}",
            transaction_directory.display()
        )
    })
}

fn recover_pending_transaction(workspace_root: &Path) -> Result<(), String> {
    let transaction_directory = transaction_directory(workspace_root);
    match transaction_directory.symlink_metadata() {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(format!(
                "artifact transaction path must be a non-symlink directory: {}",
                transaction_directory.display()
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(format!(
                "inspect artifact transaction path {}: {error}",
                transaction_directory.display()
            ));
        }
    }

    let journal_path = transaction_directory.join(TRANSACTION_JOURNAL);
    let journal_bytes = match fs::read(&journal_path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return discard_transaction_directory(workspace_root);
        }
        Err(error) => {
            return Err(format!(
                "read artifact transaction journal {}: {error}",
                journal_path.display()
            ));
        }
    };
    let journal: ArtifactTransactionJournal = serde_json::from_slice(&journal_bytes)
        .map_err(|error| format!("parse artifact transaction journal: {error}"))?;
    validate_transaction_journal(workspace_root, &journal)?;

    let mut failures = Vec::new();
    for (index, artifact) in journal.artifacts.iter().enumerate().rev() {
        let path = match validate_workspace_path(workspace_root, &artifact.relative, true) {
            Ok(path) => path,
            Err(error) => {
                failures.push(error);
                continue;
            }
        };
        let result = match (
            artifact.original_byte_length,
            artifact.original_sha256.as_deref(),
        ) {
            (Some(expected_length), Some(expected_sha256)) => {
                let backup_path = transaction_directory.join(backup_name(index));
                restore_transaction_backup(&path, &backup_path, expected_length, expected_sha256)
            }
            (None, None) => match fs::remove_file(&path) {
                Ok(()) => sync_parent(&path),
                Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error),
            },
            _ => unreachable!("validated transaction journal backup shape"),
        };
        if let Err(error) = result {
            failures.push(format!("{}: {error}", artifact.relative));
        }
    }
    if !failures.is_empty() {
        return Err(format!(
            "recover prepared artifact transaction: {}",
            failures.join(", ")
        ));
    }
    finish_transaction(workspace_root)
}

fn validate_transaction_journal(
    workspace_root: &Path,
    journal: &ArtifactTransactionJournal,
) -> Result<(), String> {
    if journal.schema_version != TRANSACTION_SCHEMA_VERSION {
        return Err(format!(
            "artifact transaction schema_version must be {TRANSACTION_SCHEMA_VERSION}"
        ));
    }
    if journal.artifacts.is_empty() {
        return Err("artifact transaction must contain at least one artifact".to_owned());
    }
    let mut paths = BTreeSet::new();
    for artifact in &journal.artifacts {
        validate_workspace_path(workspace_root, &artifact.relative, true)?;
        if artifact.relative == TRANSACTION_DIRECTORY
            || artifact
                .relative
                .starts_with(&format!("{TRANSACTION_DIRECTORY}/"))
        {
            return Err(format!(
                "artifact transaction path conflicts with transaction authority: {}",
                artifact.relative
            ));
        }
        if !paths.insert(artifact.relative.as_str()) {
            return Err(format!(
                "artifact transaction contains duplicate path: {}",
                artifact.relative
            ));
        }
        match (
            artifact.original_byte_length,
            artifact.original_sha256.as_deref(),
        ) {
            (Some(_), Some(digest)) => validate_sha256(digest)?,
            (None, None) => {}
            _ => {
                return Err(format!(
                    "artifact transaction backup metadata is incomplete for {}",
                    artifact.relative
                ));
            }
        }
    }
    Ok(())
}

fn restore_transaction_backup(
    artifact_path: &Path,
    backup_path: &Path,
    expected_length: u64,
    expected_sha256: &str,
) -> io::Result<()> {
    let metadata = backup_path.symlink_metadata()?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(io::Error::other(format!(
            "transaction backup must be a regular non-symlink file: {}",
            backup_path.display()
        )));
    }
    let contents = fs::read(backup_path)?;
    let actual_length = u64::try_from(contents.len())
        .map_err(|_| io::Error::other("transaction backup length does not fit in u64"))?;
    if actual_length != expected_length || sha256_hex(&contents) != expected_sha256 {
        return Err(io::Error::other(format!(
            "transaction backup integrity mismatch: {}",
            backup_path.display()
        )));
    }
    let permissions = metadata.permissions();
    let staging_directory = backup_path
        .parent()
        .ok_or_else(|| io::Error::other("transaction backup path has no parent"))?;
    restore_bytes(
        artifact_path,
        &contents,
        Some(&permissions),
        staging_directory,
    )
}

fn fail_and_rollback(
    workspace_root: &Path,
    committed: &[OriginalArtifact],
    failure: String,
) -> Result<(), String> {
    match rollback_artifacts(workspace_root, committed) {
        Ok(()) => match finish_transaction(workspace_root) {
            Ok(()) => Err(format!("{failure}; restored committed artifacts")),
            Err(cleanup) => Err(format!(
                "{failure}; restored committed artifacts; transaction cleanup failed: {cleanup}"
            )),
        },
        Err(rollback) => Err(format!(
            "{failure}; rollback also failed: {rollback}; prepared transaction retained for recovery"
        )),
    }
}

fn finish_transaction(workspace_root: &Path) -> Result<(), String> {
    let transaction_directory = transaction_directory(workspace_root);
    let journal_path = transaction_directory.join(TRANSACTION_JOURNAL);
    fs::remove_file(&journal_path).map_err(|error| {
        format!(
            "remove artifact transaction journal {}: {error}",
            journal_path.display()
        )
    })?;
    sync_directory(&transaction_directory).map_err(|error| {
        format!(
            "sync completed artifact transaction {}: {error}",
            transaction_directory.display()
        )
    })?;
    discard_transaction_directory(workspace_root)
}

fn discard_transaction_directory(workspace_root: &Path) -> Result<(), String> {
    let transaction_directory = transaction_directory(workspace_root);
    match fs::remove_dir_all(&transaction_directory) {
        Ok(()) => sync_directory(workspace_root).map_err(|error| {
            format!(
                "sync workspace after removing artifact transaction {}: {error}",
                transaction_directory.display()
            )
        }),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "remove artifact transaction directory {}: {error}",
            transaction_directory.display()
        )),
    }
}

fn transaction_directory(workspace_root: &Path) -> PathBuf {
    workspace_root.join(TRANSACTION_DIRECTORY)
}

fn backup_name(index: usize) -> String {
    format!("{index:08}.backup")
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn validate_sha256(digest: &str) -> Result<(), String> {
    if digest.len() != 64
        || !digest
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(
            "artifact transaction digest must be 64 lowercase hexadecimal bytes".to_owned(),
        );
    }
    Ok(())
}

pub(super) fn validate_workspace_path(
    workspace_root: &Path,
    relative: &str,
    allow_missing: bool,
) -> Result<PathBuf, String> {
    let relative_path = Path::new(relative);
    if relative.is_empty()
        || relative.contains('\\')
        || relative_path.is_absolute()
        || !relative_path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(format!(
            "artifact path must be normalized and workspace-relative: {relative}"
        ));
    }

    validate_workspace_root(workspace_root)?;

    let component_count = relative_path.components().count();
    let mut current = workspace_root.to_path_buf();
    for (index, component) in relative_path.components().enumerate() {
        let Component::Normal(segment) = component else {
            return Err(format!("artifact path is not normalized: {relative}"));
        };
        current.push(segment);
        match current.symlink_metadata() {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(format!(
                    "artifact path contains a symlink component: {}",
                    current.display()
                ));
            }
            Ok(metadata) if index + 1 == component_count && !metadata.is_file() => {
                return Err(format!("artifact must be a regular file: {relative}"));
            }
            Ok(metadata) if index + 1 < component_count && !metadata.is_dir() => {
                return Err(format!(
                    "artifact parent must be a directory: {}",
                    current.display()
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::NotFound && allow_missing => break,
            Err(error) => {
                return Err(format!(
                    "inspect artifact path {}: {error}",
                    current.display()
                ));
            }
        }
    }
    Ok(workspace_root.join(relative_path))
}

fn validate_workspace_root(workspace_root: &Path) -> Result<(), String> {
    let root_metadata = workspace_root.symlink_metadata().map_err(|error| {
        format!(
            "inspect workspace root {}: {error}",
            workspace_root.display()
        )
    })?;
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return Err(format!(
            "workspace root must be a non-symlink directory: {}",
            workspace_root.display()
        ));
    }
    Ok(())
}

fn rollback_artifacts(workspace_root: &Path, committed: &[OriginalArtifact]) -> Result<(), String> {
    let mut failures = Vec::new();
    let staging_directory = transaction_directory(workspace_root);
    for original in committed.iter().rev() {
        let result = if let Some(contents) = original.contents.as_ref() {
            restore_file(original, contents, &staging_directory)
        } else {
            match fs::remove_file(&original.path) {
                Ok(()) => sync_parent(&original.path),
                Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error),
            }
        };
        if let Err(error) = result {
            failures.push(format!("{}: {error}", original.relative));
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join(", "))
    }
}

fn restore_file(
    original: &OriginalArtifact,
    contents: &[u8],
    staging_directory: &Path,
) -> io::Result<()> {
    restore_bytes(
        &original.path,
        contents,
        original.permissions.as_ref(),
        staging_directory,
    )
}

fn restore_bytes(
    path: &Path,
    contents: &[u8],
    permissions: Option<&fs::Permissions>,
    staging_directory: &Path,
) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("artifact path has no parent"))?;
    fs::create_dir_all(parent)?;
    let mut temporary = NamedTempFile::new_in(staging_directory)?;
    temporary.write_all(contents)?;
    temporary.flush()?;
    if let Some(permissions) = permissions {
        fs::set_permissions(temporary.path(), permissions.clone())?;
    }
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|error| error.error)?;
    sync_parent(path)
}

fn sync_parent(path: &Path) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("artifact path has no parent"))?;
    sync_directory(parent)
}

fn sync_directory(path: &Path) -> io::Result<()> {
    fs::File::open(path)?.sync_all()
}

#[cfg(unix)]
fn default_permissions() -> Option<fs::Permissions> {
    use std::os::unix::fs::PermissionsExt;
    Some(fs::Permissions::from_mode(0o644))
}

#[cfg(not(unix))]
fn default_permissions() -> Option<fs::Permissions> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_rejects_duplicate_paths_before_writing() {
        let workspace = tempfile::TempDir::new().expect("workspace");
        let error = with_artifact_bundle_transaction(workspace.path(), |transaction| {
            transaction.write(vec![
                GeneratedArtifact {
                    relative: "generated/value.txt",
                    contents: b"first\n".to_vec(),
                },
                GeneratedArtifact {
                    relative: "generated/value.txt",
                    contents: b"second\n".to_vec(),
                },
            ])
        })
        .expect_err("duplicate artifact paths must fail");
        assert!(error.contains("duplicate path"));
        assert!(!workspace.path().join("generated/value.txt").exists());
    }

    #[test]
    fn workspace_lock_excludes_a_second_file_descriptor() {
        let workspace = tempfile::TempDir::new().expect("workspace");
        let first = acquire_workspace_lock(workspace.path()).expect("first workspace lock");
        let lock_path = workspace_lock_path(workspace.path()).expect("workspace lock path");
        let second = OpenOptions::new()
            .read(true)
            .write(true)
            .open(lock_path)
            .expect("second lock descriptor");

        assert!(
            second.try_lock_exclusive().is_err(),
            "the second descriptor must observe the held advisory lock"
        );
        FileExt::unlock(&first).expect("release first lock");
        second
            .try_lock_exclusive()
            .expect("second descriptor acquires released lock");
        FileExt::unlock(&second).expect("release second lock");
    }

    #[test]
    fn next_transaction_recovers_an_interrupted_multi_file_commit() {
        let workspace = tempfile::TempDir::new().expect("workspace");
        fs::create_dir_all(workspace.path().join("generated")).expect("artifact directory");
        fs::write(workspace.path().join("generated/first.txt"), b"first-old\n")
            .expect("first original");
        fs::write(
            workspace.path().join("generated/second.txt"),
            b"second-old\n",
        )
        .expect("second original");

        let interrupted = with_artifact_bundle_transaction(workspace.path(), |_| {
            write_artifact_bundle_impl(
                workspace.path(),
                vec![
                    GeneratedArtifact {
                        relative: "generated/first.txt",
                        contents: b"first-interrupted\n".to_vec(),
                    },
                    GeneratedArtifact {
                        relative: "generated/second.txt",
                        contents: b"second-interrupted\n".to_vec(),
                    },
                ],
                Some(SimulatedInterruption::AfterCommits(1)),
            )
        })
        .expect_err("simulated process interruption");
        assert!(interrupted.contains("simulated interruption"));
        assert_eq!(
            fs::read(workspace.path().join("generated/first.txt")).expect("mixed first"),
            b"first-interrupted\n"
        );
        assert_eq!(
            fs::read(workspace.path().join("generated/second.txt")).expect("mixed second"),
            b"second-old\n"
        );
        assert!(
            transaction_directory(workspace.path())
                .join(TRANSACTION_JOURNAL)
                .is_file(),
            "prepared journal must survive an interrupted commit"
        );

        with_artifact_bundle_transaction(workspace.path(), |transaction| {
            assert_eq!(
                fs::read(workspace.path().join("generated/first.txt")).expect("recovered first"),
                b"first-old\n"
            );
            assert_eq!(
                fs::read(workspace.path().join("generated/second.txt")).expect("recovered second"),
                b"second-old\n"
            );
            transaction.write(vec![
                GeneratedArtifact {
                    relative: "generated/first.txt",
                    contents: b"first-final\n".to_vec(),
                },
                GeneratedArtifact {
                    relative: "generated/second.txt",
                    contents: b"second-final\n".to_vec(),
                },
            ])
        })
        .expect("recover and replace bundle");

        assert_eq!(
            fs::read(workspace.path().join("generated/first.txt")).expect("final first"),
            b"first-final\n"
        );
        assert_eq!(
            fs::read(workspace.path().join("generated/second.txt")).expect("final second"),
            b"second-final\n"
        );
        assert!(!transaction_directory(workspace.path()).exists());
    }

    #[test]
    fn unjournaled_stages_are_recovered_without_target_directory_residue() {
        let workspace = tempfile::TempDir::new().expect("workspace");
        let generated = workspace.path().join("generated");
        fs::create_dir_all(&generated).expect("artifact directory");
        let artifact = generated.join("value.txt");
        fs::write(&artifact, b"old\n").expect("original artifact");

        with_artifact_bundle_transaction(workspace.path(), |_| {
            write_artifact_bundle_impl(
                workspace.path(),
                vec![GeneratedArtifact {
                    relative: "generated/value.txt",
                    contents: b"interrupted\n".to_vec(),
                }],
                Some(SimulatedInterruption::AfterStaging),
            )
        })
        .expect_err("simulated pre-journal interruption");

        assert_eq!(fs::read(&artifact).expect("unchanged target"), b"old\n");
        assert!(
            transaction_directory(workspace.path()).is_dir(),
            "simulated abrupt death must retain a recoverable transaction authority"
        );
        assert!(
            !transaction_directory(workspace.path())
                .join(TRANSACTION_JOURNAL)
                .exists(),
            "the simulated interruption must precede the durable journal"
        );
        let target_entries = fs::read_dir(&generated)
            .expect("target directory")
            .map(|entry| entry.expect("target entry").file_name())
            .collect::<Vec<_>>();
        assert_eq!(
            target_entries,
            vec![std::ffi::OsString::from("value.txt")],
            "staging must not leave temporary files beside governed targets"
        );
        assert!(
            fs::read_dir(transaction_directory(workspace.path()))
                .expect("transaction stages")
                .next()
                .is_some(),
            "the test interruption must retain at least one staged file"
        );

        with_artifact_bundle_transaction(workspace.path(), |_| Ok(()))
            .expect("next locked operation recovers pre-journal stages");
        assert_eq!(fs::read(&artifact).expect("recovered target"), b"old\n");
        assert!(
            !transaction_directory(workspace.path()).exists(),
            "recovery must remove every transaction-owned stage"
        );
    }

    #[test]
    fn recovery_fails_closed_and_retains_a_corrupt_prepared_transaction() {
        let workspace = tempfile::TempDir::new().expect("workspace");
        fs::create_dir_all(workspace.path().join("generated")).expect("artifact directory");
        fs::write(workspace.path().join("generated/first.txt"), b"first-old\n")
            .expect("first original");
        fs::write(
            workspace.path().join("generated/second.txt"),
            b"second-old\n",
        )
        .expect("second original");

        with_artifact_bundle_transaction(workspace.path(), |_| {
            write_artifact_bundle_impl(
                workspace.path(),
                vec![
                    GeneratedArtifact {
                        relative: "generated/first.txt",
                        contents: b"first-new\n".to_vec(),
                    },
                    GeneratedArtifact {
                        relative: "generated/second.txt",
                        contents: b"second-new\n".to_vec(),
                    },
                ],
                Some(SimulatedInterruption::AfterCommits(1)),
            )
        })
        .expect_err("simulated process interruption");
        fs::write(
            transaction_directory(workspace.path()).join(backup_name(0)),
            b"corrupt\n",
        )
        .expect("corrupt transaction backup");

        let error = with_artifact_bundle_transaction(workspace.path(), |_| Ok(()))
            .expect_err("corrupt backup must prevent validation or writing");
        assert!(error.contains("integrity mismatch"));
        assert!(
            transaction_directory(workspace.path())
                .join(TRANSACTION_JOURNAL)
                .is_file(),
            "failed recovery must retain its durable journal"
        );
    }
}
