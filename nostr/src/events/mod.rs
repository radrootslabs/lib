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
            .tags(tags)
            .allow_self_tagging();
    Ok(builder)
}

#[cfg(test)]
mod tests {
    use super::radroots_nostr_build_event;
    use crate::types::{RadrootsNostrPublicKey, RadrootsNostrTagKind};

    #[test]
    fn build_event_preserves_self_p_tag() {
        let pubkey_hex = "1bdebe7b23fccb167fc8843280b789839dfa296ae9fd86cc9769b4813d76d8a4";
        let pubkey = RadrootsNostrPublicKey::from_hex(pubkey_hex).expect("pubkey");
        let tags = vec![vec!["p".to_string(), pubkey_hex.to_string()]];

        let builder = radroots_nostr_build_event(1, "test", tags).expect("builder");
        let event = builder.build(pubkey);

        let has_self_tag = event.tags.iter().any(|tag| {
            tag.kind() == RadrootsNostrTagKind::p() && tag.content() == Some(pubkey_hex)
        });
        assert!(has_self_tag);
    }
}
