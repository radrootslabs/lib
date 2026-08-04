#[cfg(not(feature = "std"))]
use alloc::{
    format,
    string::{String, ToString},
};
use core::fmt;

use crate::{
    envelope::kind::{
        KIND_ARTICLE, KIND_CALENDAR_DATE_EVENT, KIND_CALENDAR_TIME_EVENT, KIND_CLASSIFIED_LISTING,
        KIND_COMMENT,
    },
    id::{
        AddressableCoordinate, AddressableCoordinateParts, EventId, ParseError, parse_public_key,
    },
    tag::relay_hint::NostrRelayHint,
    wire::{
        DEFAULT_CONTENT_MAX_BYTES, DEFAULT_RAW_JSON_MAX_BYTES, DEFAULT_TAG_ELEMENT_MAX_BYTES,
        DEFAULT_TAG_MAX_COUNT, DEFAULT_TAG_TOTAL_ELEMENT_MAX_COUNT, DEFAULT_TAG_TOTAL_MAX_BYTES,
    },
};
use radroots_identity::PublicKey;

pub const RADROOTS_NIP22_COMMENT_CONTENT_MAX_BYTES: usize = DEFAULT_CONTENT_MAX_BYTES;
pub const RADROOTS_NIP22_COMMENT_TAG_MAX_COUNT: usize = DEFAULT_TAG_MAX_COUNT;
pub const RADROOTS_NIP22_COMMENT_TAG_TOTAL_ELEMENT_MAX_COUNT: usize =
    DEFAULT_TAG_TOTAL_ELEMENT_MAX_COUNT;
pub const RADROOTS_NIP22_COMMENT_TAG_ELEMENT_MAX_BYTES: usize = DEFAULT_TAG_ELEMENT_MAX_BYTES;
pub const RADROOTS_NIP22_COMMENT_TAG_TOTAL_MAX_BYTES: usize = DEFAULT_TAG_TOTAL_MAX_BYTES;
pub const RADROOTS_NIP22_COMMENT_EVENT_WIRE_MAX_BYTES: usize = DEFAULT_RAW_JSON_MAX_BYTES;

const RADROOTS_NIP22_COMMENT_SIGNED_EVENT_FIXED_MAX_BYTES: usize = "{\"id\":\"".len()
    + 64
    + "\",\"pubkey\":\"".len()
    + 64
    + "\",\"created_at\":".len()
    + 20
    + ",\"kind\":1111,\"tags\":".len()
    + ",\"content\":".len()
    + ",\"sig\":\"".len()
    + 128
    + "\"}".len();

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Nip22CommentError {
    ContentMissing,
    ContentTooLarge { max: usize, actual: usize },
    RootEventIdInvalid(ParseError),
    RootAuthorInvalid(ParseError),
    RootCoordinateInvalid(ParseError),
    ParentEventIdInvalid(ParseError),
    ParentAuthorInvalid(ParseError),
    RevisionEventIdInvalid(ParseError),
    RootKindUnsupported { actual: u32 },
    RelayInvalid(ParseError),
    ParentReferenceMismatch,
    TagCountExceeded { max: usize, actual: usize },
    TagElementCountExceeded { max: usize, actual: usize },
    TagElementTooLarge { max: usize, actual: usize },
    TagBytesExceeded { max: usize, actual: usize },
    EventWireTooLarge { max: usize, actual: usize },
}

impl Nip22CommentError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::ContentMissing => "comment_content_missing",
            Self::ContentTooLarge { .. } => "comment_content_too_large",
            Self::RootEventIdInvalid(_) => "comment_root_event_id_invalid",
            Self::RootAuthorInvalid(_) => "comment_root_author_invalid",
            Self::RootCoordinateInvalid(_) => "comment_root_coordinate_invalid",
            Self::ParentEventIdInvalid(_) => "comment_parent_event_id_invalid",
            Self::ParentAuthorInvalid(_) => "comment_parent_author_invalid",
            Self::RevisionEventIdInvalid(_) => "comment_revision_event_id_invalid",
            Self::RootKindUnsupported { .. } => "comment_root_kind_unsupported",
            Self::RelayInvalid(_) => "comment_relay_invalid",
            Self::ParentReferenceMismatch => "comment_parent_reference_mismatch",
            Self::TagCountExceeded { .. } => "comment_tag_count_exceeded",
            Self::TagElementCountExceeded { .. } => "comment_tag_element_count_exceeded",
            Self::TagElementTooLarge { .. } => "comment_tag_element_too_large",
            Self::TagBytesExceeded { .. } => "comment_tag_bytes_exceeded",
            Self::EventWireTooLarge { .. } => "comment_event_wire_too_large",
        }
    }
}

