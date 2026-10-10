//! Typed intent for complete kind-0 edits against a verified baseline.

#[cfg(not(feature = "std"))]
use alloc::string::String;

use crate::{media::AuthoredImage, profile::Nip05Identifier};

/// Preserve the exact admitted value, explicitly replace it, or remove it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ProfileFieldChange<T> {
    #[default]
    Keep,
    Set(T),
    Clear,
}

/// Keeping string media requires proof for its exact existing descriptor URL.
/// Upload completion and image decode safety remain runtime responsibilities.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ProfileMediaChange {
    #[default]
    Keep,
    KeepVerified(AuthoredImage),
    Set(AuthoredImage),
    Clear,
}

/// Only these fields can change; unknown admitted metadata is always preserved.
/// String lengths, name syntax and baseline freshness are checked by the codec.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProfileEdit {
    pub name: ProfileFieldChange<String>,
    pub display_name: ProfileFieldChange<String>,
    pub about: ProfileFieldChange<String>,
    pub picture: ProfileMediaChange,
    pub banner: ProfileMediaChange,
    pub nip05: ProfileFieldChange<Nip05Identifier>,
    pub bot: ProfileFieldChange<bool>,
}
