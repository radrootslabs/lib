use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use flate2::read::GzDecoder;

use crate::error::RadrootsRuntimeManagerError;
use crate::model::ManagedRuntimeInstanceRecord;
use crate::paths::ManagedRuntimeInstancePaths;

pub fn ensure_instance_layout(
    paths: &ManagedRuntimeInstancePaths,
) -> Result<(), RadrootsRuntimeManagerError> {
    for path in [
        &paths.install_dir,
        &paths.state_dir,
        &paths.logs_dir,
        &paths.run_dir,
        &paths.secrets_dir,
    ] {
        fs::create_dir_all(path).map_err(|source| {
            RadrootsRuntimeManagerError::CreateDirectory {
                path: path.clone(),
                source,
            }
        })?;
    }
    Ok(())
}

pub fn install_binary(
    source_binary_path: impl AsRef<Path>,
    paths: &ManagedRuntimeInstancePaths,
    binary_name: &str,
) -> Result<PathBuf, RadrootsRuntimeManagerError> {
    let source_binary_path = source_binary_path.as_ref();
    ensure_instance_layout(paths)?;
    let installed_binary_path = paths.install_dir.join(binary_name);
    fs::copy(source_binary_path, &installed_binary_path).map_err(|source| {
        RadrootsRuntimeManagerError::CopyBinary {
            from: source_binary_path.to_path_buf(),
            to: installed_binary_path.clone(),
            source,
        }
    })?;
    set_executable_mode(&installed_binary_path)?;
    Ok(installed_binary_path)
}

pub fn extract_binary_archive(
    archive_path: impl AsRef<Path>,
    archive_format: &str,
    paths: &ManagedRuntimeInstancePaths,
    binary_name: &str,
) -> Result<PathBuf, RadrootsRuntimeManagerError> {
    let archive_path = archive_path.as_ref();
    remove_path_if_exists(&paths.install_dir)?;
    ensure_instance_layout(paths)?;

    match archive_format {
        "tar.gz" => unpack_tar_gz_archive(archive_path, &paths.install_dir)?,
        other => {
            return Err(RadrootsRuntimeManagerError::UnsupportedArchiveFormat {
                archive_path: archive_path.to_path_buf(),
                archive_format: other.to_owned(),
            });
        }
    }

    let installed_binary_path = paths.install_dir.join(binary_name);
    let resolved_binary_path = if installed_binary_path.is_file() {
        installed_binary_path
    } else {
        find_binary_with_name(&paths.install_dir, binary_name).ok_or_else(|| {
            RadrootsRuntimeManagerError::ReadManagedFile {
                path: paths.install_dir.join(binary_name),
                source: std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!(
                        "archive {} did not produce a `{binary_name}` binary under {}",
                        archive_path.display(),
                        paths.install_dir.display()
                    ),
                ),
            }
        })?
    };
    set_executable_mode(&resolved_binary_path)?;
    Ok(resolved_binary_path)
}

pub fn write_instance_metadata(
    paths: &ManagedRuntimeInstancePaths,
    record: &ManagedRuntimeInstanceRecord,
) -> Result<(), RadrootsRuntimeManagerError> {
    ensure_instance_layout(paths)?;
    let raw = toml::to_string_pretty(record).map_err(|details| {
        RadrootsRuntimeManagerError::SerializeInstanceMetadata(details.to_string())
    })?;
    fs::write(&paths.metadata_path, raw).map_err(|source| {
        RadrootsRuntimeManagerError::WriteInstanceMetadata {
            path: paths.metadata_path.clone(),
            source,
        }
    })
}

pub fn write_managed_file(
    path: impl AsRef<Path>,
    contents: &str,
) -> Result<(), RadrootsRuntimeManagerError> {
    let path = path.as_ref();
    ensure_parent_dir(path)?;
    fs::write(path, contents).map_err(|source| RadrootsRuntimeManagerError::WriteManagedFile {
        path: path.to_path_buf(),
        source,
    })
}

pub fn write_secret_file(
    path: impl AsRef<Path>,
    contents: &str,
) -> Result<(), RadrootsRuntimeManagerError> {
    let path = path.as_ref();
    ensure_parent_dir(path)?;
    fs::write(path, contents).map_err(|source| RadrootsRuntimeManagerError::WriteManagedFile {
        path: path.to_path_buf(),
        source,
    })?;
    set_secret_mode(path)?;
    Ok(())
}

