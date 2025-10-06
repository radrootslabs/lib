use serde::{Deserialize, Serialize};

use crate::{RadrootsNostrEvent, job::JobInputType};

#[typeshare::typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsJobRequestEventIndex {
    pub event: RadrootsNostrEvent,
    pub metadata: RadrootsJobRequestEventMetadata,
}

#[typeshare::typeshare]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadrootsJobRequestEventMetadata {
    pub id: String,
    pub author: String,
    pub published_at: u32,
    pub kind: u32,
    pub job_request: RadrootsJobRequest,
}

#[typeshare::typeshare]
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RadrootsJobInput {
    pub data: String,
    pub input_type: JobInputType,
    pub relay: Option<String>,
    pub marker: Option<String>,
}

#[typeshare::typeshare]
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RadrootsJobParam {
    pub key: String,
    pub value: String,
}

#[typeshare::typeshare]
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RadrootsJobRequest {
    pub kind: u16,
    pub inputs: Vec<RadrootsJobInput>,
    pub output: Option<String>,
    pub params: Vec<RadrootsJobParam>,
    pub bid_sat: Option<u32>,
    pub relays: Vec<String>,
    pub providers: Vec<String>,
    pub topics: Vec<String>,
    pub encrypted: bool,
}
