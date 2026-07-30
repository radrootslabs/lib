pub use radroots_event::farm::*;

pub use radroots_event_codec::encode::EventEncodeError;

use radroots_event::wire::Nip01EventWireParts;

pub fn build_draft(farm: &Farm) -> Result<Nip01EventWireParts, EventEncodeError> {
    radroots_event_codec::encode::farm::to_wire_parts(farm)
}
