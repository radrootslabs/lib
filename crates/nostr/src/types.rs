//! Low-level Nostr protocol values and checked generic authoring requests.
//!
//! This compatibility-oriented module does not grant typed Radroots product
//! admission. Prefer the focused `event`, `key`, `tag`, and `events` modules
//! for new code.

#![forbid(unsafe_code)]

#[cfg(feature = "std")]
use crate::error::RadrootsNostrError;
#[cfg(feature = "std")]
use radroots_event::listing::classified::{
    ClassifiedListingPartition, classify_classified_listing_marker_names,
};

#[cfg(feature = "events")]
pub(crate) use crate::event::Metadata as RadrootsNostrMetadata;
pub(crate) use crate::event::{
    Event as RadrootsNostrEvent, EventId as RadrootsNostrEventId, Kind as RadrootsNostrKind,
    Timestamp as RadrootsNostrTimestamp,
};
pub(crate) use crate::filter::Filter as RadrootsNostrFilter;
pub(crate) use crate::tag::{
    Tag as RadrootsNostrTag, TagKind as RadrootsNostrTagKind,
    TagStandard as RadrootsNostrTagStandard,
};
pub(crate) type RadrootsNostrEventBuilderUnchecked = nostr::EventBuilder;
pub type RadrootsNostrKeys = nostr::Keys;
pub type RadrootsNostrPublicKey = nostr::PublicKey;
pub type RadrootsNostrRelayUrl = nostr::RelayUrl;
pub type RadrootsNostrSecretKey = nostr::SecretKey;
pub type RadrootsNostrSubscriptionId = nostr::SubscriptionId;
pub type RadrootsNostrUrl = nostr::Url;

/// A checked generic event prepared for an external signer.
///
/// The request is created only after generic authoring policy succeeds. It
/// serializes as the standard Nostr unsigned-event object expected by signer
/// helpers, but it exposes no raw unsigned event, mutation, or unchecked
/// deserialization boundary.
///
/// ```compile_fail
/// use radroots_nostr::types::RadrootsNostrExternalSigningRequest;
///
/// let _: RadrootsNostrExternalSigningRequest =
///     serde_json::from_str("{}").expect("request");
/// ```
#[must_use = "external signing requests must be completed by a signer"]
pub struct RadrootsNostrExternalSigningRequest {
    unsigned_event: nostr::UnsignedEvent,
    expected_event_id: RadrootsNostrEventId,
    expected_public_key: RadrootsNostrPublicKey,
}

impl RadrootsNostrExternalSigningRequest {
    pub fn expected_event_id(&self) -> RadrootsNostrEventId {
        self.expected_event_id
    }

    pub fn expected_public_key(&self) -> RadrootsNostrPublicKey {
        self.expected_public_key
    }

    /// Accepts an external signing result only when it is the exact requested
    /// event and its NIP-01 identifier and signature are valid.
    #[cfg(feature = "std")]
    pub fn complete(
        self,
        event: RadrootsNostrEvent,
    ) -> Result<RadrootsNostrEvent, RadrootsNostrError> {
        if event.pubkey != self.expected_public_key {
            return Err(RadrootsNostrError::ExternalSigningAuthorMismatch {
                expected: self.expected_public_key,
                actual: event.pubkey,
            });
        }
        if event.id != self.expected_event_id {
            return Err(RadrootsNostrError::ExternalSigningEventIdMismatch {
                expected: self.expected_event_id,
                actual: event.id,
            });
        }
        event
            .verify()
            .map_err(RadrootsNostrError::ExternalSigningEventInvalid)?;
        Ok(event)
    }

    #[cfg(feature = "std")]
    fn sign_with_keys(
        self,
        keys: &RadrootsNostrKeys,
    ) -> Result<RadrootsNostrEvent, RadrootsNostrError> {
        let event = self.unsigned_event.clone().sign_with_keys(keys)?;
        self.complete(event)
    }
}

impl serde::Serialize for RadrootsNostrExternalSigningRequest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.unsigned_event.serialize(serializer)
    }
}

