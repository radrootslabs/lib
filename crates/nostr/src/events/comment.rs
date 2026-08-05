//! Sealed authoring for strict NIP-22 Comment events.

use crate::{
    error::Error,
    events::sealed::SealedBuilderCore,
    types::{
        ExternalSigningRequest, RadrootsNostrEvent, RadrootsNostrKeys, RadrootsNostrPublicKey,
        RadrootsNostrTimestamp,
    },
};
use radroots_event::post::comment::AuthoredNip22Comment;
use radroots_event_codec::authoring::AuthoredEventBody;

/// A sealed builder for a validated strict NIP-22 Comment.
///
/// The wrapper exposes no raw builder conversion or tag/content mutation.
///
/// ```compile_fail
/// use radroots_nostr::event::Nip22CommentBuilder;
///
/// fn expose_raw_builder(builder: Nip22CommentBuilder) {
///     let _: nostr::EventBuilder = builder.into();
/// }
/// ```
#[must_use = "NIP-22 Comment builders must be signed or published"]
pub struct Nip22CommentBuilder {
    inner: SealedBuilderCore,
}

impl Nip22CommentBuilder {
    pub fn custom_created_at(mut self, created_at: RadrootsNostrTimestamp) -> Self {
        self.inner = self.inner.custom_created_at(created_at);
        self
    }

    pub fn sign_with_keys(self, keys: &RadrootsNostrKeys) -> Result<RadrootsNostrEvent, Error> {
        self.inner.sign_with_keys(keys)
    }

    pub fn into_external_signing_request(
        self,
        public_key: RadrootsNostrPublicKey,
    ) -> Result<ExternalSigningRequest, Error> {
        self.inner.into_external_signing_request(public_key)
    }
}

pub fn build_nip22_comment_event(
    comment: &AuthoredNip22Comment,
) -> Result<Nip22CommentBuilder, Error> {
    Ok(Nip22CommentBuilder {
        inner: SealedBuilderCore::new(AuthoredEventBody::from_nip22_comment(comment)?),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::FIXTURE_BOB_PUBLIC_KEY_HEX;
    use radroots_event::{
        envelope::kind::{KIND_CLASSIFIED_LISTING, KIND_COMMENT},
        post::comment::{AuthoredNip22Comment, Nip22EventRootReference},
    };

    #[test]
    fn typed_comment_builder_signs_exact_kind_and_tags() {
        let root = Nip22EventRootReference::parse(
            "a".repeat(64),
            FIXTURE_BOB_PUBLIC_KEY_HEX,
            KIND_CLASSIFIED_LISTING,
            None,
        )
        .expect("root");
        let comment = AuthoredNip22Comment::top_level_event("Comment", root).expect("comment");
        let keys = RadrootsNostrKeys::generate();
        let event = build_nip22_comment_event(&comment)
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