pub fn read_secret_file(path: impl AsRef<Path>) -> Result<String, RadrootsRuntimeManagerError> {
    let path = path.as_ref();
    fs::read_to_string(path).map_err(|source| RadrootsRuntimeManagerError::ReadManagedFile {
        path: path.to_path_buf(),
        source,
    })
}

pub fn start_process(
    binary_path: impl AsRef<Path>,
    args: &[String],
    envs: &[(String, String)],
    paths: &ManagedRuntimeInstancePaths,
) -> Result<u32, RadrootsRuntimeManagerError> {
    let binary_path = binary_path.as_ref();
    ensure_instance_layout(paths)?;
    let stdout = open_log_file(&paths.stdout_log_path)?;
    let stderr = open_log_file(&paths.stderr_log_path)?;
    let child = Command::new(binary_path)
        .args(args)
        .envs(envs.iter().map(|(key, value)| (key, value)))
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .map_err(|source| RadrootsRuntimeManagerError::SpawnProcess {
            binary_path: binary_path.to_path_buf(),
            source,
        })?;
    let pid = child.id();
    fs::write(&paths.pid_file_path, pid.to_string()).map_err(|source| {
        RadrootsRuntimeManagerError::WritePidFile {
            path: paths.pid_file_path.clone(),
            source,
        }
    })?;
    Ok(pid)
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

    terminate_process(pid)?;
    for _ in 0..20 {
        if !process_running_for_pid(pid) {
            remove_pid_file(paths)?;
            return Ok(true);
        }
        thread::sleep(Duration::from_millis(100));
    }

    force_kill_process(pid)?;
    for _ in 0..20 {
        if !process_running_for_pid(pid) {
            remove_pid_file(paths)?;
            return Ok(true);
        }
        thread::sleep(Duration::from_millis(100));
    }

    Err(RadrootsRuntimeManagerError::StopProcess {
        pid,
        details: "process did not exit after terminate and force-kill attempts".to_owned(),
    })
}

pub fn remove_instance_artifacts(
    paths: &ManagedRuntimeInstancePaths,
) -> Result<(), RadrootsRuntimeManagerError> {
    for path in [
        &paths.install_dir,
        &paths.state_dir,
        &paths.logs_dir,
        &paths.run_dir,
        &paths.secrets_dir,
    ] {
        remove_path_if_exists(path)?;
    }
    Ok(())
}

fn unpack_tar_gz_archive(
    archive_path: &Path,
    destination_dir: &Path,
) -> Result<(), RadrootsRuntimeManagerError> {
    let archive_file = File::open(archive_path).map_err(|source| {
        RadrootsRuntimeManagerError::ReadManagedFile {
            path: archive_path.to_path_buf(),
            source,
        }
    })?;
    let decoder = GzDecoder::new(archive_file);
    let mut archive = tar::Archive::new(decoder);
    archive
        .unpack(destination_dir)
        .map_err(|source| RadrootsRuntimeManagerError::UnpackArchive {
            archive_path: archive_path.to_path_buf(),
            source,
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
            path: path.to_path_buf(),
            source,
        })
}

fn ensure_parent_dir(path: &Path) -> Result<(), RadrootsRuntimeManagerError> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    fs::create_dir_all(parent).map_err(|source| RadrootsRuntimeManagerError::CreateDirectory {
        path: parent.to_path_buf(),
        source,
    })
}

