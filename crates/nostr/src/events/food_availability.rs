//! Sealed authoring for focused Radroots FoodAvailability events.

use crate::{
    error::Error,
    types::{
        RadrootsNostrEvent, RadrootsNostrEventBuilderUnchecked, RadrootsNostrKeys,
        RadrootsNostrTimestamp,
    },
};
use radroots_event::food::availability::FoodAvailabilityDetails;
use radroots_event_codec::encode::food_availability::authored_food_availability_to_wire_parts;

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
    inner: RadrootsNostrEventBuilderUnchecked,
}

impl FoodAvailabilityBuilder {
    /// Signs the validated event directly with local keys.
    ///
    /// Media-bearing callers must prove successful BUD-02 upload first.
    pub fn sign_with_keys(self, keys: &RadrootsNostrKeys) -> Result<RadrootsNostrEvent, Error> {
        Ok(self.inner.sign_with_keys(keys)?)
    }
}

/// Builds a sealed Nostr builder from strict FoodAvailability details.
///
/// This validates media descriptors but does not attest BUD-02 upload.
pub fn build_food_availability_event(
    details: &FoodAvailabilityDetails,
    created_at: RadrootsNostrTimestamp,
) -> Result<FoodAvailabilityBuilder, Error> {
    let parts = authored_food_availability_to_wire_parts(details, created_at.as_secs())?;
    let inner = super::build_event_unchecked(parts.kind, parts.content, parts.tags)?
        .custom_created_at(created_at);
    Ok(FoodAvailabilityBuilder { inner })
}
