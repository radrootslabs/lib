use crate::social::RadrootsSocialTarget;

#[cfg(not(feature = "std"))]
use alloc::string::String;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[derive(Clone, Debug)]
pub struct RadrootsComment {
    pub root: RadrootsSocialTarget,
    pub parent: RadrootsSocialTarget,
    pub content: String,
}
