//! Sealed authoring for validated NIP-01 profile metadata events.

#[cfg(feature = "events")]
use crate::error::RadrootsNostrError;
#[cfg(feature = "events")]
use crate::types::RadrootsNostrEvent;
#[cfg(feature = "events")]
use crate::types::{RadrootsNostrEventBuilderUnchecked, RadrootsNostrKeys, RadrootsNostrTimestamp};
#[cfg(feature = "events")]
use radroots_event::profile::AuthoredProfile;
#[cfg(feature = "events")]
use radroots_event_codec::encode::profile::authored_profile_to_wire_parts;

/// A sealed builder for a validated kind-0 Profile replacement snapshot.
///
/// The wrapper exposes no raw builder conversion or tag/content mutation.
/// Media-bearing profiles still require the owning runtime to prove successful
/// BUD-02 upload completion before signing or publication.
#[cfg(feature = "events")]
#[must_use = "Profile event builders must be signed or published"]
pub struct RadrootsNostrProfileEventBuilder {
    inner: RadrootsNostrEventBuilderUnchecked,
}

#[cfg(feature = "events")]
impl RadrootsNostrProfileEventBuilder {
    /// Sets the event timestamp without changing the validated Profile shape.
    pub fn custom_created_at(mut self, created_at: RadrootsNostrTimestamp) -> Self {
        self.inner = self.inner.custom_created_at(created_at);
        self
    }

    /// Signs the validated Profile directly with local keys.
    pub fn sign_with_keys(
        self,
        keys: &RadrootsNostrKeys,
    ) -> Result<RadrootsNostrEvent, RadrootsNostrError> {
        Ok(self.inner.sign_with_keys(keys)?)
    }
}

/// Builds a sealed kind-0 event from the strict authored Profile contract.
#[cfg(feature = "events")]
pub fn radroots_nostr_build_profile_event(
    profile: &AuthoredProfile,
) -> Result<RadrootsNostrProfileEventBuilder, RadrootsNostrError> {
    let parts = authored_profile_to_wire_parts(profile)?;
    let inner =
        crate::events::radroots_nostr_build_event_unchecked(parts.kind, parts.content, parts.tags)?;
    Ok(RadrootsNostrProfileEventBuilder { inner })
}
