use crate::RadrootsNostrEvent;
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

#[cfg(not(feature = "std"))]
use alloc::string::String;

pub const RADROOTS_PROFILE_TYPE_TAG_KEY: &str = "t";
pub const RADROOTS_PROFILE_TYPE_TAG_INDIVIDUAL: &str = "radroots:type:individual";
pub const RADROOTS_PROFILE_TYPE_TAG_FARM: &str = "radroots:type:farm";
pub const RADROOTS_PROFILE_TYPE_TAG_COOP: &str = "radroots:type:coop";
pub const RADROOTS_PROFILE_TYPE_TAG_ANY: &str = "radroots:type:any";
pub const RADROOTS_PROFILE_TYPE_TAG_RADROOTSD: &str = "radroots:type:radrootsd";

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Debug, PartialEq, Eq, Copy)]
pub enum RadrootsProfileType {
    Individual,
    Farm,
    Coop,
    Any,
    Radrootsd,
}

pub fn radroots_profile_type_tag_value(profile_type: RadrootsProfileType) -> &'static str {
    match profile_type {
        RadrootsProfileType::Individual => RADROOTS_PROFILE_TYPE_TAG_INDIVIDUAL,
        RadrootsProfileType::Farm => RADROOTS_PROFILE_TYPE_TAG_FARM,
        RadrootsProfileType::Coop => RADROOTS_PROFILE_TYPE_TAG_COOP,
        RadrootsProfileType::Any => RADROOTS_PROFILE_TYPE_TAG_ANY,
        RadrootsProfileType::Radrootsd => RADROOTS_PROFILE_TYPE_TAG_RADROOTSD,
    }
}

pub fn radroots_profile_type_from_tag_value(value: &str) -> Option<RadrootsProfileType> {
    match value {
        RADROOTS_PROFILE_TYPE_TAG_INDIVIDUAL => Some(RadrootsProfileType::Individual),
        RADROOTS_PROFILE_TYPE_TAG_FARM => Some(RadrootsProfileType::Farm),
        RADROOTS_PROFILE_TYPE_TAG_COOP => Some(RadrootsProfileType::Coop),
        RADROOTS_PROFILE_TYPE_TAG_ANY => Some(RadrootsProfileType::Any),
        RADROOTS_PROFILE_TYPE_TAG_RADROOTSD => Some(RadrootsProfileType::Radrootsd),
        _ => None,
    }
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsProfileEventIndex {
    pub event: RadrootsNostrEvent,
    pub metadata: RadrootsProfileEventMetadata,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsProfileEventMetadata {
    pub id: String,
    pub author: String,
    pub published_at: u32,
    pub kind: u32,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "RadrootsProfileType | null"))]
    pub profile_type: Option<RadrootsProfileType>,
    pub profile: RadrootsProfile,
}

#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "types.ts"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct RadrootsProfile {
    pub name: String,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub display_name: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub nip05: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub about: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub website: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub picture: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub banner: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub lud06: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub lud16: Option<String>,
    #[cfg_attr(feature = "ts-rs", ts(optional, type = "string | null"))]
    pub bot: Option<String>,
}