/// An opaque builder for generic Nostr events.
///
/// Kind 0 profile events, all kind 1 events, all kind 5 deletion requests, kind
/// 1111 comments, and focused or mixed kind 30402 FoodAvailability marker
/// partitions are reserved for typed Radroots authoring. Marker-free NIP-99
/// and operational-only kind 30402 events remain available for compatibility.
/// The policy is enforced before direct signing and before a client is allowed
/// to consult its signer.
///
/// The upstream unsigned builder is intentionally inaccessible:
///
/// ```compile_fail
/// use radroots_nostr::types::RadrootsNostrGenericEventBuilder;
/// use radroots_nostr::event::Kind as RadrootsNostrKind;
///
/// let builder = RadrootsNostrGenericEventBuilder::new(
///     RadrootsNostrKind::Custom(30_001),
///     "content",
/// );
/// let _: nostr::EventBuilder = builder.into();
/// ```
///
/// ```compile_fail
/// use radroots_nostr::types::RadrootsNostrGenericEventBuilder;
/// use radroots_nostr::event::Kind as RadrootsNostrKind;
///
/// let builder = RadrootsNostrGenericEventBuilder::new(
///     RadrootsNostrKind::Custom(30_001),
///     "content",
/// );
/// let _raw: nostr::EventBuilder = builder.into_inner();
/// ```
///
/// ```compile_fail
/// use radroots_nostr::types::RadrootsNostrGenericEventBuilder;
/// use radroots_nostr::types::RadrootsNostrKeys;
/// use radroots_nostr::event::Kind as RadrootsNostrKind;
///
/// let builder = RadrootsNostrGenericEventBuilder::new(
///     RadrootsNostrKind::Custom(30_001),
///     "content",
/// );
/// let keys = RadrootsNostrKeys::generate();
/// let _unsigned = builder.build(keys.public_key());
/// ```
#[must_use = "generic event builders must be signed or published"]
pub struct RadrootsNostrGenericEventBuilder {
    inner: RadrootsNostrEventBuilderUnchecked,
}

impl RadrootsNostrGenericEventBuilder {
    pub fn new(kind: RadrootsNostrKind, content: impl Into<alloc::string::String>) -> Self {
        Self::from_unchecked(RadrootsNostrEventBuilderUnchecked::new(kind, content))
    }

    pub fn text_note(content: impl Into<alloc::string::String>) -> Self {
        Self::from_unchecked(RadrootsNostrEventBuilderUnchecked::text_note(content))
    }

    pub fn tag(mut self, tag: RadrootsNostrTag) -> Self {
        self.inner = self.inner.tag(tag);
        self
    }

    pub fn tags<I>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = RadrootsNostrTag>,
    {
        self.inner = self.inner.tags(tags);
        self
    }

    pub fn custom_created_at(mut self, created_at: RadrootsNostrTimestamp) -> Self {
        self.inner = self.inner.custom_created_at(created_at);
        self
    }

    pub fn pow(mut self, difficulty: u8) -> Self {
        self.inner = self.inner.pow(difficulty);
        self
    }

    pub fn allow_self_tagging(mut self) -> Self {
        self.inner = self.inner.allow_self_tagging();
        self
    }

    pub fn dedup_tags(mut self) -> Self {
        self.inner = self.inner.dedup_tags();
        self
    }

    /// Signs a generic event after enforcing typed-authoring reservations.
    #[cfg(feature = "std")]
    pub fn sign_with_keys(
        self,
        keys: &RadrootsNostrKeys,
    ) -> Result<RadrootsNostrEvent, RadrootsNostrError> {
        self.into_external_signing_request(keys.public_key())?
            .sign_with_keys(keys)
    }

    /// Finalizes a generic event for an external signer after enforcing typed
    /// authoring reservations.
    #[cfg(feature = "std")]
    pub fn into_external_signing_request(
        self,
        public_key: RadrootsNostrPublicKey,
    ) -> Result<RadrootsNostrExternalSigningRequest, RadrootsNostrError> {
        self.validate_generic_authoring_policy()?;
        let mut unsigned_event = self.inner.build(public_key);
        let expected_event_id = unsigned_event.id();
        Ok(RadrootsNostrExternalSigningRequest {
            unsigned_event,
            expected_event_id,
            expected_public_key: public_key,
        })
    }

    pub(crate) fn from_unchecked(inner: RadrootsNostrEventBuilderUnchecked) -> Self {
        Self { inner }
    }

