//! Sealed authoring for validated NIP-01 profile metadata events.

#[cfg(feature = "events")]
use crate::error::Error;
#[cfg(feature = "events")]
use crate::events::sealed::SealedBuilderCore;
#[cfg(feature = "events")]
use crate::types::{
    ExternalSigningRequest, RadrootsNostrEvent, RadrootsNostrKeys, RadrootsNostrPublicKey,
    RadrootsNostrTimestamp,
};
#[cfg(feature = "events")]
use radroots_event::profile::AuthoredProfile;
#[cfg(feature = "events")]
use radroots_event_codec::authoring::{AuthoredEventBody, AuthoredPlanError};

/// A sealed builder for a validated kind-0 Profile replacement snapshot.
///
/// The wrapper exposes no raw builder conversion or tag/content mutation.
/// Media-bearing profiles still require the owning runtime to prove successful
/// BUD-02 upload completion before signing or publication.
#[cfg(feature = "events")]
#[must_use = "Profile event builders must be signed or published"]
pub struct ProfileBuilder {
    inner: SealedBuilderCore,
}

#[cfg(feature = "events")]
impl ProfileBuilder {
    /// Sets the event timestamp without changing the validated Profile shape.
    pub fn custom_created_at(mut self, created_at: RadrootsNostrTimestamp) -> Self {
        self.inner = self.inner.custom_created_at(created_at);
        self
    }

    /// Signs the validated Profile directly with local keys.
    pub fn sign_with_keys(self, keys: &RadrootsNostrKeys) -> Result<RadrootsNostrEvent, Error> {
        self.inner.sign_with_keys(keys)
    }

    /// Finalizes the exact authored plan for an external signer.
    pub fn into_external_signing_request(
        self,
        public_key: RadrootsNostrPublicKey,
    ) -> Result<ExternalSigningRequest, Error> {
        self.inner.into_external_signing_request(public_key)
    }
}

/// Builds a sealed kind-0 event from the strict authored Profile contract.
#[cfg(feature = "events")]
pub fn build_profile_event(profile: &AuthoredProfile) -> Result<ProfileBuilder, Error> {
    let body = AuthoredEventBody::from_profile(profile).map_err(|error| match error {
        AuthoredPlanError::Profile(error) => Error::ProfileEncode(error),
        error => Error::AuthoredPlan(error),
    })?;
    let inner = SealedBuilderCore::new(body);
    Ok(ProfileBuilder { inner })
}
