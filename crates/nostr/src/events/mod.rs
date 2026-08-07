//! Typed Nostr event authoring profiles used by the Radroots adapter.
//!
//! Focused builders seal reserved kinds and canonical tag construction before
//! signer access. Live publication, persistence, and retry policy remain
//! outside this module.

#[cfg(feature = "events")]
pub mod application_handler;
#[cfg(feature = "events")]
pub mod calendar;
#[cfg(feature = "events")]
pub mod comment;
#[cfg(feature = "events")]
pub mod deletion;
#[cfg(feature = "events")]
pub mod food_availability;
pub mod jobs;
pub mod metadata;
pub mod post;
#[cfg(feature = "events")]
pub mod reply;
#[cfg(feature = "events")]
mod sealed;

extern crate alloc;
#[cfg(any(feature = "events", test))]
use alloc::{string::String, vec::Vec};

#[cfg(any(feature = "events", test))]
use crate::error::Error;
#[cfg(any(feature = "events", test))]
use crate::types::{
    RadrootsNostrEventBuilderUnchecked, RadrootsNostrKind, RadrootsNostrTag, RadrootsNostrTagKind,
};

#[cfg(any(feature = "events", test))]
pub(crate) fn build_event_unchecked(
    kind_u32: u32,
    content: impl Into<String>,
    tag_slices: Vec<Vec<String>>,
) -> Result<RadrootsNostrEventBuilderUnchecked, Error> {
    let kind = u16::try_from(kind_u32).map_err(|_| Error::KindOutOfRange {
        kind: kind_u32,
        max: u16::MAX,
    })?;
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
        RadrootsNostrEventBuilderUnchecked::new(RadrootsNostrKind::Custom(kind), content.into())
            .tags(tags)
            .allow_self_tagging();
    Ok(builder)
}

#[cfg(test)]
mod tests {
    use alloc::{vec, vec::Vec};

    use super::build_event_unchecked;
    use crate::error::Error;
    use crate::test_fixtures::FIXTURE_ALICE_PUBLIC_KEY_HEX;
    use crate::types::{RadrootsNostrPublicKey, RadrootsNostrTagKind};

    #[test]
    fn build_event_preserves_self_p_tag() {
        let pubkey_hex = FIXTURE_ALICE_PUBLIC_KEY_HEX;
        let pubkey = RadrootsNostrPublicKey::from_hex(pubkey_hex).expect("pubkey");
        let tags = vec![
            vec!["x".to_string(), "v".to_string()],
            vec!["p".to_string(), pubkey_hex.to_string()],
        ];

        let builder = build_event_unchecked(1, "test", tags).expect("builder");
        let event = builder.build(pubkey);

        let has_self_tag = event.tags.iter().any(|tag| {
            tag.kind() == RadrootsNostrTagKind::p() && tag.content() == Some(pubkey_hex)
        });
        assert!(has_self_tag);
        let has_other_self_tag = event.tags.iter().any(|tag| {
            tag.kind() == RadrootsNostrTagKind::p()
                && tag.content()
                    == Some("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff")
        });
        assert!(!has_other_self_tag);
    }

    #[test]
    fn build_event_accepts_maximum_nip01_kind() {
        let builder = build_event_unchecked(u32::from(u16::MAX), "test", Vec::new())
            .expect("maximum NIP-01 kind");
        let event = builder
            .build(RadrootsNostrPublicKey::from_hex(FIXTURE_ALICE_PUBLIC_KEY_HEX).expect("pubkey"));
        assert_eq!(event.kind.as_u16(), u16::MAX);
    }

    #[test]
    fn build_event_rejects_kind_overflow() {
        let kind = u32::from(u16::MAX) + 1;
        assert!(matches!(
            build_event_unchecked(kind, "test", Vec::new()),
            Err(Error::KindOutOfRange {
                kind: actual,
                max: u16::MAX
            }) if actual == kind
        ));
    }

    #[test]
    fn build_event_ignores_empty_tag_slices() {
        let builder = build_event_unchecked(1, "test", vec![Vec::new()]).expect("builder");
        let event = builder
            .build(RadrootsNostrPublicKey::from_hex(FIXTURE_ALICE_PUBLIC_KEY_HEX).expect("pubkey"));
        assert!(event.tags.is_empty());
    }
}
