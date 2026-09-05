//! Descriptor-bound, resource-bounded input handling for release tooling.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs::{self, File};
use std::io::{self, BufRead as _, BufReader, Read, Write};
use std::path::{Component, Path, PathBuf};

use flate2::{Compression, GzBuilder, bufread::GzDecoder};
use sha2::{Digest as _, Sha256};
use tar::{Builder as TarBuilder, Header as TarHeader};

const STREAM_BUFFER_BYTES: usize = 64 * 1024;
const HARD_MAX_BUFFERED_READ_BYTES: u64 = 67_108_864;
const HARD_MAX_STREAM_FILE_BYTES: u64 = 17_179_869_184;
const HARD_MAX_TRAVERSAL_ENTRIES: u64 = 65_536;
const HARD_MAX_TRAVERSAL_FILES: u64 = 65_536;
const HARD_MAX_TRAVERSAL_TOTAL_BYTES: u64 = 68_719_476_736;
const HARD_MAX_TRAVERSAL_DEPTH: usize = 64;
const HARD_MAX_PATH_BYTES: usize = 4_096;
const HARD_MAX_ARCHIVE_COMPRESSED_BYTES: u64 = 2_147_483_648;
const HARD_MAX_ARCHIVE_EXPANDED_BYTES: u64 = 17_179_869_184;
const HARD_MAX_ARCHIVE_MEMBERS: u64 = 65_536;
const HARD_MAX_ARCHIVE_MEMBER_BYTES: u64 = 17_179_869_184;
const HARD_MAX_ARCHIVE_PAYLOAD_BYTES: u64 = 17_179_869_184;
const TAR_BLOCK_BYTES: usize = 512;
const TAR_TERMINATOR_BYTES: usize = TAR_BLOCK_BYTES * 2;
const CANONICAL_GZIP_COMPRESSION_LEVEL: u32 = 6;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LimitKind {
    ArchiveCompressedBytes,
    ArchiveDepth,
    ArchiveExpandedBytes,
    ArchiveMemberBytes,
    ArchiveMembers,
    ArchivePathBytes,
    ArchivePayloadBytes,
    FileBytes,
    TraversalDepth,
    TraversalEntries,
    TraversalFiles,
    TraversalPathBytes,
    TraversalTotalBytes,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ArtifactIoFailureKind {
    ChangedDuringRead,
    InvalidObject,
    InvalidRequest,
    IoFailure,
    LimitExceeded(LimitKind),
    MalformedArchive,
    #[cfg_attr(unix, allow(dead_code))]
    UnsupportedPlatform,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ArtifactIoError {
    kind: ArtifactIoFailureKind,
}

impl ArtifactIoError {
    const fn new(kind: ArtifactIoFailureKind) -> Self {
        Self { kind }
    }

    pub(crate) const fn kind(self) -> ArtifactIoFailureKind {
        self.kind
    }
}

impl fmt::Display for ArtifactIoError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.kind {
            ArtifactIoFailureKind::ChangedDuringRead => "artifact binding changed during admission",
            ArtifactIoFailureKind::InvalidObject => "artifact object type is not admitted",
            ArtifactIoFailureKind::InvalidRequest => "artifact I/O request is invalid",
            ArtifactIoFailureKind::IoFailure => "artifact I/O operation failed",
            ArtifactIoFailureKind::LimitExceeded(_) => "artifact I/O limit was exceeded",
            ArtifactIoFailureKind::MalformedArchive => "artifact archive is malformed",
            ArtifactIoFailureKind::UnsupportedPlatform => {
                "safe artifact I/O is unsupported on this platform"
            }
        })
    }
}