fn read_pid(
    paths: &ManagedRuntimeInstancePaths,
) -> Result<Option<u32>, RadrootsRuntimeManagerError> {
    let raw = match fs::read_to_string(&paths.pid_file_path) {
        Ok(raw) => raw,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(RadrootsRuntimeManagerError::ReadPidFile {
                path: paths.pid_file_path.clone(),
                source,
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
        .map_err(|_| RadrootsRuntimeManagerError::ParsePidFile {
            path: paths.pid_file_path.clone(),
            contents: trimmed.to_owned(),
        })
}

fn remove_pid_file(paths: &ManagedRuntimeInstancePaths) -> Result<(), RadrootsRuntimeManagerError> {
    match fs::remove_file(&paths.pid_file_path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(RadrootsRuntimeManagerError::RemovePath {
            path: paths.pid_file_path.clone(),
            source,
        }),
    }
}

fn remove_path_if_exists(path: &Path) -> Result<(), RadrootsRuntimeManagerError> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => {
            fs::remove_dir_all(path).map_err(|source| RadrootsRuntimeManagerError::RemovePath {
                path: path.to_path_buf(),
                source,
            })
        }
        Ok(_) => fs::remove_file(path).map_err(|source| RadrootsRuntimeManagerError::RemovePath {
            path: path.to_path_buf(),
            source,
        }),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(RadrootsRuntimeManagerError::ReadManagedFile {
            path: path.to_path_buf(),
            source,
        }),
    }
}

#[cfg(unix)]
fn set_executable_mode(path: &Path) -> Result<(), RadrootsRuntimeManagerError> {
    use std::os::unix::fs::PermissionsExt;

    let metadata =
        fs::metadata(path).map_err(|source| RadrootsRuntimeManagerError::ReadManagedFile {
            path: path.to_path_buf(),
            source,
        })?;
    let mut permissions = metadata.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).map_err(|source| {
        RadrootsRuntimeManagerError::SetPermissions {
            path: path.to_path_buf(),
            source,
        }
    })
}

#[cfg(not(unix))]
fn set_executable_mode(_path: &Path) -> Result<(), RadrootsRuntimeManagerError> {
    Ok(())
}

#[cfg(unix)]
fn set_secret_mode(path: &Path) -> Result<(), RadrootsRuntimeManagerError> {
    use std::os::unix::fs::PermissionsExt;

    let metadata =
        fs::metadata(path).map_err(|source| RadrootsRuntimeManagerError::ReadManagedFile {
            path: path.to_path_buf(),
            source,
        })?;
    let mut permissions = metadata.permissions();
    permissions.set_mode(0o600);
    fs::set_permissions(path, permissions).map_err(|source| {
        RadrootsRuntimeManagerError::SetPermissions {
            path: path.to_path_buf(),
            source,
        }
    })
}

