//! Assigned interoperable profile identities. Assignments alone do not qualify
//! authoring, authenticate a participant or establish a capability grant.

/// Regular unsigned rumor kind for NIP-59-wrapped structured commerce.
pub const COMMERCE_KIND: u16 = 8890;
/// Replaceable per-participant capability advertisement kind.
pub const CAPABILITIES_KIND: u16 = 18890;
/// Closed structured-commerce content profile.
pub const COMMERCE_PROFILE: &str = "radroots.commerce";
/// Closed capability-advertisement content profile.
pub const CAPABILITIES_PROFILE: &str = "radroots.capabilities";
/// Supported content version for these profiles.
pub const PROFILE_VERSION: u32 = 1;

/// Complete structured content, including its record identity and header.
pub const PRIVATE_RECORD_MAX_BYTES: usize = 16_384;
/// Complete private unsigned NIP-01 rumor representation.
pub const UNSIGNED_RUMOR_MAX_BYTES: usize = 24_576;
/// Complete signed private seal or outer event representation.
pub const SIGNED_EVENT_MAX_BYTES: usize = 65_536;
