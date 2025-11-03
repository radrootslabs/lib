use serde::{Deserialize, Serialize};

#[typeshare::typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsRelayDocument {
    name: Option<String>,
    description: Option<String>,
    pubkey: Option<String>,
    contact: Option<String>,
    supported_nips: Option<Vec<u16>>,
    software: Option<String>,
    version: Option<String>,
}
