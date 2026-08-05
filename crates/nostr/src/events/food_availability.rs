//! Sealed authoring for focused Radroots FoodAvailability events.

use crate::{
    error::Error,
    events::sealed::SealedBuilderCore,
    types::{
        ExternalSigningRequest, RadrootsNostrEvent, RadrootsNostrKeys, RadrootsNostrPublicKey,
        RadrootsNostrTimestamp,
    },
};
use radroots_event::food::availability::FoodAvailabilityDetails;
use radroots_event_codec::authoring::{AuthoredEventBody, AuthoredPlanError};

/// A sealed builder for a validated focused FoodAvailability event.
///
/// The timestamp is fixed during typed construction because it participates in
/// strict domain validation and the canonical compact-wire budget.
/// Byte-verified image descriptors are not upload evidence. A media-bearing
/// caller must prove successful BUD-02 upload before signing or publication.
///
/// ```compile_fail
/// use radroots_nostr::event::{FoodAvailabilityBuilder, Timestamp};
///
/// fn replace_validated_timestamp(builder: FoodAvailabilityBuilder) {
///     let _ = builder.custom_created_at(Timestamp::from_secs(1));
/// }
/// ```
///
/// ```compile_fail
/// use radroots_nostr::event::FoodAvailabilityBuilder;
///
/// fn expose_raw_builder(builder: FoodAvailabilityBuilder) {
///     let _: nostr::EventBuilder = builder.into();
/// }
/// ```
#[must_use = "FoodAvailability event builders must be signed or published"]
pub struct FoodAvailabilityBuilder {
    inner: SealedBuilderCore,
}

impl FoodAvailabilityBuilder {
    /// Signs the validated event directly with local keys.
    ///
    /// Media-bearing callers must prove successful BUD-02 upload first.
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

/// Builds a sealed Nostr builder from strict FoodAvailability details.
///
/// This validates media descriptors but does not attest BUD-02 upload.
pub fn build_food_availability_event(
    details: &FoodAvailabilityDetails,
    created_at: RadrootsNostrTimestamp,
) -> Result<FoodAvailabilityBuilder, Error> {
    let body = AuthoredEventBody::from_food_availability(details, created_at.as_secs()).map_err(
        |error| match error {
            AuthoredPlanError::FoodAvailability(error) => Error::FoodAvailabilityEncode(error),
            error => Error::AuthoredPlan(error),
        },
    )?;
    let inner = SealedBuilderCore::at(body, created_at);
    Ok(FoodAvailabilityBuilder { inner })
}
