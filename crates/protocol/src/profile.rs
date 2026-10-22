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