    #[cfg(feature = "std")]
    fn validate_generic_authoring_policy(&self) -> Result<(), RadrootsNostrError> {
        // Inspect an unsigned clone so rejection never consults a signer. PoW
        // is irrelevant to kind/tag policy and must not delay the check.
        let mut inspection = self.inner.clone();
        inspection.custom_created_at = Some(RadrootsNostrTimestamp::from_secs(1));
        inspection.pow = None;
        inspection.allow_self_tagging = true;
        inspection.dedup_tags = false;
        let inspection_pubkey = RadrootsNostrPublicKey::from_hex(
            "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
        )?;
        let event = inspection.build(inspection_pubkey);
        let kind = event.kind.as_u16();
        let is_profile = kind == RadrootsNostrKind::Metadata.as_u16();
        let is_reserved_post = kind == RadrootsNostrKind::TextNote.as_u16();
        let is_reserved_deletion_request =
            kind == radroots_event::envelope::kind::KIND_DELETION_REQUEST as u16;
        let is_reserved_comment = kind == radroots_event::envelope::kind::KIND_COMMENT as u16;
        let classified_listing_partition =
            (kind == radroots_event::envelope::kind::KIND_CLASSIFIED_LISTING as u16).then(|| {
                classify_classified_listing_marker_names(
                    event
                        .tags
                        .iter()
                        .map(|tag| tag.as_slice().first().map(|name| name.as_str())),
                )
            });
        let is_reserved_focused_listing = matches!(
            classified_listing_partition,
            Some(
                ClassifiedListingPartition::FocusedFoodAvailability
                    | ClassifiedListingPartition::Ambiguous
            )
        );
        if is_profile
            || is_reserved_post
            || is_reserved_deletion_request
            || is_reserved_comment
            || is_reserved_focused_listing
        {
            return Err(RadrootsNostrError::TypedAuthoringRequired { kind });
        }
        Ok(())
    }
}

pub use nostr::nips::nip19::{
    FromBech32 as RadrootsNostrFromBech32, ToBech32 as RadrootsNostrToBech32,
};
pub use nostr::secp256k1::SecretKey as RadrootsNostrSecp256k1SecretKey;

#[cfg(test)]
mod tests {
    use super::*;

    fn keys() -> RadrootsNostrKeys {
        RadrootsNostrKeys::generate()
    }

    #[test]
    fn generic_direct_signing_rejects_typed_only_kinds() {
        let keys = keys();

        for (builder, expected_kind) in [
            (
                RadrootsNostrGenericEventBuilder::new(RadrootsNostrKind::Metadata, "{}"),
                RadrootsNostrKind::Metadata.as_u16(),
            ),
            (
                RadrootsNostrGenericEventBuilder::text_note("root post"),
                RadrootsNostrKind::TextNote.as_u16(),
            ),
            (
                RadrootsNostrGenericEventBuilder::new(
                    RadrootsNostrKind::Custom(
                        radroots_event::envelope::kind::KIND_DELETION_REQUEST as u16,
                    ),
                    "Deletion request",
                ),
                radroots_event::envelope::kind::KIND_DELETION_REQUEST as u16,
            ),
            (
                RadrootsNostrGenericEventBuilder::new(
                    RadrootsNostrKind::Custom(radroots_event::envelope::kind::KIND_COMMENT as u16),
                    "Comment",
                ),
                radroots_event::envelope::kind::KIND_COMMENT as u16,
            ),
        ] {
            assert!(matches!(
                builder.sign_with_keys(&keys),
                Err(RadrootsNostrError::TypedAuthoringRequired { kind })
                    if kind == expected_kind
            ));
        }
    }

    #[test]
    fn generic_direct_signing_rejects_thread_kind_one() {
        let error = RadrootsNostrGenericEventBuilder::text_note("reply")
            .tag(RadrootsNostrTag::event(RadrootsNostrEventId::all_zeros()))
            .sign_with_keys(&keys())
            .expect_err("all kind-1 authoring is typed");

        assert!(matches!(
            error,
            RadrootsNostrError::TypedAuthoringRequired { kind }
                if kind == RadrootsNostrKind::TextNote.as_u16()
        ));
    }

    #[test]
    fn generic_direct_signing_allows_non_reserved_kind() {
        let event =
            RadrootsNostrGenericEventBuilder::new(RadrootsNostrKind::Custom(30_001), "generic")
                .sign_with_keys(&keys())
                .expect("generic kind signs");

        assert_eq!(event.kind.as_u16(), 30_001);
    }

    #[test]
    fn external_signing_request_serializes_as_canonical_unsigned_event() {
        let keys = keys();
        let request =
            RadrootsNostrGenericEventBuilder::new(RadrootsNostrKind::Custom(24_133), "protocol")
                .custom_created_at(RadrootsNostrTimestamp::from_secs(1_234))
                .into_external_signing_request(keys.public_key())
                .expect("checked request");
        let expected_event_id = request.expected_event_id();

        let encoded = serde_json::to_vec(&request).expect("serialize request");
        let unsigned_event: nostr::UnsignedEvent =
            serde_json::from_slice(&encoded).expect("standard unsigned event");

        assert_eq!(unsigned_event.id, Some(expected_event_id));
        assert_eq!(unsigned_event.pubkey, keys.public_key());
        assert_eq!(unsigned_event.created_at.as_secs(), 1_234);
        assert_eq!(unsigned_event.kind.as_u16(), 24_133);
        assert_eq!(unsigned_event.content, "protocol");
        unsigned_event.verify_id().expect("canonical event id");
    }

