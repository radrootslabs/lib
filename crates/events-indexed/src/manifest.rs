#![allow(clippy::module_name_repetitions)]
#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};
use core::fmt;

#[typeshare::typeshare]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsEventsIndexedShardMetadata {
    pub file: String,
    pub count: u32,
    pub first_id: String,
    pub last_id: String,
    pub first_published_at: u32,
    pub last_published_at: u32,
    pub sha256: String,
}

#[typeshare::typeshare]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsEventsIndexedManifest {
    pub country: String,
    pub total: u32,
    pub shard_size: u32,
    pub first_published_at: u32,
    pub last_published_at: u32,
    pub shards: Vec<RadrootsEventsIndexedShardMetadata>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum RadrootsEventsIndexedManifestError {
    EmptyCountry,
    EmptyShards,
    EmptyFile(u32),
    InvalidSha256(u32),
    InconsistentTotals,
}

impl fmt::Display for RadrootsEventsIndexedManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RadrootsEventsIndexedManifestError::EmptyCountry => write!(f, "country is empty"),
            RadrootsEventsIndexedManifestError::EmptyShards => write!(f, "no shards in manifest"),
            RadrootsEventsIndexedManifestError::EmptyFile(i) => {
                write!(f, "shard {} has empty file name", i)
            }
            RadrootsEventsIndexedManifestError::InvalidSha256(i) => {
                write!(f, "shard {} has invalid sha256", i)
            }
            RadrootsEventsIndexedManifestError::InconsistentTotals => {
                write!(f, "total does not match sum of shard counts")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsEventsIndexedManifestError {}

pub fn validate_manifest(
    m: &RadrootsEventsIndexedManifest,
) -> Result<(), RadrootsEventsIndexedManifestError> {
    if m.country.trim().is_empty() {
        return Err(RadrootsEventsIndexedManifestError::EmptyCountry);
    }
    if m.shards.is_empty() {
        return Err(RadrootsEventsIndexedManifestError::EmptyShards);
    }
    let mut sum: u64 = 0;
    for (i, s) in m.shards.iter().enumerate() {
        if s.file.trim().is_empty() {
            return Err(RadrootsEventsIndexedManifestError::EmptyFile(i as u32));
        }
        if s.sha256.len() != 64 || !s.sha256.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(RadrootsEventsIndexedManifestError::InvalidSha256(i as u32));
        }
        sum += s.count as u64;
    }
    if sum as u32 != m.total {
        return Err(RadrootsEventsIndexedManifestError::InconsistentTotals);
    }
    Ok(())
}