impl std::error::Error for ArtifactIoError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TraversalLimits {
    pub(crate) max_entries: u64,
    pub(crate) max_files: u64,
    pub(crate) max_total_bytes: u64,
    pub(crate) max_file_bytes: u64,
    pub(crate) max_depth: usize,
    pub(crate) max_path_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TarGzipLimits {
    pub(crate) max_compressed_bytes: u64,
    pub(crate) max_expanded_bytes: u64,
    pub(crate) max_members: u64,
    pub(crate) max_member_bytes: u64,
    pub(crate) max_payload_bytes: u64,
    pub(crate) max_depth: usize,
    pub(crate) max_path_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FileEvidence {
    pub(crate) byte_length: u64,
    pub(crate) sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ArchiveEvidence {
    pub(crate) compressed: FileEvidence,
    pub(crate) expanded_bytes: u64,
    pub(crate) member_count: u64,
    pub(crate) payload_bytes: u64,
}

#[derive(Debug)]
pub(crate) struct MaterializedArchive {
    root_descriptor: File,
    root_identity: OutputDirectoryIdentity,
    root_chain: Vec<DirectoryIdentity>,
    parent_descriptor: File,
    parent_identity: OutputDirectoryIdentity,
    parent_path: PathBuf,
    parent_chain: Vec<OutputDirectoryIdentity>,
    snapshot: TraversalSnapshot,
    evidence: ArchiveEvidence,
    directory: tempfile::TempDir,
}

impl MaterializedArchive {
    pub(crate) fn root(&self) -> &Path {
        self.directory.path()
    }

    pub(crate) fn evidence(&self) -> &ArchiveEvidence {
        &self.evidence
    }

    pub(crate) fn snapshot(&self) -> &TraversalSnapshot {
        &self.snapshot
    }

    pub(crate) fn revalidate(&self) -> Result<(), ArtifactIoError> {
        let changed = || ArtifactIoError::new(ArtifactIoFailureKind::ChangedDuringRead);
        if output_directory_identity(identity(&self.parent_descriptor).map_err(|_| changed())?)
            != self.parent_identity
        {
            return Err(ArtifactIoError::new(
                ArtifactIoFailureKind::ChangedDuringRead,
            ));
        }
        if output_directory_identity(identity(&self.root_descriptor).map_err(|_| changed())?)
            != self.root_identity
        {
            return Err(ArtifactIoError::new(
                ArtifactIoFailureKind::ChangedDuringRead,
            ));
        }
        let (current_root, current_root_chain) =
            open_absolute_directory(self.root()).map_err(|_| changed())?;
        if output_directory_identity(identity(&current_root).map_err(|_| changed())?)
            != self.root_identity
            || current_root_chain != self.root_chain
        {
            return Err(ArtifactIoError::new(
                ArtifactIoFailureKind::ChangedDuringRead,
            ));
        }
        let (_, current_parent_chain) =
            open_trusted_output_directory(&self.parent_path).map_err(|_| changed())?;
        if current_parent_chain != self.parent_chain {
            return Err(ArtifactIoError::new(
                ArtifactIoFailureKind::ChangedDuringRead,
            ));
        }
        self.snapshot.revalidate().map_err(|_| changed())
    }
}

#[derive(Clone, Debug)]
pub(crate) struct TraversedFile {
    relative: PathBuf,
    identity: FileIdentity,
}

impl TraversedFile {
    pub(crate) fn relative_path(&self) -> &Path {
        &self.relative
    }

    pub(crate) const fn permission_mode(&self) -> u32 {
        permission_mode(self.identity)
    }
}

#[derive(Debug)]
pub(crate) struct TraversalSnapshot {
    root: PathBuf,
    root_chain: Vec<DirectoryIdentity>,
    directories: Vec<TraversedDirectory>,
    files: Vec<TraversedFile>,
    entry_count: u64,
    total_bytes: u64,
}

impl TraversalSnapshot {
    pub(crate) fn files(&self) -> &[TraversedFile] {
        &self.files
    }

    pub(crate) const fn entry_count(&self) -> u64 {
        self.entry_count
    }

    pub(crate) const fn total_bytes(&self) -> u64 {
        self.total_bytes
    }

    pub(crate) fn directories(&self) -> impl Iterator<Item = (&Path, u32)> {
        self.directories
            .iter()
            .map(|directory| (directory.relative.as_path(), directory.permission_mode))
    }

    pub(crate) fn root_permission_mode(&self) -> u32 {
        self.directories
            .iter()
            .find(|directory| directory.relative.as_os_str().is_empty())
            .map_or(0, |directory| directory.permission_mode)
    }

    pub(crate) fn read(
        &self,
        file: &TraversedFile,
        maximum: u64,
    ) -> Result<Vec<u8>, ArtifactIoError> {
        read_regular_impl(
            &self.root,
            &file.relative,
            maximum,
            Some(&file.identity),
            || {},
        )
    }

    pub(crate) fn read_evidenced(
        &self,
        file: &TraversedFile,
        maximum: u64,
    ) -> Result<(Vec<u8>, FileEvidence), ArtifactIoError> {
        read_regular_evidenced_impl(
            &self.root,
            &file.relative,
            maximum,
            Some(&file.identity),
            || {},
        )
    }

    pub(crate) fn hash(
        &self,
        file: &TraversedFile,
        maximum: u64,
    ) -> Result<FileEvidence, ArtifactIoError> {
        with_regular(
            &self.root,
            &file.relative,
            maximum,
            Some(&file.identity),
            || {},
            |input, admitted_length| {
                let evidence = stream_regular(input, maximum, None)?;
                require_observed_length(evidence.byte_length, admitted_length)?;
                Ok(evidence)
            },
        )
    }

    pub(crate) fn admit_deterministic_tar_gzip(
        &self,
        file: &TraversedFile,
        limits: TarGzipLimits,
    ) -> Result<ArchiveEvidence, ArtifactIoError> {
        admit_tar_gzip_relative(
            &self.root,
            &file.relative,
            limits,
            Some(&file.identity),
            TarGzipPolicy::DeterministicSnapshot,
        )
    }

    pub(crate) fn materialize_deterministic_tar_gzip(
        &self,
        file: &TraversedFile,
        trusted_parent: &Path,
        limits: TarGzipLimits,
    ) -> Result<MaterializedArchive, ArtifactIoError> {
        materialize_tar_gzip_relative(
            &self.root,
            &file.relative,
            trusted_parent,
            limits,
            Some(&file.identity),
        )
    }

    pub(crate) fn copy_to_new_path(
        &self,
        file: &TraversedFile,
        output: &Path,
        maximum: u64,
    ) -> Result<FileEvidence, ArtifactIoError> {
        copy_regular_to_new_path_impl(
            &self.root.join(&file.relative),
            output,
            maximum,
            Some(&file.identity),
            || {},
            || {},
        )
    }

    pub(crate) fn revalidate(&self) -> Result<(), ArtifactIoError> {
        let (_, root_chain) = open_absolute_directory(&self.root)?;
        if root_chain != self.root_chain {
            return Err(ArtifactIoError::new(
                ArtifactIoFailureKind::ChangedDuringRead,
            ));
        }
        for directory in &self.directories {
            let opened = if directory.relative.as_os_str().is_empty() {
                let (file, chain) = open_absolute_directory(&self.root)?;
                OpenedObject {
                    identity: identity(&file)?,
                    file,
                    chain,
                }
            } else {
                open_relative(&self.root, &directory.relative, ObjectKind::Directory)?
            };
            if directory_identity(opened.identity) != directory.identity
                || permission_mode(opened.identity) != directory.permission_mode
            {
                return Err(ArtifactIoError::new(
                    ArtifactIoFailureKind::ChangedDuringRead,
                ));
            }
            revalidate_directory_members(&opened.file, &directory.members)?;
        }
        for file in &self.files {
            let opened = open_relative(&self.root, &file.relative, ObjectKind::Regular)?;
            if opened.identity != file.identity {
                return Err(ArtifactIoError::new(
                    ArtifactIoFailureKind::ChangedDuringRead,
                ));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileIdentity {
    device: u64,
    inode: u64,
    mode: u32,
    owner: u32,
    links: u64,
    length: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DirectoryIdentity {
    device: u64,
    inode: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OutputDirectoryIdentity {
    directory: DirectoryIdentity,
    mode: u32,
    owner: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TraversedDirectory {
    relative: PathBuf,
    identity: DirectoryIdentity,
    permission_mode: u32,
    members: Vec<DirectoryMember>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DirectoryMember {
    name: OsString,
    identity: MemberIdentity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MemberIdentity {
    Directory(DirectoryIdentity),
    Regular(FileIdentity),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ObjectKind {
    Directory,
    Regular,
}

struct OpenedObject {
    file: File,
    identity: FileIdentity,
    chain: Vec<DirectoryIdentity>,
}

pub(crate) fn read_regular_path(path: &Path, maximum: u64) -> Result<Vec<u8>, ArtifactIoError> {
    let (root, relative) = split_absolute_file(path)?;
    read_regular(&root, &relative, maximum)
}

pub(crate) fn read_regular(
    root: &Path,
    relative: &Path,
    maximum: u64,
) -> Result<Vec<u8>, ArtifactIoError> {
    read_regular_impl(root, relative, maximum, None, || {})
}

fn read_regular_impl<F>(
    root: &Path,
    relative: &Path,
    maximum: u64,
    expected: Option<&FileIdentity>,
    after_open: F,
) -> Result<Vec<u8>, ArtifactIoError>
where
    F: FnOnce(),
{
    read_regular_evidenced_impl(root, relative, maximum, expected, after_open)
        .map(|(bytes, _)| bytes)
}

fn read_regular_evidenced_impl<F>(
    root: &Path,
    relative: &Path,
    maximum: u64,
    expected: Option<&FileIdentity>,
    after_open: F,
) -> Result<(Vec<u8>, FileEvidence), ArtifactIoError>
where
    F: FnOnce(),
{
    if maximum == 0 || maximum > HARD_MAX_BUFFERED_READ_BYTES {
        return Err(ArtifactIoError::new(ArtifactIoFailureKind::InvalidRequest));
    }
    with_regular(
        root,
        relative,
        maximum,
        expected,
        after_open,
        |file, admitted_length| {
            let initial_capacity = usize::try_from(maximum.min(STREAM_BUFFER_BYTES as u64))
                .map_err(|_| ArtifactIoError::new(ArtifactIoFailureKind::InvalidRequest))?;
            let mut bytes = Vec::with_capacity(initial_capacity);
            let mut hasher = Sha256::new();
            let mut buffer = [0_u8; STREAM_BUFFER_BYTES];
            loop {
                let read = file
                    .read(&mut buffer)
                    .map_err(|_| ArtifactIoError::new(ArtifactIoFailureKind::IoFailure))?;
                if read == 0 {
                    break;
                }
                let next = (bytes.len() as u64)
                    .checked_add(read as u64)
                    .ok_or_else(|| {
                        ArtifactIoError::new(ArtifactIoFailureKind::LimitExceeded(
                            LimitKind::FileBytes,
                        ))
                    })?;
                if next > maximum {
                    return Err(ArtifactIoError::new(ArtifactIoFailureKind::LimitExceeded(
                        LimitKind::FileBytes,
                    )));
                }
                hasher.update(&buffer[..read]);
                bytes.extend_from_slice(&buffer[..read]);
            }
            let byte_length = bytes.len() as u64;
            require_observed_length(byte_length, admitted_length)?;
            Ok((
                bytes,
                FileEvidence {
                    byte_length,
                    sha256: hex::encode(hasher.finalize()),
                },
            ))
        },
    )
}

pub(crate) fn hash_regular_path(
    path: &Path,
    maximum: u64,
) -> Result<FileEvidence, ArtifactIoError> {
    let (root, relative) = split_absolute_file(path)?;
    hash_regular(&root, &relative, maximum)
}

pub(crate) fn hash_regular(
    root: &Path,
    relative: &Path,
    maximum: u64,
) -> Result<FileEvidence, ArtifactIoError> {
    with_regular(
        root,
        relative,
        maximum,
        None,
        || {},
        |file, admitted_length| {
            let evidence = stream_regular(file, maximum, None)?;
            require_observed_length(evidence.byte_length, admitted_length)?;
            Ok(evidence)
        },
    )
}

pub(crate) fn copy_regular_to_new_path(
    source: &Path,
    output: &Path,
    maximum: u64,
) -> Result<FileEvidence, ArtifactIoError> {
    copy_regular_to_new_path_impl(source, output, maximum, None, || {}, || {})
}

fn copy_regular_to_new_path_impl<H, J>(
    source: &Path,
    output: &Path,
    maximum: u64,
    expected: Option<&FileIdentity>,
    after_open: H,
    after_install: J,
) -> Result<FileEvidence, ArtifactIoError>
where
    H: FnOnce(),
    J: FnOnce(),
{
    if !output.is_absolute() || output.file_name().is_none() {
        return Err(ArtifactIoError::new(ArtifactIoFailureKind::InvalidRequest));
    }
    let output_parent = output
        .parent()
        .ok_or_else(|| ArtifactIoError::new(ArtifactIoFailureKind::InvalidRequest))?;
    let (root, relative) = split_absolute_file(source)?;
    copy_regular_descriptor_relative(
        &root,
        &relative,
        output_parent,
        output
            .file_name()
            .ok_or_else(|| ArtifactIoError::new(ArtifactIoFailureKind::InvalidRequest))?,
        maximum,
        expected,
        after_open,
        after_install,
    )
}

#[cfg(unix)]
#[allow(clippy::too_many_arguments)]
fn copy_regular_descriptor_relative<H, J>(
    input_root: &Path,
    input_relative: &Path,
    output_parent: &Path,
    output_name: &OsStr,
    maximum: u64,
    expected: Option<&FileIdentity>,
    after_open: H,
    after_install: J,
) -> Result<FileEvidence, ArtifactIoError>
where
    H: FnOnce(),
    J: FnOnce(),
{
    use rustix::fs::{Mode, OFlags, fchmod, openat};

    validate_single_component(output_name)?;
    let (parent, parent_chain) = open_trusted_output_directory(output_parent)?;
    let parent_identity = *parent_chain
        .last()
        .ok_or_else(|| ArtifactIoError::new(ArtifactIoFailureKind::InvalidRequest))?;
    let output = openat(
        &parent,
        output_name,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::RUSR | Mode::WUSR,
    )
    .map_err(|_| ArtifactIoError::new(ArtifactIoFailureKind::InvalidRequest))?;
    let mut output = File::from(output);
    fchmod(&output, Mode::RUSR | Mode::WUSR)
        .map_err(|_| ArtifactIoError::new(ArtifactIoFailureKind::IoFailure))?;
    let initial_output = identity(&output)?;
    if initial_output.links != 1 || permission_mode(initial_output) != 0o600 {
        return Err(ArtifactIoError::new(
            ArtifactIoFailureKind::ChangedDuringRead,
        ));
    }

    let evidence = with_regular(
        input_root,
        input_relative,
        maximum,
        expected,
        after_open,
        |input, admitted_length| {
            let evidence = stream_regular(input, maximum, Some(&mut output))?;
            require_observed_length(evidence.byte_length, admitted_length)?;
            output
                .sync_all()
                .map_err(|_| ArtifactIoError::new(ArtifactIoFailureKind::IoFailure))?;
            Ok(evidence)
        },
    )?;
    let finalized_identity = identity(&output)?;
    if finalized_identity.links != 1
        || finalized_identity.length != evidence.byte_length
        || permission_mode(finalized_identity) != 0o600
    {
        return Err(ArtifactIoError::new(
            ArtifactIoFailureKind::ChangedDuringRead,
        ));
    }
    if parent.sync_all().is_err() {
        return Err(ArtifactIoError::new(ArtifactIoFailureKind::IoFailure));
    }
    after_install();
    let installed_identity = identity(&output)?;
    if installed_identity != finalized_identity {
        return Err(ArtifactIoError::new(
            ArtifactIoFailureKind::ChangedDuringRead,
        ));
    }
    let rebound = open_at(&parent, output_name);
    if rebound.as_ref().ok().and_then(|file| identity(file).ok()) != Some(finalized_identity) {
        return Err(ArtifactIoError::new(
            ArtifactIoFailureKind::ChangedDuringRead,
        ));
    }
    let retained_parent_identity = output_directory_identity(identity(&parent)?);
    let current_parent_chain = open_trusted_output_directory(output_parent)
        .map(|(_, chain)| chain)
        .map_err(|_| ArtifactIoError::new(ArtifactIoFailureKind::ChangedDuringRead))?;
    if retained_parent_identity != parent_identity || current_parent_chain != parent_chain {
        return Err(ArtifactIoError::new(
            ArtifactIoFailureKind::ChangedDuringRead,
        ));
    }
    Ok(evidence)
}

#[cfg(not(unix))]
#[allow(clippy::too_many_arguments)]
fn copy_regular_descriptor_relative<H, J>(
    _input_root: &Path,
    _input_relative: &Path,
    _output_parent: &Path,
    _output_name: &OsStr,
    _maximum: u64,
    _expected: Option<&FileIdentity>,
    _after_open: H,
    _after_install: J,
) -> Result<FileEvidence, ArtifactIoError>
where
    H: FnOnce(),
    J: FnOnce(),
{
    Err(ArtifactIoError::new(
        ArtifactIoFailureKind::UnsupportedPlatform,
    ))
}

fn require_observed_length(observed: u64, admitted: u64) -> Result<(), ArtifactIoError> {
    if observed == admitted {
        Ok(())
    } else {
        Err(ArtifactIoError::new(
            ArtifactIoFailureKind::ChangedDuringRead,
        ))
    }
}

const fn output_directory_identity(identity: FileIdentity) -> OutputDirectoryIdentity {
    OutputDirectoryIdentity {
        directory: directory_identity(identity),
        mode: permission_mode(identity),
        owner: identity.owner,
    }
}

#[cfg(unix)]
fn output_directory_is_trusted(identity: OutputDirectoryIdentity) -> bool {
    (identity.owner == 0 || identity.owner == rustix::process::geteuid().as_raw())
        && identity.mode & 0o022 == 0
}

fn stream_regular(
    file: &mut File,
    maximum: u64,
    mut output: Option<&mut File>,
) -> Result<FileEvidence, ArtifactIoError> {
    let mut hasher = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; STREAM_BUFFER_BYTES];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|_| ArtifactIoError::new(ArtifactIoFailureKind::IoFailure))?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(read as u64)
            .ok_or_else(|| ArtifactIoError::new(ArtifactIoFailureKind::InvalidObject))?;
        if total > maximum {
            return Err(ArtifactIoError::new(ArtifactIoFailureKind::LimitExceeded(
                LimitKind::FileBytes,
            )));
        }
        hasher.update(&buffer[..read]);
        if let Some(destination) = output.as_deref_mut() {
            destination
                .write_all(&buffer[..read])
                .map_err(|_| ArtifactIoError::new(ArtifactIoFailureKind::IoFailure))?;
        }
    }
    Ok(FileEvidence {
        byte_length: total,
        sha256: hex::encode(hasher.finalize()),
    })
}

fn with_regular<T, F, H>(
    root: &Path,
    relative: &Path,
    maximum: u64,
    expected: Option<&FileIdentity>,
    after_open: H,
    operation: F,
) -> Result<T, ArtifactIoError>
where
    F: FnOnce(&mut File, u64) -> Result<T, ArtifactIoError>,
    H: FnOnce(),
{
    if maximum == 0 || maximum > HARD_MAX_STREAM_FILE_BYTES {
        return Err(ArtifactIoError::new(ArtifactIoFailureKind::InvalidRequest));
    }
    let mut opened = open_relative(root, relative, ObjectKind::Regular)?;
    if opened.identity.length > maximum {
        return Err(ArtifactIoError::new(ArtifactIoFailureKind::LimitExceeded(
            LimitKind::FileBytes,
        )));
    }
    if opened.identity.links != 1 {
        return Err(ArtifactIoError::new(ArtifactIoFailureKind::InvalidObject));
    }
    if expected.is_some_and(|identity| *identity != opened.identity) {
        return Err(ArtifactIoError::new(
            ArtifactIoFailureKind::ChangedDuringRead,
        ));
    }
    let before = opened.identity;
    let chain = opened.chain.clone();
    after_open();
    let result = operation(&mut opened.file, before.length)?;
    let after = identity(&opened.file)?;
    let rebound = open_relative(root, relative, ObjectKind::Regular)?;
    if after != before || rebound.identity != before || rebound.chain != chain {
        return Err(ArtifactIoError::new(
            ArtifactIoFailureKind::ChangedDuringRead,
        ));
    }
    Ok(result)
}

pub(crate) fn traverse_regular_files(
    root: &Path,
    limits: TraversalLimits,
    excluded_directory_names: &[&str],
) -> Result<TraversalSnapshot, ArtifactIoError> {
    traverse_regular_files_impl(root, limits, excluded_directory_names, || {})
}

fn traverse_regular_files_impl<F>(
    root: &Path,
    limits: TraversalLimits,
    excluded_directory_names: &[&str],
    after_walk: F,
) -> Result<TraversalSnapshot, ArtifactIoError>
where
    F: FnOnce(),
{
    validate_traversal_limits(limits)?;
    if excluded_directory_names.iter().any(|name| {
        name.is_empty()
            || matches!(*name, "." | "..")
            || name.as_bytes().contains(&b'/')
            || name.as_bytes().contains(&b'\\')
            || name.as_bytes().contains(&0)
    }) {
        return Err(ArtifactIoError::new(ArtifactIoFailureKind::InvalidRequest));
    }
    let (root_file, root_chain) = open_absolute_directory(root)?;
    let mut state = TraversalState {
        limits,
        exclusions: excluded_directory_names,
        entries: 0,
        files: Vec::new(),
        directories: Vec::new(),
        total_bytes: 0,
    };
    let root_identity = identity(&root_file)?;
    walk_directory(&root_file, Path::new(""), 0, root_identity, &mut state)?;
    after_walk();
    let snapshot = TraversalSnapshot {
        root: root.to_path_buf(),
        root_chain,
        directories: state.directories,
        files: state.files,
        entry_count: state.entries,
        total_bytes: state.total_bytes,
    };
    snapshot.revalidate()?;
    Ok(snapshot)
}

struct TraversalState<'a> {
    limits: TraversalLimits,
    exclusions: &'a [&'a str],
    entries: u64,
    files: Vec<TraversedFile>,
    directories: Vec<TraversedDirectory>,
    total_bytes: u64,
}

fn validate_traversal_limits(limits: TraversalLimits) -> Result<(), ArtifactIoError> {
    if limits.max_entries == 0
        || limits.max_entries > HARD_MAX_TRAVERSAL_ENTRIES
        || limits.max_files == 0
        || limits.max_files > HARD_MAX_TRAVERSAL_FILES
        || limits.max_total_bytes == 0
        || limits.max_total_bytes > HARD_MAX_TRAVERSAL_TOTAL_BYTES
        || limits.max_file_bytes == 0
        || limits.max_file_bytes > HARD_MAX_STREAM_FILE_BYTES
        || limits.max_depth == 0
        || limits.max_depth > HARD_MAX_TRAVERSAL_DEPTH
        || limits.max_path_bytes == 0
        || limits.max_path_bytes > HARD_MAX_PATH_BYTES
    {
        Err(ArtifactIoError::new(ArtifactIoFailureKind::InvalidRequest))
    } else {
        Ok(())
    }
}

#[cfg(unix)]
fn walk_directory(
    directory: &File,
    relative: &Path,
    depth: usize,
    current_identity: FileIdentity,
    state: &mut TraversalState<'_>,
) -> Result<(), ArtifactIoError> {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt as _;

    let mut reader = rustix::fs::Dir::read_from(directory)
        .map_err(|_| ArtifactIoError::new(ArtifactIoFailureKind::IoFailure))?;
    let mut names = Vec::new();
    while let Some(entry) = reader.read() {
        let entry = entry.map_err(|_| ArtifactIoError::new(ArtifactIoFailureKind::IoFailure))?;
        let name = entry.file_name().to_bytes();
        if matches!(name, b"." | b"..") {
            continue;
        }
        state.entries = state.entries.checked_add(1).ok_or_else(|| {
            ArtifactIoError::new(ArtifactIoFailureKind::LimitExceeded(
                LimitKind::TraversalEntries,
            ))
        })?;
        if state.entries > state.limits.max_entries {
            return Err(ArtifactIoError::new(ArtifactIoFailureKind::LimitExceeded(
                LimitKind::TraversalEntries,
            )));
        }
        names.push(name.to_vec());
    }
    names.sort();

    let mut members = Vec::with_capacity(names.len());
    for name in names {
        if name.is_empty() || name.contains(&0) || name.contains(&b'/') {
            return Err(ArtifactIoError::new(ArtifactIoFailureKind::InvalidObject));
        }
        let component = OsStr::from_bytes(&name);
        let child_relative = relative.join(component);
        if path_byte_len(&child_relative) > state.limits.max_path_bytes {
            return Err(ArtifactIoError::new(ArtifactIoFailureKind::LimitExceeded(
                LimitKind::TraversalPathBytes,
            )));
        }
        let child = open_at(directory, component)?;
        let metadata = child
            .metadata()
            .map_err(|_| ArtifactIoError::new(ArtifactIoFailureKind::IoFailure))?;
        let child_identity = identity_from_metadata(&metadata);
        if metadata.is_dir() {
            let child_directory_identity = directory_identity(child_identity);
            members.push(DirectoryMember {
                name: component.to_os_string(),
                identity: MemberIdentity::Directory(child_directory_identity),
            });
            let next_depth = depth.checked_add(1).ok_or_else(|| {
                ArtifactIoError::new(ArtifactIoFailureKind::LimitExceeded(
                    LimitKind::TraversalDepth,
                ))
            })?;
            if next_depth > state.limits.max_depth {
                return Err(ArtifactIoError::new(ArtifactIoFailureKind::LimitExceeded(
                    LimitKind::TraversalDepth,
                )));
            }
            if state
                .exclusions
                .iter()
                .any(|excluded| name == excluded.as_bytes())
            {
                continue;
            }
            walk_directory(&child, &child_relative, next_depth, child_identity, state)?;
        } else if metadata.is_file() {
            if child_identity.links != 1 {
                return Err(ArtifactIoError::new(ArtifactIoFailureKind::InvalidObject));
            }
            if child_identity.length > state.limits.max_file_bytes {
                return Err(ArtifactIoError::new(ArtifactIoFailureKind::LimitExceeded(
                    LimitKind::FileBytes,
                )));
            }
            let file_count = state.files.len().checked_add(1).ok_or_else(|| {
                ArtifactIoError::new(ArtifactIoFailureKind::LimitExceeded(
                    LimitKind::TraversalFiles,
                ))
            })?;
            if file_count as u64 > state.limits.max_files {
                return Err(ArtifactIoError::new(ArtifactIoFailureKind::LimitExceeded(
                    LimitKind::TraversalFiles,
                )));
            }
            state.total_bytes = state
                .total_bytes
                .checked_add(child_identity.length)
                .ok_or_else(|| {
                    ArtifactIoError::new(ArtifactIoFailureKind::LimitExceeded(
                        LimitKind::TraversalTotalBytes,
                    ))
                })?;
            if state.total_bytes > state.limits.max_total_bytes {
                return Err(ArtifactIoError::new(ArtifactIoFailureKind::LimitExceeded(
                    LimitKind::TraversalTotalBytes,
                )));
            }
            state.files.push(TraversedFile {
                relative: child_relative,
                identity: child_identity,
            });
            members.push(DirectoryMember {
                name: component.to_os_string(),
                identity: MemberIdentity::Regular(child_identity),
            });
        } else {
            return Err(ArtifactIoError::new(ArtifactIoFailureKind::InvalidObject));
        }
    }
    state.directories.push(TraversedDirectory {
        relative: relative.to_path_buf(),
        identity: directory_identity(current_identity),
        permission_mode: permission_mode(current_identity),
        members,
    });
    Ok(())
}

#[cfg(unix)]
fn revalidate_directory_members(
    directory: &File,
    expected: &[DirectoryMember],
) -> Result<(), ArtifactIoError> {
    use std::os::unix::ffi::OsStrExt as _;

    let changed = || ArtifactIoError::new(ArtifactIoFailureKind::ChangedDuringRead);
    let mut reader = rustix::fs::Dir::read_from(directory).map_err(|_| changed())?;
    let mut names = Vec::with_capacity(expected.len());
    while let Some(entry) = reader.read() {
        let entry = entry.map_err(|_| changed())?;
        let name = entry.file_name().to_bytes();
        if matches!(name, b"." | b"..") {
            continue;
        }
        if names.len() == expected.len() {
            return Err(changed());
        }
        names.push(OsString::from(OsStr::from_bytes(name)));
    }
    names.sort();
    if names.len() != expected.len()
        || names
            .iter()
            .zip(expected)
            .any(|(name, member)| name != &member.name)
    {
        return Err(changed());
    }
    for member in expected {
        let child = open_at(directory, &member.name).map_err(|_| changed())?;
        let metadata = child.metadata().map_err(|_| changed())?;
        let identity = identity_from_metadata(&metadata);
        let observed = if metadata.is_dir() {
            MemberIdentity::Directory(directory_identity(identity))
        } else if metadata.is_file() && identity.links == 1 {
            MemberIdentity::Regular(identity)
        } else {
            return Err(changed());
        };
        if observed != member.identity {
            return Err(changed());
        }
    }
    Ok(())
}

#[cfg(not(unix))]
fn revalidate_directory_members(
    _directory: &File,
    _expected: &[DirectoryMember],
) -> Result<(), ArtifactIoError> {
    Err(ArtifactIoError::new(
        ArtifactIoFailureKind::UnsupportedPlatform,
    ))
}

#[cfg(not(unix))]
fn walk_directory(
    _directory: &File,
    _relative: &Path,
    _depth: usize,
    _current_identity: FileIdentity,
    _state: &mut TraversalState<'_>,
) -> Result<(), ArtifactIoError> {
    Err(ArtifactIoError::new(
        ArtifactIoFailureKind::UnsupportedPlatform,
    ))
}

pub(crate) fn admit_tar_gzip_path(
    path: &Path,
    limits: TarGzipLimits,
) -> Result<ArchiveEvidence, ArtifactIoError> {
    let (root, relative) = split_absolute_file(path)?;
    admit_tar_gzip_relative(&root, &relative, limits, None, TarGzipPolicy::Generic)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TarGzipPolicy {
    Generic,
    DeterministicSnapshot,
}

fn admit_tar_gzip_relative(
    root: &Path,
    relative: &Path,
    limits: TarGzipLimits,
    expected: Option<&FileIdentity>,
    policy: TarGzipPolicy,
) -> Result<ArchiveEvidence, ArtifactIoError> {
    validate_archive_limits(limits)?;
    with_regular(
        root,
        relative,
        limits.max_compressed_bytes,
        expected,
        || {},
        |file, admitted_length| {
            let evidence = admit_tar_gzip_reader(file, limits, policy, None)?;
            require_observed_length(evidence.compressed.byte_length, admitted_length)?;
            Ok(evidence)
        },
    )
    .map_err(|error| match error.kind() {
        ArtifactIoFailureKind::LimitExceeded(LimitKind::FileBytes) => ArtifactIoError::new(
            ArtifactIoFailureKind::LimitExceeded(LimitKind::ArchiveCompressedBytes),
        ),
        _ => error,
    })
}

fn materialize_tar_gzip_relative(
    root: &Path,
    relative: &Path,
    trusted_parent: &Path,
    limits: TarGzipLimits,
    expected: Option<&FileIdentity>,
) -> Result<MaterializedArchive, ArtifactIoError> {
    validate_archive_limits(limits)?;
    let (parent_descriptor, parent_chain) = open_trusted_output_directory(trusted_parent)?;
    let parent_identity = *parent_chain
        .last()
        .ok_or_else(|| ArtifactIoError::new(ArtifactIoFailureKind::InvalidRequest))?;
    let mut directory_builder = tempfile::Builder::new();
    directory_builder.prefix("radroots-advisory-materialized-");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;

        directory_builder.permissions(fs::Permissions::from_mode(0o700));
    }
    let directory = directory_builder
        .tempdir_in(trusted_parent)
        .map_err(|_| ArtifactIoError::new(ArtifactIoFailureKind::IoFailure))?;
    let (_, rebound_parent_chain) = open_trusted_output_directory(trusted_parent)?;
    if rebound_parent_chain != parent_chain
        || output_directory_identity(identity(&parent_descriptor)?) != parent_identity
    {
        return Err(ArtifactIoError::new(
            ArtifactIoFailureKind::ChangedDuringRead,
        ));
    }
    let (output, root_chain) = open_absolute_directory(directory.path())?;
    let output_identity = identity(&output)?;
    let parent_directory_chain = parent_chain
        .iter()
        .map(|identity| identity.directory)
        .collect::<Vec<_>>();
    if root_chain.len() != parent_directory_chain.len().saturating_add(1)
        || root_chain[..parent_directory_chain.len()] != parent_directory_chain
    {
        return Err(ArtifactIoError::new(
            ArtifactIoFailureKind::ChangedDuringRead,
        ));
    }
    #[cfg(unix)]
    if output_identity.owner != rustix::process::geteuid().as_raw()
        || permission_mode(output_identity) != 0o700
    {
        return Err(ArtifactIoError::new(
            ArtifactIoFailureKind::ChangedDuringRead,
        ));
    }
    parent_descriptor
        .sync_all()
        .map_err(|_| ArtifactIoError::new(ArtifactIoFailureKind::IoFailure))?;
    let evidence = with_regular(
        root,
        relative,
        limits.max_compressed_bytes,
        expected,
        || {},
        |file, admitted_length| {
            let evidence = admit_tar_gzip_reader(
                file,
                limits,
                TarGzipPolicy::DeterministicSnapshot,
                Some(&output),
            )?;
            require_observed_length(evidence.compressed.byte_length, admitted_length)?;
            output
                .sync_all()
                .map_err(|_| ArtifactIoError::new(ArtifactIoFailureKind::IoFailure))?;
            Ok(evidence)
        },
    )
    .map_err(|error| match error.kind() {
        ArtifactIoFailureKind::LimitExceeded(LimitKind::FileBytes) => ArtifactIoError::new(
            ArtifactIoFailureKind::LimitExceeded(LimitKind::ArchiveCompressedBytes),
        ),
        _ => error,
    })?;
    let materialized_snapshot = traverse_regular_files(
        directory.path(),
        TraversalLimits {
            max_entries: limits.max_members,
            max_files: limits.max_members,
            max_total_bytes: limits.max_payload_bytes,
            max_file_bytes: limits.max_member_bytes,
            max_depth: limits.max_depth,
            max_path_bytes: limits.max_path_bytes,
        },
        &[],
    )?;
    if materialized_snapshot.entry_count() != evidence.member_count
        || materialized_snapshot.total_bytes() != evidence.payload_bytes
    {
        return Err(ArtifactIoError::new(
            ArtifactIoFailureKind::ChangedDuringRead,
        ));
    }
    materialized_snapshot.revalidate()?;
    parent_descriptor
        .sync_all()
        .map_err(|_| ArtifactIoError::new(ArtifactIoFailureKind::IoFailure))?;
    let materialized = MaterializedArchive {
        root_descriptor: output,
        root_identity: output_directory_identity(output_identity),
        root_chain,
        parent_descriptor,
        parent_identity,
        parent_path: trusted_parent.to_owned(),
        parent_chain,
        snapshot: materialized_snapshot,
        evidence,
        directory,
    };
    materialized.revalidate()?;
    Ok(materialized)
}

fn validate_archive_limits(limits: TarGzipLimits) -> Result<(), ArtifactIoError> {
    if limits.max_compressed_bytes == 0
        || limits.max_compressed_bytes > HARD_MAX_ARCHIVE_COMPRESSED_BYTES
        || limits.max_expanded_bytes == 0
        || limits.max_expanded_bytes > HARD_MAX_ARCHIVE_EXPANDED_BYTES
        || limits.max_members == 0
        || limits.max_members > HARD_MAX_ARCHIVE_MEMBERS
        || limits.max_member_bytes == 0
        || limits.max_member_bytes > HARD_MAX_ARCHIVE_MEMBER_BYTES
        || limits.max_payload_bytes == 0
        || limits.max_payload_bytes > HARD_MAX_ARCHIVE_PAYLOAD_BYTES
        || limits.max_depth == 0
        || limits.max_depth > HARD_MAX_TRAVERSAL_DEPTH
        || limits.max_path_bytes == 0
        || limits.max_path_bytes > HARD_MAX_PATH_BYTES
    {
        Err(ArtifactIoError::new(ArtifactIoFailureKind::InvalidRequest))
    } else {
        Ok(())
    }
}

fn admit_tar_gzip_reader(
    file: &mut File,
    limits: TarGzipLimits,
    policy: TarGzipPolicy,
    materialization_root: Option<&File>,
) -> Result<ArchiveEvidence, ArtifactIoError> {
    let expected_compressed_bytes = file
        .metadata()
        .map_err(|_| ArtifactIoError::new(ArtifactIoFailureKind::IoFailure))?
        .len();
    let compressed = HashingLimitReader::new(file, limits.max_compressed_bytes);
    let mut buffered = BufReader::new(compressed);
    validate_gzip_header(&mut buffered, policy)?;
    let decoder = GzDecoder::new(buffered);
    let expanded = LimitReader::new(decoder, limits.max_expanded_bytes);
    let mut archive = tar::Archive::new(expanded);
    let mut names = BTreeMap::<Vec<u8>, ArchiveMemberKind>::new();
    let mut members = 0_u64;
    let mut payload = 0_u64;
    let mut previous_name = None::<Vec<u8>>;
    let mut canonical = (policy == TarGzipPolicy::DeterministicSnapshot).then(|| {
        GzBuilder::new().mtime(0).operating_system(255).write(
            HashingLimitWriter::new(limits.max_compressed_bytes),
            Compression::new(CANONICAL_GZIP_COMPRESSION_LEVEL),
        )
    });

    let parse_result = (|| {
        let entries = archive
            .entries()
            .map_err(|_| ArtifactIoError::new(ArtifactIoFailureKind::MalformedArchive))?
            .raw(true);
        for entry in entries {
            let mut entry =
                entry.map_err(|_| ArtifactIoError::new(ArtifactIoFailureKind::MalformedArchive))?;
            members = members.checked_add(1).ok_or_else(|| {
                ArtifactIoError::new(ArtifactIoFailureKind::LimitExceeded(
                    LimitKind::ArchiveMembers,
                ))
            })?;
            if members > limits.max_members {
                return Err(ArtifactIoError::new(ArtifactIoFailureKind::LimitExceeded(
                    LimitKind::ArchiveMembers,
                )));
            }
            let declared = entry
                .header()
                .size()
                .map_err(|_| ArtifactIoError::new(ArtifactIoFailureKind::MalformedArchive))?;
            if declared > limits.max_member_bytes {
                return Err(ArtifactIoError::new(ArtifactIoFailureKind::LimitExceeded(
                    LimitKind::ArchiveMemberBytes,
                )));
            }
            let member_kind = match entry.header().entry_type().as_byte() {
                0 | b'0' => ArchiveMemberKind::File,
                b'5' if declared == 0 => ArchiveMemberKind::Directory,
                _ => {
                    return Err(ArtifactIoError::new(ArtifactIoFailureKind::InvalidObject));
                }
            };
            let name = entry.path_bytes().into_owned();
            validate_archive_path(&name, limits.max_depth, limits.max_path_bytes)?;
            let canonical_header = if policy == TarGzipPolicy::DeterministicSnapshot {
                let header = validate_deterministic_tar_header(
                    entry.header(),
                    member_kind,
                    &name,
                    declared,
                )?;
                if previous_name
                    .as_ref()
                    .is_some_and(|previous| previous >= &name)
                {
                    return Err(ArtifactIoError::new(ArtifactIoFailureKind::InvalidObject));
                }
                previous_name = Some(name.clone());
                Some(header)
            } else {
                None
            };
            if match member_kind {
                ArchiveMemberKind::Directory => !name.ends_with(b"/"),
                ArchiveMemberKind::File => name.ends_with(b"/"),
            } {
                return Err(ArtifactIoError::new(ArtifactIoFailureKind::InvalidObject));
            }
            admit_archive_name(&mut names, &name, member_kind)?;
            if let (Some(encoder), Some(header)) = (canonical.as_mut(), canonical_header.as_ref()) {
                write_canonical_archive_bytes(encoder, header.as_bytes())?;
            }
            payload = payload.checked_add(declared).ok_or_else(|| {
                ArtifactIoError::new(ArtifactIoFailureKind::LimitExceeded(
                    LimitKind::ArchivePayloadBytes,
                ))
            })?;
            if payload > limits.max_payload_bytes {
                return Err(ArtifactIoError::new(ArtifactIoFailureKind::LimitExceeded(
                    LimitKind::ArchivePayloadBytes,
                )));
            }
            let mut materialized = materialization_root
                .map(|root| materialize_archive_member(root, member_kind, &name))
                .transpose()?
                .flatten();
            let mut actual = 0_u64;
            let mut buffer = [0_u8; STREAM_BUFFER_BYTES];
            loop {
                let read = entry
                    .read(&mut buffer)
                    .map_err(|_| ArtifactIoError::new(ArtifactIoFailureKind::MalformedArchive))?;
                if read == 0 {
                    break;
                }
                actual = actual.checked_add(read as u64).ok_or_else(|| {
                    ArtifactIoError::new(ArtifactIoFailureKind::LimitExceeded(
                        LimitKind::ArchiveMemberBytes,
                    ))
                })?;
                if actual > limits.max_member_bytes {
                    return Err(ArtifactIoError::new(ArtifactIoFailureKind::LimitExceeded(
                        LimitKind::ArchiveMemberBytes,
                    )));
                }
                if let Some(destination) = materialized.as_mut() {
                    destination
                        .write_all(&buffer[..read])
                        .map_err(|_| ArtifactIoError::new(ArtifactIoFailureKind::IoFailure))?;
                }
                if let Some(encoder) = canonical.as_mut() {
                    write_canonical_archive_bytes(encoder, &buffer[..read])?;
                }
            }
            if actual != declared {
                return Err(ArtifactIoError::new(
                    ArtifactIoFailureKind::MalformedArchive,
                ));
            }
            if let Some(encoder) = canonical.as_mut() {
                let padding = (TAR_BLOCK_BYTES as u64 - actual % TAR_BLOCK_BYTES as u64)
                    % TAR_BLOCK_BYTES as u64;
                write_canonical_archive_bytes(
                    encoder,
                    &[0_u8; TAR_BLOCK_BYTES][..padding as usize],
                )?;
            }
            if let Some(destination) = materialized {
                destination
                    .sync_all()
                    .map_err(|_| ArtifactIoError::new(ArtifactIoFailureKind::IoFailure))?;
            }
        }
        Ok(())
    })();

    let mut expanded = archive.into_inner();
    if expanded.exceeded() {
        return Err(ArtifactIoError::new(ArtifactIoFailureKind::LimitExceeded(
            LimitKind::ArchiveExpandedBytes,
        )));
    }
    if compressed_limit_exceeded(&expanded) {
        return Err(ArtifactIoError::new(ArtifactIoFailureKind::LimitExceeded(
            LimitKind::ArchiveCompressedBytes,
        )));
    }
    parse_result?;

    if let Some(encoder) = canonical.as_mut() {
        write_canonical_archive_bytes(encoder, &[0_u8; TAR_TERMINATOR_BYTES])?;
    }

    let mut trailing = [0_u8; STREAM_BUFFER_BYTES];
    let mut trailing_zero_bytes = 0_u64;
    loop {
        let read = expanded
            .read(&mut trailing)
            .map_err(|_| classify_expanded_error(&expanded))?;
        if read == 0 {
            break;
        }
        if trailing[..read].iter().any(|byte| *byte != 0) {
            return Err(ArtifactIoError::new(
                ArtifactIoFailureKind::MalformedArchive,
            ));
        }
        trailing_zero_bytes = trailing_zero_bytes
            .checked_add(read as u64)
            .ok_or_else(|| {
                ArtifactIoError::new(ArtifactIoFailureKind::LimitExceeded(
                    LimitKind::ArchiveExpandedBytes,
                ))
            })?;
    }
    if expanded.exceeded() {
        return Err(ArtifactIoError::new(ArtifactIoFailureKind::LimitExceeded(
            LimitKind::ArchiveExpandedBytes,
        )));
    }
    if compressed_limit_exceeded(&expanded) {
        return Err(ArtifactIoError::new(ArtifactIoFailureKind::LimitExceeded(
            LimitKind::ArchiveCompressedBytes,
        )));
    }
    if members == 0 || trailing_zero_bytes < 512 {
        return Err(ArtifactIoError::new(
            ArtifactIoFailureKind::MalformedArchive,
        ));
    }
    let expanded_bytes = expanded.total();
    let decoder = expanded.into_inner();
    let mut buffered = decoder.into_inner();
    if !buffered
        .fill_buf()
        .map_err(|_| ArtifactIoError::new(ArtifactIoFailureKind::MalformedArchive))?
        .is_empty()
    {
        return Err(ArtifactIoError::new(
            ArtifactIoFailureKind::MalformedArchive,
        ));
    }
    let compressed = buffered.into_inner();
    if compressed.exceeded() {
        return Err(ArtifactIoError::new(ArtifactIoFailureKind::LimitExceeded(
            LimitKind::ArchiveCompressedBytes,
        )));
    }
    let compressed_bytes = compressed.total();
    if compressed_bytes != expected_compressed_bytes {
        return Err(ArtifactIoError::new(
            ArtifactIoFailureKind::MalformedArchive,
        ));
    }
    let compressed_evidence = FileEvidence {
        byte_length: compressed_bytes,
        sha256: compressed.finalize(),
    };
    if let Some(encoder) = canonical {
        let canonical_evidence = encoder
            .finish()
            .map_err(|_| {
                ArtifactIoError::new(ArtifactIoFailureKind::LimitExceeded(
                    LimitKind::ArchiveCompressedBytes,
                ))
            })?
            .finalize();
        if canonical_evidence != compressed_evidence {
            return Err(ArtifactIoError::new(ArtifactIoFailureKind::InvalidObject));
        }
    }
    Ok(ArchiveEvidence {
        compressed: compressed_evidence,
        expanded_bytes,
        member_count: members,
        payload_bytes: payload,
    })
}

#[cfg(unix)]
fn materialize_archive_member(
    root: &File,
    kind: ArchiveMemberKind,
    raw_name: &[u8],
) -> Result<Option<File>, ArtifactIoError> {
    use rustix::fs::{Mode, OFlags, fchmod, mkdirat, openat};

    let name = std::str::from_utf8(raw_name)
        .map_err(|_| ArtifactIoError::new(ArtifactIoFailureKind::InvalidObject))?
        .trim_end_matches('/');
    let path = Path::new(name);
    let mut components = path.components().peekable();
    let mut current = root
        .try_clone()
        .map_err(|_| ArtifactIoError::new(ArtifactIoFailureKind::IoFailure))?;
    while let Some(component) = components.next() {
        let Component::Normal(component) = component else {
            return Err(ArtifactIoError::new(ArtifactIoFailureKind::InvalidObject));
        };
        let is_leaf = components.peek().is_none();
        if !is_leaf {
            current = File::from(
                openat(
                    &current,
                    component,
                    OFlags::RDONLY
                        | OFlags::DIRECTORY
                        | OFlags::NOFOLLOW
                        | OFlags::CLOEXEC
                        | OFlags::NONBLOCK,
                    Mode::empty(),
                )
                .map_err(|_| ArtifactIoError::new(ArtifactIoFailureKind::InvalidObject))?,
            );
            continue;
        }
        return match kind {
            ArchiveMemberKind::Directory => {
                mkdirat(&current, component, Mode::RUSR | Mode::WUSR | Mode::XUSR)
                    .map_err(|_| ArtifactIoError::new(ArtifactIoFailureKind::InvalidObject))?;
                let directory = File::from(
                    openat(
                        &current,
                        component,
                        OFlags::RDONLY
                            | OFlags::DIRECTORY
                            | OFlags::NOFOLLOW
                            | OFlags::CLOEXEC
                            | OFlags::NONBLOCK,
                        Mode::empty(),
                    )
                    .map_err(|_| ArtifactIoError::new(ArtifactIoFailureKind::InvalidObject))?,
                );
                fchmod(
                    &directory,
                    Mode::RUSR
                        | Mode::WUSR
                        | Mode::XUSR
                        | Mode::RGRP
                        | Mode::XGRP
                        | Mode::ROTH
                        | Mode::XOTH,
                )
                .map_err(|_| ArtifactIoError::new(ArtifactIoFailureKind::IoFailure))?;
                directory
                    .sync_all()
                    .map_err(|_| ArtifactIoError::new(ArtifactIoFailureKind::IoFailure))?;
                current
                    .sync_all()
                    .map_err(|_| ArtifactIoError::new(ArtifactIoFailureKind::IoFailure))?;
                Ok(None)
            }
            ArchiveMemberKind::File => {
                let file = File::from(
                    openat(
                        &current,
                        component,
                        OFlags::WRONLY
                            | OFlags::CREATE
                            | OFlags::EXCL
                            | OFlags::NOFOLLOW
                            | OFlags::CLOEXEC
                            | OFlags::NONBLOCK,
                        Mode::RUSR | Mode::WUSR,
                    )
                    .map_err(|_| ArtifactIoError::new(ArtifactIoFailureKind::InvalidObject))?,
                );
                fchmod(&file, Mode::RUSR | Mode::WUSR | Mode::RGRP | Mode::ROTH)
                    .map_err(|_| ArtifactIoError::new(ArtifactIoFailureKind::IoFailure))?;
                current
                    .sync_all()
                    .map_err(|_| ArtifactIoError::new(ArtifactIoFailureKind::IoFailure))?;
                Ok(Some(file))
            }
        };
    }
    Err(ArtifactIoError::new(ArtifactIoFailureKind::InvalidObject))
}

#[cfg(not(unix))]
fn materialize_archive_member(
    _root: &File,
    _kind: ArchiveMemberKind,
    _raw_name: &[u8],
) -> Result<Option<File>, ArtifactIoError> {
    Err(ArtifactIoError::new(
        ArtifactIoFailureKind::UnsupportedPlatform,
    ))
}

fn validate_gzip_header(
    reader: &mut BufReader<HashingLimitReader<'_>>,
    policy: TarGzipPolicy,
) -> Result<(), ArtifactIoError> {
    if reader.fill_buf().is_err() {
        return Err(if reader.get_ref().exceeded() {
            ArtifactIoError::new(ArtifactIoFailureKind::LimitExceeded(
                LimitKind::ArchiveCompressedBytes,
            ))
        } else {
            ArtifactIoError::new(ArtifactIoFailureKind::MalformedArchive)
        });
    }
    let header = reader.buffer();
    if header.len() < 10 || header[..3] != [0x1f, 0x8b, 8] {
        return Err(ArtifactIoError::new(
            ArtifactIoFailureKind::MalformedArchive,
        ));
    }
    if header[3] != 0 {
        return Err(ArtifactIoError::new(ArtifactIoFailureKind::InvalidObject));
    }
    if policy == TarGzipPolicy::DeterministicSnapshot
        && (header[4..8] != [0, 0, 0, 0] || header[8] != 0 || header[9] != 255)
    {
        return Err(ArtifactIoError::new(ArtifactIoFailureKind::InvalidObject));
    }
    Ok(())
}

fn validate_deterministic_tar_header(
    header: &tar::Header,
    kind: ArchiveMemberKind,
    name: &[u8],
    size: u64,
) -> Result<TarHeader, ArtifactIoError> {
    let path = std::str::from_utf8(name)
        .map_err(|_| ArtifactIoError::new(ArtifactIoFailureKind::InvalidObject))?;
    let mut expected = TarHeader::new_gnu();
    expected
        .set_path(path)
        .map_err(|_| ArtifactIoError::new(ArtifactIoFailureKind::InvalidObject))?;
    expected.set_entry_type(match kind {
        ArchiveMemberKind::Directory => tar::EntryType::Directory,
        ArchiveMemberKind::File => tar::EntryType::Regular,
    });
    expected.set_mode(match kind {
        ArchiveMemberKind::Directory => 0o755,
        ArchiveMemberKind::File => 0o644,
    });
    expected.set_uid(0);
    expected.set_gid(0);
    expected.set_mtime(0);
    expected.set_size(size);
    expected.set_cksum();
    if header.as_bytes() == expected.as_bytes() {
        Ok(expected)
    } else {
        Err(ArtifactIoError::new(ArtifactIoFailureKind::InvalidObject))
    }
}

fn write_canonical_archive_bytes(
    encoder: &mut flate2::write::GzEncoder<HashingLimitWriter>,
    bytes: &[u8],
) -> Result<(), ArtifactIoError> {
    encoder.write_all(bytes).map_err(|_| {
        if encoder.get_ref().exceeded() {
            ArtifactIoError::new(ArtifactIoFailureKind::LimitExceeded(
                LimitKind::ArchiveCompressedBytes,
            ))
        } else {
            ArtifactIoError::new(ArtifactIoFailureKind::IoFailure)
        }
    })
}

fn classify_expanded_error(
    reader: &LimitReader<GzDecoder<BufReader<HashingLimitReader<'_>>>>,
) -> ArtifactIoError {
    if reader.exceeded() {
        ArtifactIoError::new(ArtifactIoFailureKind::LimitExceeded(
            LimitKind::ArchiveExpandedBytes,
        ))
    } else if compressed_limit_exceeded(reader) {
        ArtifactIoError::new(ArtifactIoFailureKind::LimitExceeded(
            LimitKind::ArchiveCompressedBytes,
        ))
    } else {
        ArtifactIoError::new(ArtifactIoFailureKind::MalformedArchive)
    }
}

fn compressed_limit_exceeded(
    reader: &LimitReader<GzDecoder<BufReader<HashingLimitReader<'_>>>>,
) -> bool {
    reader.get_ref().get_ref().get_ref().exceeded()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ArchiveMemberKind {
    Directory,
    File,
}

fn validate_archive_path(
    path: &[u8],
    maximum_depth: usize,
    maximum_bytes: usize,
) -> Result<(), ArtifactIoError> {
    if path.len() > maximum_bytes {
        return Err(ArtifactIoError::new(ArtifactIoFailureKind::LimitExceeded(
            LimitKind::ArchivePathBytes,
        )));
    }
    if path.is_empty()
        || path.starts_with(b"/")
        || path.contains(&0)
        || path.contains(&b'\\')
        || std::str::from_utf8(path).is_err()
    {
        return Err(ArtifactIoError::new(ArtifactIoFailureKind::InvalidObject));
    }
    let trimmed = path.strip_suffix(b"/").unwrap_or(path);
    let mut depth = 0_usize;
    for component in trimmed.split(|byte| *byte == b'/') {
        if component.is_empty() || matches!(component, b"." | b"..") {
            return Err(ArtifactIoError::new(ArtifactIoFailureKind::InvalidObject));
        }
        depth = depth.checked_add(1).ok_or_else(|| {
            ArtifactIoError::new(ArtifactIoFailureKind::LimitExceeded(
                LimitKind::ArchiveDepth,
            ))
        })?;
    }
    if depth > maximum_depth {
        Err(ArtifactIoError::new(ArtifactIoFailureKind::LimitExceeded(
            LimitKind::ArchiveDepth,
        )))
    } else {
        Ok(())
    }
}

fn admit_archive_name(
    names: &mut BTreeMap<Vec<u8>, ArchiveMemberKind>,
    path: &[u8],
    kind: ArchiveMemberKind,
) -> Result<(), ArtifactIoError> {
    let mut path = path.to_vec();
    if kind == ArchiveMemberKind::Directory && path.ends_with(b"/") {
        path.pop();
    }
    if names.contains_key(&path) {
        return Err(ArtifactIoError::new(ArtifactIoFailureKind::InvalidObject));
    }
    let mut offset = 0_usize;
    while let Some(index) = path[offset..].iter().position(|byte| *byte == b'/') {
        offset = offset
            .checked_add(index)
            .ok_or_else(|| ArtifactIoError::new(ArtifactIoFailureKind::InvalidObject))?;
        if names
            .get(&path[..offset])
            .is_some_and(|existing| *existing == ArchiveMemberKind::File)
        {
            return Err(ArtifactIoError::new(ArtifactIoFailureKind::InvalidObject));
        }
        offset = offset
            .checked_add(1)
            .ok_or_else(|| ArtifactIoError::new(ArtifactIoFailureKind::InvalidObject))?;
    }
    if kind == ArchiveMemberKind::File {
        let mut prefix = path.clone();
        prefix.push(b'/');
        if names
            .range(prefix.clone()..)
            .next()
            .is_some_and(|(candidate, _)| candidate.starts_with(&prefix))
        {
            return Err(ArtifactIoError::new(ArtifactIoFailureKind::InvalidObject));
        }
    }
    names.insert(path, kind);
    Ok(())
}

struct LimitReader<R> {
    inner: R,
    maximum: u64,
    total: u64,
    exceeded: bool,
}

impl<R> LimitReader<R> {
    const fn new(inner: R, maximum: u64) -> Self {
        Self {
            inner,
            maximum,
            total: 0,
            exceeded: false,
        }
    }

    const fn total(&self) -> u64 {
        self.total
    }

    const fn exceeded(&self) -> bool {
        self.exceeded
    }

    fn into_inner(self) -> R {
        self.inner
    }

    const fn get_ref(&self) -> &R {
        &self.inner
    }
}

impl<R: Read> Read for LimitReader<R> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        let remaining = self.maximum.saturating_sub(self.total);
        let allowed = usize::try_from(remaining.saturating_add(1))
            .unwrap_or(usize::MAX)
            .min(output.len());
        let read = self.inner.read(&mut output[..allowed])?;
        self.total = self
            .total
            .checked_add(read as u64)
            .ok_or_else(|| io::Error::other("bounded stream length overflow"))?;
        if self.total > self.maximum {
            self.exceeded = true;
            Err(io::Error::other("bounded stream limit exceeded"))
        } else {
            Ok(read)
        }
    }
}

struct HashingLimitReader<'a> {
    inner: &'a mut File,
    maximum: u64,
    total: u64,
    exceeded: bool,
    hasher: Sha256,
}

impl<'a> HashingLimitReader<'a> {
    fn new(inner: &'a mut File, maximum: u64) -> Self {
        Self {
            inner,
            maximum,
            total: 0,
            exceeded: false,
            hasher: Sha256::new(),
        }
    }

    const fn total(&self) -> u64 {
        self.total
    }

    const fn exceeded(&self) -> bool {
        self.exceeded
    }

    fn finalize(self) -> String {
        hex::encode(self.hasher.finalize())
    }
}

impl Read for HashingLimitReader<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        let remaining = self.maximum.saturating_sub(self.total);
        let allowed = usize::try_from(remaining.saturating_add(1))
            .unwrap_or(usize::MAX)
            .min(output.len());
        let read = self.inner.read(&mut output[..allowed])?;
        self.total = self
            .total
            .checked_add(read as u64)
            .ok_or_else(|| io::Error::other("bounded stream length overflow"))?;
        if self.total > self.maximum {
            self.exceeded = true;
            return Err(io::Error::other("bounded stream limit exceeded"));
        }
        self.hasher.update(&output[..read]);
        Ok(read)
    }
}

struct HashingLimitWriter {
    maximum: u64,
    total: u64,
    exceeded: bool,
    hasher: Sha256,
}

impl HashingLimitWriter {
    fn new(maximum: u64) -> Self {
        Self {
            maximum,
            total: 0,
            exceeded: false,
            hasher: Sha256::new(),
        }
    }

    const fn exceeded(&self) -> bool {
        self.exceeded
    }

    fn finalize(self) -> FileEvidence {
        FileEvidence {
            byte_length: self.total,
            sha256: hex::encode(self.hasher.finalize()),
        }
    }
}

impl Write for HashingLimitWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let next = self
            .total
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| io::Error::other("bounded stream length overflow"))?;
        if next > self.maximum {
            self.exceeded = true;
            return Err(io::Error::other("bounded stream limit exceeded"));
        }
        self.hasher.update(bytes);
        self.total = next;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn split_absolute_file(path: &Path) -> Result<(PathBuf, PathBuf), ArtifactIoError> {
    if !path.is_absolute() {
        return Err(ArtifactIoError::new(ArtifactIoFailureKind::InvalidRequest));
    }
    let parent = path
        .parent()
        .ok_or_else(|| ArtifactIoError::new(ArtifactIoFailureKind::InvalidRequest))?;
    let name = path
        .file_name()
        .ok_or_else(|| ArtifactIoError::new(ArtifactIoFailureKind::InvalidRequest))?;
    Ok((parent.to_path_buf(), PathBuf::from(name)))
}

#[cfg(unix)]
fn open_absolute_directory(path: &Path) -> Result<(File, Vec<DirectoryIdentity>), ArtifactIoError> {
    use rustix::fs::{Mode, OFlags, open, openat};

    if !path.is_absolute() {
        return Err(ArtifactIoError::new(ArtifactIoFailureKind::InvalidRequest));
    }
    let flags =
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::DIRECTORY;
    let root = open("/", flags, Mode::empty())
        .map_err(|_| ArtifactIoError::new(ArtifactIoFailureKind::IoFailure))?;
    let mut current = File::from(root);
    let mut chain = vec![directory_identity(identity(&current)?)];
    for component in path.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(name) => {
                let next = openat(&current, name, flags, Mode::empty())
                    .map_err(|_| ArtifactIoError::new(ArtifactIoFailureKind::InvalidObject))?;
                current = File::from(next);
                chain.push(directory_identity(identity(&current)?));
            }
            _ => {
                return Err(ArtifactIoError::new(ArtifactIoFailureKind::InvalidRequest));
            }
        }
    }
    Ok((current, chain))
}

pub(crate) fn validate_trusted_output_directory(path: &Path) -> Result<(), ArtifactIoError> {
    let _ = open_trusted_output_directory(path)?;
    Ok(())
}

#[cfg(unix)]
fn open_trusted_output_directory(
    path: &Path,
) -> Result<(File, Vec<OutputDirectoryIdentity>), ArtifactIoError> {
    use rustix::fs::{Mode, OFlags, open, openat};

    if !path.is_absolute() {
        return Err(ArtifactIoError::new(ArtifactIoFailureKind::InvalidRequest));
    }
    let flags =
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::DIRECTORY;
    let root = open("/", flags, Mode::empty())
        .map_err(|_| ArtifactIoError::new(ArtifactIoFailureKind::IoFailure))?;
    let mut current = File::from(root);
    let mut chain = Vec::new();
    let root_identity = output_directory_identity(identity(&current)?);
    if !output_directory_is_trusted(root_identity) {
        return Err(ArtifactIoError::new(ArtifactIoFailureKind::InvalidRequest));
    }
    chain.push(root_identity);
    for component in path.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(name) => {
                let next = openat(&current, name, flags, Mode::empty())
                    .map_err(|_| ArtifactIoError::new(ArtifactIoFailureKind::InvalidObject))?;
                current = File::from(next);
                let current_identity = output_directory_identity(identity(&current)?);
                if !output_directory_is_trusted(current_identity) {
                    return Err(ArtifactIoError::new(ArtifactIoFailureKind::InvalidRequest));
                }
                chain.push(current_identity);
            }
            _ => {
                return Err(ArtifactIoError::new(ArtifactIoFailureKind::InvalidRequest));
            }
        }
    }
    Ok((current, chain))
}

