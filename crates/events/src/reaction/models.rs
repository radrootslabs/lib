use crate::{RadrootsNostrEvent, RadrootsNostrEventRef};
use serde::{Deserialize, Serialize};

#[typeshare::typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsReactionEventIndex {
    pub event: RadrootsNostrEvent,
    pub metadata: RadrootsReactionEventMetadata,
}

#[typeshare::typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsReactionEventMetadata {
    pub id: String,
    pub author: String,
    pub published_at: u32,
    pub kind: u32,
    pub reaction: RadrootsReaction,
}

#[typeshare::typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsReaction {
    pub root: RadrootsNostrEventRef,
    pub content: String,
}
