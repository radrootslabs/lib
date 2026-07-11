#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::string::String;

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsSeal {
    pub content: String,
}
