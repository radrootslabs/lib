use crate::RadrootsNostrEvent;
use serde::{Deserialize, Serialize};

#[typeshare::typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsPostEventIndex {
    pub event: RadrootsNostrEvent,
    pub metadata: RadrootsPostEventMetadata,
}

#[typeshare::typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsPostEventMetadata {
    pub id: String,
    pub author: String,
    pub published_at: u64,
    pub kind: u32,
    pub post: RadrootsPost,
}

#[typeshare::typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsPost {
    pub content: String,
}
