pub mod lib;

pub mod comment {
    pub mod models;
}

pub mod listing {
    pub mod models;
}

pub mod profile {
    pub mod models;
}

pub mod reaction {
    pub mod models;
}

use serde::{Deserialize, Serialize};
use typeshare::typeshare;

#[typeshare]
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
