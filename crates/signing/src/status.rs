//! Signer progress and status models.

/// Opaque status vocabulary for the object-safe SPI.
///
/// Step 102 defines the capability, progress, and challenge state model before
/// consumer migration.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SignerStatus;
