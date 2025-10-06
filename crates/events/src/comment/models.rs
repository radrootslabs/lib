use serde::{Deserialize, Serialize};

use crate::{RadrootsNostrEvent, RadrootsNostrEventRef};

#[typeshare::typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsCommentEventIndex {
    pub event: RadrootsNostrEvent,
    pub metadata: RadrootsCommentEventMetadata,
}

#[typeshare::typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsCommentEventMetadata {
    pub id: String,
    pub author: String,
    pub published_at: u32,
    pub kind: u32,
    pub comment: RadrootsComment,
}

#[typeshare::typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsComment {
    pub root: RadrootsNostrEventRef,
    pub parent: RadrootsNostrEventRef,
    pub content: String,
}
