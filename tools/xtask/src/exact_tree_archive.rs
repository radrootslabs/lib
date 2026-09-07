use std::{
    collections::BTreeSet,
    fs,
    io::{Cursor, Read as _},
    path::{Component, Path},
    process::{Command, Stdio},
};

use sha2::{Digest as _, Sha256};
use tar::{Builder, EntryType, Header};

const MAX_TREE_LIST_BYTES: usize = 8 * 1024 * 1024;
const MAX_SOURCE_MEMBER_BYTES: usize = 64 * 1024 * 1024;
const MAX_SOURCE_TREE_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_SOURCE_MEMBERS: usize = 65_536;
const MAX_USTAR_PATH_BYTES: usize = 255;
const ARCHIVE_MODE: u32 = 0o644;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExactTreeArchiveEvidence {
    pub(crate) byte_length: u64,
    pub(crate) sha256: String,
    pub(crate) members: u64,
    pub(crate) payload_bytes: u64,
}

#[derive(Debug)]
struct TreeMember {
    path: String,
    object_id: String,
    mode: u32,
}

#[derive(Debug)]
struct WrittenMember {
    path: String,
    mode: u32,
    byte_length: u64,
    sha256: String,
}

pub(crate) fn commit_timestamp(root: &Path, revision: &str) -> Result<u64, String> {
    validate_root_and_revision(root, revision)?;
    let output = git_output(root, &["show", "-s", "--format=%ct", revision], 64)?;
    let value = std::str::from_utf8(&output)
        .map_err(|_| "exact-tree commit timestamp is not UTF-8".to_owned())?
        .trim()
        .parse::<u64>()
        .map_err(|_| "exact-tree commit timestamp is invalid".to_owned())?;
    if value == 0 {
        Err("exact-tree commit timestamp is invalid".to_owned())
    } else {
        Ok(value)
    }
}

pub(crate) fn create(
    root: &Path,
    revision: &str,
    output: &Path,
    mtime: u64,
) -> Result<ExactTreeArchiveEvidence, String> {
    validate_root_and_revision(root, revision)?;
    let output_parent = output
        .parent()
        .ok_or_else(|| "exact-tree archive output is invalid".to_owned())?;
    if !output.is_absolute()
        || fs::canonicalize(output_parent)
            .map_err(|_| "exact-tree archive output parent is invalid".to_owned())?
            != output_parent
        || mtime == 0
        || output.exists()
    {
        return Err("exact-tree archive request is invalid".to_owned());
    }
    let members = tree_members(root, revision)?;
    let output_file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(output)
        .map_err(|_| "exact-tree archive output could not be created".to_owned())?;
    let mut archive = Builder::new(output_file);
    let mut payload_bytes = 0_u64;
    let mut written = Vec::with_capacity(members.len());
    for member in &members {
        let contents = git_output(
            root,
            &["cat-file", "blob", member.object_id.as_str()],
            MAX_SOURCE_MEMBER_BYTES,
        )?;
        payload_bytes = payload_bytes
            .checked_add(contents.len() as u64)
            .ok_or_else(|| "exact-tree archive payload is too large".to_owned())?;
        if payload_bytes > MAX_SOURCE_TREE_BYTES {
            return Err("exact-tree archive payload is too large".to_owned());
        }
        let mut header = Header::new_ustar();
        header
            .set_path(&member.path)
            .map_err(|_| "exact-tree archive path is not representable in ustar".to_owned())?;
        header.set_entry_type(EntryType::Regular);
        header.set_size(contents.len() as u64);
        header.set_mode(member.mode);
        header.set_uid(0);
        header.set_gid(0);
        header.set_mtime(mtime);
        header
            .set_username("")
            .map_err(|_| "exact-tree archive username is invalid".to_owned())?;
        header
            .set_groupname("")
            .map_err(|_| "exact-tree archive group name is invalid".to_owned())?;
        header.set_cksum();
        archive
            .append(&header, Cursor::new(&contents))
            .map_err(|_| "exact-tree archive member could not be written".to_owned())?;
        written.push(WrittenMember {
            path: member.path.clone(),
            mode: member.mode,
            byte_length: contents.len() as u64,
            sha256: hex::encode(Sha256::digest(&contents)),
        });
    }
    let output_file = archive
        .into_inner()
        .map_err(|_| "exact-tree archive could not be finalized".to_owned())?;
    output_file
        .sync_all()
        .map_err(|_| "exact-tree archive could not be synchronized".to_owned())?;
    set_mode(output)?;
    let bytes = fs::read(output).map_err(|_| "exact-tree archive could not be read".to_owned())?;
    validate_archive_bytes(&bytes, &written, mtime)?;
    Ok(ExactTreeArchiveEvidence {
        byte_length: bytes.len() as u64,
        sha256: hex::encode(Sha256::digest(&bytes)),
        members: written.len() as u64,
        payload_bytes,
    })
}

