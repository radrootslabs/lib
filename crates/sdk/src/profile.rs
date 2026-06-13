pub use radroots_events::profile::{RadrootsProfile, RadrootsProfileType};
pub use radroots_events_codec::profile::error::ProfileEncodeError;

use crate::WireEventParts;

#[cfg(feature = "serde_json")]
pub fn build_draft(
    profile: &RadrootsProfile,
    profile_type: Option<RadrootsProfileType>,
) -> Result<WireEventParts, ProfileEncodeError> {
    radroots_events_codec::profile::encode::to_wire_parts_with_profile_type(profile, profile_type)
}
