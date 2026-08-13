use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

#[cfg(unix)]
use std::process::{ExitStatus, Output};

use flate2::read::GzDecoder;

use crate::error::RadrootsRuntimeManagerError;
use crate::paths::ManagedRuntimeInstancePaths;

type SpawnProcess = fn(&Path, &[String], &[(String, String)], File, File) -> std::io::Result<u32>;

/// A validated single-component manager-owned executable artifact name.
#[derive(Clone, PartialEq, Eq)]
pub struct ManagedRuntimeArtifactName(String);

impl ManagedRuntimeArtifactName {
    pub fn new(value: &str) -> Result<Self, RadrootsRuntimeManagerError> {
        if value.is_empty()
            || value.len() > 128
            || !value.as_bytes()[0].is_ascii_alphanumeric()
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
            || Path::new(value).components().count() != 1
        {
            return Err(RadrootsRuntimeManagerError::InvalidArtifactName);
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl core::fmt::Debug for ManagedRuntimeArtifactName {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("ManagedRuntimeArtifactName([redacted])")
    }
}

pub fn ensure_instance_layout(
    paths: &ManagedRuntimeInstancePaths,
) -> Result<(), RadrootsRuntimeManagerError> {
    for path in [paths.install_dir(), paths.logs_dir(), paths.run_dir()] {
        fs::create_dir_all(path).map_err(|source| {
            RadrootsRuntimeManagerError::CreateDirectory {
                kind: source.kind(),
            }
        })?;
    }
    Ok(())
}

pub fn install_binary(
    source_binary_path: impl AsRef<Path>,
    paths: &ManagedRuntimeInstancePaths,
    binary_name: &ManagedRuntimeArtifactName,
) -> Result<PathBuf, RadrootsRuntimeManagerError> {
    install_binary_path(source_binary_path.as_ref(), paths, binary_name)
}

fn install_binary_path(
    source_binary_path: &Path,
    paths: &ManagedRuntimeInstancePaths,
    binary_name: &ManagedRuntimeArtifactName,
) -> Result<PathBuf, RadrootsRuntimeManagerError> {
    ensure_instance_layout(paths)?;
    let installed_binary_path = paths.install_dir().join(binary_name.as_str());
    fs::copy(source_binary_path, &installed_binary_path).map_err(|source| {
        RadrootsRuntimeManagerError::CopyBinary {
            kind: source.kind(),
        }
    })?;
    set_executable_mode(&installed_binary_path)?;
    Ok(installed_binary_path)
}

pub fn extract_binary_archive(
    archive_path: impl AsRef<Path>,
    archive_format: &str,
    paths: &ManagedRuntimeInstancePaths,
    binary_name: &ManagedRuntimeArtifactName,
) -> Result<PathBuf, RadrootsRuntimeManagerError> {
    extract_binary_archive_path(archive_path.as_ref(), archive_format, paths, binary_name)
}

fn extract_binary_archive_path(
    archive_path: &Path,
    archive_format: &str,
    paths: &ManagedRuntimeInstancePaths,
    binary_name: &ManagedRuntimeArtifactName,
) -> Result<PathBuf, RadrootsRuntimeManagerError> {
    remove_path_if_exists(paths.install_dir())?;
    ensure_instance_layout(paths)?;

    match archive_format {
        "tar.gz" => unpack_tar_gz_archive(archive_path, paths.install_dir())?,
        _ => return Err(RadrootsRuntimeManagerError::UnsupportedArchiveFormat),
    }

    let installed_binary_path = paths.install_dir().join(binary_name.as_str());
    let resolved_binary_path = if installed_binary_path.is_file() {
        installed_binary_path
    } else {
        find_binary_with_name(paths.install_dir(), binary_name.as_str()).ok_or(
            RadrootsRuntimeManagerError::ReadManagedFile {
                kind: std::io::ErrorKind::NotFound,
            },
        )?
    };
    set_executable_mode(&resolved_binary_path)?;
    Ok(resolved_binary_path)
}

pub fn write_instance_config(
    paths: &ManagedRuntimeInstancePaths,
    contents: &str,
) -> Result<(), RadrootsRuntimeManagerError> {
    let path = paths.config_path();
    ensure_parent_dir(&path)?;
    fs::write(path, contents).map_err(|source| RadrootsRuntimeManagerError::WriteManagedConfig {
        kind: source.kind(),
    })
}

pub fn start_process(
    paths: &ManagedRuntimeInstancePaths,
    binary_name: &ManagedRuntimeArtifactName,
    args: &[String],
    envs: &[(String, String)],
) -> Result<u32, RadrootsRuntimeManagerError> {
    start_process_path(
        &paths.install_dir().join(binary_name.as_str()),
        args,
        envs,
        paths,
    )
}

fn start_process_path(
    binary_path: &Path,
    args: &[String],
    envs: &[(String, String)],
    paths: &ManagedRuntimeInstancePaths,
) -> Result<u32, RadrootsRuntimeManagerError> {
    start_process_with(binary_path, args, envs, paths, spawn_process)
}

fn start_process_with(
    binary_path: &Path,
    args: &[String],
    envs: &[(String, String)],
    paths: &ManagedRuntimeInstancePaths,
    spawn: SpawnProcess,
) -> Result<u32, RadrootsRuntimeManagerError> {
    ensure_instance_layout(paths)?;
    let stdout = open_log_file(paths.stdout_log_path())?;
    let stderr = open_log_file(paths.stderr_log_path())?;
    let pid = spawn(binary_path, args, envs, stdout, stderr).map_err(|source| {
        RadrootsRuntimeManagerError::SpawnProcess {
            kind: source.kind(),
        }
    })?;
    fs::write(paths.pid_file_path(), pid.to_string()).map_err(|source| {
        RadrootsRuntimeManagerError::WritePidFile {
            kind: source.kind(),
        }
    })?;
    Ok(pid)
}

fn spawn_process(
    binary_path: &Path,
    args: &[String],
    envs: &[(String, String)],
    stdout: File,
    stderr: File,
) -> std::io::Result<u32> {
    Command::new(binary_path)
        .args(args)
        .envs(envs.iter().map(|(key, value)| (key, value)))
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .map(|child| child.id())
}

pub fn process_running(
    paths: &ManagedRuntimeInstancePaths,
) -> Result<bool, RadrootsRuntimeManagerError> {
    let Some(pid) = read_pid(paths)? else {
        return Ok(false);
    };
    Ok(process_running_for_pid(pid))
}

pub fn stop_process(
    paths: &ManagedRuntimeInstancePaths,
) -> Result<bool, RadrootsRuntimeManagerError> {
    let Some(pid) = read_pid(paths)? else {
        return Ok(false);
    };
    if !process_running_for_pid(pid) {
        remove_pid_file(paths)?;
        return Ok(false);
    }

    let mut is_running = process_running_for_pid;
    let mut terminate = terminate_process;
    let mut force_kill = force_kill_process;
    let mut sleep = thread::sleep;
    stop_process_for_pid(
        paths,
        pid,
        &mut is_running,
        &mut terminate,
        &mut force_kill,
        &mut sleep,
    )
}

pub fn remove_instance_artifacts(
    paths: &ManagedRuntimeInstancePaths,
) -> Result<(), RadrootsRuntimeManagerError> {
    for path in [paths.install_dir(), paths.logs_dir(), paths.run_dir()] {
        remove_path_if_exists(path)?;
    }
    Ok(())
}

fn stop_process_for_pid(
    paths: &ManagedRuntimeInstancePaths,
    pid: u32,
    is_running: &mut dyn FnMut(u32) -> bool,
    terminate: &mut dyn FnMut(u32) -> Result<(), RadrootsRuntimeManagerError>,
    force_kill: &mut dyn FnMut(u32) -> Result<(), RadrootsRuntimeManagerError>,
    sleep: &mut dyn FnMut(Duration),
) -> Result<bool, RadrootsRuntimeManagerError> {
    terminate(pid)?;
    for _ in 0..20 {
        if !is_running(pid) {
            remove_pid_file(paths)?;
            return Ok(true);
        }
        sleep(Duration::from_millis(100));
    }

    force_kill(pid)?;
    for _ in 0..20 {
        if !is_running(pid) {
            remove_pid_file(paths)?;
            return Ok(true);
        }
        sleep(Duration::from_millis(100));
    }

    Err(RadrootsRuntimeManagerError::StopProcess)
}

fn unpack_tar_gz_archive(
    archive_path: &Path,
    destination_dir: &Path,
) -> Result<(), RadrootsRuntimeManagerError> {
    let archive_file = File::open(archive_path).map_err(|source| {
        RadrootsRuntimeManagerError::ReadManagedFile {
            kind: source.kind(),
        }
    })?;
    let decoder = GzDecoder::new(archive_file);
    let mut archive = tar::Archive::new(decoder);
    archive
        .unpack(destination_dir)
        .map_err(|source| RadrootsRuntimeManagerError::UnpackArchive {
            kind: source.kind(),
        })
}

fn find_binary_with_name(root: &Path, binary_name: &str) -> Option<PathBuf> {
    let entries = fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_binary_with_name(&path, binary_name) {
                return Some(found);
            }
            continue;
        }
        if path.file_name().and_then(|name| name.to_str()) == Some(binary_name) {
            return Some(path);
        }
    }
    None
}