pub(crate) fn read_blob(
    root: &Path,
    revision: &str,
    path: &str,
    maximum: usize,
) -> Result<Vec<u8>, String> {
    if maximum == 0 || maximum > MAX_SOURCE_MEMBER_BYTES || !valid_archive_path(path) {
        return Err("exact-tree blob request is invalid".to_owned());
    }
    validate_root_and_revision(root, revision)?;
    let member = tree_members(root, revision)?
        .into_iter()
        .find(|member| member.path == path)
        .ok_or_else(|| "exact-tree blob is absent".to_owned())?;
    git_output(root, &["cat-file", "blob", &member.object_id], maximum)
}

fn validate_root_and_revision(root: &Path, revision: &str) -> Result<(), String> {
    if !root.is_absolute()
        || fs::canonicalize(root).map_err(|_| "exact-tree root is invalid".to_owned())? != root
        || !valid_lower_hex(revision, 40)
    {
        return Err("exact-tree root or revision is invalid".to_owned());
    }
    let object = format!("{revision}^{{commit}}");
    git_status(root, &["cat-file", "-e", &object])
        .map_err(|_| "exact-tree revision is not a commit".to_owned())
}

fn tree_members(root: &Path, revision: &str) -> Result<Vec<TreeMember>, String> {
    let bytes = git_output(
        root,
        &["ls-tree", "-rz", "--full-tree", revision],
        MAX_TREE_LIST_BYTES,
    )?;
    let mut members = Vec::new();
    let mut paths = BTreeSet::new();
    for record in bytes
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
    {
        let (metadata, path) = split_once(record, b'\t')
            .ok_or_else(|| "exact-tree inventory record is malformed".to_owned())?;
        let fields = metadata.split(|byte| *byte == b' ').collect::<Vec<_>>();
        if fields.len() != 3 || fields[1] != b"blob" {
            return Err("exact-tree inventory contains a forbidden object".to_owned());
        }
        let mode = match fields[0] {
            b"100644" => 0o644,
            b"100755" => 0o755,
            _ => return Err("exact-tree inventory contains a forbidden mode".to_owned()),
        };
        let object_id = std::str::from_utf8(fields[2])
            .map_err(|_| "exact-tree object ID is invalid".to_owned())?;
        let path =
            std::str::from_utf8(path).map_err(|_| "exact-tree path is not UTF-8".to_owned())?;
        if !valid_lower_hex(object_id, 40)
            || !valid_archive_path(path)
            || !paths.insert(path.to_owned())
        {
            return Err("exact-tree inventory contains an invalid member".to_owned());
        }
        members.push(TreeMember {
            path: path.to_owned(),
            object_id: object_id.to_owned(),
            mode,
        });
        if members.len() > MAX_SOURCE_MEMBERS {
            return Err("exact-tree inventory is too large".to_owned());
        }
    }
    if members.is_empty() {
        return Err("exact-tree inventory is empty".to_owned());
    }
    members.sort_by(|left, right| left.path.as_bytes().cmp(right.path.as_bytes()));
    Ok(members)
}

fn validate_archive_bytes(
    bytes: &[u8],
    expected: &[WrittenMember],
    mtime: u64,
) -> Result<(), String> {
    let expected_length = expected.iter().try_fold(1024_u64, |total, member| {
        let padded = member
            .byte_length
            .checked_add(511)?
            .checked_div(512)?
            .checked_mul(512)?;
        total.checked_add(512)?.checked_add(padded)
    });
    if expected_length != Some(bytes.len() as u64)
        || bytes.len() < 1024
        || !bytes.len().is_multiple_of(512)
        || bytes[bytes.len() - 1024..].iter().any(|byte| *byte != 0)
    {
        return Err("exact-tree archive trailer is not canonical".to_owned());
    }
    let mut archive = tar::Archive::new(Cursor::new(bytes));
    let mut observed = Vec::new();
    for entry in archive
        .entries()
        .map_err(|_| "exact-tree archive is malformed".to_owned())?
    {
        let mut entry = entry.map_err(|_| "exact-tree archive is malformed".to_owned())?;
        let path = entry
            .path()
            .map_err(|_| "exact-tree archive path is invalid".to_owned())?
            .to_str()
            .ok_or_else(|| "exact-tree archive path is not UTF-8".to_owned())?
            .to_owned();
        if !entry.header().entry_type().is_file()
            || entry.header().uid().ok() != Some(0)
            || entry.header().gid().ok() != Some(0)
            || entry.header().mtime().ok() != Some(mtime)
            || entry.header().username().ok().flatten() != Some("")
            || entry.header().groupname().ok().flatten() != Some("")
        {
            return Err("exact-tree archive metadata is not canonical".to_owned());
        }
        let mut contents = Vec::new();
        entry
            .read_to_end(&mut contents)
            .map_err(|_| "exact-tree archive member is unreadable".to_owned())?;
        observed.push((
            path,
            entry.header().mode().unwrap_or(0),
            contents.len() as u64,
            hex::encode(Sha256::digest(&contents)),
        ));
    }
    if observed.len() != expected.len()
        || observed
            .iter()
            .zip(expected)
            .any(|((path, mode, length, digest), member)| {
                path != &member.path
                    || mode != &member.mode
                    || length != &member.byte_length
                    || digest != &member.sha256
            })
    {
        return Err("exact-tree archive inventory or payload differs".to_owned());
    }
    Ok(())
}

