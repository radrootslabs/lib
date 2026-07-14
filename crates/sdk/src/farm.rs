pub use radroots_event::farm::*;

pub use radroots_event_codec::error::EventEncodeError;

use radroots_event::wire::RadrootsNip01EventWireParts;

pub fn build_draft(farm: &RadrootsFarm) -> Result<RadrootsNip01EventWireParts, EventEncodeError> {
    radroots_event_codec::farm::encode::to_wire_parts(farm)
}
