//! Signing receipts.

/// Opaque receipt vocabulary for the object-safe SPI.
///
/// Step 102 defines the validated receipt contract before consumer migration.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SignReceipt;
