#[cfg(not(feature = "std"))]
use alloc::string::String;
use core::{fmt, str::FromStr};

use crate::media::RadrootsAuthoredImage;

pub const RADROOTS_PROFILE_TYPE_TAG_KEY: &str = "t";
pub const RADROOTS_PROFILE_TYPE_TAG_INDIVIDUAL: &str = "radroots:type:individual";
pub const RADROOTS_PROFILE_TYPE_TAG_FARM: &str = "radroots:type:farm";
pub const RADROOTS_PROFILE_TYPE_TAG_COOP: &str = "radroots:type:coop";
pub const RADROOTS_PROFILE_TYPE_TAG_ANY: &str = "radroots:type:any";
pub const RADROOTS_PROFILE_TYPE_TAG_RADROOTSD: &str = "radroots:type:radrootsd";
pub const RADROOTS_PROFILE_METADATA_MAX_CONTENT_BYTES: usize =
    crate::wire::v1::DEFAULT_CONTENT_MAX_BYTES;

#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[cfg_attr(feature = "dto-bindgen", dto(export))]
#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(any(feature = "serde", test), serde(rename_all = "snake_case"))]
#[derive(Clone, Debug, PartialEq, Eq, Copy)]
pub enum RadrootsProfileType {
    #[cfg_attr(any(feature = "serde", test), serde(rename = "individual"))]
    Individual,
    #[cfg_attr(any(feature = "serde", test), serde(rename = "farm"))]
    Farm,
    #[cfg_attr(any(feature = "serde", test), serde(rename = "coop"))]
    Coop,
    #[cfg_attr(any(feature = "serde", test), serde(rename = "any"))]
    Any,
    #[cfg_attr(any(feature = "serde", test), serde(rename = "radrootsd"))]
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

#[cfg_attr(any(feature = "serde", test), derive(serde::Deserialize))]
/// Read-only compatibility projection used by legacy inbound codecs.
///
/// This type deliberately has no serializer or DTO export. Its media fields are
/// unverified strings and its `bot` field is not a JSON Boolean, so it must not
/// cross an authored or publication boundary. New reads use the tolerant
/// inbound metadata model; new authoring must use `RadrootsAuthoredProfile`.
#[derive(Clone, Debug)]
pub struct RadrootsProfile {
    pub name: String,
    pub display_name: Option<String>,
    pub nip05: Option<String>,
    pub about: Option<String>,
    pub website: Option<String>,
    pub picture: Option<String>,
    pub banner: Option<String>,
    pub lud06: Option<String>,
    pub lud16: Option<String>,
    pub bot: Option<String>,
}

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsNip05IdentifierError {
    MissingSeparator,
    MultipleSeparators,
    InvalidLocalPart,
    InvalidDomain,
}

impl RadrootsNip05IdentifierError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::MissingSeparator => "missing_separator",
            Self::MultipleSeparators => "multiple_separators",
            Self::InvalidLocalPart => "invalid_local_part",
            Self::InvalidDomain => "invalid_domain",
        }
    }
}

impl fmt::Display for RadrootsNip05IdentifierError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSeparator => f.write_str("NIP-05 identifier must contain one @ separator"),
            Self::MultipleSeparators => {
                f.write_str("NIP-05 identifier must not contain multiple @ separators")
            }
            Self::InvalidLocalPart => f.write_str("NIP-05 identifier local part is invalid"),
            Self::InvalidDomain => f.write_str("NIP-05 identifier domain is invalid"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsNip05IdentifierError {}

/// A syntax-checked NIP-05 internet identifier.
///
/// The DNS domain is canonicalized to lowercase. This type performs no network
/// resolution and makes no identity-verification claim.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RadrootsNip05Identifier(String);

impl RadrootsNip05Identifier {
    pub fn parse(value: &str) -> Result<Self, RadrootsNip05IdentifierError> {
        let Some((local_part, domain)) = value.split_once('@') else {
            return Err(RadrootsNip05IdentifierError::MissingSeparator);
        };
        if domain.contains('@') {
            return Err(RadrootsNip05IdentifierError::MultipleSeparators);
        }
        if local_part.is_empty()
            || !local_part.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'-' | b'_' | b'.')
            })
        {
            return Err(RadrootsNip05IdentifierError::InvalidLocalPart);
        }
        let domain = domain.to_ascii_lowercase();
        if !valid_nip05_domain(&domain) {
            return Err(RadrootsNip05IdentifierError::InvalidDomain);
        }
        let mut canonical = String::with_capacity(value.len());
        canonical.push_str(local_part);
        canonical.push('@');
        canonical.push_str(&domain);
        Ok(Self(canonical))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn local_part(&self) -> &str {
        self.0
            .split_once('@')
            .expect("validated NIP-05 identifiers always contain one separator")
            .0
    }

    pub fn domain(&self) -> &str {
        self.0
            .split_once('@')
            .expect("validated NIP-05 identifiers always contain one separator")
            .1
    }
}

