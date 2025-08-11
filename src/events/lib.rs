use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsNostrEventRef {
    pub id: String,
    pub author: String,
    pub kind: u32,
    pub d_tag: Option<String>,
    pub relays: Option<Vec<String>>,
}
