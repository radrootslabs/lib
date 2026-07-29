pub use radroots_event::farm::*;

pub use radroots_event_codec::error::EventEncodeError;

use radroots_event::wire::Nip01EventWireParts;

pub fn build_draft(farm: &Farm) -> Result<Nip01EventWireParts, EventEncodeError> {
    radroots_event_codec::farm::encode::to_wire_parts(farm)
}
