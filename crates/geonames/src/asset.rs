//! Host-supplied GeoNames asset identity and passive status.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use sha2::{Digest, Sha256};
use url::Url;

use crate::Error;

/// Version of the first governed Radroots GeoNames asset.
pub const OFFICIAL_ASSET_VERSION: &str = "1.0";
/// File name of the first governed Radroots GeoNames asset.
pub const OFFICIAL_ASSET_FILE_NAME: &str = "geonames-1.0.db";
/// HTTPS source of the first governed Radroots GeoNames asset.
pub const OFFICIAL_ASSET_SOURCE: &str = "https://assets.radroots.io/data/geonames/geonames-1.0.db";
/// Exact byte size of the first governed Radroots GeoNames asset.
pub const OFFICIAL_ASSET_BYTE_SIZE: u64 = 12_951_552;
/// Exact SHA-256 of the first governed Radroots GeoNames asset.
pub const OFFICIAL_ASSET_SHA256: [u8; 32] = [
    0x6c, 0xa5, 0xf1, 0xa3, 0x24, 0xde, 0x02, 0x92, 0x2d, 0x40, 0xb1, 0xff, 0x33, 0xee, 0xdf, 0x3a,
    0x5a, 0x13, 0x3c, 0x97, 0x8d, 0xe9, 0x21, 0xee, 0xe5, 0x13, 0x0a, 0x0c, 0x78, 0x76, 0x07, 0x9c,
];

/// Returns the immutable specification for the governed Radroots asset.
#[must_use]
pub fn official_asset_spec() -> AssetSpec {
    AssetSpec {
        version: OFFICIAL_ASSET_VERSION.to_owned(),
        file_name: OFFICIAL_ASSET_FILE_NAME.to_owned(),
        source: OFFICIAL_ASSET_SOURCE.to_owned(),
        allowed_host: "assets.radroots.io".to_owned(),
        byte_size: OFFICIAL_ASSET_BYTE_SIZE,
        sha256: OFFICIAL_ASSET_SHA256,
    }
}

/// The expected identity of one immutable GeoNames database asset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetSpec {
    version: String,
    file_name: String,
    source: String,
    allowed_host: String,
    byte_size: u64,
    sha256: [u8; 32],
}

impl AssetSpec {
    /// Creates an explicit asset specification without reading or downloading it.
    pub fn new(
        version: impl Into<String>,
        file_name: impl Into<String>,
        source: impl Into<String>,
        allowed_host: impl Into<String>,
        byte_size: u64,
        sha256: [u8; 32],
    ) -> Result<Self, Error> {
        let version = version.into();
        if !is_normalized_non_empty(&version) {
            return Err(Error::InvalidAssetVersion);
        }

        let file_name = file_name.into();
        if !is_safe_file_name(&file_name) {
            return Err(Error::InvalidAssetFileName);
        }

        let source = source.into();
        let allowed_host = allowed_host.into();
        if !is_normalized_non_empty(&source) || !is_normalized_non_empty(&allowed_host) {
            return Err(Error::InvalidAssetSource);
        }
        validate_source(&source, &allowed_host)?;
        if byte_size == 0 {
            return Err(Error::InvalidAssetByteSize);
        }

        Ok(Self {
            version,
            file_name,
            source,
            allowed_host,
            byte_size,
            sha256,
        })
    }

    /// Returns the provider asset version.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Returns the expected destination file name.
    #[must_use]
    pub fn file_name(&self) -> &str {
        &self.file_name
    }

    /// Returns the explicit HTTPS source.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Returns the exact HTTPS host allowed for acquisition.
    #[must_use]
    pub fn allowed_host(&self) -> &str {
        &self.allowed_host
    }

    /// Returns the exact expected byte size.
    #[must_use]
    pub const fn byte_size(&self) -> u64 {
        self.byte_size
    }

    /// Returns the expected SHA-256 digest bytes.
    #[must_use]
    pub const fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }
}

/// Passive state of an explicitly inspected asset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum AssetStatus {
    /// No filesystem entry exists at the inspected path.
    Missing,
    /// The entry matches the complete [`AssetSpec`].
    Available,
    /// The entry exists but does not match the specification.
    Invalid,
}

/// Inspects an explicit path without creating, repairing, or downloading it.
pub fn inspect(path: impl AsRef<Path>, spec: &AssetSpec) -> Result<AssetStatus, Error> {
    let path = path.as_ref();
    let metadata = match path.symlink_metadata() {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(AssetStatus::Missing);
        }
        Err(error) => return Err(io_error("inspect asset metadata", error)),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(Error::UnsafeAssetDestination);
    }
    match verify_file(path, spec) {
        Ok(()) => Ok(AssetStatus::Available),
        Err(Error::AssetSizeMismatch { .. } | Error::AssetHashMismatch) => Ok(AssetStatus::Invalid),
        Err(error) => Err(error),
    }
}

