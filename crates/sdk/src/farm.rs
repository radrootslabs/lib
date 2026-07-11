pub use radroots_event::farm::*;

pub use radroots_event_codec::error::EventEncodeError;

use radroots_event_codec::wire::WireEventParts;

pub fn build_draft(farm: &RadrootsFarm) -> Result<WireEventParts, EventEncodeError> {
    radroots_event_codec::farm::encode::to_wire_parts(farm)
}