#[cfg(not(unix))]
fn open_absolute_directory(
    _path: &Path,
) -> Result<(File, Vec<DirectoryIdentity>), ArtifactIoError> {
    Err(ArtifactIoError::new(
        ArtifactIoFailureKind::UnsupportedPlatform,
    ))
}

#[cfg(not(unix))]
fn open_trusted_output_directory(
    _path: &Path,
) -> Result<(File, Vec<OutputDirectoryIdentity>), ArtifactIoError> {
    Err(ArtifactIoError::new(
        ArtifactIoFailureKind::UnsupportedPlatform,
    ))
}

fn open_relative(
    root: &Path,
    relative: &Path,
    expected: ObjectKind,
) -> Result<OpenedObject, ArtifactIoError> {
    validate_relative(relative)?;
    let (mut current, mut chain) = open_absolute_directory(root)?;
    let components = relative.components().collect::<Vec<_>>();
    let mut object_identity = None;
    for (index, component) in components.iter().enumerate() {
        let Component::Normal(name) = component else {
            return Err(ArtifactIoError::new(ArtifactIoFailureKind::InvalidRequest));
        };
        let object = open_at(&current, name)?;
        let metadata = object
            .metadata()
            .map_err(|_| ArtifactIoError::new(ArtifactIoFailureKind::IoFailure))?;
        let final_component = index + 1 == components.len();
        if (!final_component && !metadata.is_dir())
            || (final_component
                && match expected {
                    ObjectKind::Directory => !metadata.is_dir(),
                    ObjectKind::Regular => !metadata.is_file(),
                })
        {
            return Err(ArtifactIoError::new(ArtifactIoFailureKind::InvalidObject));
        }
        let current_identity = identity_from_metadata(&metadata);
        current = object;
        if metadata.is_dir() {
            chain.push(directory_identity(current_identity));
        }
        object_identity = Some(current_identity);
    }
    let object_identity = object_identity
        .ok_or_else(|| ArtifactIoError::new(ArtifactIoFailureKind::InvalidRequest))?;
    Ok(OpenedObject {
        file: current,
        identity: object_identity,
        chain,
    })
}

