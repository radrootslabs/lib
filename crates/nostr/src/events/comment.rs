use crate::{
    error::RadrootsNostrError,
    types::{
        RadrootsNostrEvent, RadrootsNostrEventBuilderUnchecked, RadrootsNostrKeys,
        RadrootsNostrTimestamp,
    },
};
use radroots_event::{comment::RadrootsAuthoredNip22Comment, wire::RadrootsNip01EventWireParts};
use radroots_event_codec::comment::authored::authored_nip22_comment_to_wire_parts;

/// A sealed builder for a validated strict NIP-22 Comment.
///
/// The wrapper exposes no raw builder conversion or tag/content mutation.
///
/// ```compile_fail
/// use radroots_nostr::prelude::RadrootsNostrNip22CommentEventBuilder;
///
/// fn expose_raw_builder(builder: RadrootsNostrNip22CommentEventBuilder) {
///     let _: nostr::EventBuilder = builder.into();
/// }
/// ```
#[must_use = "NIP-22 Comment builders must be signed or published"]
pub struct RadrootsNostrNip22CommentEventBuilder {
    inner: RadrootsNostrEventBuilderUnchecked,
}

impl RadrootsNostrNip22CommentEventBuilder {
    pub fn custom_created_at(mut self, created_at: RadrootsNostrTimestamp) -> Self {
        self.inner = self.inner.custom_created_at(created_at);
        self
    }

    pub fn sign_with_keys(
        self,
        keys: &RadrootsNostrKeys,
    ) -> Result<RadrootsNostrEvent, RadrootsNostrError> {
        Ok(self.inner.sign_with_keys(keys)?)
    }

    #[cfg(feature = "client")]
    pub(crate) fn into_event_builder(self) -> RadrootsNostrEventBuilderUnchecked {
        self.inner
    }
}

pub fn radroots_nostr_build_nip22_comment_event(
    comment: &RadrootsAuthoredNip22Comment,
) -> Result<RadrootsNostrNip22CommentEventBuilder, RadrootsNostrError> {
    builder_from_wire_parts(authored_nip22_comment_to_wire_parts(comment))
}

fn builder_from_wire_parts(
    parts: RadrootsNip01EventWireParts,
) -> Result<RadrootsNostrNip22CommentEventBuilder, RadrootsNostrError> {
    let inner =
        crate::events::radroots_nostr_build_event_unchecked(parts.kind, parts.content, parts.tags)?;
    Ok(RadrootsNostrNip22CommentEventBuilder { inner })
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_event::{
        comment::{RadrootsAuthoredNip22Comment, RadrootsNip22EventRootReference},
        kinds::{KIND_CLASSIFIED_LISTING, KIND_COMMENT},
    };

    #[test]
    fn typed_comment_builder_signs_exact_kind_and_tags() {
        let root = RadrootsNip22EventRootReference::parse(
            "a".repeat(64),
            "b".repeat(64),
            KIND_CLASSIFIED_LISTING,
            None,
        )
        .expect("root");
        let comment =
            RadrootsAuthoredNip22Comment::top_level_event("Comment", root).expect("comment");
        let keys = RadrootsNostrKeys::generate();
        let event = radroots_nostr_build_nip22_comment_event(&comment)
            .expect("builder")
            .custom_created_at(RadrootsNostrTimestamp::from_secs(1_800_000_000))
            .sign_with_keys(&keys)
            .expect("signed event");
        assert_eq!(event.kind.as_u16(), KIND_COMMENT as u16);
        assert_eq!(event.tags.len(), 6);
        assert_eq!(event.content, "Comment");
        assert!(event.verify().is_ok());
    }
}
