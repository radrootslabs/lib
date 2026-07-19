#![forbid(unsafe_code)]

use crate::error::RadrootsNostrError;
use radroots_event::classified_listing::{
    RadrootsClassifiedListingPartition, classify_classified_listing_marker_names,
};

pub type RadrootsNostrCoordinate = nostr::nips::nip01::Coordinate;
pub type RadrootsNostrEvent = nostr::Event;
pub(crate) type RadrootsNostrEventBuilderUnchecked = nostr::EventBuilder;
pub type RadrootsNostrEventId = nostr::EventId;
pub type RadrootsNostrFilter = nostr::Filter;
pub type RadrootsNostrKind = nostr::Kind;
pub type RadrootsNostrKeys = nostr::Keys;
pub type RadrootsNostrMetadata = nostr::Metadata;
pub type RadrootsNostrPublicKey = nostr::PublicKey;
pub type RadrootsNostrRelayUrl = nostr::RelayUrl;
pub type RadrootsNostrSecretKey = nostr::SecretKey;
pub type RadrootsNostrSubscriptionId = nostr::SubscriptionId;
pub type RadrootsNostrTag = nostr::Tag;
pub type RadrootsNostrTagKind<'a> = nostr::TagKind<'a>;
pub type RadrootsNostrTagStandard = nostr::TagStandard;
pub type RadrootsNostrTimestamp = nostr::Timestamp;
pub type RadrootsNostrUrl = nostr::Url;

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
/// use radroots_nostr::prelude::{
///     RadrootsNostrGenericEventBuilder, RadrootsNostrKind,
/// };
///
/// let builder = RadrootsNostrGenericEventBuilder::new(
///     RadrootsNostrKind::Custom(30_001),
///     "content",
/// );
/// let _: nostr::EventBuilder = builder.into();
/// ```
///
/// ```compile_fail
/// use radroots_nostr::prelude::{
///     RadrootsNostrGenericEventBuilder, RadrootsNostrKind,
/// };
///
/// let builder = RadrootsNostrGenericEventBuilder::new(
///     RadrootsNostrKind::Custom(30_001),
///     "content",
/// );
/// let _raw: nostr::EventBuilder = builder.into_inner();
/// ```
///
/// ```compile_fail
/// use radroots_nostr::prelude::{
///     RadrootsNostrGenericEventBuilder, RadrootsNostrKeys,
///     RadrootsNostrKind,
/// };
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
    pub fn sign_with_keys(
        self,
        keys: &RadrootsNostrKeys,
    ) -> Result<RadrootsNostrEvent, RadrootsNostrError> {
        self.validate_generic_authoring_policy()?;
        Ok(self.inner.sign_with_keys(keys)?)
    }

    pub(crate) fn from_unchecked(inner: RadrootsNostrEventBuilderUnchecked) -> Self {
        Self { inner }
    }

    #[cfg(feature = "client")]
    pub(crate) fn into_checked_event_builder(
        self,
    ) -> Result<RadrootsNostrEventBuilderUnchecked, RadrootsNostrError> {
        self.validate_generic_authoring_policy()?;
        Ok(self.inner)
    }

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
            kind == radroots_event::kinds::KIND_DELETION_REQUEST as u16;
        let is_reserved_comment = kind == radroots_event::kinds::KIND_COMMENT as u16;
        let classified_listing_partition =
            (kind == radroots_event::kinds::KIND_CLASSIFIED_LISTING as u16).then(|| {
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
                RadrootsClassifiedListingPartition::FocusedFoodAvailability
                    | RadrootsClassifiedListingPartition::Ambiguous
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

#[cfg(feature = "client")]
pub type RadrootsNostrMonitor = nostr_sdk::prelude::Monitor;

#[cfg(feature = "client")]
pub type RadrootsNostrMonitorNotification = nostr_sdk::prelude::MonitorNotification;

#[cfg(feature = "client")]
pub type RadrootsNostrOutput<T> = nostr_sdk::prelude::Output<T>;

#[cfg(feature = "client")]
pub type RadrootsNostrEventStream = nostr_sdk::pool::stream::BoxedStream<RadrootsNostrEvent>;

#[cfg(feature = "client")]
pub type RadrootsNostrRelay = nostr_sdk::Relay;

#[cfg(feature = "client")]
pub type RadrootsNostrRelayPoolNotification = nostr_sdk::RelayPoolNotification;

#[cfg(feature = "client")]
pub type RadrootsNostrRelayStatus = nostr_sdk::RelayStatus;

#[cfg(feature = "client")]
pub type RadrootsNostrSubscribeAutoCloseOptions = nostr_sdk::SubscribeAutoCloseOptions;

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
                    RadrootsNostrKind::Custom(radroots_event::kinds::KIND_DELETION_REQUEST as u16),
                    "Deletion request",
                ),
                radroots_event::kinds::KIND_DELETION_REQUEST as u16,
            ),
            (
                RadrootsNostrGenericEventBuilder::new(
                    RadrootsNostrKind::Custom(radroots_event::kinds::KIND_COMMENT as u16),
                    "Comment",
                ),
                radroots_event::kinds::KIND_COMMENT as u16,
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
}