#[cfg(unix)]
fn open_at(directory: &File, name: &std::ffi::OsStr) -> Result<File, ArtifactIoError> {
    use rustix::fs::{Mode, OFlags, openat};

    let flags = OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK;
    let descriptor = openat(directory, name, flags, Mode::empty())
        .map_err(|_| ArtifactIoError::new(ArtifactIoFailureKind::InvalidObject))?;
    Ok(File::from(descriptor))
}

#[cfg(not(unix))]
fn open_at(_directory: &File, _name: &std::ffi::OsStr) -> Result<File, ArtifactIoError> {
    Err(ArtifactIoError::new(
        ArtifactIoFailureKind::UnsupportedPlatform,
    ))
}

fn validate_relative(path: &Path) -> Result<(), ArtifactIoError> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        Err(ArtifactIoError::new(ArtifactIoFailureKind::InvalidRequest))
    } else {
        Ok(())
    }
}

fn validate_single_component(name: &OsStr) -> Result<(), ArtifactIoError> {
    let path = Path::new(name);
    let mut components = path.components();
    if matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none() {
        Ok(())
    } else {
        Err(ArtifactIoError::new(ArtifactIoFailureKind::InvalidRequest))
    }
}

#[cfg(unix)]
fn path_byte_len(path: &Path) -> usize {
    use std::os::unix::ffi::OsStrExt as _;
    path.as_os_str().as_bytes().len()
}

