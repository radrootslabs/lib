#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use serde::{Deserialize, Serialize};

pub const RADROOTS_TANGLE_TRANSFER_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsTangleEventDraft {
    pub kind: u32,
    pub author: String,
    pub content: String,
    pub tags: Vec<Vec<String>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsTangleSyncBundle {
    pub version: u32,
    pub events: Vec<RadrootsTangleEventDraft>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsTangleFarmSelector {
    pub id: Option<String>,
    pub d_tag: Option<String>,
    pub pubkey: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsTangleSyncOptions {
    pub include_profiles: Option<bool>,
    pub include_list_sets: Option<bool>,
    pub include_membership_claims: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsTangleSyncRequest {
    pub farm: RadrootsTangleFarmSelector,
    pub options: Option<RadrootsTangleSyncOptions>,
}
