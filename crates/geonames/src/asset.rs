//! Host-supplied GeoNames asset identity and passive status.

use crate::Error;

/// The expected identity of one immutable GeoNames database asset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetSpec {
    version: String,
    file_name: String,
    source: String,
    byte_size: u64,
    sha256: [u8; 32],
}

impl AssetSpec {
    /// Creates an explicit asset specification without reading or downloading it.
    pub fn new(
        version: impl Into<String>,
        file_name: impl Into<String>,
        source: impl Into<String>,
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
        if !is_normalized_non_empty(&source) {
            return Err(Error::InvalidAssetSource);
        }
        if byte_size == 0 {
            return Err(Error::InvalidAssetByteSize);
        }

        Ok(Self {
            version,
            file_name,
            source,
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

    /// Returns the host-supplied source identifier.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
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

fn is_normalized_non_empty(value: &str) -> bool {
    !value.is_empty() && value.trim() == value
}

fn is_safe_file_name(value: &str) -> bool {
    is_normalized_non_empty(value) && value != "." && value != ".." && !value.contains(['/', '\\'])
}

#[cfg(test)]
mod tests {
    use super::{AssetSpec, AssetStatus};
    use crate::Error;

    fn spec() -> AssetSpec {
        AssetSpec::new(
            "2026-08",
            "geonames-2026-08.db",
            "https://assets.example/geonames-2026-08.db",
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
        assert_eq!(spec.byte_size(), 42);
        assert_eq!(spec.sha256(), &[7; 32]);
    }

    #[test]
    fn asset_spec_rejects_ambient_or_unsafe_values() {
        assert_eq!(
            AssetSpec::new(" ", "asset.db", "source", 1, [0; 32]),
            Err(Error::InvalidAssetVersion)
        );
        assert_eq!(
            AssetSpec::new("v1", "../asset.db", "source", 1, [0; 32]),
            Err(Error::InvalidAssetFileName)
        );
        assert_eq!(
            AssetSpec::new("v1", "asset.db", " source", 1, [0; 32]),
            Err(Error::InvalidAssetSource)
        );
        assert_eq!(
            AssetSpec::new("v1", "asset.db", "source", 0, [0; 32]),
            Err(Error::InvalidAssetByteSize)
        );
    }

    #[test]
    fn asset_status_is_passive_and_exhaustive_for_v1() {
        assert_ne!(AssetStatus::Missing, AssetStatus::Available);
        assert_ne!(AssetStatus::Available, AssetStatus::Invalid);
    }
}
