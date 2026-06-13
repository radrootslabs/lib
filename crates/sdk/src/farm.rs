pub use radroots_events::farm::*;
pub use radroots_events_codec::error::EventEncodeError;

use crate::WireEventParts;

#[cfg(feature = "serde_json")]
pub fn build_draft(farm: &RadrootsFarm) -> Result<WireEventParts, EventEncodeError> {
    radroots_events_codec::farm::encode::to_wire_parts(farm)
}
