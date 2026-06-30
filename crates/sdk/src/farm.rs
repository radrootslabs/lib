pub use radroots_events::farm::*;

pub use radroots_events_codec::error::EventEncodeError;

use radroots_events_codec::wire::WireEventParts;

pub fn build_draft(farm: &RadrootsFarm) -> Result<WireEventParts, EventEncodeError> {
    radroots_events_codec::farm::encode::to_wire_parts(farm)
}