fn open_log_file(path: &Path) -> Result<File, RadrootsRuntimeManagerError> {
    ensure_parent_dir(path)?;
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|source| RadrootsRuntimeManagerError::OpenLogFile {
            kind: source.kind(),
        })
}

fn ensure_parent_dir(path: &Path) -> Result<(), RadrootsRuntimeManagerError> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    fs::create_dir_all(parent).map_err(|source| RadrootsRuntimeManagerError::CreateDirectory {
        kind: source.kind(),
    })
}

fn read_pid(
    paths: &ManagedRuntimeInstancePaths,
) -> Result<Option<u32>, RadrootsRuntimeManagerError> {
    let raw = match fs::read_to_string(paths.pid_file_path()) {
        Ok(raw) => raw,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(RadrootsRuntimeManagerError::ReadPidFile {
                kind: source.kind(),
            });
        }
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    trimmed
        .parse::<u32>()
        .map(Some)
        .map_err(|_| RadrootsRuntimeManagerError::ParsePidFile)
}

fn remove_pid_file(paths: &ManagedRuntimeInstancePaths) -> Result<(), RadrootsRuntimeManagerError> {
    match fs::remove_file(paths.pid_file_path()) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(RadrootsRuntimeManagerError::RemovePath {
            kind: source.kind(),
        }),
    }
}

