use crate::kinds::KIND_ACCOUNT_CLAIM as KIND_ACCOUNT_CLAIM_EVENT;

#[cfg(not(feature = "std"))]
use alloc::string::String;

pub const KIND_ACCOUNT_CLAIM: u32 = KIND_ACCOUNT_CLAIM_EVENT;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsAccountClaim {
    pub username: String,
    pub pubkey: String,
    pub nip05: Option<String>,
}
