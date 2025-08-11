use serde::{Deserialize, Serialize};
use typeshare::typeshare;
use crate::events::{lib::RadrootsNostrEventRef, RadrootsNostrEvent};

#[typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsCommentEventIndex {
    pub event: RadrootsNostrEvent,
    pub metadata: RadrootsCommentEventMetadata,
}

#[typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsCommentEventMetadata {
    pub id: String,
    pub author: String,
    pub published_at: u32,
    pub comment: RadrootsComment,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsComment {
    pub root: RadrootsNostrEventRef,
    pub parent: RadrootsNostrEventRef,
    pub content: String,
}
