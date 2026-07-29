#[cfg(not(feature = "std"))]
use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use core::{fmt, ops::RangeInclusive, str::FromStr};

use crate::id::{
    AddressableCoordinate, AddressableCoordinateParts, DTag, EventId, RelayUrl, parse_public_key,
};
use crate::media::AuthoredImage;
use crate::wire::v1::{
    DEFAULT_CONTENT_MAX_BYTES, DEFAULT_TAG_ELEMENT_MAX_BYTES, DEFAULT_TAG_MAX_COUNT,
    DEFAULT_TAG_TOTAL_MAX_BYTES,
};
use radroots_blossom::BlobUrl;
use radroots_identity::PublicKey;
use url_nostd::Url;

pub const RADROOTS_CALENDAR_SECONDS_PER_DAY: u64 = 86_400;
pub const RADROOTS_CALENDAR_MAX_COVERED_UTC_DAYS: u64 = 366;
pub const RADROOTS_CALENDAR_MAX_PARTICIPANTS: usize = DEFAULT_TAG_MAX_COUNT - 16;

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CalendarParticipant {
    pub pubkey: String,
    pub relay: Option<String>,
    pub role: Option<String>,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(any(feature = "serde", test), serde(rename_all = "snake_case"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CalendarEventRsvpStatus {
    Accepted,
    Declined,
    Tentative,
}

#[cfg_attr(
    any(feature = "serde", test),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(any(feature = "serde", test), serde(rename_all = "snake_case"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CalendarEventFreeBusy {
    Free,
    Busy,
}

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CalendarEventError {
    InvalidIdentifier,
    InvalidCalendarUid,
    InvalidEventReference,
    InvalidRevisionReference,
    InvalidAuthorReference,
    DuplicateEventReference,
    AuthorHintMismatch,
    DeclinedFreeBusyForbidden,
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

impl CalendarEventError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidIdentifier => "invalid_identifier",
            Self::InvalidCalendarUid => "invalid_calendar_uid",
            Self::InvalidEventReference => "invalid_event_reference",
            Self::InvalidRevisionReference => "invalid_revision_reference",
            Self::InvalidAuthorReference => "invalid_author_reference",
            Self::DuplicateEventReference => "duplicate_event_reference",
            Self::AuthorHintMismatch => "author_hint_mismatch",
            Self::DeclinedFreeBusyForbidden => "declined_free_busy_forbidden",
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

impl fmt::Display for CalendarEventError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIdentifier => f.write_str("calendar identifier is invalid"),
            Self::InvalidCalendarUid => f.write_str(
                "calendar UID must be an unpadded 22-character base64url encoding of 128 bits",
            ),
            Self::InvalidEventReference => f.write_str("calendar event reference is invalid"),
            Self::InvalidRevisionReference => {
                f.write_str("calendar event revision reference is invalid")
            }
            Self::InvalidAuthorReference => {
                f.write_str("calendar event author reference is invalid")
            }
            Self::DuplicateEventReference => {
                f.write_str("calendar event references must have unique coordinates")
            }
            Self::AuthorHintMismatch => {
                f.write_str("calendar event author hint does not match the event coordinate")
            }
            Self::DeclinedFreeBusyForbidden => {
                f.write_str("an authored declined RSVP cannot carry free/busy state")
            }
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
impl std::error::Error for CalendarEventError {}

/// A syntax-valid 128-bit calendar identifier encoded as unpadded base64url.
///
/// This type validates the identifier's entropy shape, not uniqueness. The
/// authoring runtime must still generate a fresh value for every calendar or
/// RSVP identity it creates.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CalendarUid(String);

impl CalendarUid {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, CalendarEventError> {
        let value = value.as_ref();
        let valid_alphabet = value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));
        let valid_final_quantum = value
            .as_bytes()
            .last()
            .is_some_and(|byte| matches!(byte, b'A' | b'Q' | b'g' | b'w'));
        if value.len() != 22 || !valid_alphabet || !valid_final_quantum {
            return Err(CalendarEventError::InvalidCalendarUid);
        }
        Ok(Self(value.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for CalendarUid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl AsRef<str> for CalendarUid {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl FromStr for CalendarUid {
    type Err = CalendarEventError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl TryFrom<&str> for CalendarUid {
    type Error = CalendarEventError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl TryFrom<String> for CalendarUid {
    type Error = CalendarEventError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

#[cfg(any(feature = "serde", test))]
impl serde::Serialize for CalendarUid {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

/// A raw-preserving NIP-52 address reference to a kind-31922 or kind-31923 event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CalendarEventReference {
    coordinate: AddressableCoordinate,
    kind: u32,
    author: PublicKey,
    d_tag: DTag,
    relay: Option<String>,
}

impl CalendarEventReference {
    pub fn parse(
        coordinate: impl AsRef<str>,
        relay: Option<&str>,
    ) -> Result<Self, CalendarEventError> {
        let coordinate = AddressableCoordinate::parse(coordinate.as_ref())
            .map_err(|_| CalendarEventError::InvalidEventReference)?;
        let parts = AddressableCoordinateParts::parse(coordinate.as_str())
            .map_err(|_| CalendarEventError::InvalidEventReference)?;
        if !matches!(
            parts.kind,
            crate::envelope::kind::KIND_CALENDAR_DATE_EVENT
                | crate::envelope::kind::KIND_CALENDAR_TIME_EVENT
        ) {
            return Err(CalendarEventError::InvalidEventReference);
        }
        let relay =
            parse_calendar_reference_relay(relay, CalendarEventError::InvalidEventReference)?;
        Ok(Self {
            coordinate,
            kind: parts.kind,
            author: parts.pubkey,
            d_tag: parts.d_tag,
            relay,
        })
    }

    pub fn coordinate(&self) -> &AddressableCoordinate {
        &self.coordinate
    }

    pub const fn kind(&self) -> u32 {
        self.kind
    }

    pub fn author(&self) -> &PublicKey {
        &self.author
    }

    pub fn d_tag(&self) -> &DTag {
        &self.d_tag
    }

    pub fn relay(&self) -> Option<&str> {
        self.relay.as_deref()
    }

    pub fn is_canonical(&self) -> bool {
        // Nostr does not define relay URL normalization. Strict admission validates the
        // lowercase ws/wss syntax while preserving the caller's host, port, path, and query.
        self.coordinate.as_str() == format!("{}:{}:{}", self.kind, self.author, self.d_tag)
            && self
                .relay()
                .is_none_or(|relay| RelayUrl::parse(relay).is_ok())
    }

    fn has_same_coordinate(&self, other: &Self) -> bool {
        self.kind == other.kind && self.author == other.author && self.d_tag == other.d_tag
    }
}

/// A raw-preserving event-id reference to a specific NIP-52 event revision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CalendarEventRevisionReference {
    raw_event_id: String,
    event_id: EventId,
    relay: Option<String>,
}

impl CalendarEventRevisionReference {
    pub fn parse(
        event_id: impl AsRef<str>,
        relay: Option<&str>,
    ) -> Result<Self, CalendarEventError> {
        let raw_event_id = event_id.as_ref();
        let event_id = EventId::parse(raw_event_id)
            .map_err(|_| CalendarEventError::InvalidRevisionReference)?;
        let relay =
            parse_calendar_reference_relay(relay, CalendarEventError::InvalidRevisionReference)?;
        Ok(Self {
            raw_event_id: raw_event_id.to_string(),
            event_id,
            relay,
        })
    }

    pub fn raw_event_id(&self) -> &str {
        &self.raw_event_id
    }

    pub fn event_id(&self) -> &EventId {
        &self.event_id
    }

    pub fn relay(&self) -> Option<&str> {
        self.relay.as_deref()
    }

    pub fn is_canonical(&self) -> bool {
        // Relay hints use strict Radroots syntax; their raw URL spelling is not normalized.
        self.raw_event_id == self.event_id.to_hex()
            && self
                .relay()
                .is_none_or(|relay| RelayUrl::parse(relay).is_ok())
    }
}

/// A raw-preserving optional RSVP author hint. This reference has no role field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CalendarEventAuthorReference {
    raw_pubkey: String,
    pubkey: PublicKey,
    relay: Option<String>,
}

impl CalendarEventAuthorReference {
    pub fn parse(pubkey: impl AsRef<str>, relay: Option<&str>) -> Result<Self, CalendarEventError> {
        let raw_pubkey = pubkey.as_ref();
        let pubkey =
            parse_public_key(raw_pubkey).map_err(|_| CalendarEventError::InvalidAuthorReference)?;
        let relay =
            parse_calendar_reference_relay(relay, CalendarEventError::InvalidAuthorReference)?;
        Ok(Self {
            raw_pubkey: raw_pubkey.to_string(),
            pubkey,
            relay,
        })
    }

    pub fn raw_pubkey(&self) -> &str {
        &self.raw_pubkey
    }

    pub fn pubkey(&self) -> &PublicKey {
        &self.pubkey
    }

    pub fn relay(&self) -> Option<&str> {
        self.relay.as_deref()
    }

    pub fn is_canonical(&self) -> bool {
        // Relay hints use strict Radroots syntax; their raw URL spelling is not normalized.
        self.raw_pubkey == self.pubkey.to_hex()
            && self
                .relay()
                .is_none_or(|relay| RelayUrl::parse(relay).is_ok())
    }
}

/// A canonical IANA Time Zone Database identifier.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IanaTimeZoneId(String);

impl IanaTimeZoneId {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, CalendarEventError> {
        let value = value.as_ref();
        let Some((canonical, _)) = jiff_tzdb::get(value) else {
            return Err(CalendarEventError::InvalidTimeZone);
        };
        if canonical != value || !canonical_calendar_tag_text_is_valid(value) {
            return Err(CalendarEventError::InvalidTimeZone);
        }
        Ok(Self(value.into()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for IanaTimeZoneId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl AsRef<str> for IanaTimeZoneId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl FromStr for IanaTimeZoneId {
    type Err = CalendarEventError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

#[cfg(any(feature = "serde", test))]
impl serde::Serialize for IanaTimeZoneId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

#[cfg(any(feature = "serde", test))]
impl<'de> serde::Deserialize<'de> for IanaTimeZoneId {
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
pub struct CalendarUri(String);

impl CalendarUri {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, CalendarEventError> {
        let value = value.as_ref();
        if value.trim() != value
            || value
                .chars()
                .any(|character| character.is_whitespace() || character.is_control())
            || value.len() > DEFAULT_TAG_ELEMENT_MAX_BYTES
            || Url::parse(value).is_err()
        {
            return Err(CalendarEventError::InvalidUrl("URI"));
        }
        Ok(Self(value.into()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for CalendarUri {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl FromStr for CalendarUri {
    type Err = CalendarEventError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl fmt::Display for CalendarUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(any(feature = "serde", test))]
impl serde::Serialize for CalendarUri {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

#[cfg(any(feature = "serde", test))]
impl<'de> serde::Deserialize<'de> for CalendarUri {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = <String as serde::Deserialize>::deserialize(deserializer)?;
        Self::parse(value).map_err(serde::de::Error::custom)
    }
}

/// Strict authored representation of a NIP-52 kind-31924 calendar collection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthoredCalendar {
    uid: CalendarUid,
    title: String,
    content: String,
    event_references: Vec<CalendarEventReference>,
    list_description: Option<String>,
    image: Option<AuthoredImage>,
}

impl AuthoredCalendar {
    pub fn new(
        uid: CalendarUid,
        title: impl Into<String>,
        content: impl Into<String>,
        event_references: Vec<CalendarEventReference>,
    ) -> Result<Self, CalendarEventError> {
        let title = validated_title(title.into())?;
        let content = content.into();
        validate_calendar_content(&content)?;
        validate_strict_calendar_event_references(&event_references)?;
        let authored = Self {
            uid,
            title,
            content,
            event_references,
            list_description: None,
            image: None,
        };
        authored.validate_budget()?;
        Ok(authored)
    }

    pub fn with_list_description(
        mut self,
        value: impl Into<String>,
    ) -> Result<Self, CalendarEventError> {
        let value = value.into();
        validate_canonical_calendar_tag_text(&value, "description")?;
        self.list_description = Some(value);
        self.validate_budget()?;
        Ok(self)
    }

    pub fn with_image(mut self, image: AuthoredImage) -> Result<Self, CalendarEventError> {
        self.image = Some(image);
        self.validate_budget()?;
        Ok(self)
    }

    pub fn uid(&self) -> &CalendarUid {
        &self.uid
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn event_references(&self) -> &[CalendarEventReference] {
        &self.event_references
    }

    pub fn list_description(&self) -> Option<&str> {
        self.list_description.as_deref()
    }

    pub fn image(&self) -> Option<&AuthoredImage> {
        self.image.as_ref()
    }

    fn validate_budget(&self) -> Result<(), CalendarEventError> {
        validate_calendar_collection_budget(
            self.uid.as_str(),
            &self.title,
            &self.content,
            &self.event_references,
            self.list_description.as_deref(),
            self.image
                .as_ref()
                .map(|image| image.descriptor().url().as_str()),
        )
    }
}

/// Constructor input for the tolerant, raw-preserving kind-31924 parse layer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedNip52CalendarParts {
    pub d_tag: String,
    pub title: String,
    pub content: String,
    pub event_references: Vec<CalendarEventReference>,
    pub list_description: Option<String>,
    pub image: Option<CalendarUri>,
}

/// Structurally valid kind-31924 data. Canonical admission is a separate step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedNip52Calendar {
    d_tag: String,
    title: String,
    content: String,
    event_references: Vec<CalendarEventReference>,
    list_description: Option<String>,
    image: Option<CalendarUri>,
}

impl ParsedNip52Calendar {
    pub fn try_new(parts: ParsedNip52CalendarParts) -> Result<Self, CalendarEventError> {
        validate_calendar_tag_text(&parts.d_tag, "d")?;
        validate_calendar_tag_text(&parts.title, "title")?;
        validate_calendar_content(&parts.content)?;
        if let Some(description) = parts.list_description.as_deref() {
            validate_calendar_tag_text(description, "description")?;
        }
        validate_calendar_collection_budget(
            &parts.d_tag,
            &parts.title,
            &parts.content,
            &parts.event_references,
            parts.list_description.as_deref(),
            parts.image.as_ref().map(CalendarUri::as_str),
        )?;
        Ok(Self {
            d_tag: parts.d_tag,
            title: parts.title,
            content: parts.content,
            event_references: parts.event_references,
            list_description: parts.list_description,
            image: parts.image,
        })
    }

    pub fn d_tag(&self) -> &str {
        &self.d_tag
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn event_references(&self) -> &[CalendarEventReference] {
        &self.event_references
    }

    pub fn list_description(&self) -> Option<&str> {
        self.list_description.as_deref()
    }

    pub fn image(&self) -> Option<&CalendarUri> {
        self.image.as_ref()
    }
}

/// Canonical Radroots admission of a parsed kind-31924 calendar collection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdmittedCalendar {
    parsed: ParsedNip52Calendar,
    uid: CalendarUid,
    blossom_image: Option<BlobUrl>,
}

impl AdmittedCalendar {
    pub fn try_from_parsed(parsed: ParsedNip52Calendar) -> Result<Self, CalendarAdmissionError> {
        let uid = CalendarUid::parse(parsed.d_tag())
            .map_err(|_| CalendarAdmissionError::NonCanonicalField("d"))?;
        if !canonical_calendar_tag_text_is_valid(parsed.title()) {
            return Err(CalendarAdmissionError::NonCanonicalField("title"));
        }
        if parsed
            .list_description()
            .is_some_and(|description| !canonical_calendar_tag_text_is_valid(description))
        {
            return Err(CalendarAdmissionError::NonCanonicalField("description"));
        }
        if parsed
            .event_references()
            .iter()
            .any(|reference| !reference.is_canonical())
        {
            return Err(CalendarAdmissionError::NonCanonicalField("event_reference"));
        }
        if calendar_event_references_have_duplicates(parsed.event_references()) {
            return Err(CalendarAdmissionError::DuplicateEventReference);
        }
        let blossom_image = parsed
            .image()
            .map(|image| {
                BlobUrl::parse(image.as_str()).map_err(|_| CalendarAdmissionError::NonBlossomImage)
            })
            .transpose()?;
        Ok(Self {
            parsed,
            uid,
            blossom_image,
        })
    }

    pub fn parsed(&self) -> &ParsedNip52Calendar {
        &self.parsed
    }

    pub fn uid(&self) -> &CalendarUid {
        &self.uid
    }

    pub fn title(&self) -> &str {
        self.parsed.title()
    }

    pub fn content(&self) -> &str {
        self.parsed.content()
    }

    pub fn event_references(&self) -> &[CalendarEventReference] {
        self.parsed.event_references()
    }

    pub fn list_description(&self) -> Option<&str> {
        self.parsed.list_description()
    }

    pub fn blossom_image(&self) -> Option<&BlobUrl> {
        self.blossom_image.as_ref()
    }
}

#[cfg_attr(any(feature = "serde", test), derive(serde::Serialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CalendarRequest {
    calendar: AddressableCoordinate,
    relay: Option<String>,
}

impl CalendarRequest {
    pub fn new(calendar: impl AsRef<str>, relay: Option<&str>) -> Result<Self, CalendarEventError> {
        let calendar = AddressableCoordinate::parse(calendar.as_ref())
            .map_err(|_| CalendarEventError::InvalidUrl("calendar request"))?;
        let parts = crate::id::AddressableCoordinateParts::parse(calendar.as_str())
            .map_err(|_| CalendarEventError::InvalidUrl("calendar request"))?;
        if parts.kind != crate::envelope::kind::KIND_CALENDAR {
            return Err(CalendarEventError::InvalidUrl("calendar request"));
        }
        let relay = relay
            .map(|value| {
                if calendar_relay_url_is_valid(value) {
                    Ok(value.to_string())
                } else {
                    Err(CalendarEventError::InvalidUrl("calendar request relay"))
                }
            })
            .transpose()?;
        Ok(Self { calendar, relay })
    }

    pub fn calendar(&self) -> &AddressableCoordinate {
        &self.calendar
    }

    pub fn relay(&self) -> Option<&str> {
        self.relay.as_deref()
    }

    pub fn is_canonical(&self) -> bool {
        let Ok(parts) = crate::id::AddressableCoordinateParts::parse(self.calendar.as_str()) else {
            return false;
        };
        self.calendar.as_str() == format!("{}:{}:{}", parts.kind, parts.pubkey, parts.d_tag)
            && self
                .relay
                .as_deref()
                .is_none_or(|relay| RelayUrl::parse(relay).is_ok())
    }
}

/// A canonical Gregorian calendar date in `YYYY-MM-DD` form.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CalendarDate(String);

impl CalendarDate {
    pub fn parse(value: &str) -> Result<Self, CalendarEventError> {
        let bytes = value.as_bytes();
        if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
            return Err(CalendarEventError::InvalidDate);
        }
        let year = parse_date_digits(&bytes[0..4])?;
        let month = parse_date_digits(&bytes[5..7])?;
        let day = parse_date_digits(&bytes[8..10])?;
        if year == 0 || month == 0 || month > 12 || day == 0 || day > days_in_month(year, month) {
            return Err(CalendarEventError::InvalidDate);
        }
        Ok(Self(value.into()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CalendarDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl AsRef<str> for CalendarDate {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl FromStr for CalendarDate {
    type Err = CalendarEventError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

#[cfg(any(feature = "serde", test))]
impl serde::Serialize for CalendarDate {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

#[cfg(any(feature = "serde", test))]
impl<'de> serde::Deserialize<'de> for CalendarDate {
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
pub struct AuthoredCalendarDateEvent {
    d_tag: DTag,
    title: String,
    start: CalendarDate,
    description: Option<String>,
    end: Option<CalendarDate>,
    locations: Vec<String>,
    geohash: Option<String>,
    summary: Option<String>,
    image: Option<AuthoredImage>,
    participants: Option<Vec<CalendarParticipant>>,
    categories: Vec<String>,
    references: Vec<CalendarUri>,
    calendar_requests: Vec<CalendarRequest>,
}

impl AuthoredCalendarDateEvent {
    pub fn new(
        d_tag: impl AsRef<str>,
        title: impl Into<String>,
        start: CalendarDate,
    ) -> Result<Self, CalendarEventError> {
        let d_tag =
            DTag::parse(d_tag.as_ref()).map_err(|_| CalendarEventError::InvalidIdentifier)?;
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

    pub fn with_end(mut self, end: CalendarDate) -> Result<Self, CalendarEventError> {
        if end <= self.start {
            return Err(CalendarEventError::InvalidRange);
        }
        self.end = Some(end);
        self.validate_budget()?;
        Ok(self)
    }

    pub fn with_description(
        mut self,
        value: impl Into<String>,
    ) -> Result<Self, CalendarEventError> {
        let value = value.into();
        validate_calendar_content(&value)?;
        self.description = Some(value);
        self.validate_budget()?;
        Ok(self)
    }

    pub fn with_locations(mut self, value: Vec<String>) -> Result<Self, CalendarEventError> {
        validate_authored_calendar_locations(&value)?;
        self.locations = value;
        self.validate_budget()?;
        Ok(self)
    }

    pub fn with_geohash(mut self, value: impl Into<String>) -> Result<Self, CalendarEventError> {
        let value = value.into();
        if !canonical_calendar_geohash_is_valid(&value) {
            return Err(CalendarEventError::InvalidGeohash);
        }
        self.geohash = Some(value);
        self.validate_budget()?;
        Ok(self)
    }

    pub fn with_summary(mut self, value: impl Into<String>) -> Result<Self, CalendarEventError> {
        let value = value.into();
        validate_canonical_calendar_tag_text(&value, "summary")?;
        self.summary = Some(value);
        self.validate_budget()?;
        Ok(self)
    }

    pub fn with_image(mut self, value: AuthoredImage) -> Result<Self, CalendarEventError> {
        self.image = Some(value);
        self.validate_budget()?;
        Ok(self)
    }

    pub fn with_participants(
        mut self,
        value: Vec<CalendarParticipant>,
    ) -> Result<Self, CalendarEventError> {
        validate_authored_calendar_participants(&value)?;
        self.participants = Some(value);
        self.validate_budget()?;
        Ok(self)
    }

    pub fn with_categories(mut self, value: Vec<String>) -> Result<Self, CalendarEventError> {
        validate_authored_calendar_categories(&value)?;
        self.categories = value;
        self.validate_budget()?;
        Ok(self)
    }

    pub fn with_references(mut self, value: Vec<CalendarUri>) -> Result<Self, CalendarEventError> {
        self.references = value;
        self.validate_budget()?;
        Ok(self)
    }

    pub fn with_calendar_requests(
        mut self,
        value: Vec<CalendarRequest>,
    ) -> Result<Self, CalendarEventError> {
        validate_authored_calendar_requests(&value)?;
        self.calendar_requests = value;
        self.validate_budget()?;
        Ok(self)
    }

    pub fn d_tag(&self) -> &DTag {
        &self.d_tag
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn start(&self) -> &CalendarDate {
        &self.start
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    pub fn end(&self) -> Option<&CalendarDate> {
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

    pub fn image(&self) -> Option<&AuthoredImage> {
        self.image.as_ref()
    }

    pub fn participants(&self) -> Option<&Vec<CalendarParticipant>> {
        self.participants.as_ref()
    }

    pub fn categories(&self) -> &[String] {
        &self.categories
    }

    pub fn references(&self) -> &[CalendarUri] {
        &self.references
    }

    pub fn calendar_requests(&self) -> &[CalendarRequest] {
        &self.calendar_requests
    }

    fn validate_budget(&self) -> Result<(), CalendarEventError> {
        validate_authored_calendar_budget(self.description(), authored_date_tags_for_budget(self))
    }
}

/// Strict authored NIP-52 kind-31923 event.
///
/// Covered uppercase-`D` values are derived from the timestamp range and are
/// never accepted from callers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthoredCalendarTimeEvent {
    d_tag: DTag,
    title: String,
    start: u64,
    description: Option<String>,
    end: Option<u64>,
    start_tzid: Option<IanaTimeZoneId>,
    end_tzid: Option<IanaTimeZoneId>,
    locations: Vec<String>,
    geohash: Option<String>,
    summary: Option<String>,
    image: Option<AuthoredImage>,
    participants: Option<Vec<CalendarParticipant>>,
    categories: Vec<String>,
    references: Vec<CalendarUri>,
    calendar_requests: Vec<CalendarRequest>,
}

impl AuthoredCalendarTimeEvent {
    pub fn new(
        d_tag: impl AsRef<str>,
        title: impl Into<String>,
        start: u64,
    ) -> Result<Self, CalendarEventError> {
        let d_tag =
            DTag::parse(d_tag.as_ref()).map_err(|_| CalendarEventError::InvalidIdentifier)?;
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

    pub fn with_end(mut self, end: u64) -> Result<Self, CalendarEventError> {
        covered_utc_days(self.start, Some(end))?;
        self.end = Some(end);
        self.validate_budget()?;
        Ok(self)
    }

    pub fn with_description(
        mut self,
        value: impl Into<String>,
    ) -> Result<Self, CalendarEventError> {
        let value = value.into();
        validate_calendar_content(&value)?;
        self.description = Some(value);
        self.validate_budget()?;
        Ok(self)
    }

    pub fn with_start_tzid(mut self, value: impl AsRef<str>) -> Result<Self, CalendarEventError> {
        self.start_tzid = Some(IanaTimeZoneId::parse(value)?);
        self.validate_budget()?;
        Ok(self)
    }

    pub fn with_end_tzid(mut self, value: impl AsRef<str>) -> Result<Self, CalendarEventError> {
        self.end_tzid = Some(IanaTimeZoneId::parse(value)?);
        self.validate_budget()?;
        Ok(self)
    }

    pub fn with_locations(mut self, value: Vec<String>) -> Result<Self, CalendarEventError> {
        validate_authored_calendar_locations(&value)?;
        self.locations = value;
        self.validate_budget()?;
        Ok(self)
    }

    pub fn with_geohash(mut self, value: impl Into<String>) -> Result<Self, CalendarEventError> {
        let value = value.into();
        if !canonical_calendar_geohash_is_valid(&value) {
            return Err(CalendarEventError::InvalidGeohash);
        }
        self.geohash = Some(value);
        self.validate_budget()?;
        Ok(self)
    }

    pub fn with_summary(mut self, value: impl Into<String>) -> Result<Self, CalendarEventError> {
        let value = value.into();
        validate_canonical_calendar_tag_text(&value, "summary")?;
        self.summary = Some(value);
        self.validate_budget()?;
        Ok(self)
    }

    pub fn with_image(mut self, value: AuthoredImage) -> Result<Self, CalendarEventError> {
        self.image = Some(value);
        self.validate_budget()?;
        Ok(self)
    }

    pub fn with_participants(
        mut self,
        value: Vec<CalendarParticipant>,
    ) -> Result<Self, CalendarEventError> {
        validate_authored_calendar_participants(&value)?;
        self.participants = Some(value);
        self.validate_budget()?;
        Ok(self)
    }

    pub fn with_categories(mut self, value: Vec<String>) -> Result<Self, CalendarEventError> {
        validate_authored_calendar_categories(&value)?;
        self.categories = value;
        self.validate_budget()?;
        Ok(self)
    }

    pub fn with_references(mut self, value: Vec<CalendarUri>) -> Result<Self, CalendarEventError> {
        self.references = value;
        self.validate_budget()?;
        Ok(self)
    }

    pub fn with_calendar_requests(
        mut self,
        value: Vec<CalendarRequest>,
    ) -> Result<Self, CalendarEventError> {
        validate_authored_calendar_requests(&value)?;
        self.calendar_requests = value;
        self.validate_budget()?;
        Ok(self)
    }

    pub fn d_tag(&self) -> &DTag {
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

    pub fn start_tzid(&self) -> Option<&IanaTimeZoneId> {
        self.start_tzid.as_ref()
    }

    pub fn end_tzid(&self) -> Option<&IanaTimeZoneId> {
        self.end_tzid.as_ref()
    }

    pub fn effective_end_tzid(&self) -> Option<&IanaTimeZoneId> {
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

    pub fn image(&self) -> Option<&AuthoredImage> {
        self.image.as_ref()
    }

    pub fn participants(&self) -> Option<&Vec<CalendarParticipant>> {
        self.participants.as_ref()
    }

    pub fn categories(&self) -> &[String] {
        &self.categories
    }

    pub fn references(&self) -> &[CalendarUri] {
        &self.references
    }

    pub fn calendar_requests(&self) -> &[CalendarRequest] {
        &self.calendar_requests
    }

    fn validate_budget(&self) -> Result<(), CalendarEventError> {
        validate_authored_calendar_budget(self.description(), authored_time_tags_for_budget(self)?)
    }
}

#[cfg_attr(any(feature = "serde", test), derive(serde::Serialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedNip52CalendarCommon {
    d_tag: String,
    title: String,
    description: Option<String>,
    locations: Vec<String>,
    geohash: Option<String>,
    summary: Option<String>,
    image: Option<CalendarUri>,
    participants: Vec<CalendarParticipant>,
    categories: Vec<String>,
    references: Vec<CalendarUri>,
    calendar_requests: Vec<CalendarRequest>,
    legacy_name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedNip52CalendarCommonParts {
    pub d_tag: String,
    pub title: String,
    pub description: Option<String>,
    pub locations: Vec<String>,
    pub geohash: Option<String>,
    pub summary: Option<String>,
    pub image: Option<CalendarUri>,
    pub participants: Vec<CalendarParticipant>,
    pub categories: Vec<String>,
    pub references: Vec<CalendarUri>,
    pub calendar_requests: Vec<CalendarRequest>,
    pub legacy_name: Option<String>,
}

impl ParsedNip52CalendarCommon {
    pub fn try_new(parts: ParsedNip52CalendarCommonParts) -> Result<Self, CalendarEventError> {
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
            return Err(CalendarEventError::InvalidGeohash);
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

    pub fn image(&self) -> Option<&CalendarUri> {
        self.image.as_ref()
    }

    pub fn participants(&self) -> &[CalendarParticipant] {
        &self.participants
    }

    pub fn categories(&self) -> &[String] {
        &self.categories
    }

    pub fn references(&self) -> &[CalendarUri] {
        &self.references
    }

    pub fn calendar_requests(&self) -> &[CalendarRequest] {
        &self.calendar_requests
    }

    pub fn legacy_name(&self) -> Option<&str> {
        self.legacy_name.as_deref()
    }
}

#[cfg_attr(any(feature = "serde", test), derive(serde::Serialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedNip52CalendarDateEvent {
    common: ParsedNip52CalendarCommon,
    start: CalendarDate,
    end: Option<CalendarDate>,
    extension_day_tags: Vec<Vec<String>>,
}

impl ParsedNip52CalendarDateEvent {
    pub fn try_new(
        common: ParsedNip52CalendarCommon,
        start: CalendarDate,
        end: Option<CalendarDate>,
        extension_day_tags: Vec<Vec<String>>,
    ) -> Result<Self, CalendarEventError> {
        if end.as_ref().is_some_and(|end| end <= &start) {
            return Err(CalendarEventError::InvalidRange);
        }
        Ok(Self {
            common,
            start,
            end,
            extension_day_tags,
        })
    }

    pub fn common(&self) -> &ParsedNip52CalendarCommon {
        &self.common
    }

    pub fn start(&self) -> &CalendarDate {
        &self.start
    }

    pub fn end(&self) -> Option<&CalendarDate> {
        self.end.as_ref()
    }

    pub fn extension_day_tags(&self) -> &[Vec<String>] {
        &self.extension_day_tags
    }
}

#[cfg_attr(any(feature = "serde", test), derive(serde::Serialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObservedUtcDay {
    wire_value: String,
    index: u64,
}

impl ObservedUtcDay {
    pub fn parse(value: impl Into<String>) -> Result<Self, CalendarEventError> {
        let wire_value = value.into();
        if wire_value.is_empty() || !wire_value.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(CalendarEventError::InvalidText("D"));
        }
        let index = wire_value
            .parse()
            .map_err(|_| CalendarEventError::InvalidText("D"))?;
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
pub struct ParsedNip52CalendarTimeEvent {
    common: ParsedNip52CalendarCommon,
    start_wire: String,
    start: u64,
    end_wire: Option<String>,
    end: Option<u64>,
    observed_day_indices: Vec<ObservedUtcDay>,
    start_tzid: Option<IanaTimeZoneId>,
    end_tzid: Option<IanaTimeZoneId>,
}

impl ParsedNip52CalendarTimeEvent {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        common: ParsedNip52CalendarCommon,
        start_wire: String,
        start: u64,
        end_wire: Option<String>,
        end: Option<u64>,
        observed_day_indices: Vec<ObservedUtcDay>,
        start_tzid: Option<IanaTimeZoneId>,
        end_tzid: Option<IanaTimeZoneId>,
    ) -> Result<Self, CalendarEventError> {
        if parse_calendar_decimal(&start_wire) != Some(start)
            || end_wire
                .as_deref()
                .zip(end)
                .is_some_and(|(wire, end)| parse_calendar_decimal(wire) != Some(end))
            || end_wire.is_some() != end.is_some()
        {
            return Err(CalendarEventError::InvalidRange);
        }
        if end.is_some_and(|end| end <= start) {
            return Err(CalendarEventError::InvalidRange);
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
            return Err(CalendarEventError::InvalidRange);
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

    pub fn common(&self) -> &ParsedNip52CalendarCommon {
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

    pub fn observed_day_indices(&self) -> &[ObservedUtcDay] {
        &self.observed_day_indices
    }

    pub fn start_tzid(&self) -> Option<&IanaTimeZoneId> {
        self.start_tzid.as_ref()
    }

    pub fn end_tzid(&self) -> Option<&IanaTimeZoneId> {
        self.end_tzid.as_ref()
    }

    pub fn effective_end_tzid(&self) -> Option<&IanaTimeZoneId> {
        self.end_tzid.as_ref().or(self.start_tzid.as_ref())
    }
}

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CalendarAdmissionError {
    NonCanonicalField(&'static str),
    DuplicateEventReference,
    AuthorHintMismatch,
    ForbiddenDateDayIndex,
    IncompleteDayCoverage,
    CoveredDayLimitExceeded { max: u64, actual: u64 },
    NonBlossomImage,
}

impl CalendarAdmissionError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::NonCanonicalField(_) => "non_canonical_field",
            Self::DuplicateEventReference => "duplicate_event_reference",
            Self::AuthorHintMismatch => "author_hint_mismatch",
            Self::ForbiddenDateDayIndex => "forbidden_date_day_index",
            Self::IncompleteDayCoverage => "incomplete_day_coverage",
            Self::CoveredDayLimitExceeded { .. } => "covered_day_limit_exceeded",
            Self::NonBlossomImage => "non_blossom_image",
        }
    }
}

impl fmt::Display for CalendarAdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonCanonicalField(field) => write!(f, "calendar {field} is not canonical"),
            Self::DuplicateEventReference => {
                f.write_str("calendar event references must have unique coordinates")
            }
            Self::AuthorHintMismatch => {
                f.write_str("calendar event author hint does not match the event coordinate")
            }
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
impl std::error::Error for CalendarAdmissionError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdmittedCalendarDateEvent {
    parsed: ParsedNip52CalendarDateEvent,
    d_tag: DTag,
    blossom_image: Option<BlobUrl>,
}

impl AdmittedCalendarDateEvent {
    pub fn try_from_parsed(
        parsed: ParsedNip52CalendarDateEvent,
    ) -> Result<Self, CalendarAdmissionError> {
        if !parsed.extension_day_tags.is_empty() {
            return Err(CalendarAdmissionError::ForbiddenDateDayIndex);
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

    pub fn parsed(&self) -> &ParsedNip52CalendarDateEvent {
        &self.parsed
    }

    pub fn d_tag(&self) -> &DTag {
        &self.d_tag
    }

    pub fn blossom_image(&self) -> Option<&BlobUrl> {
        self.blossom_image.as_ref()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdmittedCalendarTimeEvent {
    parsed: ParsedNip52CalendarTimeEvent,
    d_tag: DTag,
    covered_utc_days: Vec<u64>,
    blossom_image: Option<BlobUrl>,
}

impl AdmittedCalendarTimeEvent {
    pub fn try_from_parsed(
        parsed: ParsedNip52CalendarTimeEvent,
    ) -> Result<Self, CalendarAdmissionError> {
        validate_admitted_calendar_common(&parsed.common)?;
        let d_tag = admitted_d_tag(&parsed.common)?;
        if parsed.start_wire != parsed.start.to_string()
            || parsed
                .end_wire
                .as_deref()
                .zip(parsed.end)
                .is_some_and(|(wire, end)| wire != end.to_string())
        {
            return Err(CalendarAdmissionError::NonCanonicalField("timestamp"));
        }
        let expected = covered_utc_days(parsed.start, parsed.end).map_err(|error| match error {
            CalendarEventError::CoveredDayLimitExceeded { max, actual } => {
                CalendarAdmissionError::CoveredDayLimitExceeded { max, actual }
            }
            _ => CalendarAdmissionError::IncompleteDayCoverage,
        })?;
        let covered_utc_days = expected.clone().collect::<Vec<_>>();
        if parsed.observed_day_indices.len() != covered_utc_days.len()
            || parsed
                .observed_day_indices
                .iter()
                .zip(&covered_utc_days)
                .any(|(observed, expected)| !observed.is_canonical() || observed.index != *expected)
        {
            return Err(CalendarAdmissionError::IncompleteDayCoverage);
        }
        let blossom_image = admitted_blossom_image(&parsed.common)?;
        Ok(Self {
            parsed,
            d_tag,
            covered_utc_days,
            blossom_image,
        })
    }

    pub fn parsed(&self) -> &ParsedNip52CalendarTimeEvent {
        &self.parsed
    }

    pub fn d_tag(&self) -> &DTag {
        &self.d_tag
    }

    pub fn covered_utc_days(&self) -> &[u64] {
        &self.covered_utc_days
    }

    pub fn blossom_image(&self) -> Option<&BlobUrl> {
        self.blossom_image.as_ref()
    }
}

pub fn covered_utc_days(
    start: u64,
    end: Option<u64>,
) -> Result<RangeInclusive<u64>, CalendarEventError> {
    if end.is_some_and(|end| end <= start) {
        return Err(CalendarEventError::InvalidRange);
    }
    let first = start / RADROOTS_CALENDAR_SECONDS_PER_DAY;
    let last = end
        .map(|end| (end - 1) / RADROOTS_CALENDAR_SECONDS_PER_DAY)
        .unwrap_or(first);
    let actual = last - first + 1;
    if actual > RADROOTS_CALENDAR_MAX_COVERED_UTC_DAYS {
        return Err(CalendarEventError::CoveredDayLimitExceeded {
            max: RADROOTS_CALENDAR_MAX_COVERED_UTC_DAYS,
            actual,
        });
    }
    Ok(first..=last)
}

fn validated_title(value: String) -> Result<String, CalendarEventError> {
    if !canonical_calendar_tag_text_is_valid(&value) {
        return Err(CalendarEventError::InvalidTitle);
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

fn parse_calendar_reference_relay(
    relay: Option<&str>,
    error: CalendarEventError,
) -> Result<Option<String>, CalendarEventError> {
    relay
        .map(|relay| {
            if calendar_relay_url_is_valid(relay) {
                Ok(relay.to_string())
            } else {
                Err(error.clone())
            }
        })
        .transpose()
}

fn calendar_event_references_have_duplicates(references: &[CalendarEventReference]) -> bool {
    references.iter().enumerate().any(|(index, reference)| {
        references
            .iter()
            .skip(index + 1)
            .any(|candidate| reference.has_same_coordinate(candidate))
    })
}

fn validate_strict_calendar_event_references(
    references: &[CalendarEventReference],
) -> Result<(), CalendarEventError> {
    if references.iter().any(|reference| !reference.is_canonical()) {
        return Err(CalendarEventError::InvalidEventReference);
    }
    if calendar_event_references_have_duplicates(references) {
        return Err(CalendarEventError::DuplicateEventReference);
    }
    Ok(())
}

fn validate_strict_calendar_author_hint(
    event_reference: &CalendarEventReference,
    author_hint: &CalendarEventAuthorReference,
) -> Result<(), CalendarEventError> {
    if !author_hint.is_canonical() {
        return Err(CalendarEventError::InvalidAuthorReference);
    }
    if author_hint.pubkey() != event_reference.author() {
        return Err(CalendarEventError::AuthorHintMismatch);
    }
    Ok(())
}

fn validate_calendar_tag_text(value: &str, field: &'static str) -> Result<(), CalendarEventError> {
    if !calendar_tag_text_is_valid(value) {
        return Err(if value.len() > DEFAULT_TAG_ELEMENT_MAX_BYTES {
            CalendarEventError::TagElementTooLarge {
                field,
                max: DEFAULT_TAG_ELEMENT_MAX_BYTES,
                actual: value.len(),
            }
        } else {
            CalendarEventError::InvalidText(field)
        });
    }
    Ok(())
}

fn validate_canonical_calendar_tag_text(
    value: &str,
    field: &'static str,
) -> Result<(), CalendarEventError> {
    validate_calendar_tag_text(value, field)?;
    if value.trim() != value {
        return Err(CalendarEventError::InvalidText(field));
    }
    Ok(())
}

fn validate_calendar_content(value: &str) -> Result<(), CalendarEventError> {
    if value.len() > DEFAULT_CONTENT_MAX_BYTES {
        return Err(CalendarEventError::ContentTooLarge {
            max: DEFAULT_CONTENT_MAX_BYTES,
            actual: value.len(),
        });
    }
    Ok(())
}

fn validate_authored_calendar_locations(locations: &[String]) -> Result<(), CalendarEventError> {
    for location in locations {
        validate_canonical_calendar_tag_text(location, "location")?;
    }
    Ok(())
}

fn validate_authored_calendar_requests(
    requests: &[CalendarRequest],
) -> Result<(), CalendarEventError> {
    if requests.iter().any(|request| !request.is_canonical()) {
        return Err(CalendarEventError::InvalidUrl("calendar request"));
    }
    Ok(())
}

fn validate_inbound_calendar_participants(
    participants: &[CalendarParticipant],
) -> Result<(), CalendarEventError> {
    for (index, participant) in participants.iter().enumerate() {
        if parse_public_key(&participant.pubkey).is_err()
            || participant
                .relay
                .as_deref()
                .is_some_and(|relay| !calendar_relay_url_is_valid(relay))
            || participant
                .role
                .as_deref()
                .is_some_and(|role| !calendar_tag_text_is_valid(role))
        {
            return Err(CalendarEventError::InvalidParticipant { index });
        }
    }
    Ok(())
}

fn validate_authored_calendar_participants(
    participants: &[CalendarParticipant],
) -> Result<(), CalendarEventError> {
    if participants.len() > RADROOTS_CALENDAR_MAX_PARTICIPANTS {
        return Err(CalendarEventError::TooManyParticipants {
            max: RADROOTS_CALENDAR_MAX_PARTICIPANTS,
            actual: participants.len(),
        });
    }
    validate_inbound_calendar_participants(participants)?;
    for (index, participant) in participants.iter().enumerate() {
        let pubkey = parse_public_key(&participant.pubkey)
            .map_err(|_| CalendarEventError::InvalidParticipant { index })?;
        if pubkey.to_hex() != participant.pubkey
            || participant
                .relay
                .as_deref()
                .is_some_and(|relay| RelayUrl::parse(relay).is_err())
            || participant
                .role
                .as_deref()
                .is_some_and(|role| !canonical_calendar_tag_text_is_valid(role))
        {
            return Err(CalendarEventError::InvalidParticipant { index });
        }
    }
    Ok(())
}

fn validate_authored_calendar_categories(categories: &[String]) -> Result<(), CalendarEventError> {
    for category in categories {
        validate_canonical_calendar_tag_text(category, "category")?;
    }
    Ok(())
}

fn validate_admitted_calendar_common(
    common: &ParsedNip52CalendarCommon,
) -> Result<(), CalendarAdmissionError> {
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
            .all(CalendarRequest::is_canonical);
    if !canonical {
        return Err(CalendarAdmissionError::NonCanonicalField("metadata"));
    }
    validate_authored_calendar_participants(&common.participants)
        .map_err(|_| CalendarAdmissionError::NonCanonicalField("participant"))?;
    Ok(())
}

fn admitted_blossom_image(
    common: &ParsedNip52CalendarCommon,
) -> Result<Option<BlobUrl>, CalendarAdmissionError> {
    common
        .image
        .as_ref()
        .map(|image| {
            BlobUrl::parse(image.as_str()).map_err(|_| CalendarAdmissionError::NonBlossomImage)
        })
        .transpose()
}

fn admitted_d_tag(common: &ParsedNip52CalendarCommon) -> Result<DTag, CalendarAdmissionError> {
    let d_tag =
        DTag::parse(&common.d_tag).map_err(|_| CalendarAdmissionError::NonCanonicalField("d"))?;
    if d_tag.as_str() != common.d_tag {
        return Err(CalendarAdmissionError::NonCanonicalField("d"));
    }
    Ok(d_tag)
}

fn authored_date_tags_for_budget(event: &AuthoredCalendarDateEvent) -> Vec<Vec<String>> {
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
    event: &AuthoredCalendarTimeEvent,
) -> Result<Vec<Vec<String>>, CalendarEventError> {
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
    image: Option<&AuthoredImage>,
    participants: Option<&Vec<CalendarParticipant>>,
    categories: &[String],
    references: &[CalendarUri],
    calendar_requests: &[CalendarRequest],
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

fn validate_calendar_collection_budget(
    d_tag: &str,
    title: &str,
    content: &str,
    event_references: &[CalendarEventReference],
    list_description: Option<&str>,
    image: Option<&str>,
) -> Result<(), CalendarEventError> {
    let mut tags = vec![budget_tag("d", d_tag), budget_tag("title", title)];
    for reference in event_references {
        let mut tag = budget_tag("a", reference.coordinate().as_str());
        if let Some(relay) = reference.relay() {
            tag.push(relay.to_string());
        }
        tags.push(tag);
    }
    if let Some(description) = list_description {
        tags.push(budget_tag("description", description));
    }
    if let Some(image) = image {
        tags.push(budget_tag("image", image));
    }
    validate_authored_calendar_budget(Some(content), tags)
}

#[allow(clippy::too_many_arguments)]
fn validate_calendar_rsvp_budget(
    d_tag: &str,
    event_reference: &CalendarEventReference,
    revision_reference: Option<&CalendarEventRevisionReference>,
    status: &CalendarEventRsvpStatus,
    free_busy: Option<&CalendarEventFreeBusy>,
    author_hint: Option<&CalendarEventAuthorReference>,
    note: Option<&str>,
) -> Result<(), CalendarEventError> {
    let mut event_tag = budget_tag("a", event_reference.coordinate().as_str());
    if let Some(relay) = event_reference.relay() {
        event_tag.push(relay.to_string());
    }
    let mut tags = vec![
        budget_tag("d", d_tag),
        event_tag,
        budget_tag("status", calendar_rsvp_status_wire_value(status)),
    ];
    if let Some(reference) = revision_reference {
        let mut tag = budget_tag("e", reference.raw_event_id());
        if let Some(relay) = reference.relay() {
            tag.push(relay.to_string());
        }
        tags.push(tag);
    }
    if let Some(free_busy) = free_busy {
        tags.push(budget_tag("fb", calendar_free_busy_wire_value(free_busy)));
    }
    if let Some(author_hint) = author_hint {
        let mut tag = budget_tag("p", author_hint.raw_pubkey());
        if let Some(relay) = author_hint.relay() {
            tag.push(relay.to_string());
        }
        tags.push(tag);
    }
    validate_authored_calendar_budget(note, tags)
}

fn calendar_rsvp_status_wire_value(status: &CalendarEventRsvpStatus) -> &'static str {
    match status {
        CalendarEventRsvpStatus::Accepted => "accepted",
        CalendarEventRsvpStatus::Declined => "declined",
        CalendarEventRsvpStatus::Tentative => "tentative",
    }
}

fn calendar_free_busy_wire_value(free_busy: &CalendarEventFreeBusy) -> &'static str {
    match free_busy {
        CalendarEventFreeBusy::Free => "free",
        CalendarEventFreeBusy::Busy => "busy",
    }
}

fn validate_authored_calendar_budget(
    description: Option<&str>,
    tags: Vec<Vec<String>>,
) -> Result<(), CalendarEventError> {
    if let Some(description) = description {
        validate_calendar_content(description)?;
    }
    if tags.len() > DEFAULT_TAG_MAX_COUNT {
        return Err(CalendarEventError::TagCountExceeded {
            max: DEFAULT_TAG_MAX_COUNT,
            actual: tags.len(),
        });
    }
    let mut total = 0usize;
    for tag in &tags {
        for value in tag {
            if value.len() > DEFAULT_TAG_ELEMENT_MAX_BYTES {
                return Err(CalendarEventError::TagElementTooLarge {
                    field: "tag",
                    max: DEFAULT_TAG_ELEMENT_MAX_BYTES,
                    actual: value.len(),
                });
            }
            total = total.saturating_add(value.len());
        }
    }
    if total > DEFAULT_TAG_TOTAL_MAX_BYTES {
        return Err(CalendarEventError::TagBytesExceeded {
            max: DEFAULT_TAG_TOTAL_MAX_BYTES,
            actual: total,
        });
    }
    Ok(())
}

fn parse_date_digits(bytes: &[u8]) -> Result<u16, CalendarEventError> {
    bytes.iter().try_fold(0_u16, |value, byte| {
        if byte.is_ascii_digit() {
            Ok(value * 10 + u16::from(byte - b'0'))
        } else {
            Err(CalendarEventError::InvalidDate)
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

/// Strict authored representation of a NIP-52 kind-31925 RSVP.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthoredCalendarEventRsvp {
    uid: CalendarUid,
    event_reference: CalendarEventReference,
    revision_reference: Option<CalendarEventRevisionReference>,
    status: CalendarEventRsvpStatus,
    free_busy: Option<CalendarEventFreeBusy>,
    author_hint: Option<CalendarEventAuthorReference>,
    note: Option<String>,
}

impl AuthoredCalendarEventRsvp {
    pub fn new(
        uid: CalendarUid,
        event_reference: CalendarEventReference,
        status: CalendarEventRsvpStatus,
    ) -> Result<Self, CalendarEventError> {
        if !event_reference.is_canonical() {
            return Err(CalendarEventError::InvalidEventReference);
        }
        let authored = Self {
            uid,
            event_reference,
            revision_reference: None,
            status,
            free_busy: None,
            author_hint: None,
            note: None,
        };
        authored.validate_budget()?;
        Ok(authored)
    }

    pub fn with_revision_reference(
        mut self,
        revision_reference: CalendarEventRevisionReference,
    ) -> Result<Self, CalendarEventError> {
        if !revision_reference.is_canonical() {
            return Err(CalendarEventError::InvalidRevisionReference);
        }
        self.revision_reference = Some(revision_reference);
        self.validate_budget()?;
        Ok(self)
    }

    pub fn with_free_busy(
        mut self,
        free_busy: CalendarEventFreeBusy,
    ) -> Result<Self, CalendarEventError> {
        if self.status == CalendarEventRsvpStatus::Declined {
            return Err(CalendarEventError::DeclinedFreeBusyForbidden);
        }
        self.free_busy = Some(free_busy);
        self.validate_budget()?;
        Ok(self)
    }

    pub fn with_author_hint(
        mut self,
        author_hint: CalendarEventAuthorReference,
    ) -> Result<Self, CalendarEventError> {
        validate_strict_calendar_author_hint(&self.event_reference, &author_hint)?;
        self.author_hint = Some(author_hint);
        self.validate_budget()?;
        Ok(self)
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Result<Self, CalendarEventError> {
        let note = note.into();
        validate_calendar_content(&note)?;
        self.note = (!note.is_empty()).then_some(note);
        self.validate_budget()?;
        Ok(self)
    }

    pub fn uid(&self) -> &CalendarUid {
        &self.uid
    }

    pub fn event_reference(&self) -> &CalendarEventReference {
        &self.event_reference
    }

    pub fn revision_reference(&self) -> Option<&CalendarEventRevisionReference> {
        self.revision_reference.as_ref()
    }

    pub fn status(&self) -> &CalendarEventRsvpStatus {
        &self.status
    }

    pub fn observed_free_busy(&self) -> Option<&CalendarEventFreeBusy> {
        self.free_busy.as_ref()
    }

    pub fn effective_free_busy(&self) -> Option<&CalendarEventFreeBusy> {
        self.free_busy.as_ref()
    }

    pub fn author_hint(&self) -> Option<&CalendarEventAuthorReference> {
        self.author_hint.as_ref()
    }

    pub fn note(&self) -> Option<&str> {
        self.note.as_deref()
    }

    fn validate_budget(&self) -> Result<(), CalendarEventError> {
        validate_calendar_rsvp_budget(
            self.uid.as_str(),
            &self.event_reference,
            self.revision_reference.as_ref(),
            &self.status,
            self.free_busy.as_ref(),
            self.author_hint.as_ref(),
            self.note.as_deref(),
        )
    }
}

/// Constructor input for the tolerant, raw-preserving kind-31925 parse layer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedNip52CalendarEventRsvpParts {
    pub d_tag: String,
    pub event_reference: CalendarEventReference,
    pub revision_reference: Option<CalendarEventRevisionReference>,
    pub status: CalendarEventRsvpStatus,
    pub observed_free_busy: Option<CalendarEventFreeBusy>,
    pub author_hint: Option<CalendarEventAuthorReference>,
    pub note: Option<String>,
}

/// Structurally valid kind-31925 data. Canonical admission is a separate step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedNip52CalendarEventRsvp {
    d_tag: String,
    event_reference: CalendarEventReference,
    revision_reference: Option<CalendarEventRevisionReference>,
    status: CalendarEventRsvpStatus,
    observed_free_busy: Option<CalendarEventFreeBusy>,
    author_hint: Option<CalendarEventAuthorReference>,
    note: Option<String>,
}

impl ParsedNip52CalendarEventRsvp {
    pub fn try_new(parts: ParsedNip52CalendarEventRsvpParts) -> Result<Self, CalendarEventError> {
        validate_calendar_tag_text(&parts.d_tag, "d")?;
        if let Some(note) = parts.note.as_deref() {
            validate_calendar_content(note)?;
        }
        validate_calendar_rsvp_budget(
            &parts.d_tag,
            &parts.event_reference,
            parts.revision_reference.as_ref(),
            &parts.status,
            parts.observed_free_busy.as_ref(),
            parts.author_hint.as_ref(),
            parts.note.as_deref(),
        )?;
        Ok(Self {
            d_tag: parts.d_tag,
            event_reference: parts.event_reference,
            revision_reference: parts.revision_reference,
            status: parts.status,
            observed_free_busy: parts.observed_free_busy,
            author_hint: parts.author_hint,
            note: parts.note,
        })
    }

    pub fn d_tag(&self) -> &str {
        &self.d_tag
    }

    pub fn event_reference(&self) -> &CalendarEventReference {
        &self.event_reference
    }

    pub fn revision_reference(&self) -> Option<&CalendarEventRevisionReference> {
        self.revision_reference.as_ref()
    }

    pub fn status(&self) -> &CalendarEventRsvpStatus {
        &self.status
    }

    pub fn observed_free_busy(&self) -> Option<&CalendarEventFreeBusy> {
        self.observed_free_busy.as_ref()
    }

    pub fn effective_free_busy(&self) -> Option<&CalendarEventFreeBusy> {
        if self.status == CalendarEventRsvpStatus::Declined {
            None
        } else {
            self.observed_free_busy.as_ref()
        }
    }

    pub fn author_hint(&self) -> Option<&CalendarEventAuthorReference> {
        self.author_hint.as_ref()
    }

    pub fn note(&self) -> Option<&str> {
        self.note.as_deref()
    }
}

/// Canonical Radroots admission of a parsed kind-31925 RSVP.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdmittedCalendarEventRsvp {
    parsed: ParsedNip52CalendarEventRsvp,
    uid: CalendarUid,
}

impl AdmittedCalendarEventRsvp {
    pub fn try_from_parsed(
        parsed: ParsedNip52CalendarEventRsvp,
    ) -> Result<Self, CalendarAdmissionError> {
        let uid = CalendarUid::parse(parsed.d_tag())
            .map_err(|_| CalendarAdmissionError::NonCanonicalField("d"))?;
        if !parsed.event_reference().is_canonical() {
            return Err(CalendarAdmissionError::NonCanonicalField("event_reference"));
        }
        if parsed
            .revision_reference()
            .is_some_and(|reference| !reference.is_canonical())
        {
            return Err(CalendarAdmissionError::NonCanonicalField(
                "revision_reference",
            ));
        }
        if let Some(author_hint) = parsed.author_hint() {
            if !author_hint.is_canonical() {
                return Err(CalendarAdmissionError::NonCanonicalField("author_hint"));
            }
            if author_hint.pubkey() != parsed.event_reference().author() {
                return Err(CalendarAdmissionError::AuthorHintMismatch);
            }
        }
        Ok(Self { parsed, uid })
    }

    pub fn parsed(&self) -> &ParsedNip52CalendarEventRsvp {
        &self.parsed
    }

    pub fn uid(&self) -> &CalendarUid {
        &self.uid
    }

    pub fn event_reference(&self) -> &CalendarEventReference {
        self.parsed.event_reference()
    }

    pub fn revision_reference(&self) -> Option<&CalendarEventRevisionReference> {
        self.parsed.revision_reference()
    }

    pub fn status(&self) -> &CalendarEventRsvpStatus {
        self.parsed.status()
    }

    pub fn observed_free_busy(&self) -> Option<&CalendarEventFreeBusy> {
        self.parsed.observed_free_busy()
    }

    pub fn effective_free_busy(&self) -> Option<&CalendarEventFreeBusy> {
        self.parsed.effective_free_busy()
    }

    pub fn author_hint(&self) -> Option<&CalendarEventAuthorReference> {
        self.parsed.author_hint()
    }

    pub fn note(&self) -> Option<&str> {
        self.parsed.note()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authored_date_event_validates_each_optional_field_at_construction() {
        let event = AuthoredCalendarDateEvent::new(
            "market-day",
            "Market day",
            CalendarDate::parse("2026-06-20").unwrap(),
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

        let error = AuthoredCalendarDateEvent::new(
            "market-day",
            " Market day ",
            CalendarDate::parse("2026-06-20").unwrap(),
        )
        .unwrap_err();
        assert_eq!(error, CalendarEventError::InvalidTitle);

        let error = AuthoredCalendarDateEvent::new(
            "market-day",
            "Market day",
            CalendarDate::parse("2026-06-20").unwrap(),
        )
        .unwrap()
        .with_geohash("C23NB62W20ST")
        .unwrap_err();
        assert_eq!(error, CalendarEventError::InvalidGeohash);
    }

    #[test]
    fn authored_time_event_uses_exact_iana_ids_and_end_timezone_fallback() {
        assert_eq!(jiff_tzdb::VERSION, Some("2026c"));

        let event = AuthoredCalendarTimeEvent::new("wash-pack", "Wash pack shift", 1_781_895_600)
            .unwrap()
            .with_end(1_781_899_200)
            .unwrap()
            .with_description("Pack CSA shares before pickup.")
            .unwrap()
            .with_start_tzid("America/Vancouver")
            .unwrap()
            .with_participants(vec![CalendarParticipant {
                pubkey: "a".repeat(64),
                relay: None,
                role: Some("host".to_string()),
            }])
            .unwrap();

        assert_eq!(event.start(), 1_781_895_600);
        assert_eq!(event.end(), Some(1_781_899_200));
        assert_eq!(
            event.start_tzid().map(IanaTimeZoneId::as_str),
            Some("America/Vancouver")
        );
        assert_eq!(event.end_tzid(), None);
        assert_eq!(
            event.effective_end_tzid().map(IanaTimeZoneId::as_str),
            Some("America/Vancouver")
        );
        assert_eq!(event.participants().map(Vec::len), Some(1));

        for invalid in ["america/vancouver", "America/Imaginary", " UTC ", ""] {
            assert_eq!(
                IanaTimeZoneId::parse(invalid),
                Err(CalendarEventError::InvalidTimeZone),
                "{invalid}"
            );
        }
    }

    #[test]
    fn calendar_dates_validate_gregorian_semantics_and_strict_ranges() {
        for valid in ["0001-01-01", "2000-02-29", "2024-02-29", "9999-12-31"] {
            assert_eq!(CalendarDate::parse(valid).unwrap().as_str(), valid);
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
                CalendarDate::parse(invalid),
                Err(CalendarEventError::InvalidDate),
                "{invalid}"
            );
        }

        let start = CalendarDate::parse("2026-06-20").unwrap();
        let event = AuthoredCalendarDateEvent::new("market-day", "Market", start.clone()).unwrap();
        assert_eq!(
            event
                .clone()
                .with_end(CalendarDate::parse("2026-06-20").unwrap()),
            Err(CalendarEventError::InvalidRange)
        );
        assert_eq!(
            event.with_end(CalendarDate::parse("2026-06-19").unwrap()),
            Err(CalendarEventError::InvalidRange)
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
            Err(CalendarEventError::InvalidRange)
        );
        assert_eq!(
            covered_utc_days(10, Some(9)),
            Err(CalendarEventError::InvalidRange)
        );
        assert_eq!(
            covered_utc_days(0, Some(367 * RADROOTS_CALENDAR_SECONDS_PER_DAY)),
            Err(CalendarEventError::CoveredDayLimitExceeded {
                max: RADROOTS_CALENDAR_MAX_COVERED_UTC_DAYS,
                actual: 367,
            })
        );
    }

    #[test]
    fn baseline_wire_types_preserve_noncanonical_but_structural_values() {
        let uri = CalendarUri::parse("ipfs://bafybeigdyrzt/calendar.webp").unwrap();
        assert_eq!(uri.as_str(), "ipfs://bafybeigdyrzt/calendar.webp");

        let observed = ObservedUtcDay::parse("00042").unwrap();
        assert_eq!(observed.wire_value(), "00042");
        assert_eq!(observed.index(), 42);
        assert!(!observed.is_canonical());

        assert_eq!(
            CalendarUri::parse("not an absolute URI"),
            Err(CalendarEventError::InvalidUrl("URI"))
        );
        assert_eq!(
            ObservedUtcDay::parse("+42"),
            Err(CalendarEventError::InvalidText("D"))
        );
    }

    #[test]
    fn authored_models_enforce_content_and_participant_limits() {
        let event = AuthoredCalendarDateEvent::new(
            "market-day",
            "Market day",
            CalendarDate::parse("2026-06-20").unwrap(),
        )
        .unwrap();
        let oversized = "x".repeat(DEFAULT_CONTENT_MAX_BYTES + 1);
        assert_eq!(
            event.clone().with_description(oversized),
            Err(CalendarEventError::ContentTooLarge {
                max: DEFAULT_CONTENT_MAX_BYTES,
                actual: DEFAULT_CONTENT_MAX_BYTES + 1,
            })
        );

        let participant = CalendarParticipant {
            pubkey: "a".repeat(64),
            relay: None,
            role: None,
        };
        assert_eq!(
            event.with_participants(vec![participant; RADROOTS_CALENDAR_MAX_PARTICIPANTS + 1]),
            Err(CalendarEventError::TooManyParticipants {
                max: RADROOTS_CALENDAR_MAX_PARTICIPANTS,
                actual: RADROOTS_CALENDAR_MAX_PARTICIPANTS + 1,
            })
        );
    }

    #[test]
    fn calendar_uid_requires_exact_unpadded_128_bit_base64url_shape() {
        for valid in [
            "AAAAAAAAAAAAAAAAAAAAAA",
            "AAAAAAAAAAAAAAAAAAAAAQ",
            "_____________________w",
        ] {
            assert_eq!(CalendarUid::parse(valid).unwrap().as_str(), valid);
        }
        for invalid in [
            "AAAAAAAAAAAAAAAAAAAAA",
            "AAAAAAAAAAAAAAAAAAAAAAA",
            "AAAAAAAAAAAAAAAAAAAAA=",
            "AAAAAAAAAAAAAAAAAAAAA+",
            "AAAAAAAAAAAAAAAAAAAAAB",
        ] {
            assert_eq!(
                CalendarUid::parse(invalid),
                Err(CalendarEventError::InvalidCalendarUid),
                "{invalid}"
            );
        }
    }

    #[test]
    fn calendar_references_preserve_structural_wire_values_and_expose_canonicality() {
        let uppercase_author = "A".repeat(64);
        let event = CalendarEventReference::parse(
            format!("031923:{uppercase_author}:wash-pack"),
            Some("WSS://Relay.Example/events"),
        )
        .unwrap();
        assert_eq!(
            event.coordinate().as_str(),
            format!("031923:{uppercase_author}:wash-pack")
        );
        assert_eq!(event.author().to_hex(), "a".repeat(64));
        assert_eq!(event.relay(), Some("WSS://Relay.Example/events"));
        assert!(!event.is_canonical());

        let revision =
            CalendarEventRevisionReference::parse("B".repeat(64), Some("WSS://revision.example"))
                .unwrap();
        assert_eq!(revision.raw_event_id(), "B".repeat(64));
        assert_eq!(revision.event_id().to_hex(), "b".repeat(64));
        assert!(!revision.is_canonical());

        let author = CalendarEventAuthorReference::parse(
            uppercase_author.clone(),
            Some("WSS://author.example"),
        )
        .unwrap();
        assert_eq!(author.raw_pubkey(), uppercase_author);
        assert_eq!(author.pubkey().to_hex(), "a".repeat(64));
        assert!(!author.is_canonical());

        assert_eq!(
            CalendarEventReference::parse(format!("31924:{}:not-an-event", "a".repeat(64)), None,),
            Err(CalendarEventError::InvalidEventReference)
        );
    }

    #[test]
    fn calendar_collection_layers_keep_baseline_tolerance_out_of_strict_states() {
        let uid = CalendarUid::parse("AAAAAAAAAAAAAAAAAAAAAA").unwrap();
        let event = canonical_event_reference("wash-pack");

        let empty = AuthoredCalendar::new(
            uid.clone(),
            "Farm calendar",
            "Shared farm operations schedule.",
            Vec::new(),
        )
        .unwrap();
        assert!(empty.event_references().is_empty());

        assert_eq!(
            AuthoredCalendar::new(
                uid.clone(),
                "Farm calendar",
                "",
                vec![event.clone(), event.clone()],
            ),
            Err(CalendarEventError::DuplicateEventReference)
        );

        let noncanonical =
            CalendarEventReference::parse(format!("031923:{}:wash-pack", "A".repeat(64)), None)
                .unwrap();
        assert_eq!(
            AuthoredCalendar::new(uid.clone(), "Farm calendar", "", vec![noncanonical],),
            Err(CalendarEventError::InvalidEventReference)
        );

        let parsed = ParsedNip52Calendar::try_new(ParsedNip52CalendarParts {
            d_tag: uid.to_string(),
            title: " Farm calendar ".to_string(),
            content: "Shared farm operations schedule.".to_string(),
            event_references: vec![event.clone(), event],
            list_description: Some(" Local operations ".to_string()),
            image: None,
        })
        .unwrap();
        assert_eq!(parsed.event_references().len(), 2);
        assert_eq!(
            AdmittedCalendar::try_from_parsed(parsed),
            Err(CalendarAdmissionError::NonCanonicalField("title"))
        );

        let parsed = ParsedNip52Calendar::try_new(ParsedNip52CalendarParts {
            d_tag: uid.to_string(),
            title: "Farm calendar".to_string(),
            content: String::new(),
            event_references: Vec::new(),
            list_description: None,
            image: None,
        })
        .unwrap();
        assert!(
            AdmittedCalendar::try_from_parsed(parsed)
                .unwrap()
                .event_references()
                .is_empty()
        );
    }

    #[test]
    fn calendar_admission_requires_unique_refs_and_structural_blossom_images() {
        let uid = "AAAAAAAAAAAAAAAAAAAAAA";
        let event = canonical_event_reference("market-day");
        let duplicate = ParsedNip52Calendar::try_new(ParsedNip52CalendarParts {
            d_tag: uid.to_string(),
            title: "Market calendar".to_string(),
            content: String::new(),
            event_references: vec![event.clone(), event.clone()],
            list_description: None,
            image: None,
        })
        .unwrap();
        let error = AdmittedCalendar::try_from_parsed(duplicate).unwrap_err();
        assert_eq!(error, CalendarAdmissionError::DuplicateEventReference);
        assert_eq!(error.code(), "duplicate_event_reference");

        let ordinary_image = ParsedNip52Calendar::try_new(ParsedNip52CalendarParts {
            d_tag: uid.to_string(),
            title: "Market calendar".to_string(),
            content: String::new(),
            event_references: vec![event.clone()],
            list_description: None,
            image: Some(CalendarUri::parse("https://media.example/market.webp").unwrap()),
        })
        .unwrap();
        assert_eq!(
            AdmittedCalendar::try_from_parsed(ordinary_image),
            Err(CalendarAdmissionError::NonBlossomImage)
        );

        let blossom_url = format!("https://media.example/{}.webp", "c".repeat(64));
        let blossom = ParsedNip52Calendar::try_new(ParsedNip52CalendarParts {
            d_tag: uid.to_string(),
            title: "Market calendar".to_string(),
            content: String::new(),
            event_references: vec![event],
            list_description: Some("Victoria farm markets".to_string()),
            image: Some(CalendarUri::parse(&blossom_url).unwrap()),
        })
        .unwrap();
        assert_eq!(
            AdmittedCalendar::try_from_parsed(blossom)
                .unwrap()
                .blossom_image()
                .unwrap()
                .as_str(),
            blossom_url
        );
    }

    #[test]
    fn calendar_collection_enforces_wire_tag_budgets() {
        let references = (0..DEFAULT_TAG_MAX_COUNT)
            .map(|index| canonical_event_reference(&format!("event-{index}")))
            .collect::<Vec<_>>();
        assert_eq!(
            AuthoredCalendar::new(
                CalendarUid::parse("AAAAAAAAAAAAAAAAAAAAAA").unwrap(),
                "Farm calendar",
                "",
                references,
            ),
            Err(CalendarEventError::TagCountExceeded {
                max: DEFAULT_TAG_MAX_COUNT,
                actual: DEFAULT_TAG_MAX_COUNT + 2,
            })
        );
    }

    #[test]
    fn declined_rsvp_retains_observed_fb_only_in_parsed_and_admitted_layers() {
        let uid = CalendarUid::parse("AAAAAAAAAAAAAAAAAAAAAQ").unwrap();
        let event = canonical_event_reference("wash-pack");
        assert_eq!(
            AuthoredCalendarEventRsvp::new(
                uid.clone(),
                event.clone(),
                CalendarEventRsvpStatus::Declined,
            )
            .unwrap()
            .with_free_busy(CalendarEventFreeBusy::Busy),
            Err(CalendarEventError::DeclinedFreeBusyForbidden)
        );

        let parsed = ParsedNip52CalendarEventRsvp::try_new(ParsedNip52CalendarEventRsvpParts {
            d_tag: uid.to_string(),
            event_reference: event,
            revision_reference: Some(
                CalendarEventRevisionReference::parse(
                    "b".repeat(64),
                    Some("wss://revision.example"),
                )
                .unwrap(),
            ),
            status: CalendarEventRsvpStatus::Declined,
            observed_free_busy: Some(CalendarEventFreeBusy::Busy),
            author_hint: Some(
                CalendarEventAuthorReference::parse("a".repeat(64), Some("wss://author.example"))
                    .unwrap(),
            ),
            note: Some("Unable to attend".to_string()),
        })
        .unwrap();
        assert_eq!(
            parsed.observed_free_busy(),
            Some(&CalendarEventFreeBusy::Busy)
        );
        assert_eq!(parsed.effective_free_busy(), None);

        let admitted = AdmittedCalendarEventRsvp::try_from_parsed(parsed).unwrap();
        assert_eq!(
            admitted.observed_free_busy(),
            Some(&CalendarEventFreeBusy::Busy)
        );
        assert_eq!(admitted.effective_free_busy(), None);
        assert_eq!(
            admitted.revision_reference().unwrap().relay(),
            Some("wss://revision.example")
        );
        assert_eq!(
            admitted.author_hint().unwrap().relay(),
            Some("wss://author.example")
        );
    }

    #[test]
    fn rsvp_author_hint_must_match_event_coordinate_in_strict_layers() {
        let uid = CalendarUid::parse("AAAAAAAAAAAAAAAAAAAAAQ").unwrap();
        let event = canonical_event_reference("wash-pack");
        let mismatched =
            CalendarEventAuthorReference::parse(crate::test_valid_hex_64('b'), None).unwrap();
        assert_eq!(
            AuthoredCalendarEventRsvp::new(
                uid.clone(),
                event.clone(),
                CalendarEventRsvpStatus::Accepted,
            )
            .unwrap()
            .with_author_hint(mismatched.clone()),
            Err(CalendarEventError::AuthorHintMismatch)
        );

        let parsed = ParsedNip52CalendarEventRsvp::try_new(ParsedNip52CalendarEventRsvpParts {
            d_tag: uid.to_string(),
            event_reference: event,
            revision_reference: None,
            status: CalendarEventRsvpStatus::Accepted,
            observed_free_busy: None,
            author_hint: Some(mismatched),
            note: None,
        })
        .unwrap();
        let error = AdmittedCalendarEventRsvp::try_from_parsed(parsed).unwrap_err();
        assert_eq!(error, CalendarAdmissionError::AuthorHintMismatch);
        assert_eq!(error.code(), "author_hint_mismatch");
    }

    fn canonical_event_reference(d_tag: &str) -> CalendarEventReference {
        CalendarEventReference::parse(
            format!("31923:{}:{d_tag}", "a".repeat(64)),
            Some("wss://relay.example"),
        )
        .unwrap()
    }
}
