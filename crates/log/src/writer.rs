use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::options::LogRotation;

pub(crate) struct SizeRotatingWriter {
    path: PathBuf,
    file: Option<File>,
    bytes_written: u64,
    policy: LogRotation,
}

impl SizeRotatingWriter {
    pub(crate) fn new(path: PathBuf, policy: LogRotation) -> io::Result<Self> {
        validate_policy(policy)?;
        reject_unsafe_target(&path)?;
        let file = open_append(&path)?;
        let bytes_written = file.metadata()?.len();
        let mut writer = Self {
            path,
            file: Some(file),
            bytes_written,
            policy,
        };
        if writer.bytes_written >= writer.policy.max_file_bytes {
            writer.rotate()?;
        }
        Ok(writer)
    }

    fn rotate(&mut self) -> io::Result<()> {
        if let Some(mut file) = self.file.take() {
            file.flush()?;
            file.sync_data()?;
        }
        if self.policy.retained_files == 1 {
            remove_if_present(&self.path)?;
        } else {
            let oldest = rotated_path(&self.path, self.policy.retained_files - 1);
            remove_if_present(&oldest)?;
            for index in (2..self.policy.retained_files).rev() {
                let source = rotated_path(&self.path, index - 1);
                let target = rotated_path(&self.path, index);
                rename_if_present(&source, &target)?;
            }
            rename_if_present(&self.path, &rotated_path(&self.path, 1))?;
        }
        self.file = Some(open_append(&self.path)?);
        self.bytes_written = 0;
        Ok(())
    }

    fn file_mut(&mut self) -> io::Result<&mut File> {
        self.file
            .as_mut()
            .ok_or_else(|| io::Error::other("log file is unavailable"))
    }
}

impl Write for SizeRotatingWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let incoming = u64::try_from(buffer.len())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "log event is too large"))?;
        if incoming > self.policy.max_file_bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "log event exceeds the configured file limit",
            ));
        }
        if self.bytes_written > 0
            && self.bytes_written.saturating_add(incoming) > self.policy.max_file_bytes
        {
            self.rotate()?;
        }
        let written = self.file_mut()?.write(buffer)?;
        self.bytes_written = self
            .bytes_written
            .saturating_add(u64::try_from(written).unwrap_or(u64::MAX));
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file_mut()?.flush()
    }
}

fn validate_policy(policy: LogRotation) -> io::Result<()> {
    if policy.max_file_bytes == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "max_file_bytes must be greater than zero",
        ));
    }
    if policy.retained_files == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "retained_files must be greater than zero",
        ));
    }
    Ok(())
}

fn reject_unsafe_target(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => Err(
            io::Error::new(io::ErrorKind::InvalidInput, "log target is not a safe file"),
        ),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn open_append(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        options.mode(0o600);
        let file = options.open(path)?;
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
        Ok(file)
    }
    #[cfg(not(unix))]
    options.open(path)
}

fn rotated_path(path: &Path, index: usize) -> PathBuf {
    let mut value = path.as_os_str().to_owned();
    value.push(format!(".{index}"));
    PathBuf::from(value)
}

fn remove_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn rename_if_present(source: &Path, target: &Path) -> io::Result<()> {
    match fs::rename(source, target) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::{SizeRotatingWriter, rotated_path};
    use crate::LogRotation;
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(1);

    fn temp_log_path(name: &str) -> PathBuf {
        let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "radroots-log-{name}-{}-{sequence}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create log test directory");
        dir.join("service.jsonl")
    }

    #[test]
    fn rotates_before_crossing_limit_and_bounds_retention() {
        let path = temp_log_path("rotation");
        let mut writer = SizeRotatingWriter::new(
            path.clone(),
            LogRotation {
                max_file_bytes: 5,
                retained_files: 3,
            },
        )
        .expect("writer");
        writer.write_all(b"1111").expect("first");
        writer.write_all(b"2222").expect("second");
        writer.write_all(b"3333").expect("third");
        writer.write_all(b"4444").expect("fourth");
        writer.flush().expect("flush");

        assert_eq!(std::fs::read(&path).expect("current"), b"4444");
        assert_eq!(
            std::fs::read(rotated_path(&path, 1)).expect("first retained"),
            b"3333"
        );
        assert_eq!(
            std::fs::read(rotated_path(&path, 2)).expect("second retained"),
            b"2222"
        );
        assert!(!rotated_path(&path, 3).exists());
        std::fs::remove_dir_all(path.parent().expect("parent")).expect("cleanup");
    }

    #[test]
    fn rejects_an_event_larger_than_the_file_limit() {
        let path = temp_log_path("oversized");
        let mut writer = SizeRotatingWriter::new(
            path.clone(),
            LogRotation {
                max_file_bytes: 4,
                retained_files: 2,
            },
        )
        .expect("writer");
        let error = writer.write_all(b"12345").expect_err("oversized event");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert_eq!(std::fs::metadata(&path).expect("metadata").len(), 0);
        std::fs::remove_dir_all(path.parent().expect("parent")).expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_targets_and_uses_private_file_mode() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let path = temp_log_path("symlink");
        let writer = SizeRotatingWriter::new(path.clone(), LogRotation::default()).expect("writer");
        drop(writer);
        assert_eq!(
            std::fs::metadata(&path)
                .expect("metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        std::fs::remove_file(&path).expect("remove target");
        let outside = path.with_file_name("outside");
        std::fs::write(&outside, b"").expect("outside");
        symlink(&outside, &path).expect("symlink");
        assert!(SizeRotatingWriter::new(path.clone(), LogRotation::default()).is_err());
        std::fs::remove_dir_all(path.parent().expect("parent")).expect("cleanup");
    }
}