impl fmt::Display for Nip22CommentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ContentMissing => {
                formatter.write_str("authored NIP-22 comment content must be non-whitespace")
            }
            Self::ContentTooLarge { max, actual } => write!(
                formatter,
                "authored NIP-22 comment content is {actual} bytes; max is {max}"
            ),
            Self::RootEventIdInvalid(error) => {
                write!(
                    formatter,
                    "NIP-22 comment root event id is invalid: {error}"
                )
            }
            Self::RootAuthorInvalid(error) => {
                write!(formatter, "NIP-22 comment root author is invalid: {error}")
            }
            Self::RootCoordinateInvalid(error) => {
                write!(
                    formatter,
                    "NIP-22 comment root coordinate is invalid: {error}"
                )
            }
            Self::ParentEventIdInvalid(error) => {
                write!(
                    formatter,
                    "NIP-22 comment parent event id is invalid: {error}"
                )
            }
            Self::ParentAuthorInvalid(error) => {
                write!(
                    formatter,
                    "NIP-22 comment parent author is invalid: {error}"
                )
            }
            Self::RevisionEventIdInvalid(error) => {
                write!(
                    formatter,
                    "NIP-22 comment revision event id is invalid: {error}"
                )
            }
            Self::RootKindUnsupported { actual } => write!(
                formatter,
                "NIP-22 comment root kind {actual} is outside the supported profile"
            ),
            Self::RelayInvalid(error) => {
                write!(formatter, "NIP-22 comment relay hint is invalid: {error}")
            }
            Self::ParentReferenceMismatch => {
                formatter.write_str("nested NIP-22 comment parent must differ from its event root")
            }
            Self::TagCountExceeded { max, actual } => write!(
                formatter,
                "authored NIP-22 comment has {actual} tags; max is {max}"
            ),
            Self::TagElementCountExceeded { max, actual } => write!(
                formatter,
                "authored NIP-22 comment has {actual} total tag elements; max is {max}"
            ),
            Self::TagElementTooLarge { max, actual } => write!(
                formatter,
                "authored NIP-22 comment tag element is {actual} bytes; max is {max}"
            ),
            Self::TagBytesExceeded { max, actual } => write!(
                formatter,
                "authored NIP-22 comment tag bytes are {actual}; max is {max}"
            ),
            Self::EventWireTooLarge { max, actual } => write!(
                formatter,
                "authored NIP-22 comment maximum canonical signed event size is {actual} bytes; max is {max}"
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Nip22CommentError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RootEventIdInvalid(error)
            | Self::RootAuthorInvalid(error)
            | Self::RootCoordinateInvalid(error)
            | Self::ParentEventIdInvalid(error)
            | Self::ParentAuthorInvalid(error)
            | Self::RevisionEventIdInvalid(error)
            | Self::RelayInvalid(error) => Some(error),
            _ => None,
        }
    }
}

/// Root event kinds supported by the strict Radroots NIP-22 profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Nip22CommentRootKind {
    Article,
    ClassifiedListing,
    CalendarDateEvent,
    CalendarTimeEvent,
}

impl Nip22CommentRootKind {
    pub const fn as_u32(self) -> u32 {
        match self {
            Self::Article => KIND_ARTICLE,
            Self::ClassifiedListing => KIND_CLASSIFIED_LISTING,
            Self::CalendarDateEvent => KIND_CALENDAR_DATE_EVENT,
            Self::CalendarTimeEvent => KIND_CALENDAR_TIME_EVENT,
        }
    }

    pub const fn parse(kind: u32) -> Result<Self, Nip22CommentError> {
        match kind {
            KIND_ARTICLE => Ok(Self::Article),
            KIND_CLASSIFIED_LISTING => Ok(Self::ClassifiedListing),
            KIND_CALENDAR_DATE_EVENT => Ok(Self::CalendarDateEvent),
            KIND_CALENDAR_TIME_EVENT => Ok(Self::CalendarTimeEvent),
            actual => Err(Nip22CommentError::RootKindUnsupported { actual }),
        }
    }
}

impl From<Nip22CommentRootKind> for u32 {
    fn from(value: Nip22CommentRootKind) -> Self {
        value.as_u32()
    }
}

