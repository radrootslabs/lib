#[cfg(not(feature = "std"))]
use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use core::{fmt, ops::RangeInclusive, str::FromStr};

use crate::ids::{
    RadrootsAddressableCoordinate, RadrootsDTag, RadrootsPublicKey, RadrootsRelayUrl,
};
use crate::media::RadrootsAuthoredImage;
use crate::social::{
    RadrootsCalendarEventFreeBusy, RadrootsCalendarEventRsvpStatus, RadrootsCalendarParticipant,
    RadrootsSocialTarget,
};
use crate::wire::{
    DEFAULT_CONTENT_MAX_BYTES, DEFAULT_TAG_ELEMENT_MAX_BYTES, DEFAULT_TAG_MAX_COUNT,
    DEFAULT_TAG_TOTAL_MAX_BYTES,
};
use radroots_blossom::RadrootsBlossomBlobUrl;
use url_nostd::Url;

pub const RADROOTS_CALENDAR_SECONDS_PER_DAY: u64 = 86_400;
pub const RADROOTS_CALENDAR_MAX_COVERED_UTC_DAYS: u64 = 366;
pub const RADROOTS_CALENDAR_MAX_PARTICIPANTS: usize = DEFAULT_TAG_MAX_COUNT - 16;

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug)]
pub struct RadrootsCalendar {
    pub d_tag: String,
    pub title: String,
    pub events: Vec<RadrootsSocialTarget>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub description: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub summary: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub image: Option<String>,
}

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsCalendarEventError {
    InvalidIdentifier,
    InvalidTitle,
    InvalidText(&'static str),
    InvalidUrl(&'static str),
    InvalidGeohash,
    InvalidTimeZone,
    InvalidParticipant {
        index: usize,
    },
    TooManyParticipants {
        max: usize,
        actual: usize,
    },
    ContentTooLarge {
        max: usize,
        actual: usize,
    },
    TagElementTooLarge {
        field: &'static str,
        max: usize,
        actual: usize,
    },
    TagCountExceeded {
        max: usize,
        actual: usize,
    },
    TagBytesExceeded {
        max: usize,
        actual: usize,
    },
    InvalidDate,
    InvalidRange,
    CoveredDayLimitExceeded {
        max: u64,
        actual: u64,
    },
}

impl RadrootsCalendarEventError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidIdentifier => "invalid_identifier",
            Self::InvalidTitle => "invalid_title",
            Self::InvalidText(_) => "invalid_text",
            Self::InvalidUrl(_) => "invalid_url",
            Self::InvalidGeohash => "invalid_geohash",
            Self::InvalidTimeZone => "invalid_time_zone",
            Self::InvalidParticipant { .. } => "invalid_participant",
            Self::TooManyParticipants { .. } => "too_many_participants",
            Self::ContentTooLarge { .. } => "content_too_large",
            Self::TagElementTooLarge { .. } => "tag_element_too_large",
            Self::TagCountExceeded { .. } => "tag_count_exceeded",
            Self::TagBytesExceeded { .. } => "tag_bytes_exceeded",
            Self::InvalidDate => "invalid_date",
            Self::InvalidRange => "invalid_range",
            Self::CoveredDayLimitExceeded { .. } => "covered_day_limit_exceeded",
        }
    }
}

