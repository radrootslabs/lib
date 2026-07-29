#![forbid(unsafe_code)]

use crate::envelope::kind::KIND_APP_DATA as KIND_APP_DATA_EVENT;

#[cfg(not(feature = "std"))]
use alloc::string::String;

pub const KIND_APP_DATA: u32 = KIND_APP_DATA_EVENT;

#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug)]
pub struct AppData {
    pub d_tag: String,
    pub content: String,
}