impl TryFrom<u32> for Nip22CommentRootKind {
    type Error = Nip22CommentError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

/// One revision-specific NIP-22 event root.
///
/// The caller asserts the target kind and author. This does not prove event
/// existence, signature validity, actual authorship, or relay availability.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Nip22EventRootReference {
    event_id: EventId,
    author: PublicKey,
    kind: Nip22CommentRootKind,
    relay: Option<NostrRelayHint>,
}

impl Nip22EventRootReference {
    pub fn new(
        event_id: EventId,
        author: PublicKey,
        kind: Nip22CommentRootKind,
        relay: Option<NostrRelayHint>,
    ) -> Result<Self, Nip22CommentError> {
        validate_optional_relay(&relay)?;
        Ok(Self {
            event_id,
            author,
            kind,
            relay,
        })
    }

    pub fn parse(
        event_id: impl AsRef<str>,
        author: impl AsRef<str>,
        kind: u32,
        relay: Option<&str>,
    ) -> Result<Self, Nip22CommentError> {
        Self::new(
            EventId::parse(event_id).map_err(Nip22CommentError::RootEventIdInvalid)?,
            parse_public_key(author).map_err(Nip22CommentError::RootAuthorInvalid)?,
            Nip22CommentRootKind::parse(kind)?,
            parse_optional_relay(relay)?,
        )
    }

    pub const fn event_id(&self) -> &EventId {
        &self.event_id
    }

    pub const fn author(&self) -> &PublicKey {
        &self.author
    }

    pub const fn kind(&self) -> Nip22CommentRootKind {
        self.kind
    }

    pub const fn relay(&self) -> Option<&NostrRelayHint> {
        self.relay.as_ref()
    }

    pub fn relay_or_empty(&self) -> &str {
        relay_or_empty(self.relay())
    }
}

/// One coordinate-stable NIP-22 address root.
///
/// This proves coordinate syntax and the focused root-kind allowlist only. It
/// does not prove event existence, signature validity, a current revision, or
/// relay availability.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Nip22AddressRootReference {
    coordinate: AddressableCoordinate,
    author: PublicKey,
    kind: Nip22CommentRootKind,
    relay: Option<NostrRelayHint>,
}

impl Nip22AddressRootReference {
    pub fn new(
        coordinate: AddressableCoordinate,
        relay: Option<NostrRelayHint>,
    ) -> Result<Self, Nip22CommentError> {
        let parts = AddressableCoordinateParts::parse(coordinate.as_str())
            .map_err(Nip22CommentError::RootCoordinateInvalid)?;
        let kind = Nip22CommentRootKind::parse(parts.kind)?;
        let coordinate = AddressableCoordinate::parse(format!(
            "{}:{}:{}",
            kind.as_u32(),
            parts.pubkey,
            parts.d_tag
        ))
        .map_err(Nip22CommentError::RootCoordinateInvalid)?;
        validate_tag_element(coordinate.as_str())?;
        validate_optional_relay(&relay)?;
        Ok(Self {
            coordinate,
            author: parts.pubkey,
            kind,
            relay,
        })
    }

    pub fn parse(
        coordinate: impl AsRef<str>,
        relay: Option<&str>,
    ) -> Result<Self, Nip22CommentError> {
        Self::new(
            AddressableCoordinate::parse(coordinate)
                .map_err(Nip22CommentError::RootCoordinateInvalid)?,
            parse_optional_relay(relay)?,
        )
    }

    pub const fn coordinate(&self) -> &AddressableCoordinate {
        &self.coordinate
    }

    pub const fn author(&self) -> &PublicKey {
        &self.author
    }

    pub const fn kind(&self) -> Nip22CommentRootKind {
        self.kind
    }

    pub const fn relay(&self) -> Option<&NostrRelayHint> {
        self.relay.as_ref()
    }

    pub fn relay_or_empty(&self) -> &str {
        relay_or_empty(self.relay())
    }
}

/// One kind-1111 parent reference for a nested NIP-22 comment.
///
/// The caller asserts the target is a Comment by the stated author. This does
/// not prove event existence, target kind, signature validity, actual
/// authorship, or relay availability.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Nip22CommentParentReference {
    event_id: EventId,
    author: PublicKey,
    relay: Option<NostrRelayHint>,
}

impl Nip22CommentParentReference {
    pub fn new(
        event_id: EventId,
        author: PublicKey,
        relay: Option<NostrRelayHint>,
    ) -> Result<Self, Nip22CommentError> {
        validate_optional_relay(&relay)?;
        Ok(Self {
            event_id,
            author,
            relay,
        })
    }

