#[cfg(not(feature = "std"))]
extern crate alloc;

use radroots_event::profile::ProfileType;

#[cfg(feature = "std")]
type LegacyProfileString = std::string::String;
#[cfg(not(feature = "std"))]
type LegacyProfileString = alloc::string::String;

#[cfg(feature = "json")]
pub mod admission;

#[cfg(feature = "json")]
pub mod authored;

#[cfg(feature = "json")]
pub mod decode;

#[cfg(feature = "json")]
pub mod inbound;

/// Temporary lossy compatibility projection for pre-v1 profile consumers.
///
/// Strict reads use `inbound::RadrootsInboundProfileMetadata`. This type is
/// quarantined in the non-publishable intermediate codec surface and must be
/// removed with the superseded codec APIs in Step 087.
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct LegacyProfile {
    pub name: LegacyProfileString,
    pub display_name: Option<LegacyProfileString>,
    pub nip05: Option<LegacyProfileString>,
    pub about: Option<LegacyProfileString>,
    pub website: Option<LegacyProfileString>,
    pub picture: Option<LegacyProfileString>,
    pub banner: Option<LegacyProfileString>,
    pub lud06: Option<LegacyProfileString>,
    pub lud16: Option<LegacyProfileString>,
    pub bot: Option<LegacyProfileString>,
}

#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsProfileData {
    pub profile_type: Option<ProfileType>,
    pub profile: LegacyProfile,
}