#[cfg(not(unix))]
fn path_byte_len(path: &Path) -> usize {
    path.as_os_str().to_string_lossy().len()
}

fn identity(file: &File) -> Result<FileIdentity, ArtifactIoError> {
    let metadata = file
        .metadata()
        .map_err(|_| ArtifactIoError::new(ArtifactIoFailureKind::IoFailure))?;
    Ok(identity_from_metadata(&metadata))
}

const fn directory_identity(identity: FileIdentity) -> DirectoryIdentity {
    DirectoryIdentity {
        device: identity.device,
        inode: identity.inode,
    }
}

const fn permission_mode(identity: FileIdentity) -> u32 {
    identity.mode & 0o7777
}

#[cfg(unix)]
fn identity_from_metadata(metadata: &std::fs::Metadata) -> FileIdentity {
    use std::os::unix::fs::MetadataExt as _;
    FileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
        mode: metadata.mode(),
        owner: metadata.uid(),
        links: metadata.nlink(),
        length: metadata.len(),
        modified_seconds: metadata.mtime(),
        modified_nanoseconds: metadata.mtime_nsec(),
        changed_seconds: metadata.ctime(),
        changed_nanoseconds: metadata.ctime_nsec(),
    }
}

#[cfg(not(unix))]
fn identity_from_metadata(metadata: &std::fs::Metadata) -> FileIdentity {
    FileIdentity {
        device: 0,
        inode: 0,
        mode: 0,
        owner: 0,
        links: 0,
        length: metadata.len(),
        modified_seconds: 0,
        modified_nanoseconds: 0,
        changed_seconds: 0,
        changed_nanoseconds: 0,
    }
}