    #[test]
    fn external_signing_request_rejects_reserved_authoring_before_finalization() {
        let error = match RadrootsNostrGenericEventBuilder::text_note("reserved")
            .into_external_signing_request(keys().public_key())
        {
            Ok(_) => panic!("kind 1 remains typed-only"),
            Err(error) => error,
        };

        assert!(matches!(
            error,
            RadrootsNostrError::TypedAuthoringRequired { kind }
                if kind == RadrootsNostrKind::TextNote.as_u16()
        ));
    }

    #[test]
    fn external_signing_request_accepts_only_the_exact_valid_event() {
        let keys = keys();
        let request =
            RadrootsNostrGenericEventBuilder::new(RadrootsNostrKind::Custom(24_133), "protocol")
                .custom_created_at(RadrootsNostrTimestamp::from_secs(1_234))
                .into_external_signing_request(keys.public_key())
                .expect("checked request");
        let unsigned_event: nostr::UnsignedEvent =
            serde_json::from_value(serde_json::to_value(&request).expect("request value"))
                .expect("unsigned event");
        let valid_event = unsigned_event
            .sign_with_keys(&keys)
            .expect("valid signing result");

        let wrong_author =
            RadrootsNostrGenericEventBuilder::new(RadrootsNostrKind::Custom(24_133), "protocol")
                .custom_created_at(RadrootsNostrTimestamp::from_secs(1_234))
                .sign_with_keys(&RadrootsNostrKeys::generate())
                .expect("other author event");
        assert!(matches!(
            request.complete(wrong_author),
            Err(RadrootsNostrError::ExternalSigningAuthorMismatch { .. })
        ));

        let request =
            RadrootsNostrGenericEventBuilder::new(RadrootsNostrKind::Custom(24_133), "protocol")
                .custom_created_at(RadrootsNostrTimestamp::from_secs(1_234))
                .into_external_signing_request(keys.public_key())
                .expect("checked request");
        let wrong_event_id =
            RadrootsNostrGenericEventBuilder::new(RadrootsNostrKind::Custom(24_133), "different")
                .sign_with_keys(&keys)
                .expect("different event");
        assert!(matches!(
            request.complete(wrong_event_id),
            Err(RadrootsNostrError::ExternalSigningEventIdMismatch { .. })
        ));

        for mutate in [
            |event: &mut RadrootsNostrEvent| event.content.push_str(" tampered"),
            |event: &mut RadrootsNostrEvent| {
                event.kind = RadrootsNostrKind::Custom(24_134);
            },
            |event: &mut RadrootsNostrEvent| {
                event.tags = nostr::Tags::from_list(vec![RadrootsNostrTag::custom(
                    RadrootsNostrTagKind::custom("x"),
                    ["tampered"],
                )]);
            },
        ] {
            let request = RadrootsNostrGenericEventBuilder::new(
                RadrootsNostrKind::Custom(24_133),
                "protocol",
            )
            .custom_created_at(RadrootsNostrTimestamp::from_secs(1_234))
            .into_external_signing_request(keys.public_key())
            .expect("checked request");
            let mut tampered = valid_event.clone();
            mutate(&mut tampered);
            assert!(matches!(
                request.complete(tampered),
                Err(RadrootsNostrError::ExternalSigningEventInvalid(_))
            ));
        }

        let other_signature =
            RadrootsNostrGenericEventBuilder::new(RadrootsNostrKind::Custom(24_133), "different")
                .sign_with_keys(&keys)
                .expect("other event")
                .sig;
        let request =
            RadrootsNostrGenericEventBuilder::new(RadrootsNostrKind::Custom(24_133), "protocol")
                .custom_created_at(RadrootsNostrTimestamp::from_secs(1_234))
                .into_external_signing_request(keys.public_key())
                .expect("checked request");
        let mut invalid_signature = valid_event;
        invalid_signature.sig = other_signature;
        assert!(matches!(
            request.complete(invalid_signature),
            Err(RadrootsNostrError::ExternalSigningEventInvalid(_))
        ));
    }
}