fn remove_path_if_exists(path: &Path) -> Result<(), RadrootsRuntimeManagerError> {
    let state = match fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => Ok(Some(ExistingPathKind::Directory)),
        Ok(_) => Ok(Some(ExistingPathKind::File)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(source),
    };
    remove_path_from_state(path, state, remove_dir_all_path, remove_file_path)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExistingPathKind {
    Directory,
    File,
}

fn remove_path_from_state(
    path: &Path,
    state: Result<Option<ExistingPathKind>, std::io::Error>,
    remove_dir_all: fn(&Path) -> std::io::Result<()>,
    remove_file: fn(&Path) -> std::io::Result<()>,
) -> Result<(), RadrootsRuntimeManagerError> {
    match state {
        Ok(Some(ExistingPathKind::Directory)) => {
            remove_dir_all(path).map_err(|source| RadrootsRuntimeManagerError::RemovePath {
                kind: source.kind(),
            })
        }
        Ok(Some(ExistingPathKind::File)) => {
            remove_file(path).map_err(|source| RadrootsRuntimeManagerError::RemovePath {
                kind: source.kind(),
            })
        }
        Ok(None) => Ok(()),
        Err(source) => Err(RadrootsRuntimeManagerError::ReadManagedFile {
            kind: source.kind(),
        }),
    }
}

fn remove_dir_all_path(path: &Path) -> std::io::Result<()> {
    fs::remove_dir_all(path)
}

fn remove_file_path(path: &Path) -> std::io::Result<()> {
    fs::remove_file(path)
}

#[cfg(unix)]
fn set_executable_mode(path: &Path) -> Result<(), RadrootsRuntimeManagerError> {
    apply_mode(path, 0o755, set_permissions_path)
}

#[cfg(not(unix))]
fn set_executable_mode(_path: &Path) -> Result<(), RadrootsRuntimeManagerError> {
    Ok(())
}

#[cfg(unix)]
fn apply_mode(
    path: &Path,
    mode: u32,
    set_permissions: fn(&Path, fs::Permissions) -> std::io::Result<()>,
) -> Result<(), RadrootsRuntimeManagerError> {
    use std::os::unix::fs::PermissionsExt;

    let metadata =
        fs::metadata(path).map_err(|source| RadrootsRuntimeManagerError::ReadManagedFile {
            kind: source.kind(),
        })?;
    let mut permissions = metadata.permissions();
    permissions.set_mode(mode);
    set_permissions(path, permissions).map_err(|source| {
        RadrootsRuntimeManagerError::SetPermissions {
            kind: source.kind(),
        }
    })
}

#[cfg(unix)]
fn set_permissions_path(path: &Path, permissions: fs::Permissions) -> std::io::Result<()> {
    fs::set_permissions(path, permissions)
}

#[cfg(unix)]
fn process_running_for_pid(pid: u32) -> bool {
    let pid_arg = pid.to_string();
    let running = Command::new("kill")
        .args(["-0", pid_arg.as_str()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false);
    if !running {
        return false;
    }

    ps_output_for_pid(pid_arg.as_str())
        .map(process_running_state_from_ps_output)
        .unwrap_or(true)
}

#[cfg(unix)]
fn ps_output_for_pid(pid_arg: &str) -> std::io::Result<Output> {
    Command::new("ps")
        .args(["-o", "stat=", "-p", pid_arg])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
}

#[cfg(unix)]
fn process_running_state_from_ps_output(output: Output) -> bool {
    if !output.status.success() {
        return true;
    }
    let state = String::from_utf8_lossy(output.stdout.as_slice());
    !state.trim_start().starts_with('Z')
}

#[cfg(windows)]
fn process_running_for_pid(pid: u32) -> bool {
    Command::new("tasklist")
        .args(["/FI", format!("PID eq {pid}").as_str()])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .map(|output| {
            output.status.success()
                && String::from_utf8_lossy(output.stdout.as_slice())
                    .contains(pid.to_string().as_str())
        })
        .unwrap_or(false)
}

#[cfg(not(any(unix, windows)))]
fn process_running_for_pid(_pid: u32) -> bool {
    false
}

#[cfg(unix)]
fn terminate_process(pid: u32) -> Result<(), RadrootsRuntimeManagerError> {
    signal_process(pid, "-TERM")
}

#[cfg(unix)]
fn force_kill_process(pid: u32) -> Result<(), RadrootsRuntimeManagerError> {
    signal_process(pid, "-KILL")
}

#[cfg(unix)]
fn signal_process(pid: u32, signal: &str) -> Result<(), RadrootsRuntimeManagerError> {
    signal_process_with(pid, signal, execute_signal_command)
}

#[cfg(unix)]
fn execute_signal_command(pid: u32, signal: &str) -> std::io::Result<ExitStatus> {
    Command::new("kill")
        .args([signal, pid.to_string().as_str()])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()
}

#[cfg(unix)]
fn signal_process_with(
    pid: u32,
    signal: &str,
    runner: fn(u32, &str) -> std::io::Result<ExitStatus>,
) -> Result<(), RadrootsRuntimeManagerError> {
    let status = runner(pid, signal).map_err(|source| {
        RadrootsRuntimeManagerError::ExecuteProcessSignal {
            kind: source.kind(),
        }
    })?;
    if status.success() {
        Ok(())
    } else {
        Err(RadrootsRuntimeManagerError::StopProcess)
    }
}

#[cfg(windows)]
fn terminate_process(pid: u32) -> Result<(), RadrootsRuntimeManagerError> {
    force_kill_process(pid)
}

#[cfg(windows)]
fn force_kill_process(pid: u32) -> Result<(), RadrootsRuntimeManagerError> {
    let status = Command::new("taskkill")
        .args(["/PID", pid.to_string().as_str(), "/T", "/F"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|source| RadrootsRuntimeManagerError::ExecuteProcessSignal {
            kind: source.kind(),
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(RadrootsRuntimeManagerError::StopProcess)
    }
}

#[cfg(not(any(unix, windows)))]
fn terminate_process(_pid: u32) -> Result<(), RadrootsRuntimeManagerError> {
    Err(RadrootsRuntimeManagerError::StopProcess)
}

#[cfg(not(any(unix, windows)))]
fn force_kill_process(_pid: u32) -> Result<(), RadrootsRuntimeManagerError> {
    Err(RadrootsRuntimeManagerError::StopProcess)
}

#[cfg(test)]
mod tests {
    use std::fs;
    #[cfg(unix)]
    use std::fs::File;
    use std::io;
    use std::path::Path;
    #[cfg(unix)]
    use std::process::ExitStatus;
    #[cfg(unix)]
    use std::thread;
    use std::time::Duration;

    use radroots_runtime_paths::{
        InstanceId, RadrootsHostEnvironment, RadrootsPathProfile, RadrootsPathResolver,
        RadrootsPlatform, RuntimeContext, RuntimeContextBootstrap, RuntimeContextSource, ServiceId,
    };
    use tempfile::tempdir;

    use super::{
        ExistingPathKind, ManagedRuntimeArtifactName, ensure_instance_layout, ensure_parent_dir,
        extract_binary_archive, find_binary_with_name, install_binary, open_log_file,
        process_running, read_pid, remove_instance_artifacts, remove_path_from_state,
        remove_path_if_exists, start_process_with, stop_process, stop_process_for_pid,
        write_instance_config,
    };
    #[cfg(unix)]
    use super::{
        apply_mode, force_kill_process, process_running_for_pid,
        process_running_state_from_ps_output, set_executable_mode, signal_process,
        signal_process_with, start_process, terminate_process,
    };
    use crate::error::RadrootsRuntimeManagerError;
    use crate::paths::{ManagedRuntimeInstancePaths, resolve_instance_paths, resolve_shared_paths};

    fn sample_paths(root: &Path) -> ManagedRuntimeInstancePaths {
        fn context(root: &Path, service: &str) -> RuntimeContext {
            RuntimeContext::resolve(
                &RadrootsPathResolver::new(
                    RadrootsPlatform::Linux,
                    RadrootsHostEnvironment::default(),
                ),
                RuntimeContextBootstrap::new(
                    RadrootsPathProfile::RepoLocal,
                    Some(root.to_path_buf()),
                    RuntimeContextSource::BootstrapCli,
                    RuntimeContextSource::BootstrapCli,
                )
                .expect("bootstrap"),
                ServiceId::new(service).expect("service"),
                InstanceId::new("local").expect("instance"),
            )
            .expect("context")
        }

        let shared = resolve_shared_paths(&context(root, "runtime-manager"));
        resolve_instance_paths(&shared, &context(root, "radrootsd"))
    }

    fn artifact(value: &str) -> ManagedRuntimeArtifactName {
        ManagedRuntimeArtifactName::new(value).expect("artifact name")
    }

    fn assert_safe_error(err: &RadrootsRuntimeManagerError, expected: &str, forbidden: &[&str]) {
        use std::error::Error as _;

        let rendered = format!("{err} {err:?}");
        assert!(
            rendered.contains(expected),
            "expected `{rendered}` to contain `{expected}`"
        );
        for part in forbidden {
            assert!(
                !rendered.contains(part),
                "expected `{rendered}` not to contain `{part}`"
            );
        }
        assert!(err.source().is_none());
    }

    #[cfg(unix)]
    fn exit_status(code: i32) -> ExitStatus {
        std::process::Command::new("sh")
            .args(["-c", &format!("exit {code}")])
            .status()
            .expect("exit status")
    }

    #[cfg(unix)]
    fn output_with_status(status: ExitStatus, stdout: &[u8]) -> std::process::Output {
        std::process::Output {
            status,
            stdout: stdout.to_vec(),
            stderr: Vec::new(),
        }
    }

    fn ok_remove_path(_path: &Path) -> io::Result<()> {
        Ok(())
    }

    fn deny_remove_path(_path: &Path) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "remove path denied",
        ))
    }

    fn ok_runtime_signal(_pid: u32) -> Result<(), RadrootsRuntimeManagerError> {
        Ok(())
    }

    fn noop_runtime_sleep(_duration: Duration) {}

    fn runtime_is_stopped(_pid: u32) -> bool {
        false
    }

    fn runtime_is_running(_pid: u32) -> bool {
        true
    }

    #[test]
    fn layout_creates_only_manager_owned_install_and_tracking_roots() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        ensure_instance_layout(&paths).expect("layout");
        assert!(paths.install_dir().is_dir());
        assert!(paths.logs_dir().is_dir());
        assert!(paths.run_dir().is_dir());
        assert!(!paths.config_dir().exists());
        assert!(!paths.state_dir().exists());
        assert!(!paths.secrets_dir().exists());
    }

    #[test]
    fn install_binary_copies_source_into_install_dir() {
        let dir = tempdir().expect("tempdir");
        let source = dir.path().join("radrootsd");
        fs::write(&source, "#!/bin/sh\nexit 0\n").expect("source");
        let paths = sample_paths(dir.path());
        let installed = install_binary(&source, &paths, &artifact("radrootsd")).expect("install");
        assert!(installed.is_file());
        assert!(installed.starts_with(paths.install_dir()));
    }

    #[test]
    fn artifact_names_reject_absolute_parent_and_multicomponent_escapes() {
        for invalid in [
            "",
            "/tmp/escape",
            "../escape",
            "nested/escape",
            r"nested\escape",
            ".",
            "..",
            " secret",
        ] {
            let err = ManagedRuntimeArtifactName::new(invalid).expect_err("reject artifact name");
            if invalid.is_empty() {
                assert_safe_error(&err, "artifact name is invalid", &[]);
            } else {
                assert_safe_error(&err, "artifact name is invalid", &[invalid]);
            }
        }

        let maximum = format!("a{}", "b".repeat(127));
        assert!(ManagedRuntimeArtifactName::new(&maximum).is_ok());
        assert!(ManagedRuntimeArtifactName::new(&format!("{maximum}c")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn extract_binary_archive_unpacks_tar_gz() {
        let dir = tempdir().expect("tempdir");
        let archive_root = dir.path().join("archive");
        fs::create_dir_all(archive_root.join("bin")).expect("archive dir");
        fs::write(archive_root.join("bin/radrootsd"), "#!/bin/sh\nexit 0\n").expect("binary");
        let archive_path = dir.path().join("radrootsd.tar.gz");
        let file = File::create(&archive_path).expect("archive file");
        let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let mut builder = tar::Builder::new(encoder);
        builder
            .append_path_with_name(
                archive_root.join("bin/radrootsd"),
                "radrootsd/bin/radrootsd",
            )
            .expect("append path");
        builder.finish().expect("finish archive");
        let encoder = builder.into_inner().expect("into encoder");
        encoder.finish().expect("finish gzip");

        let paths = sample_paths(dir.path());
        let installed =
            extract_binary_archive(&archive_path, "tar.gz", &paths, &artifact("radrootsd"))
                .expect("extract");
        assert!(installed.is_file());
    }

    #[cfg(unix)]
    #[test]
    fn extract_binary_archive_uses_direct_binary_when_present_at_root() {
        let dir = tempdir().expect("tempdir");
        let archive_root = dir.path().join("archive");
        fs::create_dir_all(&archive_root).expect("archive dir");
        fs::write(archive_root.join("radrootsd"), "#!/bin/sh\nexit 0\n").expect("binary");
        let archive_path = dir.path().join("radrootsd.tar.gz");
        let file = File::create(&archive_path).expect("archive file");
        let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let mut builder = tar::Builder::new(encoder);
        builder
            .append_path_with_name(archive_root.join("radrootsd"), "radrootsd")
            .expect("append path");
        builder.finish().expect("finish archive");
        let encoder = builder.into_inner().expect("into encoder");
        encoder.finish().expect("finish gzip");

        let paths = sample_paths(dir.path());
        let installed =
            extract_binary_archive(&archive_path, "tar.gz", &paths, &artifact("radrootsd"))
                .expect("extract");
        assert_eq!(installed, paths.install_dir().join("radrootsd"));
    }

    #[cfg(unix)]
    #[test]
    fn start_and_stop_process_manage_pid_file() {
        let dir = tempdir().expect("tempdir");
        let binary = dir.path().join("sleepy.sh");
        fs::write(&binary, "#!/bin/sh\nexec sleep 30\n").expect("script");
        let paths = sample_paths(dir.path());
        install_binary(&binary, &paths, &artifact("sleepy.sh")).expect("install");
        let envs = vec![("RADROOTS_RUNTIME_MANAGER_TEST".to_owned(), "1".to_owned())];
        let pid = start_process(&paths, &artifact("sleepy.sh"), &Vec::new(), &envs).expect("start");
        assert!(pid > 0);
        thread::sleep(Duration::from_millis(100));
        assert!(paths.pid_file_path().is_file());
        assert!(process_running(&paths).expect("running"));
        assert!(stop_process(&paths).expect("stop"));
        assert!(!paths.pid_file_path().exists());
    }

    #[test]
    fn remove_instance_artifacts_removes_layout_roots() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        ensure_instance_layout(&paths).expect("layout");
        fs::create_dir_all(paths.state_dir()).expect("canonical state sentinel");
        fs::create_dir_all(paths.secrets_dir()).expect("canonical secrets sentinel");
        fs::write(paths.state_dir().join("state.sqlite"), "state").expect("state sentinel");
        fs::write(paths.secrets_dir().join("identity.secret"), "secret").expect("secret sentinel");
        remove_instance_artifacts(&paths).expect("remove");
        assert!(!paths.install_dir().exists());
        assert!(!paths.logs_dir().exists());
        assert!(!paths.run_dir().exists());
        assert!(paths.state_dir().join("state.sqlite").is_file());
        assert!(paths.secrets_dir().join("identity.secret").is_file());
    }

    #[test]
    fn ensure_instance_layout_reports_directory_errors() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        fs::create_dir_all(paths.install_dir().parent().expect("install parent")).expect("parent");
        fs::write(paths.install_dir(), "occupied").expect("file");

        let err = ensure_instance_layout(&paths).expect_err("file path should fail");
        assert_safe_error(
            &err,
            "create managed runtime directory",
            &[paths.install_dir().to_string_lossy().as_ref()],
        );
    }

    #[test]
    fn install_binary_reports_copy_errors() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        let err = install_binary(dir.path().join("missing"), &paths, &artifact("radrootsd"))
            .expect_err("missing source should fail");
        assert_safe_error(
            &err,
            "copy managed runtime binary",
            &[
                dir.path().join("missing").to_string_lossy().as_ref(),
                paths
                    .install_dir()
                    .join("radrootsd")
                    .to_string_lossy()
                    .as_ref(),
            ],
        );
    }

    #[test]
    fn extract_binary_archive_reports_unsupported_format() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        let archive_path = dir.path().join("radrootsd.zip");

        let err = extract_binary_archive(&archive_path, "zip", &paths, &artifact("radrootsd"))
            .expect_err("unsupported archive format should fail");
        assert_safe_error(
            &err,
            "archive format is unsupported",
            &[archive_path.to_string_lossy().as_ref(), "zip"],
        );
    }

    #[test]
    fn extract_binary_archive_reports_missing_archive() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        let archive_path = dir.path().join("missing.tar.gz");

        let err = extract_binary_archive(&archive_path, "tar.gz", &paths, &artifact("radrootsd"))
            .expect_err("missing archive should fail");
        assert_safe_error(
            &err,
            "read managed runtime file",
            &[archive_path.to_string_lossy().as_ref()],
        );
    }

    #[cfg(unix)]
    #[test]
    fn extract_binary_archive_reports_missing_binary_in_archive() {
        let dir = tempdir().expect("tempdir");
        let archive_root = dir.path().join("archive");
        fs::create_dir_all(archive_root.join("bin")).expect("archive dir");
        fs::write(archive_root.join("bin/other"), "#!/bin/sh\nexit 0\n").expect("binary");
        let archive_path = dir.path().join("radrootsd.tar.gz");
        let file = File::create(&archive_path).expect("archive file");
        let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let mut builder = tar::Builder::new(encoder);
        builder
            .append_path_with_name(archive_root.join("bin/other"), "radrootsd/bin/other")
            .expect("append path");
        builder.finish().expect("finish archive");
        let encoder = builder.into_inner().expect("into encoder");
        encoder.finish().expect("finish gzip");

        let paths = sample_paths(dir.path());
        let err = extract_binary_archive(&archive_path, "tar.gz", &paths, &artifact("radrootsd"))
            .expect_err("archive should not resolve missing binary");
        assert_safe_error(
            &err,
            "read managed runtime file",
            &[
                paths
                    .install_dir()
                    .join("radrootsd")
                    .to_string_lossy()
                    .as_ref(),
                archive_path.to_string_lossy().as_ref(),
            ],
        );
    }

    #[cfg(unix)]
    #[test]
    fn extract_binary_archive_reports_unpack_errors() {
        let dir = tempdir().expect("tempdir");
        let archive_path = dir.path().join("invalid.tar.gz");
        fs::write(&archive_path, "not a gzip archive").expect("write archive");
        let paths = sample_paths(dir.path());

        let err = extract_binary_archive(&archive_path, "tar.gz", &paths, &artifact("radrootsd"))
            .expect_err("invalid archive should fail");
        assert_safe_error(
            &err,
            "unpack managed runtime archive",
            &[
                archive_path.to_string_lossy().as_ref(),
                "not a gzip archive",
            ],
        );
    }

    #[test]
    fn write_instance_config_is_context_bound_and_reports_redacted_errors() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        let config = paths.config_path();
        write_instance_config(&paths, "enabled = true").expect("write config");
        assert_eq!(
            fs::read_to_string(&config).expect("read config"),
            "enabled = true"
        );
        assert!(!paths.secrets_dir().exists());

        fs::remove_file(&config).expect("remove config");
        fs::create_dir(&config).expect("occupy config path");
        let err = write_instance_config(&paths, "credential = 'secret-value'")
            .expect_err("directory write should fail");
        assert_safe_error(
            &err,
            "write managed runtime config",
            &[config.to_string_lossy().as_ref(), "secret-value"],
        );
    }

    #[test]
    fn start_process_reports_spawn_errors() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        let binary = paths.install_dir().join("unavailable");
        let err = start_process_with(
            &binary,
            &[],
            &[],
            &paths,
            |_binary, _args, _envs, _stdout, _stderr| {
                Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "injected spawn denial",
                ))
            },
        )
        .expect_err("injected spawn failure");
        assert_safe_error(
            &err,
            "spawn managed runtime process",
            &[binary.to_string_lossy().as_ref(), "injected spawn denial"],
        );
    }

    #[cfg(unix)]
    #[test]
    fn start_process_reports_pid_file_write_errors() {
        let dir = tempdir().expect("tempdir");
        let binary = dir.path().join("sleepy.sh");
        fs::write(&binary, "#!/bin/sh\nexec sleep 1\n").expect("script");
        let paths = sample_paths(dir.path());
        fs::create_dir_all(paths.pid_file_path()).expect("occupy pid path");
        install_binary(&binary, &sample_paths(dir.path()), &artifact("sleepy.sh"))
            .expect("install");

        let err = start_process(&paths, &artifact("sleepy.sh"), &[], &[])
            .expect_err("pid file write should fail");
        assert_safe_error(
            &err,
            "write managed runtime pid",
            &[paths.pid_file_path().to_string_lossy().as_ref()],
        );
    }

    #[test]
    fn process_running_and_stop_process_handle_missing_pid_file() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());

        assert!(!process_running(&paths).expect("missing pid should be false"));
        assert!(!stop_process(&paths).expect("missing pid stop should be false"));
    }

    #[test]
    fn process_running_reports_invalid_pid_file() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        ensure_instance_layout(&paths).expect("layout");
        fs::write(paths.pid_file_path(), "not-a-pid").expect("write pid");

        let err = process_running(&paths).expect_err("invalid pid should fail");
        assert_safe_error(
            &err,
            "managed runtime pid is malformed",
            &[
                paths.pid_file_path().to_string_lossy().as_ref(),
                "not-a-pid",
            ],
        );
    }

    #[test]
    fn stop_process_clears_stale_pid_file() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        ensure_instance_layout(&paths).expect("layout");
        fs::write(paths.pid_file_path(), "999999").expect("write pid");

        assert!(!stop_process(&paths).expect("stale pid should return false"));
        assert!(!paths.pid_file_path().exists());
    }

    #[test]
    fn stop_process_for_pid_uses_force_kill_after_terminate_attempts() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        ensure_instance_layout(&paths).expect("layout");
        fs::write(paths.pid_file_path(), "42").expect("write pid");

        let mut polls = 0_u32;
        let mut is_running = |_pid| {
            polls += 1;
            polls <= 20
        };
        let mut terminate = ok_runtime_signal;
        let mut force_kill = ok_runtime_signal;
        let mut sleep = noop_runtime_sleep;
        let stopped = stop_process_for_pid(
            &paths,
            42,
            &mut is_running,
            &mut terminate,
            &mut force_kill,
            &mut sleep,
        )
        .expect("force-kill path should stop");

        assert!(stopped);
        assert!(!paths.pid_file_path().exists());
        assert_eq!(polls, 21);
    }

    #[test]
    fn stop_process_for_pid_stops_after_terminate_poll() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        ensure_instance_layout(&paths).expect("layout");
        fs::write(paths.pid_file_path(), "42").expect("write pid");

        let mut is_running = runtime_is_stopped;
        let mut terminate = ok_runtime_signal;
        let mut force_kill = ok_runtime_signal;
        let mut sleep = noop_runtime_sleep;
        let stopped = stop_process_for_pid(
            &paths,
            42,
            &mut is_running,
            &mut terminate,
            &mut force_kill,
            &mut sleep,
        )
        .expect("terminate poll should stop");

        assert!(stopped);
        assert!(!paths.pid_file_path().exists());
    }

    #[test]
    fn stop_process_for_pid_reports_failure_after_force_kill_attempts() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        ensure_instance_layout(&paths).expect("layout");
        fs::write(paths.pid_file_path(), "42").expect("write pid");

        let mut sleeps = 0_u32;
        let mut is_running = runtime_is_running;
        let mut terminate = ok_runtime_signal;
        let mut force_kill = ok_runtime_signal;
        let mut sleep = |_duration| {
            sleeps += 1;
        };
        let err = stop_process_for_pid(
            &paths,
            42,
            &mut is_running,
            &mut terminate,
            &mut force_kill,
            &mut sleep,
        )
        .expect_err("force-kill exhaustion should fail");

        assert_safe_error(&err, "managed runtime process did not stop", &["42"]);
        assert_eq!(sleeps, 40);
        assert!(paths.pid_file_path().exists());
    }

    #[test]
    fn ensure_parent_dir_without_parent_is_a_noop() {
        ensure_parent_dir(Path::new("/")).expect("root path should have no parent");
    }

    #[test]
    fn ensure_parent_dir_reports_directory_creation_errors() {
        let dir = tempdir().expect("tempdir");
        let file_parent = dir.path().join("occupied");
        fs::write(&file_parent, "file").expect("parent file");

        let err =
            ensure_parent_dir(&file_parent.join("child")).expect_err("file parent should fail");
        assert_safe_error(
            &err,
            "create managed runtime directory",
            &[file_parent.to_string_lossy().as_ref()],
        );
    }

    #[test]
    fn find_binary_with_name_handles_nested_and_missing_files() {
        let dir = tempdir().expect("tempdir");
        fs::create_dir_all(dir.path().join("nested")).expect("nested dir");
        fs::write(dir.path().join("nested/radrootsd"), "binary").expect("binary");

        assert_eq!(
            find_binary_with_name(dir.path(), "radrootsd"),
            Some(dir.path().join("nested/radrootsd"))
        );
        assert_eq!(find_binary_with_name(dir.path(), "missing"), None);
    }

    #[test]
    fn open_log_file_creates_file_and_reports_directory_errors() {
        let dir = tempdir().expect("tempdir");
        let file_path = dir.path().join("logs/stdout.log");
        let file = open_log_file(&file_path).expect("open log");
        drop(file);
        assert!(file_path.is_file());

        let bad_path = dir.path().join("bad");
        fs::create_dir(&bad_path).expect("create dir");
        let err = open_log_file(&bad_path).expect_err("directory open should fail");
        assert_safe_error(
            &err,
            "open managed runtime log",
            &[bad_path.to_string_lossy().as_ref()],
        );
    }

    #[test]
    fn read_pid_handles_empty_missing_and_read_error_cases() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());

        assert_eq!(read_pid(&paths).expect("missing pid"), None);

        ensure_instance_layout(&paths).expect("layout");
        fs::write(paths.pid_file_path(), "   ").expect("write pid");
        assert_eq!(read_pid(&paths).expect("empty pid"), None);

        fs::remove_file(paths.pid_file_path()).expect("remove pid file");
        fs::create_dir(paths.pid_file_path()).expect("occupy pid path");
        let err = read_pid(&paths).expect_err("directory pid file should fail");
        assert_safe_error(
            &err,
            "read managed runtime pid",
            &[paths.pid_file_path().to_string_lossy().as_ref()],
        );
    }

    #[test]
    fn remove_path_if_exists_handles_files_directories_and_missing_paths() {
        let dir = tempdir().expect("tempdir");
        let file_path = dir.path().join("file.txt");
        let dir_path = dir.path().join("subdir");
        fs::write(&file_path, "data").expect("file");
        fs::create_dir(&dir_path).expect("dir");

        remove_path_if_exists(&file_path).expect("remove file");
        remove_path_if_exists(&dir_path).expect("remove dir");
        remove_path_if_exists(dir.path().join("missing").as_path()).expect("remove missing");

        assert!(!file_path.exists());
        assert!(!dir_path.exists());
    }

    #[test]
    fn remove_path_from_state_reports_dir_file_and_metadata_errors() {
        let dir = tempdir().expect("tempdir");
        let dir_path = dir.path().join("subdir");
        let file_path = dir.path().join("file.txt");
        let metadata_path = dir.path().join("metadata");
        ok_remove_path(Path::new("/")).expect("noop remove path");

        let dir_err = remove_path_from_state(
            &dir_path,
            Ok(Some(ExistingPathKind::Directory)),
            deny_remove_path,
            ok_remove_path,
        )
        .expect_err("directory removal should fail");
        assert_safe_error(
            &dir_err,
            "remove manager-owned runtime path",
            &[dir_path.to_string_lossy().as_ref(), "remove path denied"],
        );

        let file_err = remove_path_from_state(
            &file_path,
            Ok(Some(ExistingPathKind::File)),
            ok_remove_path,
            deny_remove_path,
        )
        .expect_err("file removal should fail");
        assert_safe_error(
            &file_err,
            "remove manager-owned runtime path",
            &[file_path.to_string_lossy().as_ref(), "remove path denied"],
        );

        let metadata_err = remove_path_from_state(
            &metadata_path,
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "metadata lookup failed",
            )),
            ok_remove_path,
            ok_remove_path,
        )
        .expect_err("metadata lookup should fail");
        assert_safe_error(
            &metadata_err,
            "read managed runtime file",
            &[
                metadata_path.to_string_lossy().as_ref(),
                "metadata lookup failed",
            ],
        );
    }

    #[test]
    fn remove_pid_file_reports_directory_errors() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        ensure_instance_layout(&paths).expect("layout");
        fs::create_dir(paths.pid_file_path()).expect("occupy pid path");

        let err = super::remove_pid_file(&paths).expect_err("directory pid path should fail");
        assert_safe_error(
            &err,
            "remove manager-owned runtime path",
            &[paths.pid_file_path().to_string_lossy().as_ref()],
        );
    }

    #[test]
    fn remove_pid_file_accepts_missing_pid_paths() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        super::remove_pid_file(&paths).expect("missing pid file should be ignored");
    }

    #[cfg(unix)]
    #[test]
    fn executable_mode_reports_missing_path_errors() {
        let dir = tempdir().expect("tempdir");
        let missing = dir.path().join("missing");

        let err = set_executable_mode(&missing).expect_err("missing executable should fail");
        assert_safe_error(
            &err,
            "read managed runtime file",
            &[missing.to_string_lossy().as_ref()],
        );
    }

    #[cfg(unix)]
    #[test]
    fn apply_mode_reports_set_permissions_errors() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("radrootsd");
        fs::write(&path, "binary").expect("binary");

        let err = apply_mode(&path, 0o755, |_path, _permissions| {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "set permissions failed",
            ))
        })
        .expect_err("set permissions should fail");
        assert_safe_error(
            &err,
            "set managed runtime file permissions",
            &[path.to_string_lossy().as_ref(), "set permissions failed"],
        );
    }

    #[cfg(unix)]
    #[test]
    fn signal_helpers_cover_failure_paths() {
        let missing_pid = 999_999_u32;
        assert!(!process_running_for_pid(missing_pid));

        let err = terminate_process(missing_pid).expect_err("terminate should fail");
        assert_safe_error(&err, "managed runtime process did not stop", &["999999"]);

        let err = force_kill_process(missing_pid).expect_err("force kill should fail");
        assert_safe_error(&err, "managed runtime process did not stop", &["999999"]);

        let err = signal_process(missing_pid, "-BOGUS").expect_err("invalid signal should fail");
        assert_safe_error(&err, "managed runtime process did not stop", &["999999"]);
    }

    #[cfg(unix)]
    #[test]
    fn signal_process_with_reports_execution_errors() {
        let err = signal_process_with(42, "-TERM", |_pid, _signal| {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "kill executable missing",
            ))
        })
        .expect_err("signal execution should fail");
        assert_safe_error(
            &err,
            "signal managed runtime process",
            &["42", "-TERM", "kill executable missing"],
        );
    }

    #[cfg(unix)]
    #[test]
    fn process_running_state_from_ps_output_handles_non_success_and_zombies() {
        assert!(process_running_state_from_ps_output(output_with_status(
            exit_status(1),
            b"",
        )));
        assert!(!process_running_state_from_ps_output(output_with_status(
            exit_status(0),
            b"Z+",
        )));
        assert!(process_running_state_from_ps_output(output_with_status(
            exit_status(0),
            b"S+",
        )));
    }
}
