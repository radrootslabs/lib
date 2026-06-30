pub use radroots_events::farm::*;

#[cfg(feature = "runtime")]
pub use radroots_events_codec::error::EventEncodeError;

#[cfg(feature = "runtime")]
use radroots_events_codec::wire::WireEventParts;

#[cfg(feature = "runtime")]
pub fn build_draft(farm: &RadrootsFarm) -> Result<WireEventParts, EventEncodeError> {
    radroots_events_codec::farm::encode::to_wire_parts(farm)
}
