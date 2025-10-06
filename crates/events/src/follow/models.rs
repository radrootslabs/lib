use crate::RadrootsNostrEvent;
use serde::{Deserialize, Serialize};

#[typeshare::typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsFollowEventIndex {
    pub event: RadrootsNostrEvent,
    pub metadata: RadrootsFollowEventMetadata,
}

#[typeshare::typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsFollowEventMetadata {
    pub id: String,
    pub author: String,
    pub published_at: u32,
    pub kind: u32,
    pub follow: RadrootsFollow,
}

#[typeshare::typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsFollow {
    pub list: Vec<RadrootsFollowProfile>,
}

#[typeshare::typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsFollowProfile {
    pub published_at: u32,
    pub public_key: String,
    pub relay_url: Option<String>,
    pub contact_name: Option<String>,
}