impl fmt::Display for RadrootsNip05Identifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for RadrootsNip05Identifier {
    type Err = RadrootsNip05IdentifierError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

fn valid_nip05_domain(domain: &str) -> bool {
    !domain.is_empty()
        && domain.len() <= 253
        && domain.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
                && label
                    .as_bytes()
                    .first()
                    .is_some_and(u8::is_ascii_alphanumeric)
                && label
                    .as_bytes()
                    .last()
                    .is_some_and(u8::is_ascii_alphanumeric)
        })
}

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsAuthoredProfileError {
    InvalidName,
}

impl RadrootsAuthoredProfileError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidName => "invalid_name",
        }
    }
}

impl fmt::Display for RadrootsAuthoredProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName => {
                f.write_str("authored Profile name must be non-whitespace and control-free")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsAuthoredProfileError {}

/// Metadata accepted by the strict authored kind-0 Profile operation.
///
/// A non-whitespace, control-free name is required. Media values can only enter
/// through image-only, byte-verified Blossom descriptors. That state proves
/// descriptor/byte agreement, not successful network upload.
///
/// This model represents a complete kind-0 replacement snapshot, not a patch.
/// Omitting an existing field removes it from the authored replacement.
///
/// ```compile_fail
/// let _: radroots_event::profile::RadrootsAuthoredProfile =
///     serde_json::from_str(r#"{"name":"alice"}"#).unwrap();
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsAuthoredProfile {
    name: String,
    display_name: Option<String>,
    about: Option<String>,
    picture: Option<RadrootsAuthoredImage>,
    banner: Option<RadrootsAuthoredImage>,
    nip05: Option<RadrootsNip05Identifier>,
    bot: Option<bool>,
}

impl RadrootsAuthoredProfile {
    pub fn new(name: impl Into<String>) -> Result<Self, RadrootsAuthoredProfileError> {
        let name = name.into();
        if name.trim().is_empty() || name.chars().any(char::is_control) {
            return Err(RadrootsAuthoredProfileError::InvalidName);
        }
        Ok(Self {
            name,
            display_name: None,
            about: None,
            picture: None,
            banner: None,
            nip05: None,
            bot: None,
        })
    }

    #[must_use]
    pub fn with_display_name(mut self, value: impl Into<String>) -> Self {
        self.display_name = Some(value.into());
        self
    }

    #[must_use]
    pub fn with_about(mut self, value: impl Into<String>) -> Self {
        self.about = Some(value.into());
        self
    }

    #[must_use]
    pub fn with_picture(mut self, value: RadrootsAuthoredImage) -> Self {
        self.picture = Some(value);
        self
    }

    #[must_use]
    pub fn with_banner(mut self, value: RadrootsAuthoredImage) -> Self {
        self.banner = Some(value);
        self
    }

    #[must_use]
    pub fn with_nip05(mut self, value: RadrootsNip05Identifier) -> Self {
        self.nip05 = Some(value);
        self
    }

