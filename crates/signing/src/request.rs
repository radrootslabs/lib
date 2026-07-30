//! Validated signing requests.

/// Opaque request vocabulary for the object-safe SPI.
///
/// Step 102 defines the actor, frozen-draft, deadline, policy, and progress
/// fields before consumer migration.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SignRequest;
