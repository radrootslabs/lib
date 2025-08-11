use serde::{Deserialize, Serialize};
use typeshare::typeshare;
use crate::events::RadrootsNostrEvent;

#[typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsProfileEventIndex {
    pub event: RadrootsNostrEvent,
    pub metadata: RadrootsProfileEventMetadata,
}

#[typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsProfileEventMetadata {
    pub id: String,
    pub author: String,
    pub published_at: u32,
    pub profile: RadrootsProfile,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsProfile {
    pub name: String,
    pub display_name: Option<String>,
    pub nip05: Option<String>,
    pub about: Option<String>,
    pub website: Option<String>,
    pub picture: Option<String>,
    pub banner: Option<String>,
    pub lud06: Option<String>,
    pub lud16: Option<String>,
    pub bot: Option<String>,
}