#[cfg(not(unix))]
fn set_secret_mode(_path: &Path) -> Result<(), RadrootsRuntimeManagerError> {
    Ok(())
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

    Command::new("ps")
        .args(["-o", "stat=", "-p", pid_arg.as_str()])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .map(|output| {
            if !output.status.success() {
                return true;
            }
            let state = String::from_utf8_lossy(output.stdout.as_slice());
            !state.trim_start().starts_with('Z')
        })
        .unwrap_or(true)
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
    let status = Command::new("kill")
        .args([signal, pid.to_string().as_str()])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()
        .map_err(|source| RadrootsRuntimeManagerError::ExecuteProcessSignal {
            pid,
            signal: signal.to_owned(),
            source,
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(RadrootsRuntimeManagerError::StopProcess {
            pid,
            details: format!("`kill {signal}` returned {status}"),
        })
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
            pid,
            signal: "taskkill".to_owned(),
            source,
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(RadrootsRuntimeManagerError::StopProcess {
            pid,
            details: format!("`taskkill` returned {status}"),
        })
    }
}

#[cfg(not(any(unix, windows)))]
fn terminate_process(pid: u32) -> Result<(), RadrootsRuntimeManagerError> {
    Err(RadrootsRuntimeManagerError::StopProcess {
        pid,
        details: "process signaling is unsupported on this platform".to_owned(),
    })
}

#[cfg(not(any(unix, windows)))]
fn force_kill_process(pid: u32) -> Result<(), RadrootsRuntimeManagerError> {
    Err(RadrootsRuntimeManagerError::StopProcess {
        pid,
        details: "process signaling is unsupported on this platform".to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::fs::File;
    use std::path::Path;
    use std::thread;
    use std::time::Duration;

    use tempfile::tempdir;

    use super::{
        ensure_instance_layout, ensure_parent_dir, extract_binary_archive, find_binary_with_name,
        force_kill_process, install_binary, open_log_file, process_running,
        process_running_for_pid, read_pid, read_secret_file, remove_instance_artifacts,
        remove_path_if_exists, set_executable_mode, set_secret_mode, signal_process, start_process,
        stop_process, terminate_process, write_instance_metadata, write_managed_file,
        write_secret_file,
    };
    use crate::error::RadrootsRuntimeManagerError;
    use crate::model::{ManagedRuntimeInstallState, ManagedRuntimeInstanceRecord};
    use crate::paths::ManagedRuntimeInstancePaths;

    fn sample_paths(root: &Path) -> ManagedRuntimeInstancePaths {
        ManagedRuntimeInstancePaths {
            install_dir: root.join("install"),
            state_dir: root.join("state"),
            logs_dir: root.join("logs"),
            run_dir: root.join("run"),
            secrets_dir: root.join("secrets"),
            pid_file_path: root.join("run/runtime.pid"),
            stdout_log_path: root.join("logs/stdout.log"),
            stderr_log_path: root.join("logs/stderr.log"),
            metadata_path: root.join("state/instance.toml"),
        }
    }

    fn assert_error_contains(err: &RadrootsRuntimeManagerError, parts: &[&str]) {
        let rendered = err.to_string();
        for part in parts {
            assert!(
                rendered.contains(part),
                "expected `{rendered}` to contain `{part}`"
            );
        }
    }

    #[test]
    fn layout_and_metadata_helpers_write_expected_files() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        ensure_instance_layout(&paths).expect("layout");
        write_managed_file(paths.state_dir.join("config.toml"), "value = true").expect("config");
        write_secret_file(paths.secrets_dir.join("token.txt"), "secret").expect("secret");
        write_instance_metadata(
            &paths,
            &ManagedRuntimeInstanceRecord {
                runtime_id: "radrootsd".to_owned(),
                instance_id: "local".to_owned(),
                management_mode: "interactive_user_managed".to_owned(),
                install_state: ManagedRuntimeInstallState::Configured,
                binary_path: paths.install_dir.join("radrootsd"),
                config_path: paths.state_dir.join("config.toml"),
                logs_path: paths.logs_dir.clone(),
                run_path: paths.run_dir.clone(),
                installed_version: "0.1.0".to_owned(),
                health_endpoint: Some("http://127.0.0.1:7070".to_owned()),
                secret_material_ref: Some(
                    paths.secrets_dir.join("token.txt").display().to_string(),
                ),
                last_started_at: None,
                last_stopped_at: None,
                notes: Some("test".to_owned()),
            },
        )
        .expect("metadata");
        assert_eq!(
            read_secret_file(paths.secrets_dir.join("token.txt")).expect("read secret"),
            "secret"
        );
        assert!(paths.metadata_path.is_file());
        assert!(paths.state_dir.join("config.toml").is_file());
    }

    #[test]
    fn install_binary_copies_source_into_install_dir() {
        let dir = tempdir().expect("tempdir");
        let source = dir.path().join("radrootsd");
        fs::write(&source, "#!/bin/sh\nexit 0\n").expect("source");
        let paths = sample_paths(dir.path());
        let installed = install_binary(&source, &paths, "radrootsd").expect("install");
        assert!(installed.is_file());
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
            extract_binary_archive(&archive_path, "tar.gz", &paths, "radrootsd").expect("extract");
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
            extract_binary_archive(&archive_path, "tar.gz", &paths, "radrootsd").expect("extract");
        assert_eq!(installed, paths.install_dir.join("radrootsd"));
    }

    #[cfg(unix)]
    #[test]
    fn start_and_stop_process_manage_pid_file() {
        let dir = tempdir().expect("tempdir");
        let binary = dir.path().join("sleepy.sh");
        fs::write(&binary, "#!/bin/sh\nexec sleep 30\n").expect("script");
        let paths = sample_paths(dir.path());
        let installed = install_binary(&binary, &paths, "sleepy.sh").expect("install");
        let pid = start_process(&installed, &Vec::new(), &Vec::new(), &paths).expect("start");
        assert!(pid > 0);
        thread::sleep(Duration::from_millis(100));
        assert!(paths.pid_file_path.is_file());
        assert!(process_running(&paths).expect("running"));
        assert!(stop_process(&paths).expect("stop"));
        assert!(!paths.pid_file_path.exists());
    }

    #[test]
    fn remove_instance_artifacts_removes_layout_roots() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        ensure_instance_layout(&paths).expect("layout");
        remove_instance_artifacts(&paths).expect("remove");
        assert!(!paths.install_dir.exists());
        assert!(!paths.state_dir.exists());
        assert!(!paths.logs_dir.exists());
        assert!(!paths.run_dir.exists());
        assert!(!paths.secrets_dir.exists());
    }

    #[test]
    fn ensure_instance_layout_reports_directory_errors() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        fs::write(&paths.install_dir, "occupied").expect("file");

        let err = ensure_instance_layout(&paths).expect_err("file path should fail");
        assert_error_contains(
            &err,
            &[
                paths.install_dir.to_string_lossy().as_ref(),
                "create directory",
            ],
        );
    }

    #[test]
    fn install_binary_reports_copy_errors() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        let err = install_binary(dir.path().join("missing"), &paths, "radrootsd")
            .expect_err("missing source should fail");
        assert_error_contains(
            &err,
            &[
                dir.path().join("missing").to_string_lossy().as_ref(),
                paths
                    .install_dir
                    .join("radrootsd")
                    .to_string_lossy()
                    .as_ref(),
                "copy runtime binary",
            ],
        );
    }

    #[test]
    fn extract_binary_archive_reports_unsupported_format() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        let archive_path = dir.path().join("radrootsd.zip");

        let err = extract_binary_archive(&archive_path, "zip", &paths, "radrootsd")
            .expect_err("unsupported archive format should fail");
        assert_error_contains(&err, &[archive_path.to_string_lossy().as_ref(), "zip"]);
    }

    #[test]
    fn extract_binary_archive_reports_missing_archive() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        let archive_path = dir.path().join("missing.tar.gz");

        let err = extract_binary_archive(&archive_path, "tar.gz", &paths, "radrootsd")
            .expect_err("missing archive should fail");
        assert_error_contains(
            &err,
            &[archive_path.to_string_lossy().as_ref(), "read managed file"],
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
        let err = extract_binary_archive(&archive_path, "tar.gz", &paths, "radrootsd")
            .expect_err("archive should not resolve missing binary");
        assert_error_contains(
            &err,
            &[
                paths
                    .install_dir
                    .join("radrootsd")
                    .to_string_lossy()
                    .as_ref(),
                "did not produce",
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

        let err = extract_binary_archive(&archive_path, "tar.gz", &paths, "radrootsd")
            .expect_err("invalid archive should fail");
        assert_error_contains(
            &err,
            &[archive_path.to_string_lossy().as_ref(), "unpack archive"],
        );
    }

    #[test]
    fn write_managed_file_reports_write_errors() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("as-dir");
        fs::create_dir(&path).expect("create directory target");

        let err = write_managed_file(&path, "value").expect_err("directory write should fail");
        assert_error_contains(
            &err,
            &[path.to_string_lossy().as_ref(), "write managed file"],
        );
    }

    #[test]
    fn write_secret_file_reports_write_errors() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("as-dir");
        fs::create_dir(&path).expect("create directory target");

        let err = write_secret_file(&path, "secret").expect_err("directory write should fail");
        assert_error_contains(
            &err,
            &[path.to_string_lossy().as_ref(), "write managed file"],
        );
    }

    #[test]
    fn read_secret_file_reports_missing_path() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("missing.secret");
        let err = read_secret_file(&path).expect_err("missing secret should fail");
        assert_error_contains(
            &err,
            &[path.to_string_lossy().as_ref(), "read managed file"],
        );
    }

    #[test]
    fn write_instance_metadata_reports_write_errors() {
        let dir = tempdir().expect("tempdir");
        let mut paths = sample_paths(dir.path());
        ensure_instance_layout(&paths).expect("layout");
        fs::create_dir_all(&paths.metadata_path).expect("create metadata dir");
        paths.metadata_path = paths.metadata_path.clone();

        let err = write_instance_metadata(
            &paths,
            &ManagedRuntimeInstanceRecord {
                runtime_id: "radrootsd".to_owned(),
                instance_id: "local".to_owned(),
                management_mode: "interactive_user_managed".to_owned(),
                install_state: ManagedRuntimeInstallState::Configured,
                binary_path: paths.install_dir.join("radrootsd"),
                config_path: paths.state_dir.join("config.toml"),
                logs_path: paths.logs_dir.clone(),
                run_path: paths.run_dir.clone(),
                installed_version: "0.1.0".to_owned(),
                health_endpoint: None,
                secret_material_ref: None,
                last_started_at: None,
                last_stopped_at: None,
                notes: None,
            },
        )
        .expect_err("metadata dir target should fail");
        assert_error_contains(
            &err,
            &[
                paths.metadata_path.to_string_lossy().as_ref(),
                "write runtime instance metadata",
            ],
        );
    }

    #[test]
    fn start_process_reports_spawn_errors() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        let err = start_process(dir.path().join("missing"), &[], &[], &paths)
            .expect_err("missing binary should fail");
        assert_error_contains(
            &err,
            &[
                dir.path().join("missing").to_string_lossy().as_ref(),
                "spawn managed runtime process",
            ],
        );
    }

    #[cfg(unix)]
    #[test]
    fn start_process_reports_pid_file_write_errors() {
        let dir = tempdir().expect("tempdir");
        let binary = dir.path().join("sleepy.sh");
        fs::write(&binary, "#!/bin/sh\nexec sleep 1\n").expect("script");
        let mut paths = sample_paths(dir.path());
        paths.pid_file_path = paths.run_dir.clone();
        let installed =
            install_binary(&binary, &sample_paths(dir.path()), "sleepy.sh").expect("install");

        let err =
            start_process(&installed, &[], &[], &paths).expect_err("pid file write should fail");
        assert_error_contains(
            &err,
            &[paths.run_dir.to_string_lossy().as_ref(), "write pid file"],
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
        fs::write(&paths.pid_file_path, "not-a-pid").expect("write pid");

        let err = process_running(&paths).expect_err("invalid pid should fail");
        assert_error_contains(
            &err,
            &[paths.pid_file_path.to_string_lossy().as_ref(), "not-a-pid"],
        );
    }

    #[test]
    fn stop_process_clears_stale_pid_file() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        ensure_instance_layout(&paths).expect("layout");
        fs::write(&paths.pid_file_path, "999999").expect("write pid");

        assert!(!stop_process(&paths).expect("stale pid should return false"));
        assert!(!paths.pid_file_path.exists());
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
        assert_error_contains(
            &err,
            &[file_parent.to_string_lossy().as_ref(), "create directory"],
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
        assert_error_contains(
            &err,
            &[bad_path.to_string_lossy().as_ref(), "open runtime log file"],
        );
    }

    #[test]
    fn read_pid_handles_empty_missing_and_read_error_cases() {
        let dir = tempdir().expect("tempdir");
        let mut paths = sample_paths(dir.path());

        assert_eq!(read_pid(&paths).expect("missing pid"), None);

        ensure_instance_layout(&paths).expect("layout");
        fs::write(&paths.pid_file_path, "   ").expect("write pid");
        assert_eq!(read_pid(&paths).expect("empty pid"), None);

        paths.pid_file_path = paths.run_dir.clone();
        let err = read_pid(&paths).expect_err("directory pid file should fail");
        assert_error_contains(
            &err,
            &[paths.run_dir.to_string_lossy().as_ref(), "read pid file"],
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
    fn remove_pid_file_reports_directory_errors() {
        let dir = tempdir().expect("tempdir");
        let mut paths = sample_paths(dir.path());
        ensure_instance_layout(&paths).expect("layout");
        paths.pid_file_path = paths.run_dir.clone();

        let err = super::remove_pid_file(&paths).expect_err("directory pid path should fail");
        assert_error_contains(
            &err,
            &[
                paths.run_dir.to_string_lossy().as_ref(),
                "remove managed path",
            ],
        );
    }

    #[cfg(unix)]
    #[test]
    fn set_mode_helpers_report_missing_path_errors() {
        let dir = tempdir().expect("tempdir");
        let missing = dir.path().join("missing");

        let err = set_executable_mode(&missing).expect_err("missing executable should fail");
        assert_error_contains(
            &err,
            &[missing.to_string_lossy().as_ref(), "read managed file"],
        );

        let err = set_secret_mode(&missing).expect_err("missing secret should fail");
        assert_error_contains(
            &err,
            &[missing.to_string_lossy().as_ref(), "read managed file"],
        );
    }

    #[cfg(unix)]
    #[test]
    fn signal_helpers_cover_failure_paths() {
        let missing_pid = 999_999_u32;
        assert!(!process_running_for_pid(missing_pid));

        let err = terminate_process(missing_pid).expect_err("terminate should fail");
        assert_error_contains(&err, &[&missing_pid.to_string(), "stop pid"]);

        let err = force_kill_process(missing_pid).expect_err("force kill should fail");
        assert_error_contains(&err, &[&missing_pid.to_string(), "stop pid"]);

        let err = signal_process(missing_pid, "-BOGUS").expect_err("invalid signal should fail");
        assert_error_contains(&err, &[&missing_pid.to_string(), "stop pid"]);
    }
}