pub(crate) fn verify_file(path: &Path, spec: &AssetSpec) -> Result<(), Error> {
    let mut file = File::open(path).map_err(|error| io_error("open asset", error))?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut observed = 0_u64;
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| io_error("read asset", error))?;
        if read == 0 {
            break;
        }
        observed = observed.saturating_add(u64::try_from(read).unwrap_or(u64::MAX));
        if observed > spec.byte_size {
            return Err(Error::AssetSizeMismatch {
                expected: spec.byte_size,
                actual: observed,
            });
        }
        digest.update(&buffer[..read]);
    }
    if observed != spec.byte_size {
        return Err(Error::AssetSizeMismatch {
            expected: spec.byte_size,
            actual: observed,
        });
    }
    let actual: [u8; 32] = digest.finalize().into();
    if actual != spec.sha256 {
        return Err(Error::AssetHashMismatch);
    }
    Ok(())
}

pub(crate) fn io_error(operation: &'static str, error: std::io::Error) -> Error {
    Error::Io {
        operation,
        kind: error.kind(),
    }
}

fn is_normalized_non_empty(value: &str) -> bool {
    !value.is_empty() && value.trim() == value
}

fn is_safe_file_name(value: &str) -> bool {
    is_normalized_non_empty(value)
        && value != "."
        && value != ".."
        && !value.contains(['/', '\\', ':', '\0'])
}

fn validate_source(source: &str, allowed_host: &str) -> Result<(), Error> {
    let parsed = Url::parse(source).map_err(|_| Error::UntrustedAssetSource)?;
    let trusted = parsed.scheme() == "https"
        && parsed.host_str() == Some(allowed_host)
        && parsed.port_or_known_default() == Some(443)
        && parsed.username().is_empty()
        && parsed.password().is_none()
        && parsed.query().is_none()
        && parsed.fragment().is_none();
    if !trusted {
        return Err(Error::UntrustedAssetSource);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{AssetSpec, AssetStatus, OFFICIAL_ASSET_SHA256, official_asset_spec};
    use crate::Error;

    fn spec() -> AssetSpec {
        AssetSpec::new(
            "2026-08",
            "geonames-2026-08.db",
            "https://assets.example/geonames-2026-08.db",
            "assets.example",
            42,
            [7; 32],
        )
        .expect("valid asset specification")
    }

    #[test]
    fn asset_spec_preserves_explicit_identity() {
        let spec = spec();
        assert_eq!(spec.version(), "2026-08");
        assert_eq!(spec.file_name(), "geonames-2026-08.db");
        assert_eq!(spec.source(), "https://assets.example/geonames-2026-08.db");
        assert_eq!(spec.allowed_host(), "assets.example");
        assert_eq!(spec.byte_size(), 42);
        assert_eq!(spec.sha256(), &[7; 32]);
    }

    #[test]
    fn official_asset_identity_is_byte_pinned_without_fixture_features() {
        let spec = official_asset_spec();
        assert_eq!(spec.version(), "1.0");
        assert_eq!(spec.file_name(), "geonames-1.0.db");
        assert_eq!(spec.allowed_host(), "assets.radroots.io");
        assert_eq!(spec.byte_size(), 12_951_552);
        assert_eq!(spec.sha256(), &OFFICIAL_ASSET_SHA256);
    }

    #[test]
    fn asset_spec_rejects_ambient_unsafe_or_untrusted_values() {
        let valid_source = "https://assets.example/a";
        assert_eq!(
            AssetSpec::new(" ", "asset.db", valid_source, "assets.example", 1, [0; 32]),
            Err(Error::InvalidAssetVersion)
        );
        assert_eq!(
            AssetSpec::new(
                "v1",
                "../asset.db",
                valid_source,
                "assets.example",
                1,
                [0; 32]
            ),
            Err(Error::InvalidAssetFileName)
        );
        assert_eq!(
            AssetSpec::new("v1", "asset.db", " source", "assets.example", 1, [0; 32]),
            Err(Error::InvalidAssetSource)
        );
        assert_eq!(
            AssetSpec::new("v1", "asset.db", valid_source, "assets.example", 0, [0; 32]),
            Err(Error::InvalidAssetByteSize)
        );
        assert_eq!(
            AssetSpec::new(
                "v1",
                "asset.db",
                "http://assets.example/a",
                "assets.example",
                1,
                [0; 32]
            ),
            Err(Error::UntrustedAssetSource)
        );
        assert_eq!(
            AssetSpec::new(
                "v1",
                "asset.db",
                "https://other.example/a",
                "assets.example",
                1,
                [0; 32]
            ),
            Err(Error::UntrustedAssetSource)
        );
    }

    #[test]
    fn asset_status_is_passive_and_exhaustive_for_v1() {
        assert_ne!(AssetStatus::Missing, AssetStatus::Available);
        assert_ne!(AssetStatus::Available, AssetStatus::Invalid);
    }
}
