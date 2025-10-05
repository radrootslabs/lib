pub mod job;
pub mod kinds;
pub mod tag;

pub mod comment {
    pub mod models;
}

pub mod follow {
    pub mod models;
}

pub mod listing {
    pub mod models;
}

pub mod post {
    pub mod models;
}

pub mod profile {
    pub mod models;
}

pub mod reaction {
    pub mod models;
}

pub mod relay_document {
    pub mod models;
}

use serde::{Deserialize, Serialize};

#[typeshare::typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsNostrEvent {
    pub id: String,
    pub author: String,
    pub created_at: u32,
    pub kind: u32,
    pub tags: Vec<Vec<String>>,
    pub content: String,
    pub sig: String,
}

#[typeshare::typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsNostrEventRef {
    pub id: String,
    pub author: String,
    pub kind: u32,
    pub d_tag: Option<String>,
    pub relays: Option<Vec<String>>,
}

#[typeshare::typeshare]
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RadrootsNostrEventPtr {
    pub id: String,
    pub relays: Option<String>,
}
