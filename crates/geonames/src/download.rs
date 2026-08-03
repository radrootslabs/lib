//! Explicit, caller-driven asset acquisition.

use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use fs2::FileExt;
use tempfile::NamedTempFile;

use crate::asset::{inspect, io_error, verify_file};
use crate::{AssetSpec, AssetStatus, Error};

/// Stable phase attached to an injected fetch failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum FetchFailurePhase {
    /// Establishing the source connection or opening the source.
    Connect,
    /// Receiving source metadata or an initial response.
    Response,
    /// Streaming source bytes.
    Read,
    /// Caller-requested cancellation.
    Cancelled,
}

impl fmt::Display for FetchFailurePhase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Connect => "connect",
            Self::Response => "response",
            Self::Read => "read",
            Self::Cancelled => "cancellation",
        })
    }
}

/// Explicit byte source supplied by the host.
///
/// Implementations own transport execution and cancellation. Failure details
/// cannot enter this API, preventing source URLs or credentials from leaking.
pub trait Fetcher {
    /// Streams the requested source into the bounded destination.
    fn fetch(&self, source: &str, destination: &mut dyn Write) -> Result<(), Error>;
}

/// Installs a verified asset into a caller-selected existing directory.
///
/// The fetcher is invoked only when the final asset is absent or invalid. The
/// existing destination remains untouched until a fully written staging file
/// passes exact length and SHA-256 validation.
pub fn acquire(
    directory: impl AsRef<Path>,
    spec: &AssetSpec,
    fetcher: &dyn Fetcher,
) -> Result<AssetStatus, Error> {
    let directory = safe_directory(directory.as_ref())?;
    let destination = directory.join(spec.file_name());
    match inspect(&destination, spec)? {
        AssetStatus::Available => return Ok(AssetStatus::Available),
        AssetStatus::Missing | AssetStatus::Invalid => {}
    }

    reject_symlink(&destination)?;
    let lock_path = directory.join(format!(".{}.lock", spec.file_name()));
    reject_symlink(&lock_path)?;
    let lock = open_lock(&lock_path)?;
    lock.try_lock_exclusive()
        .map_err(|_| Error::AssetDestinationBusy)?;

    if inspect(&destination, spec)? == AssetStatus::Available {
        return Ok(AssetStatus::Available);
    }

    let mut staging = NamedTempFile::new_in(&directory)
        .map_err(|error| io_error("create asset staging file", error))?;
    let (fetch_result, observed, overflowed) = {
        let mut writer = BoundedWriter::new(staging.as_file_mut(), spec.byte_size());
        let fetch_result = fetcher.fetch(spec.source(), &mut writer);
        (fetch_result, writer.observed, writer.overflowed)
    };
    if overflowed {
        return Err(Error::AssetSizeMismatch {
            expected: spec.byte_size(),
            actual: spec.byte_size().saturating_add(1),
        });
    }
    fetch_result?;
    if observed != spec.byte_size() {
        return Err(Error::AssetSizeMismatch {
            expected: spec.byte_size(),
            actual: observed,
        });
    }
    staging
        .as_file_mut()
        .sync_all()
        .map_err(|error| io_error("sync asset staging file", error))?;
    verify_file(staging.path(), spec)?;
    reject_symlink(&destination)?;
    staging
        .persist(&destination)
        .map_err(|error| io_error("finalize asset", error.error))?;
    sync_directory(&directory)?;
    verify_file(&destination, spec)?;
    Ok(AssetStatus::Available)
}

fn safe_directory(directory: &Path) -> Result<PathBuf, Error> {
    let metadata = directory
        .symlink_metadata()
        .map_err(|error| io_error("inspect asset directory", error))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(Error::UnsafeAssetDestination);
    }
    directory
        .canonicalize()
        .map_err(|error| io_error("resolve asset directory", error))
}

fn reject_symlink(path: &Path) -> Result<(), Error> {
    match path.symlink_metadata() {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(Error::UnsafeAssetDestination),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error("inspect asset destination", error)),
    }
}

fn open_lock(path: &Path) -> Result<File, Error> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(|error| io_error("open asset lock", error))
}

#[cfg(unix)]
fn sync_directory(directory: &Path) -> Result<(), Error> {
    File::open(directory)
        .and_then(|file| file.sync_all())
        .map_err(|error| io_error("sync asset directory", error))
}

