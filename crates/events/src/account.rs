use crate::{RadrootsNostrEvent, kinds::KIND_ACCOUNT_CLAIM as KIND_ACCOUNT_CLAIM_EVENT};
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

#[cfg(not(feature = "std"))]
use alloc::string::String;

pub const KIND_ACCOUNT_CLAIM: u32 = KIND_ACCOUNT_CLAIM_EVENT;

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsAccountClaimEventIndex {
    pub event: RadrootsNostrEvent,
    pub metadata: RadrootsAccountClaimEventMetadata,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsAccountClaimEventMetadata {
    pub id: String,
    pub author: String,
    pub published_at: u32,
    pub kind: u32,
    pub account: RadrootsAccountClaim,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsAccountClaim {
    pub username: String,
    pub pubkey: String,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub nip05: Option<String>,
}
