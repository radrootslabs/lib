#[cfg(not(feature = "std"))]
extern crate alloc;

use radroots_events::profile::{RadrootsProfile, RadrootsProfileType};

pub mod error;

#[cfg(feature = "nostr")]
pub mod encode;

#[cfg(feature = "serde_json")]
pub mod decode;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsProfileData {
    pub profile_type: Option<RadrootsProfileType>,
    pub profile: RadrootsProfile,
}
