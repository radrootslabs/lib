pub mod jobs;

extern crate alloc;
use alloc::{string::String, vec::Vec};

use nostr::event::{EventBuilder, Kind, Tag, TagKind};

use crate::error::NostrUtilsError;

pub fn nostr_build_events(
    kind_u32: u32,
    content: impl Into<String>,
    tag_slices: Vec<Vec<String>>,
) -> Result<EventBuilder, NostrUtilsError> {
    let mut tags: Vec<Tag> = Vec::new();
    for mut s in tag_slices {
        if s.is_empty() {
            continue;
        }
        let key = s.remove(0);
        let values = s;
        tags.push(Tag::custom(TagKind::Custom(key.into()), values));
    }
    let builder = EventBuilder::new(Kind::Custom(kind_u32 as u16), content.into()).tags(tags);
    Ok(builder)
}