#[cfg(not(unix))]
fn sync_directory(_directory: &Path) -> Result<(), Error> {
    Ok(())
}

struct BoundedWriter<'a> {
    destination: &'a mut File,
    maximum: u64,
    observed: u64,
    overflowed: bool,
}

impl<'a> BoundedWriter<'a> {
    fn new(destination: &'a mut File, maximum: u64) -> Self {
        Self {
            destination,
            maximum,
            observed: 0,
            overflowed: false,
        }
    }
}

impl Write for BoundedWriter<'_> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let incoming = u64::try_from(buffer.len()).unwrap_or(u64::MAX);
        if self.observed.saturating_add(incoming) > self.maximum {
            self.overflowed = true;
            return Err(io::Error::new(
                io::ErrorKind::FileTooLarge,
                "asset exceeds declared size",
            ));
        }
        let written = self.destination.write(buffer)?;
        self.observed = self
            .observed
            .saturating_add(u64::try_from(written).unwrap_or(u64::MAX));
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.destination.flush()
    }
}

#[cfg(test)]
mod tests {
    use std::fs::{self, OpenOptions};
    use std::io::Write;

    use fs2::FileExt;
    use sha2::{Digest, Sha256};
    use tempfile::tempdir;

    use super::{BoundedWriter, FetchFailurePhase, Fetcher, acquire};
    use crate::asset::inspect;
    use crate::{AssetSpec, AssetStatus, Error};

    struct BytesFetcher(Vec<u8>);

    impl Fetcher for BytesFetcher {
        fn fetch(&self, _source: &str, destination: &mut dyn Write) -> Result<(), Error> {
            destination.write_all(&self.0).map_err(|_| Error::Fetch {
                phase: FetchFailurePhase::Read,
            })
        }
    }

    struct InterruptedFetcher;

    impl Fetcher for InterruptedFetcher {
        fn fetch(&self, _source: &str, destination: &mut dyn Write) -> Result<(), Error> {
            destination
                .write_all(b"partial")
                .map_err(|_| Error::Fetch {
                    phase: FetchFailurePhase::Read,
                })?;
            Err(Error::Fetch {
                phase: FetchFailurePhase::Cancelled,
            })
        }
    }

    struct PanicFetcher;

    impl Fetcher for PanicFetcher {
        fn fetch(&self, _source: &str, _destination: &mut dyn Write) -> Result<(), Error> {
            panic!("available assets must not invoke the fetcher")
        }
    }

    fn spec(bytes: &[u8]) -> AssetSpec {
        AssetSpec::new(
            "test-v1",
            "geonames-test.db",
            "https://assets.example/geonames-test.db",
            "assets.example",
            u64::try_from(bytes.len()).expect("fixture length"),
            Sha256::digest(bytes).into(),
        )
        .expect("asset spec")
    }

    #[test]
    fn missing_and_successful_acquisition_are_explicit() {
        let directory = tempdir().expect("tempdir");
        let bytes = b"verified geonames fixture";
        let spec = spec(bytes);
        let path = directory.path().join(spec.file_name());
        assert_eq!(inspect(&path, &spec), Ok(AssetStatus::Missing));
        assert_eq!(
            acquire(directory.path(), &spec, &BytesFetcher(bytes.to_vec())),
            Ok(AssetStatus::Available)
        );
        assert_eq!(inspect(&path, &spec), Ok(AssetStatus::Available));
        assert_eq!(fs::read(path).expect("read installed asset"), bytes);
    }

    #[test]
    fn interrupted_acquisition_leaves_no_destination_or_staging_file() {
        let directory = tempdir().expect("tempdir");
        let spec = spec(b"complete bytes");
        let error = acquire(directory.path(), &spec, &InterruptedFetcher)
            .expect_err("interrupted fetch must fail");
        assert!(matches!(
            error,
            Error::Fetch {
                phase: FetchFailurePhase::Cancelled
            }
        ));
        assert!(!directory.path().join(spec.file_name()).exists());
        let entries = fs::read_dir(directory.path())
            .expect("read directory")
            .filter_map(Result::ok)
            .map(|entry| entry.file_name())
            .collect::<Vec<_>>();
        assert_eq!(
            entries,
            vec![std::ffi::OsString::from(format!(
                ".{}.lock",
                spec.file_name()
            ))]
        );
    }

