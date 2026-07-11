#![allow(clippy::module_name_repetitions)]
#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};
use core::fmt;

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsEventIndexShardMetadata {
    pub file: String,
    pub count: u32,
    pub first_id: String,
    pub last_id: String,
    pub first_published_at: u32,
    pub last_published_at: u32,
    pub sha256: String,
}

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsEventIndexManifest {
    pub country: String,
    pub total: u32,
    pub shard_size: u32,
    pub first_published_at: u32,
    pub last_published_at: u32,
    pub shards: Vec<RadrootsEventIndexShardMetadata>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum RadrootsEventIndexManifestError {
    EmptyCountry,
    EmptyShards,
    EmptyFile(u32),
    InvalidSha256(u32),
    InconsistentTotals,
}

impl fmt::Display for RadrootsEventIndexManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RadrootsEventIndexManifestError::EmptyCountry => write!(f, "country is empty"),
            RadrootsEventIndexManifestError::EmptyShards => write!(f, "no shards in manifest"),
            RadrootsEventIndexManifestError::EmptyFile(i) => {
                write!(f, "shard {} has empty file name", i)
            }
            RadrootsEventIndexManifestError::InvalidSha256(i) => {
                write!(f, "shard {} has invalid sha256", i)
            }
            RadrootsEventIndexManifestError::InconsistentTotals => {
                write!(f, "total does not match sum of shard counts")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsEventIndexManifestError {}

pub fn validate_manifest(
    m: &RadrootsEventIndexManifest,
) -> Result<(), RadrootsEventIndexManifestError> {
    if m.country.trim().is_empty() {
        return Err(RadrootsEventIndexManifestError::EmptyCountry);
    }
    if m.shards.is_empty() {
        return Err(RadrootsEventIndexManifestError::EmptyShards);
    }
    let mut sum: u64 = 0;
    for (i, s) in m.shards.iter().enumerate() {
        if s.file.trim().is_empty() {
            return Err(RadrootsEventIndexManifestError::EmptyFile(i as u32));
        }
        if s.sha256.len() != 64 || !s.sha256.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(RadrootsEventIndexManifestError::InvalidSha256(i as u32));
        }
        sum += s.count as u64;
    }
    if sum > u32::MAX as u64 || sum != m.total as u64 {
        return Err(RadrootsEventIndexManifestError::InconsistentTotals);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        RadrootsEventIndexManifest, RadrootsEventIndexManifestError,
        RadrootsEventIndexShardMetadata, validate_manifest,
    };
    #[cfg(not(feature = "std"))]
    use alloc::{format, string::String, vec, vec::Vec};
    #[cfg(feature = "std")]
    use std::{format, string::String, vec::Vec};

    fn shard(file: &str, count: u32, sha256: &str) -> RadrootsEventIndexShardMetadata {
        RadrootsEventIndexShardMetadata {
            file: String::from(file),
            count,
            first_id: String::from("a"),
            last_id: String::from("b"),
            first_published_at: 0,
            last_published_at: 0,
            sha256: String::from(sha256),
        }
    }

    fn base_manifest() -> RadrootsEventIndexManifest {
        RadrootsEventIndexManifest {
            country: String::from("us"),
            total: 1,
            shard_size: 1,
            first_published_at: 0,
            last_published_at: 0,
            shards: vec![shard(
                "a.json",
                1,
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            )],
        }
    }

    #[test]
    fn validate_manifest_rejects_empty_country() {
        let mut m = base_manifest();
        m.country = String::from(" ");
        let err = validate_manifest(&m).unwrap_err();
        assert_eq!(err, RadrootsEventIndexManifestError::EmptyCountry);
    }

    #[test]
    fn validate_manifest_rejects_empty_shards() {
        let mut m = base_manifest();
        m.shards = Vec::new();
        let err = validate_manifest(&m).unwrap_err();
        assert_eq!(err, RadrootsEventIndexManifestError::EmptyShards);
    }

    #[test]
    fn validate_manifest_rejects_empty_file() {
        let mut m = base_manifest();
        m.shards[0].file = String::from("");
        let err = validate_manifest(&m).unwrap_err();
        assert_eq!(err, RadrootsEventIndexManifestError::EmptyFile(0));
    }

    #[test]
    fn validate_manifest_rejects_invalid_sha256() {
        let mut m = base_manifest();
        m.shards[0].sha256 = String::from("zz");
        let err = validate_manifest(&m).unwrap_err();
        assert_eq!(err, RadrootsEventIndexManifestError::InvalidSha256(0));
    }

    #[test]
    fn validate_manifest_rejects_invalid_sha256_with_valid_length() {
        let mut m = base_manifest();
        m.shards[0].sha256 =
            String::from("g123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef");
        let err = validate_manifest(&m).unwrap_err();
        assert_eq!(err, RadrootsEventIndexManifestError::InvalidSha256(0));
    }

    #[test]
    fn validate_manifest_rejects_total_overflow() {
        let m = RadrootsEventIndexManifest {
            country: String::from("us"),
            total: 1,
            shard_size: 1,
            first_published_at: 0,
            last_published_at: 0,
            shards: vec![
                RadrootsEventIndexShardMetadata {
                    file: String::from("a.json"),
                    count: u32::MAX,
                    first_id: String::from("a"),
                    last_id: String::from("b"),
                    first_published_at: 0,
                    last_published_at: 0,
                    sha256: String::from(
                        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                    ),
                },
                RadrootsEventIndexShardMetadata {
                    file: String::from("b.json"),
                    count: 1,
                    first_id: String::from("c"),
                    last_id: String::from("d"),
                    first_published_at: 0,
                    last_published_at: 0,
                    sha256: String::from(
                        "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210",
                    ),
                },
            ],
        };

        let err = validate_manifest(&m).unwrap_err();
        assert_eq!(err, RadrootsEventIndexManifestError::InconsistentTotals);
    }

    #[test]
    fn validate_manifest_rejects_mismatched_total_without_overflow() {
        let mut m = base_manifest();
        m.total = 2;
        let err = validate_manifest(&m).unwrap_err();
        assert_eq!(err, RadrootsEventIndexManifestError::InconsistentTotals);
    }

    #[test]
    fn validate_manifest_accepts_consistent_totals() {
        let m = base_manifest();
        let result = validate_manifest(&m);
        assert!(result.is_ok());
    }

    #[test]
    fn manifest_error_display_messages_are_stable() {
        assert_eq!(
            format!("{}", RadrootsEventIndexManifestError::EmptyCountry),
            "country is empty"
        );
        assert_eq!(
            format!("{}", RadrootsEventIndexManifestError::EmptyShards),
            "no shards in manifest"
        );
        assert_eq!(
            format!("{}", RadrootsEventIndexManifestError::EmptyFile(3)),
            "shard 3 has empty file name"
        );
        assert_eq!(
            format!("{}", RadrootsEventIndexManifestError::InvalidSha256(4)),
            "shard 4 has invalid sha256"
        );
        assert_eq!(
            format!("{}", RadrootsEventIndexManifestError::InconsistentTotals),
            "total does not match sum of shard counts"
        );
    }
}
