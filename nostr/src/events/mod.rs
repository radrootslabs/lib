pub mod jobs;
pub mod metadata;
pub mod post;

extern crate alloc;
use alloc::{string::String, vec::Vec};

use crate::error::RadrootsNostrError;
use crate::types::{
    RadrootsNostrEventBuilder,
    RadrootsNostrKind,
    RadrootsNostrTag,
    RadrootsNostrTagKind,
};

pub fn radroots_nostr_build_event(
    kind_u32: u32,
    content: impl Into<String>,
    tag_slices: Vec<Vec<String>>,
) -> Result<RadrootsNostrEventBuilder, RadrootsNostrError> {
    let mut tags: Vec<RadrootsNostrTag> = Vec::new();
    for mut s in tag_slices {
        if s.is_empty() {
            continue;
        }
        let key = s.remove(0);
        let values = s;
        tags.push(RadrootsNostrTag::custom(
            RadrootsNostrTagKind::Custom(key.into()),
            values,
        ));
    }
    let builder =
        RadrootsNostrEventBuilder::new(RadrootsNostrKind::Custom(kind_u32 as u16), content.into())
            .tags(tags);
    Ok(builder)
}