pub(crate) fn self_test() -> Result<(), String> {
    #[cfg(unix)]
    {
        self_test_suite()?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        Err(ArtifactIoError::new(ArtifactIoFailureKind::UnsupportedPlatform).to_string())
    }
}

#[cfg(unix)]
fn self_test_suite() -> Result<(), String> {
    let directory =
        tempfile::tempdir().map_err(|_| "safe artifact I/O self-test setup failed".to_owned())?;
    let root = directory
        .path()
        .canonicalize()
        .map_err(|_| "safe artifact I/O self-test setup failed".to_owned())?;
    fs::write(root.join("input"), b"step-293-safe-artifact")
        .map_err(|_| "safe artifact I/O self-test setup failed".to_owned())?;

    let bytes = read_regular(&root, Path::new("input"), 64).map_err(|error| error.to_string())?;
    let evidence =
        hash_regular(&root, Path::new("input"), 64).map_err(|error| error.to_string())?;
    if bytes != b"step-293-safe-artifact"
        || hash_regular(&root, Path::new("input"), 64).map_err(|error| error.to_string())?
            != evidence
    {
        return Err("safe artifact I/O self-test evidence failed".to_owned());
    }
    let traversal = traverse_regular_files(
        &root,
        TraversalLimits {
            max_entries: 4,
            max_files: 4,
            max_total_bytes: 256,
            max_file_bytes: 128,
            max_depth: 2,
            max_path_bytes: 128,
        },
        &[],
    )
    .map_err(|error| error.to_string())?;
    if traversal.entry_count() != 1
        || traversal.total_bytes() != evidence.byte_length
        || traversal
            .read(&traversal.files()[0], 64)
            .map_err(|error| error.to_string())?
            != bytes
        || traversal
            .read_evidenced(&traversal.files()[0], 64)
            .map_err(|error| error.to_string())?
            != (bytes.clone(), evidence.clone())
    {
        return Err("safe artifact I/O self-test traversal failed".to_owned());
    }

    let archive_path = root.join("archive.tar.gz");
    let output = File::create(&archive_path)
        .map_err(|_| "safe artifact I/O self-test setup failed".to_owned())?;
    let encoder = GzBuilder::new().write(output, Compression::fast());
    let mut archive = TarBuilder::new(encoder);
    let mut header = TarHeader::new_gnu();
    header.set_size(bytes.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    archive
        .append_data(&mut header, "payload", bytes.as_slice())
        .map_err(|_| "safe artifact I/O self-test archive failed".to_owned())?;
    let encoder = archive
        .into_inner()
        .map_err(|_| "safe artifact I/O self-test archive failed".to_owned())?;
    encoder
        .finish()
        .map_err(|_| "safe artifact I/O self-test archive failed".to_owned())?;
    let admitted = admit_tar_gzip_path(
        &archive_path,
        TarGzipLimits {
            max_compressed_bytes: 65_536,
            max_expanded_bytes: 65_536,
            max_members: 4,
            max_member_bytes: 256,
            max_payload_bytes: 256,
            max_depth: 4,
            max_path_bytes: 128,
        },
    )
    .map_err(|error| error.to_string())?;
    if admitted.member_count != 1 || admitted.payload_bytes != bytes.len() as u64 {
        return Err("safe artifact I/O self-test archive failed".to_owned());
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use std::fs::{self, OpenOptions};
    use std::os::unix::fs::{FileTypeExt as _, PermissionsExt as _, symlink};
    use std::time::Duration;

    use flate2::{Compression, GzBuilder};
    use tar::{Builder, EntryType, Header};
    use tempfile::TempDir;

    use super::*;

    fn root(directory: &TempDir) -> PathBuf {
        directory
            .path()
            .canonicalize()
            .expect("canonical test root")
    }

    fn traversal_limits() -> TraversalLimits {
        TraversalLimits {
            max_entries: 8,
            max_files: 4,
            max_total_bytes: 64,
            max_file_bytes: 32,
            max_depth: 3,
            max_path_bytes: 64,
        }
    }

    fn archive_limits() -> TarGzipLimits {
        TarGzipLimits {
            max_compressed_bytes: 32 * 1024,
            max_expanded_bytes: 64 * 1024,
            max_members: 8,
            max_member_bytes: 8 * 1024,
            max_payload_bytes: 16 * 1024,
            max_depth: 4,
            max_path_bytes: 128,
        }
    }

    #[test]
    fn hard_maximums_reject_invalid_requests() {
        assert_eq!(HARD_MAX_BUFFERED_READ_BYTES, 67_108_864);
        assert_eq!(HARD_MAX_STREAM_FILE_BYTES, 17_179_869_184);
        assert_eq!(HARD_MAX_TRAVERSAL_ENTRIES, 65_536);
        assert_eq!(HARD_MAX_TRAVERSAL_FILES, 65_536);
        assert_eq!(HARD_MAX_TRAVERSAL_TOTAL_BYTES, 68_719_476_736);
        assert_eq!(HARD_MAX_TRAVERSAL_DEPTH, 64);
        assert_eq!(HARD_MAX_PATH_BYTES, 4_096);
        assert_eq!(HARD_MAX_ARCHIVE_COMPRESSED_BYTES, 2_147_483_648);
        assert_eq!(HARD_MAX_ARCHIVE_EXPANDED_BYTES, 17_179_869_184);
        assert_eq!(HARD_MAX_ARCHIVE_MEMBERS, 65_536);
        assert_eq!(HARD_MAX_ARCHIVE_MEMBER_BYTES, 17_179_869_184);
        assert_eq!(HARD_MAX_ARCHIVE_PAYLOAD_BYTES, 17_179_869_184);
        let directory = TempDir::new().expect("tempdir");
        let root = root(&directory);
        fs::write(root.join("input"), b"x").expect("input");
        assert_eq!(
            read_regular(&root, Path::new("input"), HARD_MAX_BUFFERED_READ_BYTES + 1,)
                .expect_err("buffered read hard maximum")
                .kind(),
            ArtifactIoFailureKind::InvalidRequest
        );
        assert_eq!(
            hash_regular(&root, Path::new("input"), HARD_MAX_STREAM_FILE_BYTES + 1,)
                .expect_err("streaming file hard maximum")
                .kind(),
            ArtifactIoFailureKind::InvalidRequest
        );

        for mutate in [
            |limits: &mut TraversalLimits| limits.max_entries = HARD_MAX_TRAVERSAL_ENTRIES + 1,
            |limits: &mut TraversalLimits| limits.max_files = HARD_MAX_TRAVERSAL_FILES + 1,
            |limits: &mut TraversalLimits| {
                limits.max_total_bytes = HARD_MAX_TRAVERSAL_TOTAL_BYTES + 1
            },
            |limits: &mut TraversalLimits| limits.max_file_bytes = HARD_MAX_STREAM_FILE_BYTES + 1,
            |limits: &mut TraversalLimits| limits.max_depth = HARD_MAX_TRAVERSAL_DEPTH + 1,
            |limits: &mut TraversalLimits| limits.max_path_bytes = HARD_MAX_PATH_BYTES + 1,
        ] {
            let mut limits = traversal_limits();
            mutate(&mut limits);
            assert_eq!(
                validate_traversal_limits(limits)
                    .expect_err("traversal hard maximum")
                    .kind(),
                ArtifactIoFailureKind::InvalidRequest
            );
        }

        for mutate in [
            |limits: &mut TarGzipLimits| {
                limits.max_compressed_bytes = HARD_MAX_ARCHIVE_COMPRESSED_BYTES + 1
            },
            |limits: &mut TarGzipLimits| {
                limits.max_expanded_bytes = HARD_MAX_ARCHIVE_EXPANDED_BYTES + 1
            },
            |limits: &mut TarGzipLimits| limits.max_members = HARD_MAX_ARCHIVE_MEMBERS + 1,
            |limits: &mut TarGzipLimits| {
                limits.max_member_bytes = HARD_MAX_ARCHIVE_MEMBER_BYTES + 1
            },
            |limits: &mut TarGzipLimits| {
                limits.max_payload_bytes = HARD_MAX_ARCHIVE_PAYLOAD_BYTES + 1
            },
            |limits: &mut TarGzipLimits| limits.max_depth = HARD_MAX_TRAVERSAL_DEPTH + 1,
            |limits: &mut TarGzipLimits| limits.max_path_bytes = HARD_MAX_PATH_BYTES + 1,
        ] {
            let mut limits = archive_limits();
            mutate(&mut limits);
            assert_eq!(
                validate_archive_limits(limits)
                    .expect_err("archive hard maximum")
                    .kind(),
                ArtifactIoFailureKind::InvalidRequest
            );
        }
    }

    #[test]
    fn bounded_read_hash_and_copy_are_streaming_and_exact() {
        let directory = TempDir::new().expect("tempdir");
        let root = root(&directory);
        fs::write(root.join("input"), b"01234567").expect("write input");

        assert_eq!(
            read_regular(&root, Path::new("input"), 8).expect("read at cap"),
            b"01234567"
        );
        assert_eq!(
            read_regular(&root, Path::new("input"), 7)
                .expect_err("read over cap")
                .kind(),
            ArtifactIoFailureKind::LimitExceeded(LimitKind::FileBytes)
        );
        let evidence = hash_regular(&root, Path::new("input"), 8).expect("hash");
        let output_path = root.join("output");
        let copied = copy_regular_to_new_path(&root.join("input"), &output_path, 8)
            .expect("create-new copy");
        assert_eq!(evidence, copied);
        assert_eq!(fs::read(output_path).expect("output bytes"), b"01234567");
        assert_eq!(
            fs::metadata(root.join("output"))
                .expect("output metadata")
                .permissions()
                .mode()
                & 0o7777,
            0o600
        );

        let failed_output = root.join("failed-output");
        let error = copy_regular_to_new_path_impl(
            &root.join("input"),
            &failed_output,
            8,
            None,
            || {
                fs::rename(root.join("input"), root.join("moved-input")).expect("move copy input");
                fs::write(root.join("input"), b"01234567").expect("replace copy input");
            },
            || {},
        )
        .expect_err("copy binding change");
        assert_eq!(error.kind(), ArtifactIoFailureKind::ChangedDuringRead);
        assert_eq!(
            fs::read(&failed_output).expect("untrusted failed output"),
            b"01234567"
        );
        fs::remove_file(&failed_output).expect("remove untrusted failed output");

        let open_hardlink_output = root.join("open-hardlink-output");
        let open_hardlink_alias = root.join("open-hardlink-alias");
        let error = copy_regular_to_new_path_impl(
            &root.join("input"),
            &open_hardlink_output,
            8,
            None,
            || {
                fs::hard_link(&open_hardlink_output, &open_hardlink_alias)
                    .expect("hardlink open output");
            },
            || {},
        )
        .expect_err("open hardlink race");
        assert_eq!(error.kind(), ArtifactIoFailureKind::ChangedDuringRead);
        assert_eq!(
            fs::read(&open_hardlink_output).expect("untrusted output"),
            b"01234567"
        );
        assert_eq!(
            fs::read(&open_hardlink_alias).expect("hardlink alias"),
            b"01234567"
        );
        fs::remove_file(&open_hardlink_output).expect("remove untrusted output");
        fs::remove_file(&open_hardlink_alias).expect("remove hardlink alias");

        let installed_hardlink_output = root.join("installed-hardlink-output");
        let installed_hardlink_alias = root.join("installed-hardlink-alias");
        let error = copy_regular_to_new_path_impl(
            &root.join("input"),
            &installed_hardlink_output,
            8,
            None,
            || {},
            || {
                fs::hard_link(&installed_hardlink_output, &installed_hardlink_alias)
                    .expect("hardlink installed output");
            },
        )
        .expect_err("installed hardlink race");
        assert_eq!(error.kind(), ArtifactIoFailureKind::ChangedDuringRead);
        assert_eq!(
            fs::read(&installed_hardlink_output).expect("untrusted installed output"),
            b"01234567"
        );
        assert!(installed_hardlink_alias.exists());
        fs::remove_file(&installed_hardlink_output).expect("remove untrusted installed output");
        fs::remove_file(&installed_hardlink_alias).expect("remove installed hardlink alias");

        let mutated_output = root.join("mutated-output");
        let error = copy_regular_to_new_path_impl(
            &root.join("input"),
            &mutated_output,
            8,
            None,
            || {},
            || fs::write(&mutated_output, b"87654321").expect("mutate installed output"),
        )
        .expect_err("post-sync output mutation");
        assert_eq!(error.kind(), ArtifactIoFailureKind::ChangedDuringRead);
        assert_eq!(
            fs::read(&mutated_output).expect("untrusted mutated output"),
            b"87654321"
        );
        fs::remove_file(&mutated_output).expect("remove untrusted mutated output");

        let name_swap_output = root.join("name-swap-output");
        let moved_created_output = root.join("moved-created-output");
        let error = copy_regular_to_new_path_impl(
            &root.join("input"),
            &name_swap_output,
            8,
            None,
            || {
                fs::rename(&name_swap_output, &moved_created_output).expect("move created output");
                fs::write(&name_swap_output, b"unrelated").expect("replacement output");
            },
            || {},
        )
        .expect_err("created name swap");
        assert_eq!(error.kind(), ArtifactIoFailureKind::ChangedDuringRead);
        assert_eq!(
            fs::read(&name_swap_output).expect("unrelated replacement preserved"),
            b"unrelated"
        );
        assert_eq!(
            fs::read(&moved_created_output).expect("moved untrusted output"),
            b"01234567"
        );
        fs::remove_file(&name_swap_output).expect("remove replacement output");
        fs::remove_file(&moved_created_output).expect("remove moved output");

        fs::create_dir(root.join("output-parent")).expect("output parent");
        fs::write(root.join("input"), b"01234567").expect("restore input");
        let rebound_output = root.join("output-parent/installed");
        let error = copy_regular_to_new_path_impl(
            &root.join("input"),
            &rebound_output,
            8,
            None,
            || {
                fs::rename(root.join("output-parent"), root.join("old-output-parent"))
                    .expect("move output parent");
                fs::create_dir(root.join("output-parent")).expect("replace output parent");
            },
            || {},
        )
        .expect_err("output parent replacement");
        assert_eq!(error.kind(), ArtifactIoFailureKind::ChangedDuringRead);
        assert!(!rebound_output.exists());
        assert_eq!(
            fs::read(root.join("old-output-parent/installed"))
                .expect("untrusted output in moved parent"),
            b"01234567"
        );

        fs::set_permissions(&root, fs::Permissions::from_mode(0o777))
            .expect("make output parent untrusted");
        assert_eq!(
            copy_regular_to_new_path(&root.join("input"), &root.join("untrusted-parent"), 8)
                .expect_err("writable output parent rejected")
                .kind(),
            ArtifactIoFailureKind::InvalidRequest
        );
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
            .expect("restore output parent mode");

        let parent_mode_output = root.join("parent-mode-output");
        let error = copy_regular_to_new_path_impl(
            &root.join("input"),
            &parent_mode_output,
            8,
            None,
            || {},
            || {
                fs::set_permissions(&root, fs::Permissions::from_mode(0o777))
                    .expect("change output parent mode");
            },
        )
        .expect_err("output parent security change");
        assert_eq!(error.kind(), ArtifactIoFailureKind::ChangedDuringRead);
        assert!(parent_mode_output.exists());
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
            .expect("restore output parent mode");
        fs::remove_file(&parent_mode_output).expect("remove untrusted parent-mode output");

        let secure_ancestor = root.join("secure-ancestor");
        let nested_parent = secure_ancestor.join("output-parent");
        fs::create_dir(&secure_ancestor).expect("secure ancestor");
        fs::create_dir(&nested_parent).expect("nested output parent");
        let ancestor_mode_output = nested_parent.join("installed");
        let error = copy_regular_to_new_path_impl(
            &root.join("input"),
            &ancestor_mode_output,
            8,
            None,
            || {},
            || {
                fs::set_permissions(&secure_ancestor, fs::Permissions::from_mode(0o777))
                    .expect("change output ancestor mode");
            },
        )
        .expect_err("output ancestor security change");
        assert_eq!(error.kind(), ArtifactIoFailureKind::ChangedDuringRead);
        assert!(ancestor_mode_output.exists());
        fs::set_permissions(&secure_ancestor, fs::Permissions::from_mode(0o755))
            .expect("restore output ancestor mode");
        fs::remove_file(&ancestor_mode_output).expect("remove untrusted ancestor-mode output");
    }

    #[test]
    fn no_follow_admission_rejects_symlink_fifo_and_replacement() {
        let directory = TempDir::new().expect("tempdir");
        let root = root(&directory);
        fs::write(root.join("source"), b"original").expect("source");
        symlink("source", root.join("link")).expect("symlink");
        assert_eq!(
            read_regular(&root, Path::new("link"), 32)
                .expect_err("symlink rejected")
                .kind(),
            ArtifactIoFailureKind::InvalidObject
        );

        let fifo = root.join("fifo");
        let request = crate::bounded_process::ProcessRequest::new("/usr/bin/mkfifo")
            .arg(fifo.as_os_str())
            .deadline(Duration::from_secs(5))
            .output_limits(1024, 1024);
        let output = crate::bounded_process::run(&request).expect("bounded mkfifo");
        assert!(output.status().success());
        assert!(
            fs::symlink_metadata(&fifo)
                .expect("fifo metadata")
                .file_type()
                .is_fifo()
        );
        assert_eq!(
            read_regular(&root, Path::new("fifo"), 32)
                .expect_err("fifo rejected")
                .kind(),
            ArtifactIoFailureKind::InvalidObject
        );

        let error = read_regular_impl(&root, Path::new("source"), 32, None, || {
            fs::rename(root.join("source"), root.join("old")).expect("move admitted file");
            fs::write(root.join("source"), b"original").expect("replacement");
        })
        .expect_err("replacement rejected");
        assert_eq!(error.kind(), ArtifactIoFailureKind::ChangedDuringRead);

        fs::write(root.join("mutable"), b"same-size").expect("mutable");
        let error = read_regular_impl(&root, Path::new("mutable"), 32, None, || {
            fs::write(root.join("mutable"), b"new-bytes").expect("in-place mutation");
        })
        .expect_err("in-place mutation rejected");
        assert_eq!(error.kind(), ArtifactIoFailureKind::ChangedDuringRead);

        fs::create_dir(root.join("parent")).expect("parent");
        fs::write(root.join("parent/member"), b"member").expect("member");
        let error = read_regular_impl(&root, Path::new("parent/member"), 32, None, || {
            fs::rename(root.join("parent"), root.join("old-parent")).expect("move parent");
            fs::create_dir(root.join("parent")).expect("replacement parent");
            fs::write(root.join("parent/member"), b"member").expect("replacement member");
        })
        .expect_err("parent replacement rejected");
        assert_eq!(error.kind(), ArtifactIoFailureKind::ChangedDuringRead);
    }

    #[test]
    fn traversal_enforces_type_count_byte_and_depth_bounds() {
        let directory = TempDir::new().expect("tempdir");
        let root = root(&directory);
        fs::create_dir(root.join("a")).expect("a");
        fs::write(root.join("a/one"), b"1").expect("one");
        fs::write(root.join("two"), b"22").expect("two");
        let snapshot = traverse_regular_files(&root, traversal_limits(), &[]).expect("traverse");
        assert_eq!(snapshot.entry_count(), 3);
        assert_eq!(snapshot.total_bytes(), 3);
        assert_eq!(
            snapshot
                .files()
                .iter()
                .map(|file| file.relative_path().to_path_buf())
                .collect::<Vec<_>>(),
            [PathBuf::from("a/one"), PathBuf::from("two")]
        );
        assert_eq!(
            snapshot
                .read(&snapshot.files()[0], 1)
                .expect("snapshot read"),
            b"1"
        );
        let (bytes, evidenced) = snapshot
            .read_evidenced(&snapshot.files()[0], 1)
            .expect("snapshot evidenced read");
        assert_eq!(bytes, b"1");
        assert_eq!(
            evidenced,
            snapshot
                .hash(&snapshot.files()[0], 1)
                .expect("snapshot hash")
        );
        let output_directory = TempDir::new().expect("snapshot copy output");
        let output_root = output_directory
            .path()
            .canonicalize()
            .expect("canonical snapshot copy output");
        assert_eq!(
            snapshot
                .copy_to_new_path(&snapshot.files()[0], &output_root.join("copy"), 1)
                .expect("snapshot copy"),
            evidenced
        );
        assert_eq!(
            fs::read(output_root.join("copy")).expect("copied bytes"),
            b"1"
        );

        let late_error = traverse_regular_files_impl(&root, traversal_limits(), &[], || {
            fs::write(root.join("late"), b"late").expect("late insertion");
        })
        .expect_err("late insertion is detected");
        assert_eq!(late_error.kind(), ArtifactIoFailureKind::ChangedDuringRead);
        fs::remove_file(root.join("late")).expect("remove late insertion");

        fs::create_dir(root.join("excluded")).expect("excluded directory");
        let excluded = traverse_regular_files(&root, traversal_limits(), &["excluded"])
            .expect("excluded snapshot");
        fs::rename(root.join("excluded"), root.join("old-excluded"))
            .expect("move excluded directory");
        fs::create_dir(root.join("excluded")).expect("replace excluded directory");
        assert_eq!(
            excluded
                .revalidate()
                .expect_err("excluded binding change")
                .kind(),
            ArtifactIoFailureKind::ChangedDuringRead
        );

        let mut one_entry = traversal_limits();
        one_entry.max_entries = 1;
        assert_eq!(
            traverse_regular_files(&root, one_entry, &[])
                .expect_err("entry cap")
                .kind(),
            ArtifactIoFailureKind::LimitExceeded(LimitKind::TraversalEntries)
        );
        let mut one_file = traversal_limits();
        one_file.max_files = 1;
        assert_eq!(
            traverse_regular_files(&root, one_file, &[])
                .expect_err("file cap")
                .kind(),
            ArtifactIoFailureKind::LimitExceeded(LimitKind::TraversalFiles)
        );
        let mut one_file_byte = traversal_limits();
        one_file_byte.max_file_bytes = 1;
        assert_eq!(
            traverse_regular_files(&root, one_file_byte, &[])
                .expect_err("per-file cap")
                .kind(),
            ArtifactIoFailureKind::LimitExceeded(LimitKind::FileBytes)
        );
        let mut short_path = traversal_limits();
        short_path.max_path_bytes = 2;
        assert_eq!(
            traverse_regular_files(&root, short_path, &[])
                .expect_err("path cap")
                .kind(),
            ArtifactIoFailureKind::LimitExceeded(LimitKind::TraversalPathBytes)
        );
        let mut two_bytes = traversal_limits();
        two_bytes.max_total_bytes = 2;
        assert_eq!(
            traverse_regular_files(&root, two_bytes, &[])
                .expect_err("byte cap")
                .kind(),
            ArtifactIoFailureKind::LimitExceeded(LimitKind::TraversalTotalBytes)
        );
        let mut no_depth = traversal_limits();
        no_depth.max_depth = 1;
        fs::create_dir(root.join("a/deep")).expect("deep");
        fs::write(root.join("a/deep/file"), b"x").expect("deep file");
        assert_eq!(
            traverse_regular_files(&root, no_depth, &[])
                .expect_err("depth cap")
                .kind(),
            ArtifactIoFailureKind::LimitExceeded(LimitKind::TraversalDepth)
        );
    }

    fn write_archive(path: &Path, members: &[(&str, EntryType, &[u8])]) {
        let output = File::create(path).expect("archive output");
        let encoder = GzBuilder::new().write(output, Compression::fast());
        let mut builder = Builder::new(encoder);
        for (name, kind, contents) in members {
            let mut header = Header::new_gnu();
            header.set_entry_type(*kind);
            header.set_mode(0o644);
            header.set_size(contents.len() as u64);
            header.set_cksum();
            builder
                .append_data(&mut header, name, *contents)
                .expect("archive member");
        }
        let encoder = builder.into_inner().expect("finish tar");
        encoder.finish().expect("finish gzip");
    }

    fn write_raw_named_archive(path: &Path, name: &[u8], contents: &[u8]) {
        assert!(name.len() < 100);
        let output = File::create(path).expect("archive output");
        let encoder = GzBuilder::new().write(output, Compression::fast());
        let mut builder = Builder::new(encoder);
        let mut header = Header::new_gnu();
        header.as_mut_bytes()[..name.len()].copy_from_slice(name);
        header.set_entry_type(EntryType::Regular);
        header.set_mode(0o644);
        header.set_size(contents.len() as u64);
        header.set_cksum();
        builder
            .append(&header, contents)
            .expect("raw archive member");
        let encoder = builder.into_inner().expect("finish tar");
        encoder.finish().expect("finish gzip");
    }

    fn write_single_terminator_archive(path: &Path) {
        let mut header = Header::new_gnu();
        header.set_path("one").expect("path");
        header.set_entry_type(EntryType::Regular);
        header.set_mode(0o644);
        header.set_size(3);
        header.set_cksum();
        let mut bytes = Vec::from(header.as_bytes());
        bytes.extend_from_slice(b"one");
        bytes.resize(1536, 0);
        let output = File::create(path).expect("archive output");
        let mut encoder = GzBuilder::new().write(output, Compression::fast());
        encoder.write_all(&bytes).expect("write raw tar");
        encoder.finish().expect("finish gzip");
    }

    #[test]
    fn tar_gzip_admission_is_parse_only_and_bounded() {
        let directory = TempDir::new().expect("tempdir");
        let root = root(&directory);
        let valid = root.join("valid.tar.gz");
        write_archive(&valid, &[("bin/tool", EntryType::Regular, b"payload")]);
        let evidence = admit_tar_gzip_path(&valid, archive_limits()).expect("valid archive");
        assert_eq!(evidence.member_count, 1);
        assert_eq!(evidence.payload_bytes, 7);
        let valid_bytes = fs::read(&valid).expect("valid archive bytes");
        assert_eq!(evidence.compressed.byte_length, valid_bytes.len() as u64);
        assert_eq!(
            evidence.compressed.sha256,
            hex::encode(Sha256::digest(&valid_bytes))
        );
        assert!(!root.join("bin").exists());

        for (name, flags) in [
            ("optional-header.tar.gz", 0x08),
            ("reserved-header.tar.gz", 0x20),
        ] {
            let path = root.join(name);
            let mut bytes = valid_bytes.clone();
            bytes[3] = flags;
            fs::write(&path, bytes).expect("write rejected gzip header");
            assert_eq!(
                admit_tar_gzip_path(&path, archive_limits())
                    .expect_err("non-minimal gzip header rejected")
                    .kind(),
                ArtifactIoFailureKind::InvalidObject
            );
        }

        let symlink_archive = root.join("symlink.tar.gz");
        write_archive(
            &symlink_archive,
            &[("escape", EntryType::Symlink, b"target")],
        );
        assert_eq!(
            admit_tar_gzip_path(&symlink_archive, archive_limits())
                .expect_err("special member rejected")
                .kind(),
            ArtifactIoFailureKind::InvalidObject
        );

        let escape_archive = root.join("escape.tar.gz");
        write_raw_named_archive(&escape_archive, b"../escape", b"payload");
        assert_eq!(
            admit_tar_gzip_path(&escape_archive, archive_limits())
                .expect_err("escape rejected")
                .kind(),
            ArtifactIoFailureKind::InvalidObject
        );

        let bomb = root.join("bomb.tar.gz");
        write_archive(&bomb, &[("large", EntryType::Regular, &[0_u8; 8192])]);
        let mut bounded = archive_limits();
        bounded.max_expanded_bytes = 1024;
        assert_eq!(
            admit_tar_gzip_path(&bomb, bounded)
                .expect_err("expanded cap")
                .kind(),
            ArtifactIoFailureKind::LimitExceeded(LimitKind::ArchiveExpandedBytes)
        );

        let mut compressed = archive_limits();
        compressed.max_compressed_bytes = fs::metadata(&valid).expect("valid metadata").len() - 1;
        assert_eq!(
            admit_tar_gzip_path(&valid, compressed)
                .expect_err("compressed cap")
                .kind(),
            ArtifactIoFailureKind::LimitExceeded(LimitKind::ArchiveCompressedBytes)
        );

        let two_members = root.join("two-members.tar.gz");
        write_archive(
            &two_members,
            &[
                ("one", EntryType::Regular, b"1"),
                ("two", EntryType::Regular, b"2"),
            ],
        );
        let mut one_member = archive_limits();
        one_member.max_members = 1;
        assert_eq!(
            admit_tar_gzip_path(&two_members, one_member)
                .expect_err("member count cap")
                .kind(),
            ArtifactIoFailureKind::LimitExceeded(LimitKind::ArchiveMembers)
        );

        let mut one_member_byte = archive_limits();
        one_member_byte.max_member_bytes = 1;
        assert_eq!(
            admit_tar_gzip_path(&valid, one_member_byte)
                .expect_err("member byte cap")
                .kind(),
            ArtifactIoFailureKind::LimitExceeded(LimitKind::ArchiveMemberBytes)
        );

        let mut one_payload_byte = archive_limits();
        one_payload_byte.max_payload_bytes = 1;
        assert_eq!(
            admit_tar_gzip_path(&two_members, one_payload_byte)
                .expect_err("payload cap")
                .kind(),
            ArtifactIoFailureKind::LimitExceeded(LimitKind::ArchivePayloadBytes)
        );

        let deep = root.join("deep.tar.gz");
        write_archive(&deep, &[("a/b/c/d/e", EntryType::Regular, b"x")]);
        assert_eq!(
            admit_tar_gzip_path(&deep, archive_limits())
                .expect_err("archive depth cap")
                .kind(),
            ArtifactIoFailureKind::LimitExceeded(LimitKind::ArchiveDepth)
        );

        let long_path = root.join("long-path.tar.gz");
        write_archive(
            &long_path,
            &[("a-long-member-name", EntryType::Regular, b"x")],
        );
        let mut short_path = archive_limits();
        short_path.max_path_bytes = 4;
        assert_eq!(
            admit_tar_gzip_path(&long_path, short_path)
                .expect_err("archive path cap")
                .kind(),
            ArtifactIoFailureKind::LimitExceeded(LimitKind::ArchivePathBytes)
        );
    }

    #[test]
    fn archive_rejects_duplicates_prefix_conflicts_and_concatenation() {
        let directory = TempDir::new().expect("tempdir");
        let root = root(&directory);
        let duplicate = root.join("duplicate.tar.gz");
        write_archive(
            &duplicate,
            &[
                ("same", EntryType::Regular, b"one"),
                ("same", EntryType::Regular, b"two"),
            ],
        );
        assert_eq!(
            admit_tar_gzip_path(&duplicate, archive_limits())
                .expect_err("duplicate rejected")
                .kind(),
            ArtifactIoFailureKind::InvalidObject
        );

        let prefix = root.join("prefix.tar.gz");
        write_archive(
            &prefix,
            &[
                ("a", EntryType::Regular, b"one"),
                ("a/b", EntryType::Regular, b"two"),
            ],
        );
        assert_eq!(
            admit_tar_gzip_path(&prefix, archive_limits())
                .expect_err("prefix rejected")
                .kind(),
            ArtifactIoFailureKind::InvalidObject
        );

        let first = root.join("first.tar.gz");
        let second = root.join("second.tar.gz");
        write_archive(&first, &[("one", EntryType::Regular, b"one")]);
        write_archive(&second, &[("two", EntryType::Regular, b"two")]);
        let second_bytes = fs::read(&second).expect("second bytes");
        OpenOptions::new()
            .append(true)
            .open(&first)
            .expect("append archive")
            .write_all(&second_bytes)
            .expect("concatenate");
        assert_eq!(
            admit_tar_gzip_path(&first, archive_limits())
                .expect_err("concatenated gzip rejected")
                .kind(),
            ArtifactIoFailureKind::MalformedArchive
        );

        let trailing_slash = root.join("trailing-slash.tar.gz");
        write_raw_named_archive(&trailing_slash, b"file/", b"payload");
        assert_eq!(
            admit_tar_gzip_path(&trailing_slash, archive_limits())
                .expect_err("file trailing slash rejected")
                .kind(),
            ArtifactIoFailureKind::InvalidObject
        );

        let truncated = root.join("truncated-terminator.tar.gz");
        write_single_terminator_archive(&truncated);
        assert_eq!(
            admit_tar_gzip_path(&truncated, archive_limits())
                .expect_err("single terminator rejected")
                .kind(),
            ArtifactIoFailureKind::MalformedArchive
        );

        let malformed = root.join("malformed.tar.gz");
        fs::write(&malformed, b"not a gzip stream").expect("malformed archive");
        assert_eq!(
            admit_tar_gzip_path(&malformed, archive_limits())
                .expect_err("malformed gzip rejected")
                .kind(),
            ArtifactIoFailureKind::MalformedArchive
        );

        let gzip_truncated = root.join("gzip-truncated.tar.gz");
        let mut truncated_bytes = fs::read(&second).expect("archive bytes");
        truncated_bytes.truncate(truncated_bytes.len() - 4);
        fs::write(&gzip_truncated, truncated_bytes).expect("truncated gzip");
        assert_eq!(
            admit_tar_gzip_path(&gzip_truncated, archive_limits())
                .expect_err("truncated gzip rejected")
                .kind(),
            ArtifactIoFailureKind::MalformedArchive
        );

        for (index, name) in [b"/absolute".as_slice(), b"./dot", b"a//b", b"a\\b"]
            .into_iter()
            .enumerate()
        {
            let path = root.join(format!("escape-{index}.tar.gz"));
            write_raw_named_archive(&path, name, b"payload");
            assert_eq!(
                admit_tar_gzip_path(&path, archive_limits())
                    .expect_err("nonportable path rejected")
                    .kind(),
                ArtifactIoFailureKind::InvalidObject
            );
        }

        for (index, kind) in [
            EntryType::Symlink,
            EntryType::Link,
            EntryType::Fifo,
            EntryType::Char,
            EntryType::Block,
            EntryType::Continuous,
            EntryType::GNULongName,
            EntryType::GNULongLink,
            EntryType::GNUSparse,
            EntryType::XGlobalHeader,
            EntryType::XHeader,
            EntryType::new(b'Z'),
        ]
        .into_iter()
        .enumerate()
        {
            let path = root.join(format!("special-{index}.tar.gz"));
            write_archive(&path, &[("special", kind, b"")]);
            assert!(
                admit_tar_gzip_path(&path, archive_limits()).is_err(),
                "special archive member was admitted"
            );
        }
    }

    #[test]
    fn hardlinked_regular_inputs_are_rejected() {
        let directory = TempDir::new().expect("tempdir");
        let root = root(&directory);
        fs::write(root.join("original"), b"bytes").expect("original");
        fs::hard_link(root.join("original"), root.join("alias")).expect("hard link");
        assert_eq!(
            read_regular(&root, Path::new("original"), 8)
                .expect_err("hardlink rejected")
                .kind(),
            ArtifactIoFailureKind::InvalidObject
        );
        assert_eq!(
            traverse_regular_files(&root, traversal_limits(), &[])
                .expect_err("traversal hardlink rejected")
                .kind(),
            ArtifactIoFailureKind::InvalidObject
        );
    }

    #[test]
    fn diagnostics_are_redacted() {
        let error = read_regular(
            Path::new("/definitely-not-present-sensitive-root"),
            Path::new("secret-token"),
            1,
        )
        .expect_err("missing input");
        let diagnostic = format!("{error}");
        assert!(!diagnostic.contains("sensitive"));
        assert!(!diagnostic.contains("secret"));
    }
}

#[cfg(all(test, unix))]
mod step_294_tests {
    use std::fs::{self, File};
    use std::io::Write as _;
    use std::os::unix::fs::PermissionsExt as _;
    use std::path::{Path, PathBuf};

    use flate2::{Compression, GzBuilder};
    use sha2::{Digest as _, Sha256};
    use tar::{Builder, EntryType, Header};
    use tempfile::TempDir;

    use super::*;

    const REVIEWED_FLATE2_LOCK: &str = concat!(
        "name = \"flate2\"\n",
        "version = \"1.1.9\"\n",
        "source = \"registry+https://github.com/rust-lang/crates.io-index\"\n",
        "checksum = \"843fba2746e448b37e26a819579957415c8cef339bf08564fe8b7ddbd959573c\"",
    );
    const REVIEWED_MINIZ_OXIDE_LOCK: &str = concat!(
        "name = \"miniz_oxide\"\n",
        "version = \"0.8.9\"\n",
        "source = \"registry+https://github.com/rust-lang/crates.io-index\"\n",
        "checksum = \"1fa76a2c86f704bdb222d66965fb3d63269ce38518b83cb0575fca855ebb6316\"",
    );
    const CANONICAL_ARCHIVE_BYTE_LENGTH: u64 = 147;
    const CANONICAL_ARCHIVE_SHA256: &str =
        "6073c70a98ff9ab610085ed74b7886811da3fe64e9ec2771464300a94befd0fb";

    fn root(directory: &TempDir) -> PathBuf {
        directory
            .path()
            .canonicalize()
            .expect("canonical Step 294 test root")
    }

    fn trusted_tempdir(prefix: &str) -> TempDir {
        let current = std::env::current_dir()
            .expect("Step 294 current directory")
            .canonicalize()
            .expect("canonical Step 294 current directory");
        open_trusted_output_directory(&current).expect("trusted Step 294 current directory");
        let mut builder = tempfile::Builder::new();
        builder
            .prefix(prefix)
            .permissions(fs::Permissions::from_mode(0o700));
        let directory = builder
            .tempdir_in(current)
            .expect("private Step 294 tempdir");
        let canonical = root(&directory);
        let (descriptor, _) =
            open_trusted_output_directory(&canonical).expect("trusted private Step 294 tempdir");
        let actual = identity(&descriptor).expect("Step 294 tempdir identity");
        assert_eq!(actual.owner, rustix::process::geteuid().as_raw());
        assert_eq!(permission_mode(actual), 0o700);
        directory
    }

    fn archive_limits() -> TarGzipLimits {
        TarGzipLimits {
            max_compressed_bytes: 64 * 1024,
            max_expanded_bytes: 128 * 1024,
            max_members: 8,
            max_member_bytes: 32 * 1024,
            max_payload_bytes: 64 * 1024,
            max_depth: 4,
            max_path_bytes: 128,
        }
    }

    fn write_deterministic_archive(path: &Path, payload: &[u8], extra_terminator: bool) {
        let output = File::create(path).expect("Step 294 archive output");
        let encoder = GzBuilder::new()
            .mtime(0)
            .operating_system(255)
            .write(output, Compression::new(CANONICAL_GZIP_COMPRESSION_LEVEL));
        let mut builder = Builder::new(encoder);

        let mut directory = Header::new_gnu();
        directory
            .set_path("snapshot/")
            .expect("Step 294 directory path");
        directory.set_entry_type(EntryType::Directory);
        directory.set_mode(0o755);
        directory.set_uid(0);
        directory.set_gid(0);
        directory.set_mtime(0);
        directory.set_size(0);
        directory.set_cksum();
        builder
            .append(&directory, &[][..])
            .expect("Step 294 directory member");

        let mut file = Header::new_gnu();
        file.set_path("snapshot/data.json")
            .expect("Step 294 file path");
        file.set_entry_type(EntryType::Regular);
        file.set_mode(0o644);
        file.set_uid(0);
        file.set_gid(0);
        file.set_mtime(0);
        file.set_size(payload.len() as u64);
        file.set_cksum();
        builder
            .append(&file, payload)
            .expect("Step 294 file member");

        let mut encoder = builder.into_inner().expect("finish Step 294 tar");
        if extra_terminator {
            encoder
                .write_all(&[0_u8; TAR_BLOCK_BYTES])
                .expect("append noncanonical tar terminator");
        }
        encoder.finish().expect("finish Step 294 gzip");
    }

    #[test]
    fn canonical_tar_gzip_is_exactly_reencoded_before_admission() {
        let directory = trusted_tempdir("radroots-step-294-source-");
        let root = root(&directory);
        let canonical = root.join("canonical.tar.gz");
        let noncanonical = root.join("noncanonical.tar.gz");
        let payload = br#"{"schema":"radroots.step-294.test.v1"}"#;
        write_deterministic_archive(&canonical, payload, false);
        write_deterministic_archive(&noncanonical, payload, true);

        let evidence = admit_tar_gzip_relative(
            &root,
            Path::new("canonical.tar.gz"),
            archive_limits(),
            None,
            TarGzipPolicy::DeterministicSnapshot,
        )
        .expect("exact canonical archive admitted");
        let cargo_lock = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.lock"));
        assert_eq!(cargo_lock.matches(REVIEWED_FLATE2_LOCK).count(), 1);
        assert_eq!(cargo_lock.matches(REVIEWED_MINIZ_OXIDE_LOCK).count(), 1);
        assert_eq!(
            evidence.compressed.byte_length,
            CANONICAL_ARCHIVE_BYTE_LENGTH
        );
        assert_eq!(evidence.compressed.sha256, CANONICAL_ARCHIVE_SHA256);
        assert_eq!(evidence.member_count, 2);
        assert_eq!(evidence.payload_bytes, payload.len() as u64);
        assert_eq!(
            evidence.compressed.sha256,
            hex::encode(Sha256::digest(
                fs::read(&canonical).expect("canonical bytes")
            ))
        );

        admit_tar_gzip_path(&noncanonical, archive_limits())
            .expect("generic parser accepts safe extended terminator");
        assert_eq!(
            admit_tar_gzip_relative(
                &root,
                Path::new("noncanonical.tar.gz"),
                archive_limits(),
                None,
                TarGzipPolicy::DeterministicSnapshot,
            )
            .expect_err("byte-noncanonical archive rejected")
            .kind(),
            ArtifactIoFailureKind::InvalidObject
        );
    }

    #[test]
    fn materialization_retains_exact_member_and_parent_bindings() {
        let directory = trusted_tempdir("radroots-step-294-source-");
        let source_root = root(&directory);
        let archive = source_root.join("canonical.tar.gz");
        let payload = br#"{"snapshot":"immutable"}"#;
        write_deterministic_archive(&archive, payload, false);

        let parent = trusted_tempdir("radroots-step-294-materialization-");
        let parent_root = root(&parent);
        let materialized = materialize_tar_gzip_relative(
            &source_root,
            Path::new("canonical.tar.gz"),
            &parent_root,
            archive_limits(),
            None,
        )
        .expect("descriptor-bound materialization");
        let snapshot = materialized.snapshot();
        assert_eq!(snapshot.entry_count(), 2);
        assert_eq!(snapshot.files().len(), 1);
        assert_eq!(snapshot.total_bytes(), payload.len() as u64);
        assert_eq!(
            snapshot
                .hash(&snapshot.files()[0], payload.len() as u64)
                .expect("bound member hash")
                .sha256,
            hex::encode(Sha256::digest(payload))
        );
        materialized
            .revalidate()
            .expect("unchanged materialization");

        fs::write(materialized.root().join("snapshot/data.json"), b"changed")
            .expect("mutate materialized member");
        assert_eq!(
            materialized
                .revalidate()
                .expect_err("member mutation invalidates materialization")
                .kind(),
            ArtifactIoFailureKind::ChangedDuringRead
        );
    }
}
