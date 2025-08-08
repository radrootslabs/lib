use serde::{Deserialize, Serialize};
use typeshare::typeshare;

#[typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsListingIndexShardMetadata {
    pub file: String,
    pub count: u32,
    pub first_id: String,
    pub last_id: String,
    pub first_published_at: u32,
    pub last_published_at: u32,
    pub sha256: String,
}

#[typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsListingIndexCountryManifest {
    pub country: String,
    pub total: u32,
    pub shard_size: u32,
    pub first_published_at: u32,
    pub last_published_at: u32,
    pub shards: Vec<RadrootsListingIndexShardMetadata>,
}