impl fmt::Display for RadrootsCalendarEventError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIdentifier => f.write_str("calendar identifier is invalid"),
            Self::InvalidTitle => f.write_str("calendar title must be canonical visible text"),
            Self::InvalidText(field) => write!(f, "calendar {field} must be visible text"),
            Self::InvalidUrl(field) => write!(f, "calendar {field} is not a valid URI"),
            Self::InvalidGeohash => f.write_str("calendar geohash is invalid"),
            Self::InvalidTimeZone => {
                f.write_str("calendar time zone is not a canonical IANA identifier")
            }
            Self::InvalidParticipant { index } => {
                write!(f, "calendar participant at index {index} is invalid")
            }
            Self::TooManyParticipants { max, actual } => write!(
                f,
                "calendar participant count {actual} exceeds maximum {max}"
            ),
            Self::ContentTooLarge { max, actual } => {
                write!(f, "calendar content size {actual} exceeds maximum {max}")
            }
            Self::TagElementTooLarge { field, max, actual } => write!(
                f,
                "calendar {field} size {actual} exceeds tag element maximum {max}"
            ),
            Self::TagCountExceeded { max, actual } => {
                write!(f, "calendar tag count {actual} exceeds maximum {max}")
            }
            Self::TagBytesExceeded { max, actual } => {
                write!(f, "calendar tag bytes {actual} exceed maximum {max}")
            }
            Self::InvalidDate => f.write_str("calendar date is not a valid Gregorian date"),
            Self::InvalidRange => f.write_str("calendar end must be greater than start"),
            Self::CoveredDayLimitExceeded { max, actual } => write!(
                f,
                "calendar interval covers {actual} UTC days, exceeding the {max}-day limit"
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsCalendarEventError {}

/// A canonical IANA Time Zone Database identifier.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RadrootsIanaTimeZoneId(String);

impl RadrootsIanaTimeZoneId {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, RadrootsCalendarEventError> {
        let value = value.as_ref();
        let Some((canonical, _)) = jiff_tzdb::get(value) else {
            return Err(RadrootsCalendarEventError::InvalidTimeZone);
        };
        if canonical != value || !canonical_calendar_tag_text_is_valid(value) {
            return Err(RadrootsCalendarEventError::InvalidTimeZone);
        }
        Ok(Self(value.into()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RadrootsIanaTimeZoneId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl AsRef<str> for RadrootsIanaTimeZoneId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl FromStr for RadrootsIanaTimeZoneId {
    type Err = RadrootsCalendarEventError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

#[cfg(any(feature = "serde", test))]
impl serde::Serialize for RadrootsIanaTimeZoneId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

#[cfg(any(feature = "serde", test))]
impl<'de> serde::Deserialize<'de> for RadrootsIanaTimeZoneId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = <String as serde::Deserialize>::deserialize(deserializer)?;
        Self::parse(value).map_err(serde::de::Error::custom)
    }
}

/// A structurally valid absolute URI preserved exactly as it appeared on wire.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RadrootsCalendarUri(String);

impl RadrootsCalendarUri {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, RadrootsCalendarEventError> {
        let value = value.as_ref();
        if value.trim() != value
            || value
                .chars()
                .any(|character| character.is_whitespace() || character.is_control())
            || value.len() > DEFAULT_TAG_ELEMENT_MAX_BYTES
            || Url::parse(value).is_err()
        {
            return Err(RadrootsCalendarEventError::InvalidUrl("URI"));
        }
        Ok(Self(value.into()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for RadrootsCalendarUri {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl FromStr for RadrootsCalendarUri {
    type Err = RadrootsCalendarEventError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl fmt::Display for RadrootsCalendarUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(any(feature = "serde", test))]
impl serde::Serialize for RadrootsCalendarUri {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

#[cfg(any(feature = "serde", test))]
impl<'de> serde::Deserialize<'de> for RadrootsCalendarUri {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = <String as serde::Deserialize>::deserialize(deserializer)?;
        Self::parse(value).map_err(serde::de::Error::custom)
    }
}

#[cfg_attr(any(feature = "serde", test), derive(serde::Serialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsCalendarRequest {
    calendar: RadrootsAddressableCoordinate,
    relay: Option<String>,
}

impl RadrootsCalendarRequest {
    pub fn new(
        calendar: impl AsRef<str>,
        relay: Option<&str>,
    ) -> Result<Self, RadrootsCalendarEventError> {
        let calendar = RadrootsAddressableCoordinate::parse(calendar.as_ref())
            .map_err(|_| RadrootsCalendarEventError::InvalidUrl("calendar request"))?;
        let parts = crate::ids::RadrootsAddressableCoordinateParts::parse(calendar.as_str())
            .map_err(|_| RadrootsCalendarEventError::InvalidUrl("calendar request"))?;
        if parts.kind != crate::kinds::KIND_CALENDAR {
            return Err(RadrootsCalendarEventError::InvalidUrl("calendar request"));
        }
        let relay = relay
            .map(|value| {
                if calendar_relay_url_is_valid(value) {
                    Ok(value.to_string())
                } else {
                    Err(RadrootsCalendarEventError::InvalidUrl(
                        "calendar request relay",
                    ))
                }
            })
            .transpose()?;
        Ok(Self { calendar, relay })
    }

    pub fn calendar(&self) -> &RadrootsAddressableCoordinate {
        &self.calendar
    }

    pub fn relay(&self) -> Option<&str> {
        self.relay.as_deref()
    }

    pub fn is_canonical(&self) -> bool {
        let Ok(parts) =
            crate::ids::RadrootsAddressableCoordinateParts::parse(self.calendar.as_str())
        else {
            return false;
        };
        self.calendar.as_str() == format!("{}:{}:{}", parts.kind, parts.pubkey, parts.d_tag)
            && self
                .relay
                .as_deref()
                .is_none_or(|relay| RadrootsRelayUrl::parse(relay).is_ok())
    }
}

/// A canonical Gregorian calendar date in `YYYY-MM-DD` form.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RadrootsCalendarDate(String);

impl RadrootsCalendarDate {
    pub fn parse(value: &str) -> Result<Self, RadrootsCalendarEventError> {
        let bytes = value.as_bytes();
        if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
            return Err(RadrootsCalendarEventError::InvalidDate);
        }
        let year = parse_date_digits(&bytes[0..4])?;
        let month = parse_date_digits(&bytes[5..7])?;
        let day = parse_date_digits(&bytes[8..10])?;
        if year == 0 || month == 0 || month > 12 || day == 0 || day > days_in_month(year, month) {
            return Err(RadrootsCalendarEventError::InvalidDate);
        }
        Ok(Self(value.into()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RadrootsCalendarDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl AsRef<str> for RadrootsCalendarDate {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl FromStr for RadrootsCalendarDate {
    type Err = RadrootsCalendarEventError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

#[cfg(any(feature = "serde", test))]
impl serde::Serialize for RadrootsCalendarDate {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

#[cfg(any(feature = "serde", test))]
impl<'de> serde::Deserialize<'de> for RadrootsCalendarDate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = <String as serde::Deserialize>::deserialize(deserializer)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}

/// Strict authored NIP-52 kind-31922 event.
///
/// Authored images can only enter through a byte-verified image descriptor.
/// The model cannot represent the obsolete uppercase-`D` date list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsAuthoredCalendarDateEvent {
    d_tag: RadrootsDTag,
    title: String,
    start: RadrootsCalendarDate,
    description: Option<String>,
    end: Option<RadrootsCalendarDate>,
    locations: Vec<String>,
    geohash: Option<String>,
    summary: Option<String>,
    image: Option<RadrootsAuthoredImage>,
    participants: Option<Vec<RadrootsCalendarParticipant>>,
    categories: Vec<String>,
    references: Vec<RadrootsCalendarUri>,
    calendar_requests: Vec<RadrootsCalendarRequest>,
}

impl RadrootsAuthoredCalendarDateEvent {
    pub fn new(
        d_tag: impl AsRef<str>,
        title: impl Into<String>,
        start: RadrootsCalendarDate,
    ) -> Result<Self, RadrootsCalendarEventError> {
        let d_tag = RadrootsDTag::parse(d_tag.as_ref())
            .map_err(|_| RadrootsCalendarEventError::InvalidIdentifier)?;
        let title = validated_title(title.into())?;
        Ok(Self {
            d_tag,
            title,
            start,
            description: None,
            end: None,
            locations: Vec::new(),
            geohash: None,
            summary: None,
            image: None,
            participants: None,
            categories: Vec::new(),
            references: Vec::new(),
            calendar_requests: Vec::new(),
        })
    }

    pub fn with_end(
        mut self,
        end: RadrootsCalendarDate,
    ) -> Result<Self, RadrootsCalendarEventError> {
        if end <= self.start {
            return Err(RadrootsCalendarEventError::InvalidRange);
        }
        self.end = Some(end);
        self.validate_budget()?;
        Ok(self)
    }

    pub fn with_description(
        mut self,
        value: impl Into<String>,
    ) -> Result<Self, RadrootsCalendarEventError> {
        let value = value.into();
        validate_calendar_content(&value)?;
        self.description = Some(value);
        self.validate_budget()?;
        Ok(self)
    }

    pub fn with_locations(
        mut self,
        value: Vec<String>,
    ) -> Result<Self, RadrootsCalendarEventError> {
        validate_authored_calendar_locations(&value)?;
        self.locations = value;
        self.validate_budget()?;
        Ok(self)
    }

    pub fn with_geohash(
        mut self,
        value: impl Into<String>,
    ) -> Result<Self, RadrootsCalendarEventError> {
        let value = value.into();
        if !canonical_calendar_geohash_is_valid(&value) {
            return Err(RadrootsCalendarEventError::InvalidGeohash);
        }
        self.geohash = Some(value);
        self.validate_budget()?;
        Ok(self)
    }

    pub fn with_summary(
        mut self,
        value: impl Into<String>,
    ) -> Result<Self, RadrootsCalendarEventError> {
        let value = value.into();
        validate_canonical_calendar_tag_text(&value, "summary")?;
        self.summary = Some(value);
        self.validate_budget()?;
        Ok(self)
    }

    pub fn with_image(
        mut self,
        value: RadrootsAuthoredImage,
    ) -> Result<Self, RadrootsCalendarEventError> {
        self.image = Some(value);
        self.validate_budget()?;
        Ok(self)
    }

    pub fn with_participants(
        mut self,
        value: Vec<RadrootsCalendarParticipant>,
    ) -> Result<Self, RadrootsCalendarEventError> {
        validate_authored_calendar_participants(&value)?;
        self.participants = Some(value);
        self.validate_budget()?;
        Ok(self)
    }

    pub fn with_categories(
        mut self,
        value: Vec<String>,
    ) -> Result<Self, RadrootsCalendarEventError> {
        validate_authored_calendar_categories(&value)?;
        self.categories = value;
        self.validate_budget()?;
        Ok(self)
    }

    pub fn with_references(
        mut self,
        value: Vec<RadrootsCalendarUri>,
    ) -> Result<Self, RadrootsCalendarEventError> {
        self.references = value;
        self.validate_budget()?;
        Ok(self)
    }

    pub fn with_calendar_requests(
        mut self,
        value: Vec<RadrootsCalendarRequest>,
    ) -> Result<Self, RadrootsCalendarEventError> {
        validate_authored_calendar_requests(&value)?;
        self.calendar_requests = value;
        self.validate_budget()?;
        Ok(self)
    }

    pub fn d_tag(&self) -> &RadrootsDTag {
        &self.d_tag
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn start(&self) -> &RadrootsCalendarDate {
        &self.start
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    pub fn end(&self) -> Option<&RadrootsCalendarDate> {
        self.end.as_ref()
    }

    pub fn locations(&self) -> &[String] {
        &self.locations
    }

    pub fn geohash(&self) -> Option<&str> {
        self.geohash.as_deref()
    }

    pub fn summary(&self) -> Option<&str> {
        self.summary.as_deref()
    }

    pub fn image(&self) -> Option<&RadrootsAuthoredImage> {
        self.image.as_ref()
    }

    pub fn participants(&self) -> Option<&Vec<RadrootsCalendarParticipant>> {
        self.participants.as_ref()
    }

    pub fn categories(&self) -> &[String] {
        &self.categories
    }

    pub fn references(&self) -> &[RadrootsCalendarUri] {
        &self.references
    }

    pub fn calendar_requests(&self) -> &[RadrootsCalendarRequest] {
        &self.calendar_requests
    }

    fn validate_budget(&self) -> Result<(), RadrootsCalendarEventError> {
        validate_authored_calendar_budget(self.description(), authored_date_tags_for_budget(self))
    }
}

/// Strict authored NIP-52 kind-31923 event.
///
/// Covered uppercase-`D` values are derived from the timestamp range and are
/// never accepted from callers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsAuthoredCalendarTimeEvent {
    d_tag: RadrootsDTag,
    title: String,
    start: u64,
    description: Option<String>,
    end: Option<u64>,
    start_tzid: Option<RadrootsIanaTimeZoneId>,
    end_tzid: Option<RadrootsIanaTimeZoneId>,
    locations: Vec<String>,
    geohash: Option<String>,
    summary: Option<String>,
    image: Option<RadrootsAuthoredImage>,
    participants: Option<Vec<RadrootsCalendarParticipant>>,
    categories: Vec<String>,
    references: Vec<RadrootsCalendarUri>,
    calendar_requests: Vec<RadrootsCalendarRequest>,
}

impl RadrootsAuthoredCalendarTimeEvent {
    pub fn new(
        d_tag: impl AsRef<str>,
        title: impl Into<String>,
        start: u64,
    ) -> Result<Self, RadrootsCalendarEventError> {
        let d_tag = RadrootsDTag::parse(d_tag.as_ref())
            .map_err(|_| RadrootsCalendarEventError::InvalidIdentifier)?;
        let title = validated_title(title.into())?;
        covered_utc_days(start, None)?;
        Ok(Self {
            d_tag,
            title,
            start,
            description: None,
            end: None,
            start_tzid: None,
            end_tzid: None,
            locations: Vec::new(),
            geohash: None,
            summary: None,
            image: None,
            participants: None,
            categories: Vec::new(),
            references: Vec::new(),
            calendar_requests: Vec::new(),
        })
    }

    pub fn with_end(mut self, end: u64) -> Result<Self, RadrootsCalendarEventError> {
        covered_utc_days(self.start, Some(end))?;
        self.end = Some(end);
        self.validate_budget()?;
        Ok(self)
    }

    pub fn with_description(
        mut self,
        value: impl Into<String>,
    ) -> Result<Self, RadrootsCalendarEventError> {
        let value = value.into();
        validate_calendar_content(&value)?;
        self.description = Some(value);
        self.validate_budget()?;
        Ok(self)
    }

    pub fn with_start_tzid(
        mut self,
        value: impl AsRef<str>,
    ) -> Result<Self, RadrootsCalendarEventError> {
        self.start_tzid = Some(RadrootsIanaTimeZoneId::parse(value)?);
        self.validate_budget()?;
        Ok(self)
    }

    pub fn with_end_tzid(
        mut self,
        value: impl AsRef<str>,
    ) -> Result<Self, RadrootsCalendarEventError> {
        self.end_tzid = Some(RadrootsIanaTimeZoneId::parse(value)?);
        self.validate_budget()?;
        Ok(self)
    }

    pub fn with_locations(
        mut self,
        value: Vec<String>,
    ) -> Result<Self, RadrootsCalendarEventError> {
        validate_authored_calendar_locations(&value)?;
        self.locations = value;
        self.validate_budget()?;
        Ok(self)
    }

    pub fn with_geohash(
        mut self,
        value: impl Into<String>,
    ) -> Result<Self, RadrootsCalendarEventError> {
        let value = value.into();
        if !canonical_calendar_geohash_is_valid(&value) {
            return Err(RadrootsCalendarEventError::InvalidGeohash);
        }
        self.geohash = Some(value);
        self.validate_budget()?;
        Ok(self)
    }

    pub fn with_summary(
        mut self,
        value: impl Into<String>,
    ) -> Result<Self, RadrootsCalendarEventError> {
        let value = value.into();
        validate_canonical_calendar_tag_text(&value, "summary")?;
        self.summary = Some(value);
        self.validate_budget()?;
        Ok(self)
    }

    pub fn with_image(
        mut self,
        value: RadrootsAuthoredImage,
    ) -> Result<Self, RadrootsCalendarEventError> {
        self.image = Some(value);
        self.validate_budget()?;
        Ok(self)
    }

    pub fn with_participants(
        mut self,
        value: Vec<RadrootsCalendarParticipant>,
    ) -> Result<Self, RadrootsCalendarEventError> {
        validate_authored_calendar_participants(&value)?;
        self.participants = Some(value);
        self.validate_budget()?;
        Ok(self)
    }

    pub fn with_categories(
        mut self,
        value: Vec<String>,
    ) -> Result<Self, RadrootsCalendarEventError> {
        validate_authored_calendar_categories(&value)?;
        self.categories = value;
        self.validate_budget()?;
        Ok(self)
    }

    pub fn with_references(
        mut self,
        value: Vec<RadrootsCalendarUri>,
    ) -> Result<Self, RadrootsCalendarEventError> {
        self.references = value;
        self.validate_budget()?;
        Ok(self)
    }

    pub fn with_calendar_requests(
        mut self,
        value: Vec<RadrootsCalendarRequest>,
    ) -> Result<Self, RadrootsCalendarEventError> {
        validate_authored_calendar_requests(&value)?;
        self.calendar_requests = value;
        self.validate_budget()?;
        Ok(self)
    }

    pub fn d_tag(&self) -> &RadrootsDTag {
        &self.d_tag
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub const fn start(&self) -> u64 {
        self.start
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    pub const fn end(&self) -> Option<u64> {
        self.end
    }

    pub fn start_tzid(&self) -> Option<&RadrootsIanaTimeZoneId> {
        self.start_tzid.as_ref()
    }

    pub fn end_tzid(&self) -> Option<&RadrootsIanaTimeZoneId> {
        self.end_tzid.as_ref()
    }

    pub fn effective_end_tzid(&self) -> Option<&RadrootsIanaTimeZoneId> {
        self.end_tzid.as_ref().or(self.start_tzid.as_ref())
    }

    pub fn locations(&self) -> &[String] {
        &self.locations
    }

    pub fn geohash(&self) -> Option<&str> {
        self.geohash.as_deref()
    }

    pub fn summary(&self) -> Option<&str> {
        self.summary.as_deref()
    }

    pub fn image(&self) -> Option<&RadrootsAuthoredImage> {
        self.image.as_ref()
    }

    pub fn participants(&self) -> Option<&Vec<RadrootsCalendarParticipant>> {
        self.participants.as_ref()
    }

    pub fn categories(&self) -> &[String] {
        &self.categories
    }

    pub fn references(&self) -> &[RadrootsCalendarUri] {
        &self.references
    }

    pub fn calendar_requests(&self) -> &[RadrootsCalendarRequest] {
        &self.calendar_requests
    }

    fn validate_budget(&self) -> Result<(), RadrootsCalendarEventError> {
        validate_authored_calendar_budget(self.description(), authored_time_tags_for_budget(self)?)
    }
}

#[cfg_attr(any(feature = "serde", test), derive(serde::Serialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsParsedNip52CalendarCommon {
    d_tag: String,
    title: String,
    description: Option<String>,
    locations: Vec<String>,
    geohash: Option<String>,
    summary: Option<String>,
    image: Option<RadrootsCalendarUri>,
    participants: Vec<RadrootsCalendarParticipant>,
    categories: Vec<String>,
    references: Vec<RadrootsCalendarUri>,
    calendar_requests: Vec<RadrootsCalendarRequest>,
    legacy_name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsParsedNip52CalendarCommonParts {
    pub d_tag: String,
    pub title: String,
    pub description: Option<String>,
    pub locations: Vec<String>,
    pub geohash: Option<String>,
    pub summary: Option<String>,
    pub image: Option<RadrootsCalendarUri>,
    pub participants: Vec<RadrootsCalendarParticipant>,
    pub categories: Vec<String>,
    pub references: Vec<RadrootsCalendarUri>,
    pub calendar_requests: Vec<RadrootsCalendarRequest>,
    pub legacy_name: Option<String>,
}

impl RadrootsParsedNip52CalendarCommon {
    pub fn try_new(
        parts: RadrootsParsedNip52CalendarCommonParts,
    ) -> Result<Self, RadrootsCalendarEventError> {
        validate_calendar_tag_text(&parts.d_tag, "d")?;
        validate_calendar_tag_text(&parts.title, "title")?;
        if let Some(description) = parts.description.as_deref() {
            validate_calendar_content(description)?;
        }
        for location in &parts.locations {
            validate_calendar_tag_text(location, "location")?;
        }
        if let Some(geohash) = parts.geohash.as_deref()
            && !calendar_geohash_is_valid(geohash)
        {
            return Err(RadrootsCalendarEventError::InvalidGeohash);
        }
        if let Some(summary) = parts.summary.as_deref() {
            validate_calendar_tag_text(summary, "summary")?;
        }
        validate_inbound_calendar_participants(&parts.participants)?;
        for category in &parts.categories {
            validate_calendar_tag_text(category, "category")?;
        }
        if let Some(legacy_name) = parts.legacy_name.as_deref() {
            validate_calendar_tag_text(legacy_name, "legacy name")?;
        }
        Ok(Self {
            d_tag: parts.d_tag,
            title: parts.title,
            description: parts.description,
            locations: parts.locations,
            geohash: parts.geohash,
            summary: parts.summary,
            image: parts.image,
            participants: parts.participants,
            categories: parts.categories,
            references: parts.references,
            calendar_requests: parts.calendar_requests,
            legacy_name: parts.legacy_name,
        })
    }

    pub fn d_tag(&self) -> &str {
        &self.d_tag
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    pub fn locations(&self) -> &[String] {
        &self.locations
    }

    pub fn geohash(&self) -> Option<&str> {
        self.geohash.as_deref()
    }

    pub fn summary(&self) -> Option<&str> {
        self.summary.as_deref()
    }

    pub fn image(&self) -> Option<&RadrootsCalendarUri> {
        self.image.as_ref()
    }

    pub fn participants(&self) -> &[RadrootsCalendarParticipant] {
        &self.participants
    }

    pub fn categories(&self) -> &[String] {
        &self.categories
    }

    pub fn references(&self) -> &[RadrootsCalendarUri] {
        &self.references
    }

    pub fn calendar_requests(&self) -> &[RadrootsCalendarRequest] {
        &self.calendar_requests
    }

    pub fn legacy_name(&self) -> Option<&str> {
        self.legacy_name.as_deref()
    }
}

#[cfg_attr(any(feature = "serde", test), derive(serde::Serialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsParsedNip52CalendarDateEvent {
    common: RadrootsParsedNip52CalendarCommon,
    start: RadrootsCalendarDate,
    end: Option<RadrootsCalendarDate>,
    extension_day_tags: Vec<Vec<String>>,
}

impl RadrootsParsedNip52CalendarDateEvent {
    pub fn try_new(
        common: RadrootsParsedNip52CalendarCommon,
        start: RadrootsCalendarDate,
        end: Option<RadrootsCalendarDate>,
        extension_day_tags: Vec<Vec<String>>,
    ) -> Result<Self, RadrootsCalendarEventError> {
        if end.as_ref().is_some_and(|end| end <= &start) {
            return Err(RadrootsCalendarEventError::InvalidRange);
        }
        Ok(Self {
            common,
            start,
            end,
            extension_day_tags,
        })
    }

    pub fn common(&self) -> &RadrootsParsedNip52CalendarCommon {
        &self.common
    }

    pub fn start(&self) -> &RadrootsCalendarDate {
        &self.start
    }

    pub fn end(&self) -> Option<&RadrootsCalendarDate> {
        self.end.as_ref()
    }

    pub fn extension_day_tags(&self) -> &[Vec<String>] {
        &self.extension_day_tags
    }
}

#[cfg_attr(any(feature = "serde", test), derive(serde::Serialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsObservedUtcDay {
    wire_value: String,
    index: u64,
}

impl RadrootsObservedUtcDay {
    pub fn parse(value: impl Into<String>) -> Result<Self, RadrootsCalendarEventError> {
        let wire_value = value.into();
        if wire_value.is_empty() || !wire_value.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(RadrootsCalendarEventError::InvalidText("D"));
        }
        let index = wire_value
            .parse()
            .map_err(|_| RadrootsCalendarEventError::InvalidText("D"))?;
        Ok(Self { wire_value, index })
    }

    pub fn wire_value(&self) -> &str {
        &self.wire_value
    }

    pub const fn index(&self) -> u64 {
        self.index
    }

    pub fn is_canonical(&self) -> bool {
        self.wire_value == self.index.to_string()
    }
}

#[cfg_attr(any(feature = "serde", test), derive(serde::Serialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsParsedNip52CalendarTimeEvent {
    common: RadrootsParsedNip52CalendarCommon,
    start_wire: String,
    start: u64,
    end_wire: Option<String>,
    end: Option<u64>,
    observed_day_indices: Vec<RadrootsObservedUtcDay>,
    start_tzid: Option<RadrootsIanaTimeZoneId>,
    end_tzid: Option<RadrootsIanaTimeZoneId>,
}

impl RadrootsParsedNip52CalendarTimeEvent {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        common: RadrootsParsedNip52CalendarCommon,
        start_wire: String,
        start: u64,
        end_wire: Option<String>,
        end: Option<u64>,
        observed_day_indices: Vec<RadrootsObservedUtcDay>,
        start_tzid: Option<RadrootsIanaTimeZoneId>,
        end_tzid: Option<RadrootsIanaTimeZoneId>,
    ) -> Result<Self, RadrootsCalendarEventError> {
        if parse_calendar_decimal(&start_wire) != Some(start)
            || end_wire
                .as_deref()
                .zip(end)
                .is_some_and(|(wire, end)| parse_calendar_decimal(wire) != Some(end))
            || end_wire.is_some() != end.is_some()
        {
            return Err(RadrootsCalendarEventError::InvalidRange);
        }
        if end.is_some_and(|end| end <= start) {
            return Err(RadrootsCalendarEventError::InvalidRange);
        }
        let first = start / RADROOTS_CALENDAR_SECONDS_PER_DAY;
        let last = end
            .map(|end| (end - 1) / RADROOTS_CALENDAR_SECONDS_PER_DAY)
            .unwrap_or(first);
        if observed_day_indices.is_empty()
            || observed_day_indices
                .iter()
                .any(|day| !(first..=last).contains(&day.index))
        {
            return Err(RadrootsCalendarEventError::InvalidRange);
        }
        Ok(Self {
            common,
            start_wire,
            start,
            end_wire,
            end,
            observed_day_indices,
            start_tzid,
            end_tzid,
        })
    }

    pub fn common(&self) -> &RadrootsParsedNip52CalendarCommon {
        &self.common
    }

    pub fn start_wire(&self) -> &str {
        &self.start_wire
    }

    pub const fn start(&self) -> u64 {
        self.start
    }

    pub fn end_wire(&self) -> Option<&str> {
        self.end_wire.as_deref()
    }

    pub const fn end(&self) -> Option<u64> {
        self.end
    }

    pub fn observed_day_indices(&self) -> &[RadrootsObservedUtcDay] {
        &self.observed_day_indices
    }

    pub fn start_tzid(&self) -> Option<&RadrootsIanaTimeZoneId> {
        self.start_tzid.as_ref()
    }

    pub fn end_tzid(&self) -> Option<&RadrootsIanaTimeZoneId> {
        self.end_tzid.as_ref()
    }

    pub fn effective_end_tzid(&self) -> Option<&RadrootsIanaTimeZoneId> {
        self.end_tzid.as_ref().or(self.start_tzid.as_ref())
    }
}

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsCalendarAdmissionError {
    NonCanonicalField(&'static str),
    ForbiddenDateDayIndex,
    IncompleteDayCoverage,
    CoveredDayLimitExceeded { max: u64, actual: u64 },
    NonBlossomImage,
}

impl RadrootsCalendarAdmissionError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::NonCanonicalField(_) => "non_canonical_field",
            Self::ForbiddenDateDayIndex => "forbidden_date_day_index",
            Self::IncompleteDayCoverage => "incomplete_day_coverage",
            Self::CoveredDayLimitExceeded { .. } => "covered_day_limit_exceeded",
            Self::NonBlossomImage => "non_blossom_image",
        }
    }
}

impl fmt::Display for RadrootsCalendarAdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonCanonicalField(field) => write!(f, "calendar {field} is not canonical"),
            Self::ForbiddenDateDayIndex => {
                f.write_str("calendar date event carries a forbidden uppercase-D extension")
            }
            Self::IncompleteDayCoverage => {
                f.write_str("calendar time event lacks exact ordered UTC-day coverage")
            }
            Self::CoveredDayLimitExceeded { max, actual } => write!(
                f,
                "calendar interval covers {actual} UTC days, exceeding the {max}-day admission limit"
            ),
            Self::NonBlossomImage => {
                f.write_str("calendar image is not a structural Blossom hash-path URL")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsCalendarAdmissionError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsAdmittedCalendarDateEvent {
    parsed: RadrootsParsedNip52CalendarDateEvent,
    d_tag: RadrootsDTag,
    blossom_image: Option<RadrootsBlossomBlobUrl>,
}

impl RadrootsAdmittedCalendarDateEvent {
    pub fn try_from_parsed(
        parsed: RadrootsParsedNip52CalendarDateEvent,
    ) -> Result<Self, RadrootsCalendarAdmissionError> {
        if !parsed.extension_day_tags.is_empty() {
            return Err(RadrootsCalendarAdmissionError::ForbiddenDateDayIndex);
        }
        validate_admitted_calendar_common(&parsed.common)?;
        let d_tag = admitted_d_tag(&parsed.common)?;
        let blossom_image = admitted_blossom_image(&parsed.common)?;
        Ok(Self {
            parsed,
            d_tag,
            blossom_image,
        })
    }

    pub fn parsed(&self) -> &RadrootsParsedNip52CalendarDateEvent {
        &self.parsed
    }

    pub fn d_tag(&self) -> &RadrootsDTag {
        &self.d_tag
    }

    pub fn blossom_image(&self) -> Option<&RadrootsBlossomBlobUrl> {
        self.blossom_image.as_ref()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsAdmittedCalendarTimeEvent {
    parsed: RadrootsParsedNip52CalendarTimeEvent,
    d_tag: RadrootsDTag,
    covered_utc_days: Vec<u64>,
    blossom_image: Option<RadrootsBlossomBlobUrl>,
}

impl RadrootsAdmittedCalendarTimeEvent {
    pub fn try_from_parsed(
        parsed: RadrootsParsedNip52CalendarTimeEvent,
    ) -> Result<Self, RadrootsCalendarAdmissionError> {
        validate_admitted_calendar_common(&parsed.common)?;
        let d_tag = admitted_d_tag(&parsed.common)?;
        if parsed.start_wire != parsed.start.to_string()
            || parsed
                .end_wire
                .as_deref()
                .zip(parsed.end)
                .is_some_and(|(wire, end)| wire != end.to_string())
        {
            return Err(RadrootsCalendarAdmissionError::NonCanonicalField(
                "timestamp",
            ));
        }
        let expected = covered_utc_days(parsed.start, parsed.end).map_err(|error| match error {
            RadrootsCalendarEventError::CoveredDayLimitExceeded { max, actual } => {
                RadrootsCalendarAdmissionError::CoveredDayLimitExceeded { max, actual }
            }
            _ => RadrootsCalendarAdmissionError::IncompleteDayCoverage,
        })?;
        let covered_utc_days = expected.clone().collect::<Vec<_>>();
        if parsed.observed_day_indices.len() != covered_utc_days.len()
            || parsed
                .observed_day_indices
                .iter()
                .zip(&covered_utc_days)
                .any(|(observed, expected)| !observed.is_canonical() || observed.index != *expected)
        {
            return Err(RadrootsCalendarAdmissionError::IncompleteDayCoverage);
        }
        let blossom_image = admitted_blossom_image(&parsed.common)?;
        Ok(Self {
            parsed,
            d_tag,
            covered_utc_days,
            blossom_image,
        })
    }

    pub fn parsed(&self) -> &RadrootsParsedNip52CalendarTimeEvent {
        &self.parsed
    }

    pub fn d_tag(&self) -> &RadrootsDTag {
        &self.d_tag
    }

    pub fn covered_utc_days(&self) -> &[u64] {
        &self.covered_utc_days
    }

    pub fn blossom_image(&self) -> Option<&RadrootsBlossomBlobUrl> {
        self.blossom_image.as_ref()
    }
}

pub fn covered_utc_days(
    start: u64,
    end: Option<u64>,
) -> Result<RangeInclusive<u64>, RadrootsCalendarEventError> {
    if end.is_some_and(|end| end <= start) {
        return Err(RadrootsCalendarEventError::InvalidRange);
    }
    let first = start / RADROOTS_CALENDAR_SECONDS_PER_DAY;
    let last = end
        .map(|end| (end - 1) / RADROOTS_CALENDAR_SECONDS_PER_DAY)
        .unwrap_or(first);
    let actual = last - first + 1;
    if actual > RADROOTS_CALENDAR_MAX_COVERED_UTC_DAYS {
        return Err(RadrootsCalendarEventError::CoveredDayLimitExceeded {
            max: RADROOTS_CALENDAR_MAX_COVERED_UTC_DAYS,
            actual,
        });
    }
    Ok(first..=last)
}

fn validated_title(value: String) -> Result<String, RadrootsCalendarEventError> {
    if !canonical_calendar_tag_text_is_valid(&value) {
        return Err(RadrootsCalendarEventError::InvalidTitle);
    }
    Ok(value)
}

fn parse_calendar_decimal(value: &str) -> Option<u64> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value.parse().ok()
}

pub fn calendar_tag_text_is_valid(value: &str) -> bool {
    !value.trim().is_empty()
        && !value.chars().any(char::is_control)
        && value.len() <= DEFAULT_TAG_ELEMENT_MAX_BYTES
}

pub fn canonical_calendar_tag_text_is_valid(value: &str) -> bool {
    calendar_tag_text_is_valid(value) && value.trim() == value
}

pub fn calendar_geohash_is_valid(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 12
        && value.bytes().all(|byte| {
            matches!(
                byte.to_ascii_lowercase(),
                b'0'..=b'9'
                    | b'b'
                    | b'c'
                    | b'd'
                    | b'e'
                    | b'f'
                    | b'g'
                    | b'h'
                    | b'j'
                    | b'k'
                    | b'm'
                    | b'n'
                    | b'p'
                    | b'q'
                    | b'r'
                    | b's'
                    | b't'
                    | b'u'
                    | b'v'
                    | b'w'
                    | b'x'
                    | b'y'
                    | b'z'
            )
        })
}

pub fn canonical_calendar_geohash_is_valid(value: &str) -> bool {
    calendar_geohash_is_valid(value) && value.bytes().all(|byte| !byte.is_ascii_uppercase())
}

pub fn calendar_relay_url_is_valid(value: &str) -> bool {
    if value.is_empty()
        || value
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
    {
        return false;
    }
    let Some((scheme, remainder)) = value.split_once("://") else {
        return false;
    };
    if !(scheme.eq_ignore_ascii_case("ws") || scheme.eq_ignore_ascii_case("wss")) {
        return false;
    }
    let Ok(parsed) = Url::parse(value) else {
        return false;
    };
    let authority = remainder.split(['/', '?', '#']).next().unwrap_or(remainder);
    matches!(parsed.scheme(), "ws" | "wss")
        && parsed.host_str().is_some_and(|host| !host.is_empty())
        && parsed.username().is_empty()
        && parsed.password().is_none()
        && parsed.fragment().is_none()
        && parsed.port() != Some(0)
        && !authority.contains('@')
}

fn validate_calendar_tag_text(
    value: &str,
    field: &'static str,
) -> Result<(), RadrootsCalendarEventError> {
    if !calendar_tag_text_is_valid(value) {
        return Err(if value.len() > DEFAULT_TAG_ELEMENT_MAX_BYTES {
            RadrootsCalendarEventError::TagElementTooLarge {
                field,
                max: DEFAULT_TAG_ELEMENT_MAX_BYTES,
                actual: value.len(),
            }
        } else {
            RadrootsCalendarEventError::InvalidText(field)
        });
    }
    Ok(())
}

fn validate_canonical_calendar_tag_text(
    value: &str,
    field: &'static str,
) -> Result<(), RadrootsCalendarEventError> {
    validate_calendar_tag_text(value, field)?;
    if value.trim() != value {
        return Err(RadrootsCalendarEventError::InvalidText(field));
    }
    Ok(())
}

fn validate_calendar_content(value: &str) -> Result<(), RadrootsCalendarEventError> {
    if value.len() > DEFAULT_CONTENT_MAX_BYTES {
        return Err(RadrootsCalendarEventError::ContentTooLarge {
            max: DEFAULT_CONTENT_MAX_BYTES,
            actual: value.len(),
        });
    }
    Ok(())
}

fn validate_authored_calendar_locations(
    locations: &[String],
) -> Result<(), RadrootsCalendarEventError> {
    for location in locations {
        validate_canonical_calendar_tag_text(location, "location")?;
    }
    Ok(())
}

fn validate_authored_calendar_requests(
    requests: &[RadrootsCalendarRequest],
) -> Result<(), RadrootsCalendarEventError> {
    if requests.iter().any(|request| !request.is_canonical()) {
        return Err(RadrootsCalendarEventError::InvalidUrl("calendar request"));
    }
    Ok(())
}

fn validate_inbound_calendar_participants(
    participants: &[RadrootsCalendarParticipant],
) -> Result<(), RadrootsCalendarEventError> {
    for (index, participant) in participants.iter().enumerate() {
        if RadrootsPublicKey::parse(&participant.pubkey).is_err()
            || participant
                .relay
                .as_deref()
                .is_some_and(|relay| !calendar_relay_url_is_valid(relay))
            || participant
                .role
                .as_deref()
                .is_some_and(|role| !calendar_tag_text_is_valid(role))
        {
            return Err(RadrootsCalendarEventError::InvalidParticipant { index });
        }
    }
    Ok(())
}

fn validate_authored_calendar_participants(
    participants: &[RadrootsCalendarParticipant],
) -> Result<(), RadrootsCalendarEventError> {
    if participants.len() > RADROOTS_CALENDAR_MAX_PARTICIPANTS {
        return Err(RadrootsCalendarEventError::TooManyParticipants {
            max: RADROOTS_CALENDAR_MAX_PARTICIPANTS,
            actual: participants.len(),
        });
    }
    validate_inbound_calendar_participants(participants)?;
    for (index, participant) in participants.iter().enumerate() {
        let pubkey = RadrootsPublicKey::parse(&participant.pubkey)
            .map_err(|_| RadrootsCalendarEventError::InvalidParticipant { index })?;
        if pubkey.as_str() != participant.pubkey
            || participant
                .relay
                .as_deref()
                .is_some_and(|relay| RadrootsRelayUrl::parse(relay).is_err())
            || participant
                .role
                .as_deref()
                .is_some_and(|role| !canonical_calendar_tag_text_is_valid(role))
        {
            return Err(RadrootsCalendarEventError::InvalidParticipant { index });
        }
    }
    Ok(())
}

fn validate_authored_calendar_categories(
    categories: &[String],
) -> Result<(), RadrootsCalendarEventError> {
    for category in categories {
        validate_canonical_calendar_tag_text(category, "category")?;
    }
    Ok(())
}

fn validate_admitted_calendar_common(
    common: &RadrootsParsedNip52CalendarCommon,
) -> Result<(), RadrootsCalendarAdmissionError> {
    let canonical = canonical_calendar_tag_text_is_valid(&common.title)
        && common
            .locations
            .iter()
            .all(|value| canonical_calendar_tag_text_is_valid(value))
        && common
            .summary
            .as_deref()
            .is_none_or(canonical_calendar_tag_text_is_valid)
        && common
            .geohash
            .as_deref()
            .is_none_or(canonical_calendar_geohash_is_valid)
        && common
            .categories
            .iter()
            .all(|value| canonical_calendar_tag_text_is_valid(value))
        && common
            .legacy_name
            .as_deref()
            .is_none_or(canonical_calendar_tag_text_is_valid)
        && common
            .calendar_requests
            .iter()
            .all(RadrootsCalendarRequest::is_canonical);
    if !canonical {
        return Err(RadrootsCalendarAdmissionError::NonCanonicalField(
            "metadata",
        ));
    }
    validate_authored_calendar_participants(&common.participants)
        .map_err(|_| RadrootsCalendarAdmissionError::NonCanonicalField("participant"))?;
    Ok(())
}

fn admitted_blossom_image(
    common: &RadrootsParsedNip52CalendarCommon,
) -> Result<Option<RadrootsBlossomBlobUrl>, RadrootsCalendarAdmissionError> {
    common
        .image
        .as_ref()
        .map(|image| {
            RadrootsBlossomBlobUrl::parse(image.as_str())
                .map_err(|_| RadrootsCalendarAdmissionError::NonBlossomImage)
        })
        .transpose()
}

fn admitted_d_tag(
    common: &RadrootsParsedNip52CalendarCommon,
) -> Result<RadrootsDTag, RadrootsCalendarAdmissionError> {
    let d_tag = RadrootsDTag::parse(&common.d_tag)
        .map_err(|_| RadrootsCalendarAdmissionError::NonCanonicalField("d"))?;
    if d_tag.as_str() != common.d_tag {
        return Err(RadrootsCalendarAdmissionError::NonCanonicalField("d"));
    }
    Ok(d_tag)
}

fn authored_date_tags_for_budget(event: &RadrootsAuthoredCalendarDateEvent) -> Vec<Vec<String>> {
    let mut tags = vec![
        budget_tag("d", event.d_tag().as_str()),
        budget_tag("title", event.title()),
        budget_tag("start", event.start().as_str()),
    ];
    if let Some(end) = event.end() {
        tags.push(budget_tag("end", end.as_str()));
    }
    append_authored_common_tags_for_budget(
        &mut tags,
        event.locations(),
        event.geohash(),
        event.summary(),
        event.image(),
        event.participants(),
        event.categories(),
        event.references(),
        event.calendar_requests(),
    );
    tags
}

fn authored_time_tags_for_budget(
    event: &RadrootsAuthoredCalendarTimeEvent,
) -> Result<Vec<Vec<String>>, RadrootsCalendarEventError> {
    let mut tags = vec![
        budget_tag("d", event.d_tag().as_str()),
        budget_tag("title", event.title()),
        budget_tag("start", &event.start().to_string()),
    ];
    if let Some(end) = event.end() {
        tags.push(budget_tag("end", &end.to_string()));
    }
    for day in covered_utc_days(event.start(), event.end())? {
        tags.push(budget_tag("D", &day.to_string()));
    }
    if let Some(tzid) = event.start_tzid() {
        tags.push(budget_tag("start_tzid", tzid.as_str()));
    }
    if let Some(tzid) = event.end_tzid() {
        tags.push(budget_tag("end_tzid", tzid.as_str()));
    }
    append_authored_common_tags_for_budget(
        &mut tags,
        event.locations(),
        event.geohash(),
        event.summary(),
        event.image(),
        event.participants(),
        event.categories(),
        event.references(),
        event.calendar_requests(),
    );
    Ok(tags)
}

#[allow(clippy::too_many_arguments)]
fn append_authored_common_tags_for_budget(
    tags: &mut Vec<Vec<String>>,
    locations: &[String],
    geohash: Option<&str>,
    summary: Option<&str>,
    image: Option<&RadrootsAuthoredImage>,
    participants: Option<&Vec<RadrootsCalendarParticipant>>,
    categories: &[String],
    references: &[RadrootsCalendarUri],
    calendar_requests: &[RadrootsCalendarRequest],
) {
    tags.extend(
        locations
            .iter()
            .map(|location| budget_tag("location", location)),
    );
    if let Some(geohash) = geohash {
        tags.push(budget_tag("g", geohash));
    }
    if let Some(summary) = summary {
        tags.push(budget_tag("summary", summary));
    }
    if let Some(image) = image {
        tags.push(budget_tag("image", image.descriptor().url().as_str()));
    }
    if let Some(participants) = participants {
        for participant in participants {
            let mut tag = vec!["p".to_string(), participant.pubkey.clone()];
            if let Some(role) = participant.role.as_ref() {
                tag.push(participant.relay.clone().unwrap_or_default());
                tag.push(role.clone());
            } else if let Some(relay) = participant.relay.as_ref() {
                tag.push(relay.clone());
            }
            tags.push(tag);
        }
    }
    tags.extend(categories.iter().map(|category| budget_tag("t", category)));
    tags.extend(
        references
            .iter()
            .map(|reference| budget_tag("r", reference.as_str())),
    );
    for request in calendar_requests {
        let mut tag = budget_tag("a", request.calendar().as_str());
        if let Some(relay) = request.relay() {
            tag.push(relay.to_string());
        }
        tags.push(tag);
    }
}

fn budget_tag(key: &str, value: &str) -> Vec<String> {
    vec![key.to_string(), value.to_string()]
}

fn validate_authored_calendar_budget(
    description: Option<&str>,
    tags: Vec<Vec<String>>,
) -> Result<(), RadrootsCalendarEventError> {
    if let Some(description) = description {
        validate_calendar_content(description)?;
    }
    if tags.len() > DEFAULT_TAG_MAX_COUNT {
        return Err(RadrootsCalendarEventError::TagCountExceeded {
            max: DEFAULT_TAG_MAX_COUNT,
            actual: tags.len(),
        });
    }
    let mut total = 0usize;
    for tag in &tags {
        for value in tag {
            if value.len() > DEFAULT_TAG_ELEMENT_MAX_BYTES {
                return Err(RadrootsCalendarEventError::TagElementTooLarge {
                    field: "tag",
                    max: DEFAULT_TAG_ELEMENT_MAX_BYTES,
                    actual: value.len(),
                });
            }
            total = total.saturating_add(value.len());
        }
    }
    if total > DEFAULT_TAG_TOTAL_MAX_BYTES {
        return Err(RadrootsCalendarEventError::TagBytesExceeded {
            max: DEFAULT_TAG_TOTAL_MAX_BYTES,
            actual: total,
        });
    }
    Ok(())
}

fn parse_date_digits(bytes: &[u8]) -> Result<u16, RadrootsCalendarEventError> {
    bytes.iter().try_fold(0_u16, |value, byte| {
        if byte.is_ascii_digit() {
            Ok(value * 10 + u16::from(byte - b'0'))
        } else {
            Err(RadrootsCalendarEventError::InvalidDate)
        }
    })
}

const fn days_in_month(year: u16, month: u16) -> u16 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

const fn is_leap_year(year: u16) -> bool {
    year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug)]
pub struct RadrootsCalendarEventRsvp {
    pub d_tag: String,
    pub event: RadrootsSocialTarget,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub event_id: Option<String>,
    pub status: RadrootsCalendarEventRsvpStatus,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub free_busy: Option<RadrootsCalendarEventFreeBusy>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub note: Option<String>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub participants: Option<Vec<RadrootsCalendarParticipant>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authored_date_event_validates_each_optional_field_at_construction() {
        let event = RadrootsAuthoredCalendarDateEvent::new(
            "market-day",
            "Market day",
            RadrootsCalendarDate::parse("2026-06-20").unwrap(),
        )
        .unwrap()
        .with_description("Farm stand pickup window.")
        .unwrap()
        .with_locations(vec!["Farm stand".to_string()])
        .unwrap()
        .with_geohash("c23nb62w20st")
        .unwrap()
        .with_summary("Weekly pickup")
        .unwrap()
        .with_categories(vec!["farmers-market".to_string(), "vegetables".to_string()])
        .unwrap();

        assert_eq!(event.d_tag().as_str(), "market-day");
        assert_eq!(event.start().as_str(), "2026-06-20");
        assert_eq!(event.summary(), Some("Weekly pickup"));
        assert_eq!(event.categories(), ["farmers-market", "vegetables"]);

        let error = RadrootsAuthoredCalendarDateEvent::new(
            "market-day",
            " Market day ",
            RadrootsCalendarDate::parse("2026-06-20").unwrap(),
        )
        .unwrap_err();
        assert_eq!(error, RadrootsCalendarEventError::InvalidTitle);

        let error = RadrootsAuthoredCalendarDateEvent::new(
            "market-day",
            "Market day",
            RadrootsCalendarDate::parse("2026-06-20").unwrap(),
        )
        .unwrap()
        .with_geohash("C23NB62W20ST")
        .unwrap_err();
        assert_eq!(error, RadrootsCalendarEventError::InvalidGeohash);
    }

    #[test]
    fn authored_time_event_uses_exact_iana_ids_and_end_timezone_fallback() {
        assert_eq!(jiff_tzdb::VERSION, Some("2026c"));

        let event =
            RadrootsAuthoredCalendarTimeEvent::new("wash-pack", "Wash pack shift", 1_781_895_600)
                .unwrap()
                .with_end(1_781_899_200)
                .unwrap()
                .with_description("Pack CSA shares before pickup.")
                .unwrap()
                .with_start_tzid("America/Vancouver")
                .unwrap()
                .with_participants(vec![RadrootsCalendarParticipant {
                    pubkey: "a".repeat(64),
                    relay: None,
                    role: Some("host".to_string()),
                }])
                .unwrap();

        assert_eq!(event.start(), 1_781_895_600);
        assert_eq!(event.end(), Some(1_781_899_200));
        assert_eq!(
            event.start_tzid().map(RadrootsIanaTimeZoneId::as_str),
            Some("America/Vancouver")
        );
        assert_eq!(event.end_tzid(), None);
        assert_eq!(
            event
                .effective_end_tzid()
                .map(RadrootsIanaTimeZoneId::as_str),
            Some("America/Vancouver")
        );
        assert_eq!(event.participants().map(Vec::len), Some(1));

        for invalid in ["america/vancouver", "America/Imaginary", " UTC ", ""] {
            assert_eq!(
                RadrootsIanaTimeZoneId::parse(invalid),
                Err(RadrootsCalendarEventError::InvalidTimeZone),
                "{invalid}"
            );
        }
    }

    #[test]
    fn calendar_dates_validate_gregorian_semantics_and_strict_ranges() {
        for valid in ["0001-01-01", "2000-02-29", "2024-02-29", "9999-12-31"] {
            assert_eq!(RadrootsCalendarDate::parse(valid).unwrap().as_str(), valid);
        }
        for invalid in [
            "0000-01-01",
            "1900-02-29",
            "2026-02-29",
            "2026-02-30",
            "2026-13-01",
            "2026-00-01",
            "2026-06-00",
            "2026-6-20",
            "+2026-06-20",
        ] {
            assert_eq!(
                RadrootsCalendarDate::parse(invalid),
                Err(RadrootsCalendarEventError::InvalidDate),
                "{invalid}"
            );
        }

        let start = RadrootsCalendarDate::parse("2026-06-20").unwrap();
        let event =
            RadrootsAuthoredCalendarDateEvent::new("market-day", "Market", start.clone()).unwrap();
        assert_eq!(
            event
                .clone()
                .with_end(RadrootsCalendarDate::parse("2026-06-20").unwrap()),
            Err(RadrootsCalendarEventError::InvalidRange)
        );
        assert_eq!(
            event.with_end(RadrootsCalendarDate::parse("2026-06-19").unwrap()),
            Err(RadrootsCalendarEventError::InvalidRange)
        );
    }

    #[test]
    fn covered_utc_days_are_exclusive_end_and_bounded() {
        assert_eq!(
            covered_utc_days(86_399, None).unwrap().collect::<Vec<_>>(),
            vec![0]
        );
        assert_eq!(
            covered_utc_days(86_399, Some(86_401))
                .unwrap()
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
        assert_eq!(
            covered_utc_days(0, Some(86_400))
                .unwrap()
                .collect::<Vec<_>>(),
            vec![0]
        );
        assert_eq!(
            covered_utc_days(10, Some(10)),
            Err(RadrootsCalendarEventError::InvalidRange)
        );
        assert_eq!(
            covered_utc_days(10, Some(9)),
            Err(RadrootsCalendarEventError::InvalidRange)
        );
        assert_eq!(
            covered_utc_days(0, Some(367 * RADROOTS_CALENDAR_SECONDS_PER_DAY)),
            Err(RadrootsCalendarEventError::CoveredDayLimitExceeded {
                max: RADROOTS_CALENDAR_MAX_COVERED_UTC_DAYS,
                actual: 367,
            })
        );
    }

    #[test]
    fn baseline_wire_types_preserve_noncanonical_but_structural_values() {
        let uri = RadrootsCalendarUri::parse("ipfs://bafybeigdyrzt/calendar.webp").unwrap();
        assert_eq!(uri.as_str(), "ipfs://bafybeigdyrzt/calendar.webp");

        let observed = RadrootsObservedUtcDay::parse("00042").unwrap();
        assert_eq!(observed.wire_value(), "00042");
        assert_eq!(observed.index(), 42);
        assert!(!observed.is_canonical());

        assert_eq!(
            RadrootsCalendarUri::parse("not an absolute URI"),
            Err(RadrootsCalendarEventError::InvalidUrl("URI"))
        );
        assert_eq!(
            RadrootsObservedUtcDay::parse("+42"),
            Err(RadrootsCalendarEventError::InvalidText("D"))
        );
    }

    #[test]
    fn authored_models_enforce_content_and_participant_limits() {
        let event = RadrootsAuthoredCalendarDateEvent::new(
            "market-day",
            "Market day",
            RadrootsCalendarDate::parse("2026-06-20").unwrap(),
        )
        .unwrap();
        let oversized = "x".repeat(DEFAULT_CONTENT_MAX_BYTES + 1);
        assert_eq!(
            event.clone().with_description(oversized),
            Err(RadrootsCalendarEventError::ContentTooLarge {
                max: DEFAULT_CONTENT_MAX_BYTES,
                actual: DEFAULT_CONTENT_MAX_BYTES + 1,
            })
        );

        let participant = RadrootsCalendarParticipant {
            pubkey: "a".repeat(64),
            relay: None,
            role: None,
        };
        assert_eq!(
            event.with_participants(vec![participant; RADROOTS_CALENDAR_MAX_PARTICIPANTS + 1]),
            Err(RadrootsCalendarEventError::TooManyParticipants {
                max: RADROOTS_CALENDAR_MAX_PARTICIPANTS,
                actual: RADROOTS_CALENDAR_MAX_PARTICIPANTS + 1,
            })
        );
    }

    #[test]
    fn calendar_collection_represents_event_address_refs() {
        let calendar = RadrootsCalendar {
            d_tag: "farm-calendar".to_string(),
            title: "farm calendar".to_string(),
            events: vec![RadrootsSocialTarget::Address {
                address: "31923:pubkey:wash-pack".to_string(),
                author: None,
                event_kind: Some(31923),
                relays: None,
            }],
            description: Some("Shared farm operations schedule.".to_string()),
            summary: None,
            image: None,
        };

        assert_eq!(calendar.d_tag, "farm-calendar");
        assert_eq!(calendar.events.len(), 1);
        assert!(matches!(
            calendar.events[0],
            RadrootsSocialTarget::Address { .. }
        ));
    }

    #[test]
    fn rsvp_represents_status_and_free_busy_state() {
        let rsvp = RadrootsCalendarEventRsvp {
            d_tag: "rsvp-1".to_string(),
            event: RadrootsSocialTarget::Address {
                address: "31923:pubkey:wash-pack".to_string(),
                author: Some("a".repeat(64)),
                event_kind: Some(31923),
                relays: None,
            },
            event_id: Some("b".repeat(64)),
            status: RadrootsCalendarEventRsvpStatus::Tentative,
            free_busy: Some(RadrootsCalendarEventFreeBusy::Busy),
            note: Some("depends on harvest".to_string()),
            participants: None,
        };

        assert_eq!(rsvp.status, RadrootsCalendarEventRsvpStatus::Tentative);
        assert_eq!(
            rsvp.event_id.as_deref(),
            Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
        );
        assert_eq!(rsvp.free_busy, Some(RadrootsCalendarEventFreeBusy::Busy));
    }
}
