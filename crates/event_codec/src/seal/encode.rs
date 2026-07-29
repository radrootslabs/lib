#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use radroots_event::envelope::kind::KIND_SEAL;
use radroots_event::social::seal::Seal;

use crate::error::EventEncodeError;
use radroots_event::wire::Nip01EventWireParts;

const DEFAULT_KIND: u32 = KIND_SEAL;

pub fn seal_build_tags(_seal: &Seal) -> Result<Vec<Vec<String>>, EventEncodeError> {
    if _seal.content.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("content"));
    }
    Ok(Vec::new())
}

pub fn to_wire_parts(seal: &Seal) -> Result<Nip01EventWireParts, EventEncodeError> {
    let tags = seal_build_tags(seal)?;
    Ok(Nip01EventWireParts {
        kind: DEFAULT_KIND,
        content: seal.content.clone(),
        tags,
    })
}

pub fn to_wire_parts_with_kind(
    seal: &Seal,
    kind: u32,
) -> Result<Nip01EventWireParts, EventEncodeError> {
    if kind != DEFAULT_KIND {
        return Err(EventEncodeError::InvalidKind(kind));
    }
    to_wire_parts(seal)
}
