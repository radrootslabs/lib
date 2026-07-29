use crate::envelope::kind::KIND_ACCOUNT_CLAIM as KIND_ACCOUNT_CLAIM_EVENT;

#[cfg(not(feature = "std"))]
use alloc::string::String;

pub const KIND_ACCOUNT_CLAIM: u32 = KIND_ACCOUNT_CLAIM_EVENT;

#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug)]
pub struct AccountClaim {
    pub username: String,
    pub pubkey: String,
    pub nip05: Option<String>,
}
