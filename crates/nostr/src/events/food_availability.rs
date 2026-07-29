use crate::{
    error::RadrootsNostrError,
    types::{
        RadrootsNostrEvent, RadrootsNostrEventBuilderUnchecked, RadrootsNostrKeys,
        RadrootsNostrTimestamp,
    },
};
use radroots_event::food::availability::FoodAvailabilityDetails;
use radroots_event_codec::food_availability::authored::authored_food_availability_to_wire_parts;

/// A sealed builder for a validated focused FoodAvailability event.
///
/// The timestamp is fixed during typed construction because it participates in
/// strict domain validation and the canonical compact-wire budget.
/// Byte-verified image descriptors are not upload evidence. A media-bearing
/// caller must prove successful BUD-02 upload before signing or publication.
///
/// ```compile_fail
/// use radroots_nostr::prelude::{
///     RadrootsNostrFoodAvailabilityEventBuilder, RadrootsNostrTimestamp,
/// };
///
/// fn replace_validated_timestamp(builder: RadrootsNostrFoodAvailabilityEventBuilder) {
///     let _ = builder.custom_created_at(RadrootsNostrTimestamp::from_secs(1));
/// }
/// ```
///
/// ```compile_fail
/// use radroots_nostr::prelude::RadrootsNostrFoodAvailabilityEventBuilder;
///
/// fn expose_raw_builder(builder: RadrootsNostrFoodAvailabilityEventBuilder) {
///     let _: nostr::EventBuilder = builder.into();
/// }
/// ```
#[must_use = "FoodAvailability event builders must be signed or published"]
pub struct RadrootsNostrFoodAvailabilityEventBuilder {
    inner: RadrootsNostrEventBuilderUnchecked,
}

impl RadrootsNostrFoodAvailabilityEventBuilder {
    /// Signs the validated event directly with local keys.
    ///
    /// Media-bearing callers must prove successful BUD-02 upload first.
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

/// Builds a sealed Nostr builder from strict FoodAvailability details.
///
/// This validates media descriptors but does not attest BUD-02 upload.
pub fn radroots_nostr_build_food_availability_event(
    details: &FoodAvailabilityDetails,
    created_at: RadrootsNostrTimestamp,
) -> Result<RadrootsNostrFoodAvailabilityEventBuilder, RadrootsNostrError> {
    let parts = authored_food_availability_to_wire_parts(details, created_at.as_secs())?;
    let inner = super::radroots_nostr_build_event_unchecked(parts.kind, parts.content, parts.tags)?
        .custom_created_at(created_at);
    Ok(RadrootsNostrFoodAvailabilityEventBuilder { inner })
}
