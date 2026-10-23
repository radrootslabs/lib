//! Closed participant claims. Parsing authenticates no author or capability.
use crate::{
    command::{CommandError, check_frame, parse_object},
    json_array::RawArray,
    profile,
};
use alloc::{borrow::Cow, vec::Vec};
pub use radroots_domain::UnixSeconds;
use serde::Deserialize;

/// One of the specified interoperable profile families.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Profile {
    Listing,
    Commerce,
    Chat,
}
/// A claimed protocol feature; it grants no business permission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Feature {
    ListingOptions,
    BaseOrder,
    BoundedVariableWeight,
    PlaintextInquiry,
}
/// A bounded validated participant claim, requiring separate authenticity/freshness.
#[derive(Debug)]
pub struct Claim {
    profile: Profile,
    versions: Vec<u16>,
    features: Vec<Feature>,
}
impl Claim {
    pub const fn profile(&self) -> Profile {
        self.profile
    }
    pub fn versions(&self) -> &[u16] {
        &self.versions
    }
    pub fn features(&self) -> &[Feature] {
        &self.features
    }
}
/// Complete passive claims. A descriptor is never a verified running program.
#[derive(Debug)]
pub struct Advertisement {
    claims: Vec<Claim>,
    valid_until: UnixSeconds,
}
impl Advertisement {
    pub fn claims(&self) -> &[Claim] {
        &self.claims
    }
    pub const fn valid_until(&self) -> UnixSeconds {
        self.valid_until
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAdvertisement<'a> {
    #[serde(borrow)]
    profile: Cow<'a, str>,
    version: u32,
    #[serde(borrow)]
    supported_profiles: RawArray<'a, 3>,
    valid_until: UnixSeconds,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawClaim<'a> {
    #[serde(borrow)]
    profile: Cow<'a, str>,
    #[serde(borrow)]
    versions: RawArray<'a, 8>,
    #[serde(borrow)]
    features: RawArray<'a, 8>,
}
fn unique<T: PartialEq>(values: &mut Vec<T>, next: T) -> Result<(), CommandError> {
    if values.contains(&next) {
        return Err(CommandError::InvalidInput);
    }
    values.push(next);
    Ok(())
}
/// Decode a complete closed claim and bind its at-most-thirty-day validity to
/// the original author timestamp. Current-time/clock policy remains separate.
pub fn decode(raw: &[u8], created_at: u64) -> Result<Advertisement, CommandError> {
    check_frame(raw)?;
    #[derive(Deserialize)]
    struct Header<'a> {
        #[serde(borrow)]
        profile: Cow<'a, str>,
        version: u32,
    }
    let header: Header<'_> = parse_object(raw)?;
    if header.profile != profile::CAPABILITIES_PROFILE || header.version != profile::PROFILE_VERSION
    {
        return Err(CommandError::UnsupportedProfile);
    }
    let value: RawAdvertisement<'_> = parse_object(raw)?;
    if value.profile != profile::CAPABILITIES_PROFILE || value.version != profile::PROFILE_VERSION {
        return Err(CommandError::UnsupportedProfile);
    }
    let authored = UnixSeconds::new(created_at).map_err(|_| CommandError::InvalidInput)?;
    if value
        .valid_until
        .get()
        .checked_sub(authored.get())
        .is_none_or(|duration| duration == 0 || duration > 30 * 24 * 60 * 60)
    {
        return Err(CommandError::InvalidInput);
    }
    if value.supported_profiles.0.is_empty() {
        return Err(CommandError::InvalidInput);
    }
    let mut claims = Vec::new();
    for raw in value.supported_profiles.0 {
        let claim: RawClaim<'_> = parse_object(raw.get().as_bytes())?;
        let profile = match claim.profile.as_ref() {
            "radroots.listing" => Profile::Listing,
            "radroots.commerce" => Profile::Commerce,
            "radroots.chat" => Profile::Chat,
            _ => return Err(CommandError::UnsupportedProfile),
        };
        if claim.versions.0.is_empty() || claim.features.0.is_empty() {
            return Err(CommandError::InvalidInput);
        }
        let mut versions = Vec::new();
        for raw in claim.versions.0 {
            let version: u16 =
                serde_json::from_str(raw.get()).map_err(|_| CommandError::InvalidInput)?;
            if version == 0 {
                return Err(CommandError::InvalidInput);
            }
            unique(&mut versions, version)?;
        }
        let mut features = Vec::new();
        for raw in claim.features.0 {
            let feature: Cow<'_, str> =
                serde_json::from_str(raw.get()).map_err(|_| CommandError::InvalidInput)?;
            let feature = match feature.as_ref() {
                "listing-options-v1" => Feature::ListingOptions,
                "base-order-v1" => Feature::BaseOrder,
                "bounded-variable-weight-v1" => Feature::BoundedVariableWeight,
                "plaintext-inquiry-v1" => Feature::PlaintextInquiry,
                _ => return Err(CommandError::UnsupportedProfile),
            };
            unique(&mut features, feature)?;
        }
        claims.push(Claim {
            profile,
            versions,
            features,
        });
    }
    Ok(Advertisement {
        claims,
        valid_until: value.valid_until,
    })
}
