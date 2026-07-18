#[cfg(not(feature = "std"))]
extern crate alloc;

use radroots_event::profile::{RadrootsProfile, RadrootsProfileType};

#[cfg(feature = "serde_json")]
pub mod admission;

#[cfg(feature = "serde_json")]
pub mod authored;

#[cfg(feature = "serde_json")]
pub mod decode;

#[cfg(feature = "serde_json")]
pub mod inbound;

#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsProfileData {
    pub profile_type: Option<RadrootsProfileType>,
    pub profile: RadrootsProfile,
}