fn valid_archive_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= MAX_USTAR_PATH_BYTES
        && !path.contains(['\0', '\n', '\r', '\\'])
        && Path::new(path)
            .components()
            .all(|component| matches!(component, Component::Normal(value) if !value.is_empty()))
}

fn valid_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn split_once(bytes: &[u8], separator: u8) -> Option<(&[u8], &[u8])> {
    let index = bytes.iter().position(|byte| *byte == separator)?;
    Some((&bytes[..index], &bytes[index + 1..]))
}

fn git_output(root: &Path, arguments: &[&str], maximum: usize) -> Result<Vec<u8>, String> {
    let mut child = Command::new("git")
        .args(arguments)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "exact-tree Git command could not start".to_owned())?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| "exact-tree Git stdout is unavailable".to_owned())?;
    let mut bytes = Vec::new();
    if stdout
        .by_ref()
        .take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .is_err()
        || bytes.len() > maximum
    {
        let _ = child.kill();
        let _ = child.wait();
        return Err("exact-tree Git output exceeded its bound".to_owned());
    }
    let status = child
        .wait()
        .map_err(|_| "exact-tree Git command could not finish".to_owned())?;
    if status.success() {
        Ok(bytes)
    } else {
        Err("exact-tree Git command failed".to_owned())
    }
}

fn git_status(root: &Path, arguments: &[&str]) -> Result<(), ()> {
    let status = Command::new("git")
        .args(arguments)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|_| ())?;
    if status.success() { Ok(()) } else { Err(()) }
}

#[cfg(unix)]
fn set_mode(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt as _;
    fs::set_permissions(path, fs::Permissions::from_mode(ARCHIVE_MODE))
        .map_err(|_| "exact-tree archive mode could not be set".to_owned())
}

#[cfg(not(unix))]
fn set_mode(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, process::Command};

    use tempfile::TempDir;

    use super::*;

    fn git(root: &Path, arguments: &[&str]) -> String {
        let output = Command::new("git")
            .args(arguments)
            .current_dir(root)
            .output()
            .expect("run Git");
        assert!(output.status.success(), "git {arguments:?}");
        String::from_utf8(output.stdout)
            .expect("Git UTF-8")
            .trim()
            .to_owned()
    }

    fn fixture() -> (TempDir, PathBuf, String) {
        let fixture = TempDir::new().expect("fixture");
        let root = fixture.path().canonicalize().expect("canonical fixture");
        fs::create_dir(root.join("nested")).expect("nested");
        fs::write(root.join("alpha.txt"), b"alpha\n").expect("alpha");
        fs::write(root.join("nested/zeta.txt"), b"zeta\n").expect("zeta");
        git(&root, &["init", "--quiet"]);
        git(&root, &["config", "user.name", "Exact Tree Fixture"]);
        git(&root, &["config", "user.email", "fixture@radroots.test"]);
        git(&root, &["add", "."]);
        let status = Command::new("git")
            .args(["commit", "--quiet", "-m", "fixture"])
            .current_dir(&root)
            .env("GIT_AUTHOR_DATE", "@1700000000 +0000")
            .env("GIT_COMMITTER_DATE", "@1700000000 +0000")
            .status()
            .expect("commit fixture");
        assert!(status.success());
        let revision = git(&root, &["rev-parse", "HEAD"]);
        (fixture, root, revision)
    }

    #[test]
    fn exact_tree_archive_is_reproducible_and_canonical() {
        let (fixture, root, revision) = fixture();
        let first = fixture.path().join("first.tar");
        let second = fixture.path().join("second.tar");
        let first_evidence = create(&root, &revision, &first, 1_700_000_000).expect("first");
        let second_evidence = create(&root, &revision, &second, 1_700_000_000).expect("second");
        assert_eq!(first_evidence, second_evidence);
        assert_eq!(
            fs::read(&first).expect("first"),
            fs::read(&second).expect("second")
        );
        assert_eq!(first_evidence.members, 2);
        assert_eq!(first_evidence.payload_bytes, 11);
        assert_eq!(
            commit_timestamp(&root, &revision).expect("timestamp"),
            1_700_000_000
        );
    }

    #[test]
    fn exact_tree_archive_rejects_links_and_noncanonical_requests() {
        let (fixture, root, revision) = fixture();
        assert!(create(&root, &revision, Path::new("relative.tar"), 1).is_err());
        let output = fixture.path().join("zero.tar");
        assert!(create(&root, &revision, &output, 0).is_err());

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("alpha.txt", root.join("link")).expect("link");
            git(&root, &["add", "link"]);
            let status = Command::new("git")
                .args(["commit", "--quiet", "-m", "link"])
                .current_dir(&root)
                .env("GIT_AUTHOR_DATE", "@1700000001 +0000")
                .env("GIT_COMMITTER_DATE", "@1700000001 +0000")
                .status()
                .expect("commit link");
            assert!(status.success());
            let linked_revision = git(&root, &["rev-parse", "HEAD"]);
            let linked_output = fixture.path().join("linked.tar");
            assert!(create(&root, &linked_revision, &linked_output, 1).is_err());
        }
    }
}