    #[must_use]
    pub const fn with_bot(mut self, value: bool) -> Self {
        self.bot = Some(value);
        self
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn display_name(&self) -> Option<&str> {
        self.display_name.as_deref()
    }

    pub fn about(&self) -> Option<&str> {
        self.about.as_deref()
    }

    pub fn picture(&self) -> Option<&RadrootsAuthoredImage> {
        self.picture.as_ref()
    }

    pub fn banner(&self) -> Option<&RadrootsAuthoredImage> {
        self.banner.as_ref()
    }

    pub fn nip05(&self) -> Option<&RadrootsNip05Identifier> {
        self.nip05.as_ref()
    }

    pub const fn bot(&self) -> Option<bool> {
        self.bot
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use radroots_blossom::{
        RadrootsBlossomBlobDescriptor, RadrootsBlossomBlobUrl,
        RadrootsBlossomByteVerifiedDescriptor, RadrootsBlossomMediaType, RadrootsBlossomSha256,
    };

    #[test]
    fn maps_profile_type_to_tag_value() {
        assert_eq!(
            radroots_profile_type_tag_value(RadrootsProfileType::Individual),
            RADROOTS_PROFILE_TYPE_TAG_INDIVIDUAL
        );
        assert_eq!(
            radroots_profile_type_tag_value(RadrootsProfileType::Farm),
            RADROOTS_PROFILE_TYPE_TAG_FARM
        );
        assert_eq!(
            radroots_profile_type_tag_value(RadrootsProfileType::Coop),
            RADROOTS_PROFILE_TYPE_TAG_COOP
        );
        assert_eq!(
            radroots_profile_type_tag_value(RadrootsProfileType::Any),
            RADROOTS_PROFILE_TYPE_TAG_ANY
        );
        assert_eq!(
            radroots_profile_type_tag_value(RadrootsProfileType::Radrootsd),
            RADROOTS_PROFILE_TYPE_TAG_RADROOTSD
        );
    }

    #[test]
    fn maps_tag_value_to_profile_type() {
        assert_eq!(
            radroots_profile_type_from_tag_value(RADROOTS_PROFILE_TYPE_TAG_INDIVIDUAL),
            Some(RadrootsProfileType::Individual)
        );
        assert_eq!(
            radroots_profile_type_from_tag_value(RADROOTS_PROFILE_TYPE_TAG_FARM),
            Some(RadrootsProfileType::Farm)
        );
        assert_eq!(
            radroots_profile_type_from_tag_value(RADROOTS_PROFILE_TYPE_TAG_COOP),
            Some(RadrootsProfileType::Coop)
        );
        assert_eq!(
            radroots_profile_type_from_tag_value(RADROOTS_PROFILE_TYPE_TAG_ANY),
            Some(RadrootsProfileType::Any)
        );
        assert_eq!(
            radroots_profile_type_from_tag_value(RADROOTS_PROFILE_TYPE_TAG_RADROOTSD),
            Some(RadrootsProfileType::Radrootsd)
        );
        assert_eq!(radroots_profile_type_from_tag_value("unknown"), None);
    }

    #[test]
    fn nip05_identifier_accepts_the_pinned_nip05_syntax_without_claiming_trust() {
        for (value, canonical) in [
            ("alice@example.com", "alice@example.com"),
            ("_@example.com", "_@example.com"),
            (
                "farm.stand-1@markets.example",
                "farm.stand-1@markets.example",
            ),
            ("alice@farm2.example", "alice@farm2.example"),
            ("alice@xn--bcher-kva.example", "alice@xn--bcher-kva.example"),
            ("alice@Example.COM", "alice@example.com"),
        ] {
            let identifier = RadrootsNip05Identifier::parse(value).unwrap();
            assert_eq!(identifier.as_str(), canonical);
            assert_eq!(identifier.to_string(), canonical);
            assert_eq!(
                canonical.parse::<RadrootsNip05Identifier>().unwrap(),
                identifier
            );
        }

        let identifier = RadrootsNip05Identifier::parse("alice@example.com").unwrap();
        assert_eq!(identifier.local_part(), "alice");
        assert_eq!(identifier.domain(), "example.com");
    }

    #[test]
    fn nip05_identifier_rejects_noncanonical_or_ambiguous_syntax() {
        let cases = [
            ("alice", RadrootsNip05IdentifierError::MissingSeparator),
            (
                "alice@example.com@other.example",
                RadrootsNip05IdentifierError::MultipleSeparators,
            ),
            (
                "@example.com",
                RadrootsNip05IdentifierError::InvalidLocalPart,
            ),
            (
                "Alice@example.com",
                RadrootsNip05IdentifierError::InvalidLocalPart,
            ),
            (
                "alice+farm@example.com",
                RadrootsNip05IdentifierError::InvalidLocalPart,
            ),
            ("alice@", RadrootsNip05IdentifierError::InvalidDomain),
            (
                "alice@-example.com",
                RadrootsNip05IdentifierError::InvalidDomain,
            ),
            (
                "alice@example-.com",
                RadrootsNip05IdentifierError::InvalidDomain,
            ),
            (
                "alice@example..com",
                RadrootsNip05IdentifierError::InvalidDomain,
            ),
            (
                "alice@example.com.",
                RadrootsNip05IdentifierError::InvalidDomain,
            ),
            (
                "alice@example_com",
                RadrootsNip05IdentifierError::InvalidDomain,
            ),
        ];

        for (value, expected) in cases {
            let error = RadrootsNip05Identifier::parse(value).unwrap_err();
            assert_eq!(error, expected, "{value}");
            assert!(!error.code().is_empty());
            assert!(!error.to_string().is_empty());
        }

        let long_label = "a".repeat(64);
        assert_eq!(
            RadrootsNip05Identifier::parse(&format!("alice@{long_label}.example")),
            Err(RadrootsNip05IdentifierError::InvalidDomain)
        );
        let long_domain = (0..43).map(|_| "aaaaa").collect::<Vec<_>>().join(".");
        assert!(long_domain.len() > 253);
        assert_eq!(
            RadrootsNip05Identifier::parse(&format!("alice@{long_domain}")),
            Err(RadrootsNip05IdentifierError::InvalidDomain)
        );
    }

    #[test]
    fn authored_profile_requires_name_and_image_only_verified_media() {
        let media =
            RadrootsAuthoredImage::try_from(verified_descriptor("image/webp", "webp")).unwrap();
        let profile = RadrootsAuthoredProfile::new("alice")
            .unwrap()
            .with_display_name("Alice")
            .with_about("Victoria grower")
            .with_picture(media.clone())
            .with_banner(media)
            .with_nip05(RadrootsNip05Identifier::parse("alice@example.com").unwrap())
            .with_bot(false);

        assert_eq!(profile.name(), "alice");
        assert_eq!(profile.display_name(), Some("Alice"));
        assert_eq!(profile.about(), Some("Victoria grower"));
        assert_eq!(
            profile.nip05().map(RadrootsNip05Identifier::as_str),
            Some("alice@example.com")
        );
        assert_eq!(profile.bot(), Some(false));
        assert_eq!(
            profile
                .picture()
                .map(|value| value.descriptor().url().as_str()),
            Some(
                "https://media.example/2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824.webp"
            )
        );
        assert_eq!(
            profile
                .banner()
                .map(|value| value.descriptor().url().as_str()),
            profile
                .picture()
                .map(|value| value.descriptor().url().as_str())
        );

        let error =
            RadrootsAuthoredImage::try_from(verified_descriptor("text/plain", "txt")).unwrap_err();
        assert_eq!(
            error,
            crate::media::RadrootsAuthoredImageError::MediaTypeNotImage
        );
        assert_eq!(error.code(), "media_type_not_image");
        assert!(!error.to_string().is_empty());

        for invalid_name in ["", "   ", "alice\n"] {
            let error = RadrootsAuthoredProfile::new(invalid_name).unwrap_err();
            assert_eq!(error, RadrootsAuthoredProfileError::InvalidName);
            assert_eq!(error.code(), "invalid_name");
            assert!(!error.to_string().is_empty());
        }
    }

    fn verified_descriptor(
        media_type: &str,
        extension: &str,
    ) -> RadrootsBlossomByteVerifiedDescriptor {
        let bytes = b"hello";
        let hash = RadrootsBlossomSha256::digest(bytes);
        let media_type = RadrootsBlossomMediaType::parse(media_type).unwrap();
        RadrootsBlossomBlobDescriptor::new(
            RadrootsBlossomBlobUrl::parse(&format!("https://media.example/{hash}.{extension}"))
                .unwrap(),
            hash,
            bytes.len() as u64,
            media_type.clone(),
            1_784_347_200,
        )
        .unwrap()
        .approve_reference()
        .unwrap()
        .verify_bytes(bytes, &media_type)
        .unwrap()
    }
}
