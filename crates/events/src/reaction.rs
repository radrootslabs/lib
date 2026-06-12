use crate::social::RadrootsSocialTarget;

#[cfg(not(feature = "std"))]
use alloc::string::String;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsReaction {
    pub target: RadrootsSocialTarget,
    pub content: String,
}