    pub fn parse(
        event_id: impl AsRef<str>,
        author: impl AsRef<str>,
        relay: Option<&str>,
    ) -> Result<Self, Nip22CommentError> {
        Self::new(
            EventId::parse(event_id).map_err(Nip22CommentError::ParentEventIdInvalid)?,
            parse_public_key(author).map_err(Nip22CommentError::ParentAuthorInvalid)?,
            parse_optional_relay(relay)?,
        )
    }

    pub const fn event_id(&self) -> &EventId {
        &self.event_id
    }

    pub const fn author(&self) -> &PublicKey {
        &self.author
    }

    pub const fn relay(&self) -> Option<&NostrRelayHint> {
        self.relay.as_ref()
    }

    pub fn relay_or_empty(&self) -> &str {
        relay_or_empty(self.relay())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Nip22CommentRoot {
    Event(Nip22EventRootReference),
    Address(Nip22AddressRootReference),
}

impl Nip22CommentRoot {
    pub const fn kind(&self) -> Nip22CommentRootKind {
        match self {
            Self::Event(reference) => reference.kind(),
            Self::Address(reference) => reference.kind(),
        }
    }

    pub const fn author(&self) -> &PublicKey {
        match self {
            Self::Event(reference) => reference.author(),
            Self::Address(reference) => reference.author(),
        }
    }

    pub const fn relay(&self) -> Option<&NostrRelayHint> {
        match self {
            Self::Event(reference) => reference.relay(),
            Self::Address(reference) => reference.relay(),
        }
    }
}

impl From<Nip22EventRootReference> for Nip22CommentRoot {
    fn from(value: Nip22EventRootReference) -> Self {
        Self::Event(value)
    }
}

impl From<Nip22AddressRootReference> for Nip22CommentRoot {
    fn from(value: Nip22AddressRootReference) -> Self {
        Self::Address(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Nip22CommentPosition {
    TopLevelEvent,
    TopLevelAddress { current_revision: EventId },
    Nested { parent: Nip22CommentParentReference },
}

/// Strict authored kind-1111 NIP-22 comment.
///
/// This type is opaque and has no Serde construction path.
///
/// ```compile_fail
/// let _: radroots_event::post::comment::AuthoredNip22Comment =
///     serde_json::from_str(r#"{"content":"comment"}"#).unwrap();
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthoredNip22Comment {
    content: String,
    root: Nip22CommentRoot,
    position: Nip22CommentPosition,
}

impl AuthoredNip22Comment {
    pub fn top_level_event(
        content: impl Into<String>,
        root: Nip22EventRootReference,
    ) -> Result<Self, Nip22CommentError> {
        Self::new(
            content.into(),
            Nip22CommentRoot::Event(root),
            Nip22CommentPosition::TopLevelEvent,
        )
    }

    pub fn top_level_address(
        content: impl Into<String>,
        root: Nip22AddressRootReference,
        current_revision: EventId,
    ) -> Result<Self, Nip22CommentError> {
        Self::new(
            content.into(),
            Nip22CommentRoot::Address(root),
            Nip22CommentPosition::TopLevelAddress { current_revision },
        )
    }

    pub fn parse_top_level_address(
        content: impl Into<String>,
        root: Nip22AddressRootReference,
        current_revision: impl AsRef<str>,
    ) -> Result<Self, Nip22CommentError> {
        Self::top_level_address(
            content,
            root,
            EventId::parse(current_revision).map_err(Nip22CommentError::RevisionEventIdInvalid)?,
        )
    }

    pub fn nested(
        content: impl Into<String>,
        root: impl Into<Nip22CommentRoot>,
        parent: Nip22CommentParentReference,
    ) -> Result<Self, Nip22CommentError> {
        let root = root.into();
        if matches!(
            &root,
            Nip22CommentRoot::Event(reference)
                if reference.event_id() == parent.event_id()
        ) {
            return Err(Nip22CommentError::ParentReferenceMismatch);
        }
        Self::new(
            content.into(),
            root,
            Nip22CommentPosition::Nested { parent },
        )
    }

    fn new(
        content: String,
        root: Nip22CommentRoot,
        position: Nip22CommentPosition,
    ) -> Result<Self, Nip22CommentError> {
        validate_content(&content)?;
        validate_authored_comment_wire_size(&content, &root, &position)?;
        Ok(Self {
            content,
            root,
            position,
        })
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub const fn root(&self) -> &Nip22CommentRoot {
        &self.root
    }

    pub const fn position(&self) -> &Nip22CommentPosition {
        &self.position
    }
}

fn parse_optional_relay(relay: Option<&str>) -> Result<Option<NostrRelayHint>, Nip22CommentError> {
    match relay {
        None | Some("") => Ok(None),
        Some(relay) => {
            validate_tag_element(relay)?;
            NostrRelayHint::parse(relay)
                .map(Some)
                .map_err(Nip22CommentError::RelayInvalid)
        }
    }
}

fn validate_optional_relay(relay: &Option<NostrRelayHint>) -> Result<(), Nip22CommentError> {
    if let Some(relay) = relay {
        validate_tag_element(relay.as_str())?;
    }
    Ok(())
}

fn relay_or_empty(relay: Option<&NostrRelayHint>) -> &str {
    relay.map_or("", NostrRelayHint::as_str)
}

fn validate_content(content: &str) -> Result<(), Nip22CommentError> {
    crate::require_invariant(!content.trim().is_empty(), &|| {
        Nip22CommentError::ContentMissing
    })?;
    crate::require_invariant(
        content.len() <= RADROOTS_NIP22_COMMENT_CONTENT_MAX_BYTES,
        &|| Nip22CommentError::ContentTooLarge {
            max: RADROOTS_NIP22_COMMENT_CONTENT_MAX_BYTES,
            actual: content.len(),
        },
    )
}

fn validate_tag_element(element: &str) -> Result<(), Nip22CommentError> {
    crate::require_invariant(
        element.len() <= RADROOTS_NIP22_COMMENT_TAG_ELEMENT_MAX_BYTES,
        &|| Nip22CommentError::TagElementTooLarge {
            max: RADROOTS_NIP22_COMMENT_TAG_ELEMENT_MAX_BYTES,
            actual: element.len(),
        },
    )
}

fn validate_authored_comment_wire_size(
    content: &str,
    root: &Nip22CommentRoot,
    position: &Nip22CommentPosition,
) -> Result<(), Nip22CommentError> {
    let mut tag_bytes = 0usize;
    let mut tags_json_bytes = 2usize;
    let mut tag_count = 0usize;
    let root_kind = root.kind().as_u32().to_string();

    let root_tag_element_count = match root {
        Nip22CommentRoot::Event(reference) => {
            add_tag(
                &mut tag_bytes,
                &mut tags_json_bytes,
                &mut tag_count,
                &[
                    "E",
                    reference.event_id().to_hex().as_str(),
                    reference.relay_or_empty(),
                    reference.author().to_hex().as_str(),
                ],
            );
            8usize + usize::from(reference.relay().is_some())
        }
        Nip22CommentRoot::Address(reference) => {
            add_optional_relay_tag(
                &mut tag_bytes,
                &mut tags_json_bytes,
                &mut tag_count,
                "A",
                reference.coordinate().as_str(),
                reference.relay(),
            );
            6usize + 2 * usize::from(reference.relay().is_some())
        }
    };
    add_tag(
        &mut tag_bytes,
        &mut tags_json_bytes,
        &mut tag_count,
        &["K", root_kind.as_str()],
    );
    add_optional_relay_tag(
        &mut tag_bytes,
        &mut tags_json_bytes,
        &mut tag_count,
        "P",
        root.author().to_hex().as_str(),
        root.relay(),
    );

    let position_tag_element_count = match (root, position) {
        (Nip22CommentRoot::Event(reference), Nip22CommentPosition::TopLevelEvent) => {
            add_tag(
                &mut tag_bytes,
                &mut tags_json_bytes,
                &mut tag_count,
                &[
                    "e",
                    reference.event_id().to_hex().as_str(),
                    reference.relay_or_empty(),
                    reference.author().to_hex().as_str(),
                ],
            );
            add_tag(
                &mut tag_bytes,
                &mut tags_json_bytes,
                &mut tag_count,
                &["k", root_kind.as_str()],
            );
            add_optional_relay_tag(
                &mut tag_bytes,
                &mut tags_json_bytes,
                &mut tag_count,
                "p",
                reference.author().to_hex().as_str(),
                reference.relay(),
            );
            8usize + usize::from(reference.relay().is_some())
        }
        (
            Nip22CommentRoot::Address(reference),
            Nip22CommentPosition::TopLevelAddress { current_revision },
        ) => {
            add_optional_relay_tag(
                &mut tag_bytes,
                &mut tags_json_bytes,
                &mut tag_count,
                "a",
                reference.coordinate().as_str(),
                reference.relay(),
            );
            add_optional_relay_tag(
                &mut tag_bytes,
                &mut tags_json_bytes,
                &mut tag_count,
                "e",
                current_revision.to_hex().as_str(),
                reference.relay(),
            );
            add_tag(
                &mut tag_bytes,
                &mut tags_json_bytes,
                &mut tag_count,
                &["k", root_kind.as_str()],
            );
            add_optional_relay_tag(
                &mut tag_bytes,
                &mut tags_json_bytes,
                &mut tag_count,
                "p",
                reference.author().to_hex().as_str(),
                reference.relay(),
            );
            8usize + 3 * usize::from(reference.relay().is_some())
        }
        (_, Nip22CommentPosition::Nested { parent }) => {
            add_tag(
                &mut tag_bytes,
                &mut tags_json_bytes,
                &mut tag_count,
                &[
                    "e",
                    parent.event_id().to_hex().as_str(),
                    parent.relay_or_empty(),
                    parent.author().to_hex().as_str(),
                ],
            );
            add_tag(
                &mut tag_bytes,
                &mut tags_json_bytes,
                &mut tag_count,
                &["k", KIND_COMMENT.to_string().as_str()],
            );
            add_optional_relay_tag(
                &mut tag_bytes,
                &mut tags_json_bytes,
                &mut tag_count,
                "p",
                parent.author().to_hex().as_str(),
                parent.relay(),
            );
            8usize + usize::from(parent.relay().is_some())
        }
        _ => unreachable!("constructors preserve root and position compatibility"),
    };

    crate::require_invariant(tag_count <= RADROOTS_NIP22_COMMENT_TAG_MAX_COUNT, &|| {
        Nip22CommentError::TagCountExceeded {
            max: RADROOTS_NIP22_COMMENT_TAG_MAX_COUNT,
            actual: tag_count,
        }
    })?;
    let tag_element_count = root_tag_element_count.saturating_add(position_tag_element_count);
    crate::require_invariant(
        tag_element_count <= RADROOTS_NIP22_COMMENT_TAG_TOTAL_ELEMENT_MAX_COUNT,
        &|| Nip22CommentError::TagElementCountExceeded {
            max: RADROOTS_NIP22_COMMENT_TAG_TOTAL_ELEMENT_MAX_COUNT,
            actual: tag_element_count,
        },
    )?;

    crate::require_invariant(
        tag_bytes <= RADROOTS_NIP22_COMMENT_TAG_TOTAL_MAX_BYTES,
        &|| Nip22CommentError::TagBytesExceeded {
            max: RADROOTS_NIP22_COMMENT_TAG_TOTAL_MAX_BYTES,
            actual: tag_bytes,
        },
    )?;
    let actual = RADROOTS_NIP22_COMMENT_SIGNED_EVENT_FIXED_MAX_BYTES
        .saturating_add(tags_json_bytes)
        .saturating_add(canonical_json_string_bytes(content));
    crate::require_invariant(
        actual <= RADROOTS_NIP22_COMMENT_EVENT_WIRE_MAX_BYTES,
        &|| Nip22CommentError::EventWireTooLarge {
            max: RADROOTS_NIP22_COMMENT_EVENT_WIRE_MAX_BYTES,
            actual,
        },
    )
}

fn add_optional_relay_tag(
    tag_bytes: &mut usize,
    tags_json_bytes: &mut usize,
    tag_count: &mut usize,
    name: &str,
    value: &str,
    relay: Option<&NostrRelayHint>,
) {
    if let Some(relay) = relay {
        add_tag(
            tag_bytes,
            tags_json_bytes,
            tag_count,
            &[name, value, relay.as_str()],
        );
    } else {
        add_tag(tag_bytes, tags_json_bytes, tag_count, &[name, value]);
    }
}

fn add_tag(
    tag_bytes: &mut usize,
    tags_json_bytes: &mut usize,
    tag_count: &mut usize,
    elements: &[&str],
) {
    if *tag_count > 0 {
        *tags_json_bytes = tags_json_bytes.saturating_add(1);
    }
    *tags_json_bytes = tags_json_bytes.saturating_add(2);
    for (index, element) in elements.iter().enumerate() {
        if index > 0 {
            *tags_json_bytes = tags_json_bytes.saturating_add(1);
        }
        *tags_json_bytes = tags_json_bytes.saturating_add(canonical_json_string_bytes(element));
        *tag_bytes = tag_bytes.saturating_add(element.len());
    }
    *tag_count = tag_count.saturating_add(1);
}

fn canonical_json_string_bytes(value: &str) -> usize {
    value.chars().fold(2usize, |total, character| {
        total.saturating_add(match character {
            '"' | '\\' | '\u{0008}' | '\t' | '\n' | '\u{000c}' | '\r' => 2,
            '\u{0000}'..='\u{001f}' => 6,
            _ => character.len_utf8(),
        })
    })
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    fn event_root(kind: u32) -> Nip22EventRootReference {
        Nip22EventRootReference::parse(
            "a".repeat(64),
            crate::test_valid_hex_64('b'),
            kind,
            Some("wss://relay.example"),
        )
        .expect("event root")
    }

    fn address_root(kind: u32) -> Nip22AddressRootReference {
        Nip22AddressRootReference::parse(
            format!("{kind}:{}:victoria-market", crate::test_valid_hex_64('b')),
            Some("wss://relay.example"),
        )
        .expect("address root")
    }

    fn parent() -> Nip22CommentParentReference {
        Nip22CommentParentReference::parse(
            "c".repeat(64),
            "d".repeat(64),
            Some("wss://comments.example"),
        )
        .expect("parent")
    }

    #[test]
    fn supports_each_root_kind_for_event_and_address_roots() {
        for kind in [
            KIND_ARTICLE,
            KIND_CLASSIFIED_LISTING,
            KIND_CALENDAR_DATE_EVENT,
            KIND_CALENDAR_TIME_EVENT,
        ] {
            AuthoredNip22Comment::top_level_event("Comment", event_root(kind))
                .expect("event comment");
            AuthoredNip22Comment::parse_top_level_address(
                "Comment",
                address_root(kind),
                "e".repeat(64),
            )
            .expect("address comment");
            AuthoredNip22Comment::nested("Reply", event_root(kind), parent())
                .expect("nested event comment");
            AuthoredNip22Comment::nested("Reply", address_root(kind), parent())
                .expect("nested address comment");
        }
    }

    #[test]
    fn rejects_other_root_kinds_and_ambiguous_parent() {
        for kind in [1, KIND_COMMENT, 30_018] {
            assert!(matches!(
                Nip22EventRootReference::parse(
                    "a".repeat(64),
                    crate::test_valid_hex_64('b'),
                    kind,
                    None
                ),
                Err(Nip22CommentError::RootKindUnsupported { actual }) if actual == kind
            ));
        }

        let root = event_root(KIND_CLASSIFIED_LISTING);
        let parent =
            Nip22CommentParentReference::parse(root.event_id().to_hex(), "d".repeat(64), None)
                .expect("parent");
        assert_eq!(
            AuthoredNip22Comment::nested("Reply", root, parent).unwrap_err(),
            Nip22CommentError::ParentReferenceMismatch
        );
    }

    #[test]
    fn canonicalizes_address_author_and_requires_current_revision() {
        let root = Nip22AddressRootReference::parse(
            format!("30402:{}:listing", crate::test_valid_hex_64('B')),
            None,
        )
        .expect("address root");
        assert_eq!(
            root.coordinate().as_str(),
            format!("30402:{}:listing", crate::test_valid_hex_64('b'))
        );
        assert_eq!(
            root.author().to_hex().as_str(),
            crate::test_valid_hex_64('b')
        );

        let comment =
            AuthoredNip22Comment::parse_top_level_address("Comment", root, "E".repeat(64))
                .expect("top-level address");
        assert!(matches!(
            comment.position(),
            Nip22CommentPosition::TopLevelAddress { current_revision }
                if current_revision.to_hex() == "e".repeat(64)
        ));
    }

    #[test]
    fn address_root_rechecks_the_canonical_coordinate_tag_element() {
        let maximum_d_tag = "x".repeat(512);
        let root = Nip22AddressRootReference::parse(
            format!("30402:{}:{maximum_d_tag}", crate::test_valid_hex_64('b')),
            None,
        )
        .expect("maximum public d tag fits the tag-element budget");
        assert!(root.coordinate().as_str().len() <= RADROOTS_NIP22_COMMENT_TAG_ELEMENT_MAX_BYTES);

        let oversized = format!(
            "30402:{}:{}",
            crate::test_valid_hex_64('b'),
            "x".repeat(513)
        );
        assert!(matches!(
            Nip22AddressRootReference::parse(oversized, None),
            Err(Nip22CommentError::RootCoordinateInvalid(
                ParseError::TooLong {
                    max: 512,
                    actual: 513
                }
            ))
        ));
    }

    #[test]
    fn relay_element_budget_precedes_relay_syntax() {
        let oversized_noncanonical_relay = format!("WSS://relay.example/{}", "x".repeat(4096));
        assert!(matches!(
            Nip22EventRootReference::parse(
                "a".repeat(64),
                crate::test_valid_hex_64('b'),
                KIND_CLASSIFIED_LISTING,
                Some(&oversized_noncanonical_relay),
            ),
            Err(Nip22CommentError::TagElementTooLarge {
                max: RADROOTS_NIP22_COMMENT_TAG_ELEMENT_MAX_BYTES,
                actual,
            }) if actual == oversized_noncanonical_relay.len()
        ));
    }

    #[test]
    fn enforces_content_and_relay_element_boundaries() {
        AuthoredNip22Comment::top_level_event(
            "x".repeat(RADROOTS_NIP22_COMMENT_CONTENT_MAX_BYTES),
            event_root(KIND_CLASSIFIED_LISTING),
        )
        .expect("exact content limit");
        assert!(matches!(
            AuthoredNip22Comment::top_level_event(
                "x".repeat(RADROOTS_NIP22_COMMENT_CONTENT_MAX_BYTES + 1),
                event_root(KIND_CLASSIFIED_LISTING)
            ),
            Err(Nip22CommentError::ContentTooLarge { max, actual })
                if max == RADROOTS_NIP22_COMMENT_CONTENT_MAX_BYTES
                    && actual == max + 1
        ));

        let prefix = "wss://relay.example/";
        let exact_relay = format!(
            "{prefix}{}",
            "a".repeat(RADROOTS_NIP22_COMMENT_TAG_ELEMENT_MAX_BYTES - prefix.len())
        );
        Nip22EventRootReference::parse(
            "a".repeat(64),
            crate::test_valid_hex_64('b'),
            KIND_CLASSIFIED_LISTING,
            Some(&exact_relay),
        )
        .expect("exact tag-element limit");
        let overflow = format!("{exact_relay}a");
        assert!(matches!(
            Nip22EventRootReference::parse(
                "a".repeat(64),
                crate::test_valid_hex_64('b'),
                KIND_CLASSIFIED_LISTING,
                Some(&overflow)
            ),
            Err(Nip22CommentError::TagElementTooLarge { max, actual })
                if max == RADROOTS_NIP22_COMMENT_TAG_ELEMENT_MAX_BYTES
                    && actual == max + 1
        ));
    }

    #[test]
    fn uses_unicode_white_space_for_blank_content() {
        AuthoredNip22Comment::top_level_event("\u{001c}", event_root(KIND_CLASSIFIED_LISTING))
            .expect("U+001C is not Unicode White_Space");
        assert_eq!(
            AuthoredNip22Comment::top_level_event("\u{00a0}", event_root(KIND_CLASSIFIED_LISTING))
                .unwrap_err(),
            Nip22CommentError::ContentMissing
        );
    }

    #[test]
    fn escaped_content_cannot_cross_compact_signed_wire_limit() {
        let mut lower = 1usize;
        let mut upper = RADROOTS_NIP22_COMMENT_CONTENT_MAX_BYTES;
        while lower < upper {
            let candidate = lower + (upper - lower).div_ceil(2);
            if AuthoredNip22Comment::top_level_event(
                "\u{0001}".repeat(candidate),
                event_root(KIND_CLASSIFIED_LISTING),
            )
            .is_ok()
            {
                lower = candidate;
            } else {
                upper = candidate - 1;
            }
        }

        AuthoredNip22Comment::top_level_event(
            "\u{0001}".repeat(lower),
            event_root(KIND_CLASSIFIED_LISTING),
        )
        .expect("largest escaped content fitting the wire budget");
        assert!(matches!(
            AuthoredNip22Comment::top_level_event(
                "\u{0001}".repeat(lower + 1),
                event_root(KIND_CLASSIFIED_LISTING)
            ),
            Err(Nip22CommentError::EventWireTooLarge { max, .. })
                if max == RADROOTS_NIP22_COMMENT_EVENT_WIRE_MAX_BYTES
        ));
    }
}
