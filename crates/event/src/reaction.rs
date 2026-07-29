use crate::social::SocialTarget;

#[cfg(not(feature = "std"))]
use alloc::string::String;

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(all(test, feature = "std"), derive(dto_bindgen::Dto))]
#[cfg_attr(all(test, feature = "std"), dto(export))]
#[derive(Clone, Debug)]
pub struct Reaction {
    pub target: SocialTarget,
    pub content: String,
}