    #[test]
    fn oversized_and_hash_mismatched_streams_never_replace_existing_asset() {
        let directory = tempdir().expect("tempdir");
        let expected = b"expected";
        let spec = spec(expected);
        let path = directory.path().join(spec.file_name());
        fs::write(&path, b"old").expect("old destination");

        assert!(matches!(
            acquire(
                directory.path(),
                &spec,
                &BytesFetcher(b"expected-extra".to_vec())
            ),
            Err(Error::AssetSizeMismatch { .. })
        ));
        assert_eq!(fs::read(&path).expect("preserved old bytes"), b"old");

        assert_eq!(
            acquire(directory.path(), &spec, &BytesFetcher(b"notright".to_vec())),
            Err(Error::AssetHashMismatch)
        );
        assert_eq!(fs::read(path).expect("preserved old bytes"), b"old");
    }

    #[cfg(unix)]
    #[test]
    fn symlink_directories_and_destinations_are_rejected() {
        use std::os::unix::fs::symlink;

        let root = tempdir().expect("tempdir");
        let real = root.path().join("real");
        fs::create_dir(&real).expect("real directory");
        let linked = root.path().join("linked");
        symlink(&real, &linked).expect("directory symlink");
        let spec = spec(b"asset");
        assert_eq!(
            acquire(&linked, &spec, &BytesFetcher(b"asset".to_vec())),
            Err(Error::UnsafeAssetDestination)
        );

        let target = real.join(spec.file_name());
        let outside = root.path().join("outside");
        fs::write(&outside, b"outside").expect("outside file");
        symlink(&outside, &target).expect("destination symlink");
        assert_eq!(
            acquire(&real, &spec, &BytesFetcher(b"asset".to_vec())),
            Err(Error::UnsafeAssetDestination)
        );
    }

    #[test]
    fn available_short_busy_and_invalid_directory_paths_are_explicit() {
        let directory = tempdir().expect("tempdir");
        let bytes = b"asset";
        let spec = spec(bytes);
        fs::write(directory.path().join(spec.file_name()), bytes).expect("available asset");
        let owned_directory = directory.path().to_path_buf();
        assert_eq!(
            acquire(&owned_directory, &spec, &PanicFetcher),
            Ok(AssetStatus::Available)
        );

        fs::write(directory.path().join(spec.file_name()), b"old").expect("invalid asset");
        assert!(matches!(
            acquire(directory.path(), &spec, &BytesFetcher(b"a".to_vec())),
            Err(Error::AssetSizeMismatch {
                expected: 5,
                actual: 1
            })
        ));

        let lock_path = directory.path().join(format!(".{}.lock", spec.file_name()));
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(lock_path)
            .expect("lock file");
        lock.lock_exclusive().expect("exclusive lock");
        assert_eq!(
            acquire(directory.path(), &spec, &BytesFetcher(bytes.to_vec())),
            Err(Error::AssetDestinationBusy)
        );

        let not_directory = directory.path().join("plain-file");
        fs::write(&not_directory, b"file").expect("plain file");
        assert_eq!(
            acquire(&not_directory, &spec, &BytesFetcher(bytes.to_vec())),
            Err(Error::UnsafeAssetDestination)
        );
        assert!(matches!(
            acquire(
                directory.path().join("missing"),
                &spec,
                &BytesFetcher(bytes.to_vec())
            ),
            Err(Error::Io {
                operation: "inspect asset directory",
                kind: std::io::ErrorKind::NotFound,
            })
        ));
    }

    #[test]
    fn fetch_phases_and_bounded_writer_flush_are_covered() {
        assert_eq!(FetchFailurePhase::Connect.to_string(), "connect");
        assert_eq!(FetchFailurePhase::Response.to_string(), "response");
        assert_eq!(FetchFailurePhase::Read.to_string(), "read");
        assert_eq!(FetchFailurePhase::Cancelled.to_string(), "cancellation");

        let mut file = tempfile::tempfile().expect("temporary file");
        let mut writer = BoundedWriter::new(&mut file, 4);
        writer.write_all(b"data").expect("bounded write");
        writer.flush().expect("flush");
        assert_eq!(writer.observed, 4);
        assert!(!writer.overflowed);
    }
}
