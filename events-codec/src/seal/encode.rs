#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use radroots_events::kinds::KIND_SEAL;
use radroots_events::seal::RadrootsSeal;

use crate::error::EventEncodeError;
use crate::wire::WireEventParts;

const DEFAULT_KIND: u32 = KIND_SEAL;

pub fn seal_build_tags(_seal: &RadrootsSeal) -> Result<Vec<Vec<String>>, EventEncodeError> {
    Ok(Vec::new())
}

pub fn to_wire_parts(seal: &RadrootsSeal) -> Result<WireEventParts, EventEncodeError> {
    if seal.content.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("content"));
    }
    let tags = seal_build_tags(seal)?;
    Ok(WireEventParts {
        kind: DEFAULT_KIND,
        content: seal.content.clone(),
        tags,
    })
}

pub fn to_wire_parts_with_kind(
    seal: &RadrootsSeal,
    kind: u32,
) -> Result<WireEventParts, EventEncodeError> {
    if kind != DEFAULT_KIND {
        return Err(EventEncodeError::InvalidKind(kind));
    }
    to_wire_parts(seal)
}
