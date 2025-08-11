use serde::{Deserialize, Serialize};
use typeshare::typeshare;
use crate::events::{lib::RadrootsNostrEventRef, RadrootsNostrEvent};

#[typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsReactionEventIndex {
    pub event: RadrootsNostrEvent,
    pub metadata: RadrootsReactionEventMetadata,
}

#[typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsReactionEventMetadata {
    pub id: String,
    pub author: String,
    pub published_at: u32,
    pub reaction: RadrootsReaction,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsReaction {
    pub root: RadrootsNostrEventRef,
    pub content: String,
}
