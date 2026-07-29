//! Frozen NIP-22 comment semantics for event-contract registry v7.

#[cfg(not(feature = "std"))]
use alloc::{format, string::String, vec::Vec};
use core::fmt;

use radroots_event::{
    envelope::kind::KIND_COMMENT,
    id::{
        RadrootsAddressableCoordinate, RadrootsAddressableCoordinateParts, RadrootsEventId,
        RadrootsIdParseError,
    },
    post::comment::{
        RADROOTS_NIP22_COMMENT_CONTENT_MAX_BYTES, RADROOTS_NIP22_COMMENT_EVENT_WIRE_MAX_BYTES,
        RADROOTS_NIP22_COMMENT_TAG_ELEMENT_MAX_BYTES, RADROOTS_NIP22_COMMENT_TAG_MAX_COUNT,
        RADROOTS_NIP22_COMMENT_TAG_TOTAL_ELEMENT_MAX_COUNT,
        RADROOTS_NIP22_COMMENT_TAG_TOTAL_MAX_BYTES, RadrootsNip22CommentRootKind,
    },
    tag::relay_hint::RadrootsNostrRelayHint,
};
use radroots_identity::PublicKey;

use crate::verification::v1::RadrootsSignatureVerifiedEvent;

const RADROOTS_NIP22_COMMENT_SIGNED_EVENT_FIXED_MAX_BYTES: usize = "{\"id\":\"".len()
    + 64
    + "\",\"pubkey\":\"".len()
    + 64
    + "\",\"created_at\":".len()
    + ",\"kind\":1111,\"tags\":".len()
    + ",\"content\":".len()
    + ",\"sig\":\"".len()
    + 128
    + "\"}".len();

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RadrootsNip22CommentDiagnostic {
    RootRelayIgnored {
        tag_index: usize,
        raw_tag: Vec<String>,
    },
    RootAuthorRelayIgnored {
        tag_index: usize,
        raw_tag: Vec<String>,
    },
    RootAuthorHintIgnored {
        tag_index: usize,
        raw_tag: Vec<String>,
    },
    ParentRelayIgnored {
        tag_index: usize,
        raw_tag: Vec<String>,
    },
    ParentAuthorRelayIgnored {
        tag_index: usize,
        raw_tag: Vec<String>,
    },
    ParentAuthorHintIgnored {
        tag_index: usize,
        raw_tag: Vec<String>,
    },
    ParentAuthorShapeIgnored {
        tag_index: usize,
        raw_tag: Vec<String>,
    },
    ParentAuthorInvalidIgnored {
        tag_index: usize,
        raw_tag: Vec<String>,
    },
    ParentAuthorDuplicateIgnored {
        tag_index: usize,
        raw_tag: Vec<String>,
    },
    RevisionRelayIgnored {
        tag_index: usize,
        raw_tag: Vec<String>,
    },
}

impl RadrootsNip22CommentDiagnostic {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::RootRelayIgnored { .. } => "comment_root_relay_ignored",
            Self::RootAuthorRelayIgnored { .. } => "comment_root_author_relay_ignored",
            Self::RootAuthorHintIgnored { .. } => "comment_root_author_hint_ignored",
            Self::ParentRelayIgnored { .. } => "comment_parent_relay_ignored",
            Self::ParentAuthorRelayIgnored { .. } => "comment_parent_author_relay_ignored",
            Self::ParentAuthorHintIgnored { .. } => "comment_parent_author_hint_ignored",
            Self::ParentAuthorShapeIgnored { .. } => "comment_parent_author_shape_ignored",
            Self::ParentAuthorInvalidIgnored { .. } => "comment_parent_author_invalid_ignored",
            Self::ParentAuthorDuplicateIgnored { .. } => "comment_parent_author_duplicate_ignored",
            Self::RevisionRelayIgnored { .. } => "comment_revision_relay_ignored",
        }
    }

    pub const fn tag_index(&self) -> usize {
        match self {
            Self::RootRelayIgnored { tag_index, .. }
            | Self::RootAuthorRelayIgnored { tag_index, .. }
            | Self::RootAuthorHintIgnored { tag_index, .. }
            | Self::ParentRelayIgnored { tag_index, .. }
            | Self::ParentAuthorRelayIgnored { tag_index, .. }
            | Self::ParentAuthorHintIgnored { tag_index, .. }
            | Self::ParentAuthorShapeIgnored { tag_index, .. }
            | Self::ParentAuthorInvalidIgnored { tag_index, .. }
            | Self::ParentAuthorDuplicateIgnored { tag_index, .. }
            | Self::RevisionRelayIgnored { tag_index, .. } => *tag_index,
        }
    }

    pub fn raw_tag(&self) -> &[String] {
        match self {
            Self::RootRelayIgnored { raw_tag, .. }
            | Self::RootAuthorRelayIgnored { raw_tag, .. }
            | Self::RootAuthorHintIgnored { raw_tag, .. }
            | Self::ParentRelayIgnored { raw_tag, .. }
            | Self::ParentAuthorRelayIgnored { raw_tag, .. }
            | Self::ParentAuthorHintIgnored { raw_tag, .. }
            | Self::ParentAuthorShapeIgnored { raw_tag, .. }
            | Self::ParentAuthorInvalidIgnored { raw_tag, .. }
            | Self::ParentAuthorDuplicateIgnored { raw_tag, .. }
            | Self::RevisionRelayIgnored { raw_tag, .. } => raw_tag,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsInboundNip22Participant {
    tag_index: usize,
    pubkey: PublicKey,
    relay: Option<RadrootsNostrRelayHint>,
    raw_tag: Vec<String>,
}

impl RadrootsInboundNip22Participant {
    pub const fn tag_index(&self) -> usize {
        self.tag_index
    }

    pub const fn pubkey(&self) -> &PublicKey {
        &self.pubkey
    }

    pub const fn relay(&self) -> Option<&RadrootsNostrRelayHint> {
        self.relay.as_ref()
    }

    pub fn raw_tag(&self) -> &[String] {
        &self.raw_tag
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsInboundNip22EventRoot {
    tag_index: usize,
    event_id: RadrootsEventId,
    kind_tag_index: usize,
    kind_raw_tag: Vec<String>,
    kind: RadrootsNip22CommentRootKind,
    relay: Option<RadrootsNostrRelayHint>,
    author_hint: Option<PublicKey>,
    author: RadrootsInboundNip22Participant,
    raw_tag: Vec<String>,
}

impl RadrootsInboundNip22EventRoot {
    pub const fn tag_index(&self) -> usize {
        self.tag_index
    }

    pub const fn event_id(&self) -> &RadrootsEventId {
        &self.event_id
    }

    pub const fn kind_tag_index(&self) -> usize {
        self.kind_tag_index
    }

    pub fn kind_raw_tag(&self) -> &[String] {
        &self.kind_raw_tag
    }

    pub const fn kind(&self) -> RadrootsNip22CommentRootKind {
        self.kind
    }

    pub const fn relay(&self) -> Option<&RadrootsNostrRelayHint> {
        self.relay.as_ref()
    }

    pub const fn author_hint(&self) -> Option<&PublicKey> {
        self.author_hint.as_ref()
    }

    pub const fn author(&self) -> &RadrootsInboundNip22Participant {
        &self.author
    }

    pub fn raw_tag(&self) -> &[String] {
        &self.raw_tag
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsInboundNip22AddressRoot {
    tag_index: usize,
    coordinate: RadrootsAddressableCoordinate,
    kind_tag_index: usize,
    kind_raw_tag: Vec<String>,
    kind: RadrootsNip22CommentRootKind,
    relay: Option<RadrootsNostrRelayHint>,
    author: RadrootsInboundNip22Participant,
    raw_tag: Vec<String>,
}

impl RadrootsInboundNip22AddressRoot {
    pub const fn tag_index(&self) -> usize {
        self.tag_index
    }

    pub const fn coordinate(&self) -> &RadrootsAddressableCoordinate {
        &self.coordinate
    }

    pub const fn kind_tag_index(&self) -> usize {
        self.kind_tag_index
    }

    pub fn kind_raw_tag(&self) -> &[String] {
        &self.kind_raw_tag
    }

    pub const fn kind(&self) -> RadrootsNip22CommentRootKind {
        self.kind
    }

    pub const fn relay(&self) -> Option<&RadrootsNostrRelayHint> {
        self.relay.as_ref()
    }

    pub const fn author(&self) -> &RadrootsInboundNip22Participant {
        &self.author
    }

    pub fn raw_tag(&self) -> &[String] {
        &self.raw_tag
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsInboundNip22CommentRoot {
    Event(RadrootsInboundNip22EventRoot),
    Address(RadrootsInboundNip22AddressRoot),
}

impl RadrootsInboundNip22CommentRoot {
    pub const fn author(&self) -> &RadrootsInboundNip22Participant {
        match self {
            Self::Event(root) => root.author(),
            Self::Address(root) => root.author(),
        }
    }

    pub const fn kind(&self) -> RadrootsNip22CommentRootKind {
        match self {
            Self::Event(root) => root.kind(),
            Self::Address(root) => root.kind(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsInboundNip22CommentParent {
    tag_index: usize,
    event_id: RadrootsEventId,
    kind_tag_index: usize,
    kind_raw_tag: Vec<String>,
    kind: u32,
    relay: Option<RadrootsNostrRelayHint>,
    author_hint: Option<PublicKey>,
    author: RadrootsInboundNip22Participant,
    raw_tag: Vec<String>,
}

impl RadrootsInboundNip22CommentParent {
    pub const fn tag_index(&self) -> usize {
        self.tag_index
    }

    pub const fn event_id(&self) -> &RadrootsEventId {
        &self.event_id
    }

    pub const fn kind_tag_index(&self) -> usize {
        self.kind_tag_index
    }

    pub fn kind_raw_tag(&self) -> &[String] {
        &self.kind_raw_tag
    }

    pub const fn kind(&self) -> u32 {
        self.kind
    }

    pub const fn relay(&self) -> Option<&RadrootsNostrRelayHint> {
        self.relay.as_ref()
    }

    pub const fn author_hint(&self) -> Option<&PublicKey> {
        self.author_hint.as_ref()
    }

    pub const fn author(&self) -> &RadrootsInboundNip22Participant {
        &self.author
    }

    pub fn raw_tag(&self) -> &[String] {
        &self.raw_tag
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsInboundNip22CurrentRevision {
    tag_index: usize,
    event_id: RadrootsEventId,
    relay: Option<RadrootsNostrRelayHint>,
    raw_tag: Vec<String>,
}

impl RadrootsInboundNip22CurrentRevision {
    pub const fn tag_index(&self) -> usize {
        self.tag_index
    }

    pub const fn event_id(&self) -> &RadrootsEventId {
        &self.event_id
    }

    pub const fn relay(&self) -> Option<&RadrootsNostrRelayHint> {
        self.relay.as_ref()
    }

    pub fn raw_tag(&self) -> &[String] {
        &self.raw_tag
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsInboundNip22TopLevelEventReference {
    tag_index: usize,
    event_id: RadrootsEventId,
    kind_tag_index: usize,
    kind_raw_tag: Vec<String>,
    kind: u32,
    relay: Option<RadrootsNostrRelayHint>,
    author_hint: Option<PublicKey>,
    author: RadrootsInboundNip22Participant,
    raw_tag: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsInboundNip22TopLevelAddressReference {
    tag_index: usize,
    coordinate: RadrootsAddressableCoordinate,
    kind_tag_index: usize,
    kind_raw_tag: Vec<String>,
    kind: u32,
    relay: Option<RadrootsNostrRelayHint>,
    author: RadrootsInboundNip22Participant,
    raw_tag: Vec<String>,
}

impl RadrootsInboundNip22TopLevelAddressReference {
    pub const fn tag_index(&self) -> usize {
        self.tag_index
    }

    pub const fn coordinate(&self) -> &RadrootsAddressableCoordinate {
        &self.coordinate
    }

    pub const fn kind_tag_index(&self) -> usize {
        self.kind_tag_index
    }

    pub fn kind_raw_tag(&self) -> &[String] {
        &self.kind_raw_tag
    }

    pub const fn kind(&self) -> u32 {
        self.kind
    }

    pub const fn relay(&self) -> Option<&RadrootsNostrRelayHint> {
        self.relay.as_ref()
    }

    pub const fn author(&self) -> &RadrootsInboundNip22Participant {
        &self.author
    }

    pub fn raw_tag(&self) -> &[String] {
        &self.raw_tag
    }
}

impl RadrootsInboundNip22TopLevelEventReference {
    pub const fn tag_index(&self) -> usize {
        self.tag_index
    }

    pub const fn event_id(&self) -> &RadrootsEventId {
        &self.event_id
    }

    pub const fn kind_tag_index(&self) -> usize {
        self.kind_tag_index
    }

    pub fn kind_raw_tag(&self) -> &[String] {
        &self.kind_raw_tag
    }

    pub const fn kind(&self) -> u32 {
        self.kind
    }

    pub const fn relay(&self) -> Option<&RadrootsNostrRelayHint> {
        self.relay.as_ref()
    }

    pub const fn author_hint(&self) -> Option<&PublicKey> {
        self.author_hint.as_ref()
    }

    pub const fn author(&self) -> &RadrootsInboundNip22Participant {
        &self.author
    }

    pub fn raw_tag(&self) -> &[String] {
        &self.raw_tag
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsInboundNip22CommentPosition {
    TopLevelEvent {
        reference: RadrootsInboundNip22TopLevelEventReference,
    },
    TopLevelAddress {
        reference: RadrootsInboundNip22TopLevelAddressReference,
        current_revision: RadrootsInboundNip22CurrentRevision,
    },
    Nested {
        parent: RadrootsInboundNip22CommentParent,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsInboundNip22CommentProjection {
    root: RadrootsInboundNip22CommentRoot,
    position: RadrootsInboundNip22CommentPosition,
    mentions: Vec<RadrootsInboundNip22Participant>,
    diagnostics: Vec<RadrootsNip22CommentDiagnostic>,
    raw_tags: Vec<Vec<String>>,
}

impl RadrootsInboundNip22CommentProjection {
    pub const fn root(&self) -> &RadrootsInboundNip22CommentRoot {
        &self.root
    }

    pub const fn position(&self) -> &RadrootsInboundNip22CommentPosition {
        &self.position
    }

    /// Valid distinct lowercase `p` mentions other than the selected parent
    /// author, in source order.
    pub fn mentions(&self) -> &[RadrootsInboundNip22Participant] {
        &self.mentions
    }

    pub const fn is_direct(&self) -> bool {
        !matches!(
            &self.position,
            RadrootsInboundNip22CommentPosition::Nested { .. }
        )
    }

    pub fn diagnostics(&self) -> &[RadrootsNip22CommentDiagnostic] {
        &self.diagnostics
    }

    /// Exact inbound tags, including supplemental tags, in source order.
    pub fn raw_tags(&self) -> &[Vec<String>] {
        &self.raw_tags
    }

    pub const fn contract_id(&self) -> &'static str {
        "radroots.social.comment.v1"
    }
}

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsNip22CommentProjectionError {
    UnsupportedKind {
        actual: u32,
    },
    ContentMissing,
    ContentTooLarge {
        max: usize,
        actual: usize,
    },
    RootFormUnsupported {
        tag_index: usize,
    },
    ParentFormUnsupported {
        tag_index: usize,
    },
    RootCardinality {
        actual: usize,
    },
    RootKindCardinality {
        actual: usize,
    },
    RootAuthorCardinality {
        actual: usize,
    },
    ParentKindCardinality {
        actual: usize,
    },
    RootKindUnsupported {
        tag_index: usize,
    },
    RootAuthorInvalid {
        tag_index: usize,
    },
    RootReferenceShape {
        tag_index: usize,
    },
    RootEventIdInvalid {
        tag_index: usize,
        error: RadrootsIdParseError,
    },
    RootCoordinateInvalid {
        tag_index: usize,
    },
    RootKindMismatch {
        tag_index: usize,
    },
    RootAuthorMismatch {
        tag_index: usize,
    },
    ParentKindInvalid {
        tag_index: usize,
    },
    ParentCardinality {
        event_count: usize,
        address_count: usize,
    },
    ParentReferenceShape {
        tag_index: usize,
    },
    ParentEventIdInvalid {
        tag_index: usize,
        error: RadrootsIdParseError,
    },
    ParentCoordinateInvalid {
        tag_index: usize,
    },
    ParentReferenceMismatch {
        tag_index: usize,
    },
    RevisionMissing {
        actual: usize,
    },
    RevisionShape {
        tag_index: usize,
    },
    RevisionEventIdInvalid {
        tag_index: usize,
        error: RadrootsIdParseError,
    },
    ParentAuthorMissing,
    ParentAuthorAmbiguous,
    ParentAuthorMismatch,
    TagCountExceeded {
        max: usize,
        actual: usize,
    },
    TagElementCountExceeded {
        max: usize,
        actual: usize,
    },
    TagElementTooLarge {
        max: usize,
        actual: usize,
        tag_index: usize,
        element_index: usize,
    },
    TagBytesExceeded {
        max: usize,
        actual: usize,
    },
    EventWireTooLarge {
        max: usize,
        actual: usize,
    },
}

impl RadrootsNip22CommentProjectionError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::UnsupportedKind { .. } => "unsupported_kind",
            Self::ContentMissing => "comment_content_missing",
            Self::ContentTooLarge { .. } => "comment_content_too_large",
            Self::RootFormUnsupported { .. } => "comment_root_form_unsupported",
            Self::ParentFormUnsupported { .. } => "comment_parent_form_unsupported",
            Self::RootCardinality { .. } => "comment_root_cardinality",
            Self::RootKindCardinality { .. } => "comment_root_kind_cardinality",
            Self::RootAuthorCardinality { .. } => "comment_root_author_cardinality",
            Self::ParentKindCardinality { .. } => "comment_parent_kind_cardinality",
            Self::RootKindUnsupported { .. } => "comment_root_kind_unsupported",
            Self::RootAuthorInvalid { .. } => "comment_root_author_invalid",
            Self::RootReferenceShape { .. } => "comment_root_reference_shape",
            Self::RootEventIdInvalid { .. } => "comment_root_event_id_invalid",
            Self::RootCoordinateInvalid { .. } => "comment_root_coordinate_invalid",
            Self::RootKindMismatch { .. } => "comment_root_kind_mismatch",
            Self::RootAuthorMismatch { .. } => "comment_root_author_mismatch",
            Self::ParentKindInvalid { .. } => "comment_parent_kind_invalid",
            Self::ParentCardinality { .. } => "comment_parent_cardinality",
            Self::ParentReferenceShape { .. } => "comment_parent_reference_shape",
            Self::ParentEventIdInvalid { .. } => "comment_parent_event_id_invalid",
            Self::ParentCoordinateInvalid { .. } => "comment_parent_coordinate_invalid",
            Self::ParentReferenceMismatch { .. } => "comment_parent_reference_mismatch",
            Self::RevisionMissing { .. } => "comment_revision_missing",
            Self::RevisionShape { .. } => "comment_revision_shape",
            Self::RevisionEventIdInvalid { .. } => "comment_revision_event_id_invalid",
            Self::ParentAuthorMissing => "comment_parent_author_missing",
            Self::ParentAuthorAmbiguous => "comment_parent_author_ambiguous",
            Self::ParentAuthorMismatch => "comment_parent_author_mismatch",
            Self::TagCountExceeded { .. } => "comment_tag_count_exceeded",
            Self::TagElementCountExceeded { .. } => "comment_tag_element_count_exceeded",
            Self::TagElementTooLarge { .. } => "comment_tag_element_too_large",
            Self::TagBytesExceeded { .. } => "comment_tag_bytes_exceeded",
            Self::EventWireTooLarge { .. } => "comment_event_wire_too_large",
        }
    }
}

impl fmt::Display for RadrootsNip22CommentProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedKind { actual } => {
                write!(formatter, "NIP-22 comment kind must be 1111, got {actual}")
            }
            Self::ContentMissing => formatter.write_str("NIP-22 comment content is blank"),
            Self::ContentTooLarge { max, actual } => {
                write!(
                    formatter,
                    "NIP-22 comment content is {actual} bytes; max is {max}"
                )
            }
            Self::RootFormUnsupported { tag_index } => write!(
                formatter,
                "NIP-22 comment tag {tag_index} uses unsupported external root authority"
            ),
            Self::ParentFormUnsupported { tag_index } => write!(
                formatter,
                "NIP-22 comment tag {tag_index} uses unsupported external parent authority"
            ),
            Self::RootCardinality { actual } => write!(
                formatter,
                "NIP-22 comment has {actual} E/A root authority tags; expected exactly one"
            ),
            Self::RootKindCardinality { actual } => write!(
                formatter,
                "NIP-22 comment has {actual} K tags; expected exactly one"
            ),
            Self::RootAuthorCardinality { actual } => write!(
                formatter,
                "NIP-22 comment has {actual} P tags; expected exactly one"
            ),
            Self::ParentKindCardinality { actual } => write!(
                formatter,
                "NIP-22 comment has {actual} k tags; expected exactly one"
            ),
            Self::RootKindUnsupported { tag_index } => write!(
                formatter,
                "NIP-22 comment root kind at tag {tag_index} is unsupported or noncanonical"
            ),
            Self::RootAuthorInvalid { tag_index } => {
                write!(
                    formatter,
                    "NIP-22 comment root author tag {tag_index} is invalid"
                )
            }
            Self::RootReferenceShape { tag_index } => {
                write!(
                    formatter,
                    "NIP-22 comment root tag {tag_index} has an invalid shape"
                )
            }
            Self::RootEventIdInvalid { tag_index, error } => write!(
                formatter,
                "NIP-22 comment root tag {tag_index} has an invalid event id: {error}"
            ),
            Self::RootCoordinateInvalid { tag_index } => write!(
                formatter,
                "NIP-22 comment root tag {tag_index} has an invalid coordinate"
            ),
            Self::RootKindMismatch { tag_index } => write!(
                formatter,
                "NIP-22 comment root tag {tag_index} conflicts with K authority"
            ),
            Self::RootAuthorMismatch { tag_index } => write!(
                formatter,
                "NIP-22 comment root tag {tag_index} conflicts with P authority"
            ),
            Self::ParentKindInvalid { tag_index } => write!(
                formatter,
                "NIP-22 comment parent kind tag {tag_index} is invalid"
            ),
            Self::ParentCardinality {
                event_count,
                address_count,
            } => write!(
                formatter,
                "NIP-22 comment has {event_count} e and {address_count} a parent tags"
            ),
            Self::ParentReferenceShape { tag_index } => write!(
                formatter,
                "NIP-22 comment parent tag {tag_index} has an invalid shape"
            ),
            Self::ParentEventIdInvalid { tag_index, error } => write!(
                formatter,
                "NIP-22 comment parent tag {tag_index} has an invalid event id: {error}"
            ),
            Self::ParentCoordinateInvalid { tag_index } => write!(
                formatter,
                "NIP-22 comment parent tag {tag_index} has an invalid coordinate"
            ),
            Self::ParentReferenceMismatch { tag_index } => write!(
                formatter,
                "NIP-22 comment parent tag {tag_index} conflicts with root authority"
            ),
            Self::RevisionMissing { actual } => write!(
                formatter,
                "NIP-22 comment has {actual} current-revision e tags; expected exactly one"
            ),
            Self::RevisionShape { tag_index } => write!(
                formatter,
                "NIP-22 comment revision tag {tag_index} has an invalid shape"
            ),
            Self::RevisionEventIdInvalid { tag_index, error } => write!(
                formatter,
                "NIP-22 comment revision tag {tag_index} has an invalid event id: {error}"
            ),
            Self::ParentAuthorMissing => {
                formatter.write_str("NIP-22 comment parent author is missing")
            }
            Self::ParentAuthorAmbiguous => {
                formatter.write_str("NIP-22 comment parent author is ambiguous")
            }
            Self::ParentAuthorMismatch => {
                formatter.write_str("NIP-22 comment parent author conflicts with authority")
            }
            Self::TagCountExceeded { max, actual } => {
                write!(formatter, "NIP-22 comment has {actual} tags; max is {max}")
            }
            Self::TagElementCountExceeded { max, actual } => write!(
                formatter,
                "NIP-22 comment has {actual} total tag elements; max is {max}"
            ),
            Self::TagElementTooLarge {
                max,
                actual,
                tag_index,
                element_index,
            } => write!(
                formatter,
                "NIP-22 comment tag {tag_index} element {element_index} is {actual} bytes; max is {max}"
            ),
            Self::TagBytesExceeded { max, actual } => write!(
                formatter,
                "NIP-22 comment tag bytes are {actual}; max is {max}"
            ),
            Self::EventWireTooLarge { max, actual } => write!(
                formatter,
                "NIP-22 comment compact signed event is {actual} bytes; max is {max}"
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsNip22CommentProjectionError {}

/// Projects a signature-and-id verified kind-1111 event as a NIP-22 comment.
///
/// Authority tags are order-independent. Supplemental tags and valid distinct
/// lowercase `p` mentions are retained. Relay hints and event-reference author
/// hints are advisory inputs: malformed values are diagnosed, while
/// well-formed hints that conflict with required `P`/`p` authority fail
/// projection.
pub fn project_verified_nip22_comment_event(
    verified_event: &RadrootsSignatureVerifiedEvent,
) -> Result<RadrootsInboundNip22CommentProjection, RadrootsNip22CommentProjectionError> {
    project_verified_nip22_comment_event_registry_v7(verified_event)
}

/// Projects a verified NIP-22 comment with contract-registry-v7 semantics.
pub fn project_verified_nip22_comment_event_registry_v7(
    verified_event: &RadrootsSignatureVerifiedEvent,
) -> Result<RadrootsInboundNip22CommentProjection, RadrootsNip22CommentProjectionError> {
    let event = verified_event.event();
    project_nip22_comment_parts(
        event.kind_u32(),
        &event.tags_as_vec(),
        event.content(),
        decimal_digits(event.created_at_u64()),
    )
}

fn project_nip22_comment_parts(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
    created_at_digits: usize,
) -> Result<RadrootsInboundNip22CommentProjection, RadrootsNip22CommentProjectionError> {
    if kind != KIND_COMMENT {
        return Err(RadrootsNip22CommentProjectionError::UnsupportedKind { actual: kind });
    }
    if content.chars().all(char::is_whitespace) {
        return Err(RadrootsNip22CommentProjectionError::ContentMissing);
    }
    if content.len() > RADROOTS_NIP22_COMMENT_CONTENT_MAX_BYTES {
        return Err(RadrootsNip22CommentProjectionError::ContentTooLarge {
            max: RADROOTS_NIP22_COMMENT_CONTENT_MAX_BYTES,
            actual: content.len(),
        });
    }
    validate_tag_and_wire_budgets(tags, content, created_at_digits)?;

    if let Some((tag_index, _)) = find_tags(tags, "I").first() {
        return Err(RadrootsNip22CommentProjectionError::RootFormUnsupported {
            tag_index: *tag_index,
        });
    }
    if let Some((tag_index, _)) = find_tags(tags, "i").first() {
        return Err(RadrootsNip22CommentProjectionError::ParentFormUnsupported {
            tag_index: *tag_index,
        });
    }
    let event_roots = find_tags(tags, "E");
    let address_roots = find_tags(tags, "A");
    let root_count = event_roots.len() + address_roots.len();
    if root_count != 1 {
        return Err(RadrootsNip22CommentProjectionError::RootCardinality { actual: root_count });
    }
    let kind_tag = exactly_one_authority_tag(tags, "K", |actual| {
        RadrootsNip22CommentProjectionError::RootKindCardinality { actual }
    })?;
    let root_author_tag = exactly_one_authority_tag(tags, "P", |actual| {
        RadrootsNip22CommentProjectionError::RootAuthorCardinality { actual }
    })?;
    let parent_kind_tag = exactly_one_authority_tag(tags, "k", |actual| {
        RadrootsNip22CommentProjectionError::ParentKindCardinality { actual }
    })?;

    let mut diagnostics = Vec::new();
    let root_kind = parse_root_kind(kind_tag)?;
    let root_author = parse_required_root_author(root_author_tag, &mut diagnostics)?;
    let participants = project_participants(tags, &mut diagnostics);
    let parent_event_tags = find_tags(tags, "e");
    let parent_address_tags = find_tags(tags, "a");

    let (root, position, mut mentions) = if let Some(root_tag) = event_roots.first().copied() {
        let event_reference =
            parse_event_reference(root_tag, ReferenceRole::Root, &mut diagnostics)?;
        if event_reference
            .author_hint
            .as_ref()
            .is_some_and(|author_hint| author_hint != root_author.pubkey())
        {
            return Err(RadrootsNip22CommentProjectionError::RootAuthorMismatch {
                tag_index: root_tag.0,
            });
        }
        let root = RadrootsInboundNip22EventRoot {
            tag_index: root_tag.0,
            event_id: event_reference.event_id,
            kind_tag_index: kind_tag.0,
            kind_raw_tag: kind_tag.1.clone(),
            kind: root_kind,
            relay: event_reference.relay,
            author_hint: event_reference.author_hint,
            author: root_author.clone(),
            raw_tag: root_tag.1.clone(),
        };
        let parent_kind = parse_parent_kind(parent_kind_tag)?;
        if parent_kind != root_kind.as_u32() && parent_kind != KIND_COMMENT {
            return Err(RadrootsNip22CommentProjectionError::ParentKindInvalid {
                tag_index: parent_kind_tag.0,
            });
        }

        if parent_kind == root_kind.as_u32() {
            require_parent_cardinality(&parent_event_tags, &parent_address_tags, 1, 0)?;
            let parent_event_tag = parent_event_tags[0];
            let parent =
                parse_event_reference(parent_event_tag, ReferenceRole::Parent, &mut diagnostics)?;
            if parent.event_id != root.event_id {
                return Err(
                    RadrootsNip22CommentProjectionError::ParentReferenceMismatch {
                        tag_index: parent_event_tag.0,
                    },
                );
            }
            let (selected_author, mentions) = select_direct_parent_author(
                participants,
                root.author.pubkey(),
                parent.author_hint.as_ref(),
            )?;
            let position = RadrootsInboundNip22CommentPosition::TopLevelEvent {
                reference: RadrootsInboundNip22TopLevelEventReference {
                    tag_index: parent_event_tag.0,
                    event_id: parent.event_id,
                    kind_tag_index: parent_kind_tag.0,
                    kind_raw_tag: parent_kind_tag.1.clone(),
                    kind: parent_kind,
                    relay: parent.relay,
                    author_hint: parent.author_hint,
                    author: selected_author,
                    raw_tag: parent_event_tag.1.clone(),
                },
            };
            (
                RadrootsInboundNip22CommentRoot::Event(root),
                position,
                mentions,
            )
        } else {
            require_parent_cardinality(&parent_event_tags, &parent_address_tags, 1, 0)?;
            let parent_event_tag = parent_event_tags[0];
            let parent_reference =
                parse_event_reference(parent_event_tag, ReferenceRole::Parent, &mut diagnostics)?;
            if parent_reference.event_id == root.event_id {
                return Err(
                    RadrootsNip22CommentProjectionError::ParentReferenceMismatch {
                        tag_index: parent_event_tag.0,
                    },
                );
            }
            let (selected_author, mentions) =
                select_nested_parent_author(participants, parent_reference.author_hint.as_ref())?;
            let parent = RadrootsInboundNip22CommentParent {
                tag_index: parent_event_tag.0,
                event_id: parent_reference.event_id,
                kind_tag_index: parent_kind_tag.0,
                kind_raw_tag: parent_kind_tag.1.clone(),
                kind: parent_kind,
                relay: parent_reference.relay,
                author_hint: parent_reference.author_hint,
                author: selected_author,
                raw_tag: parent_event_tag.1.clone(),
            };
            (
                RadrootsInboundNip22CommentRoot::Event(root),
                RadrootsInboundNip22CommentPosition::Nested { parent },
                mentions,
            )
        }
    } else {
        let root_tag = address_roots[0];
        let address_reference =
            parse_address_reference(root_tag, ReferenceRole::Root, &mut diagnostics)?;
        if address_reference.kind != root_kind {
            return Err(RadrootsNip22CommentProjectionError::RootKindMismatch {
                tag_index: root_tag.0,
            });
        }
        if address_reference.author != *root_author.pubkey() {
            return Err(RadrootsNip22CommentProjectionError::RootAuthorMismatch {
                tag_index: root_tag.0,
            });
        }
        let root = RadrootsInboundNip22AddressRoot {
            tag_index: root_tag.0,
            coordinate: address_reference.coordinate,
            kind_tag_index: kind_tag.0,
            kind_raw_tag: kind_tag.1.clone(),
            kind: root_kind,
            relay: address_reference.relay,
            author: root_author.clone(),
            raw_tag: root_tag.1.clone(),
        };
        let parent_kind = parse_parent_kind(parent_kind_tag)?;
        if parent_kind != root_kind.as_u32() && parent_kind != KIND_COMMENT {
            return Err(RadrootsNip22CommentProjectionError::ParentKindInvalid {
                tag_index: parent_kind_tag.0,
            });
        }

        if parent_kind == root_kind.as_u32() {
            if parent_address_tags.len() != 1 {
                return Err(RadrootsNip22CommentProjectionError::ParentCardinality {
                    event_count: parent_event_tags.len(),
                    address_count: parent_address_tags.len(),
                });
            }
            if parent_event_tags.len() != 1 {
                return Err(RadrootsNip22CommentProjectionError::RevisionMissing {
                    actual: parent_event_tags.len(),
                });
            }
            let parent_address = parse_address_reference(
                parent_address_tags[0],
                ReferenceRole::Parent,
                &mut diagnostics,
            )?;
            if parent_address.coordinate != root.coordinate {
                return Err(
                    RadrootsNip22CommentProjectionError::ParentReferenceMismatch {
                        tag_index: parent_address_tags[0].0,
                    },
                );
            }
            let current_revision = parse_current_revision(parent_event_tags[0], &mut diagnostics)?;
            let (selected_author, mentions) =
                select_direct_parent_author(participants, root.author.pubkey(), None)?;
            let position = RadrootsInboundNip22CommentPosition::TopLevelAddress {
                reference: RadrootsInboundNip22TopLevelAddressReference {
                    tag_index: parent_address_tags[0].0,
                    coordinate: parent_address.coordinate,
                    kind_tag_index: parent_kind_tag.0,
                    kind_raw_tag: parent_kind_tag.1.clone(),
                    kind: parent_kind,
                    relay: parent_address.relay,
                    author: selected_author,
                    raw_tag: parent_address_tags[0].1.clone(),
                },
                current_revision,
            };
            (
                RadrootsInboundNip22CommentRoot::Address(root),
                position,
                mentions,
            )
        } else {
            require_parent_cardinality(&parent_event_tags, &parent_address_tags, 1, 0)?;
            let parent_event_tag = parent_event_tags[0];
            let parent_reference =
                parse_event_reference(parent_event_tag, ReferenceRole::Parent, &mut diagnostics)?;
            let (selected_author, mentions) =
                select_nested_parent_author(participants, parent_reference.author_hint.as_ref())?;
            let parent = RadrootsInboundNip22CommentParent {
                tag_index: parent_event_tag.0,
                event_id: parent_reference.event_id,
                kind_tag_index: parent_kind_tag.0,
                kind_raw_tag: parent_kind_tag.1.clone(),
                kind: parent_kind,
                relay: parent_reference.relay,
                author_hint: parent_reference.author_hint,
                author: selected_author,
                raw_tag: parent_event_tag.1.clone(),
            };
            (
                RadrootsInboundNip22CommentRoot::Address(root),
                RadrootsInboundNip22CommentPosition::Nested { parent },
                mentions,
            )
        }
    };

    mentions.sort_by_key(RadrootsInboundNip22Participant::tag_index);
    diagnostics.sort_by_key(RadrootsNip22CommentDiagnostic::tag_index);

    Ok(RadrootsInboundNip22CommentProjection {
        root,
        position,
        mentions,
        diagnostics,
        raw_tags: tags.to_vec(),
    })
}

type IndexedTag<'a> = (usize, &'a Vec<String>);

struct ParsedEventReference {
    event_id: RadrootsEventId,
    relay: Option<RadrootsNostrRelayHint>,
    author_hint: Option<PublicKey>,
}

struct ParsedAddressReference {
    coordinate: RadrootsAddressableCoordinate,
    author: PublicKey,
    kind: RadrootsNip22CommentRootKind,
    relay: Option<RadrootsNostrRelayHint>,
}

#[derive(Clone, Copy)]
enum ReferenceRole {
    Root,
    Parent,
}

impl ReferenceRole {
    const fn relay_role(self) -> RelayRole {
        match self {
            Self::Root => RelayRole::Root,
            Self::Parent => RelayRole::Parent,
        }
    }

    fn author_hint_diagnostic(
        self,
        tag_index: usize,
        raw_tag: Vec<String>,
    ) -> RadrootsNip22CommentDiagnostic {
        match self {
            Self::Root => {
                RadrootsNip22CommentDiagnostic::RootAuthorHintIgnored { tag_index, raw_tag }
            }
            Self::Parent => {
                RadrootsNip22CommentDiagnostic::ParentAuthorHintIgnored { tag_index, raw_tag }
            }
        }
    }

    const fn reference_shape_error(self, tag_index: usize) -> RadrootsNip22CommentProjectionError {
        match self {
            Self::Root => RadrootsNip22CommentProjectionError::RootReferenceShape { tag_index },
            Self::Parent => RadrootsNip22CommentProjectionError::ParentReferenceShape { tag_index },
        }
    }

    const fn event_id_error(
        self,
        tag_index: usize,
        error: RadrootsIdParseError,
    ) -> RadrootsNip22CommentProjectionError {
        match self {
            Self::Root => {
                RadrootsNip22CommentProjectionError::RootEventIdInvalid { tag_index, error }
            }
            Self::Parent => {
                RadrootsNip22CommentProjectionError::ParentEventIdInvalid { tag_index, error }
            }
        }
    }

    const fn coordinate_error(self, tag_index: usize) -> RadrootsNip22CommentProjectionError {
        match self {
            Self::Root => RadrootsNip22CommentProjectionError::RootCoordinateInvalid { tag_index },
            Self::Parent => {
                RadrootsNip22CommentProjectionError::ParentCoordinateInvalid { tag_index }
            }
        }
    }
}

#[derive(Clone, Copy)]
enum RelayRole {
    Root,
    RootAuthor,
    Parent,
    ParentAuthor,
    Revision,
}

impl RelayRole {
    fn diagnostic(self, tag_index: usize, raw_tag: Vec<String>) -> RadrootsNip22CommentDiagnostic {
        match self {
            Self::Root => RadrootsNip22CommentDiagnostic::RootRelayIgnored { tag_index, raw_tag },
            Self::RootAuthor => {
                RadrootsNip22CommentDiagnostic::RootAuthorRelayIgnored { tag_index, raw_tag }
            }
            Self::Parent => {
                RadrootsNip22CommentDiagnostic::ParentRelayIgnored { tag_index, raw_tag }
            }
            Self::ParentAuthor => {
                RadrootsNip22CommentDiagnostic::ParentAuthorRelayIgnored { tag_index, raw_tag }
            }
            Self::Revision => {
                RadrootsNip22CommentDiagnostic::RevisionRelayIgnored { tag_index, raw_tag }
            }
        }
    }
}

fn find_tags<'a>(tags: &'a [Vec<String>], name: &str) -> Vec<IndexedTag<'a>> {
    tags.iter()
        .enumerate()
        .filter(|(_, tag)| tag.first().is_some_and(|actual| actual == name))
        .collect()
}

fn exactly_one_authority_tag<'a>(
    tags: &'a [Vec<String>],
    name: &str,
    error: impl FnOnce(usize) -> RadrootsNip22CommentProjectionError,
) -> Result<IndexedTag<'a>, RadrootsNip22CommentProjectionError> {
    let matches = find_tags(tags, name);
    match matches.as_slice() {
        [tag] => Ok(*tag),
        _ => Err(error(matches.len())),
    }
}

fn parse_root_kind(
    (tag_index, tag): IndexedTag<'_>,
) -> Result<RadrootsNip22CommentRootKind, RadrootsNip22CommentProjectionError> {
    let kind = tag
        .get(1)
        .filter(|_| tag.len() == 2)
        .and_then(|value| parse_canonical_kind_token(value))
        .ok_or(RadrootsNip22CommentProjectionError::RootKindUnsupported { tag_index })?;
    RadrootsNip22CommentRootKind::parse(kind)
        .map_err(|_| RadrootsNip22CommentProjectionError::RootKindUnsupported { tag_index })
}

fn parse_parent_kind(
    (tag_index, tag): IndexedTag<'_>,
) -> Result<u32, RadrootsNip22CommentProjectionError> {
    let kind = tag
        .get(1)
        .filter(|_| tag.len() == 2)
        .and_then(|value| parse_canonical_kind_token(value))
        .ok_or(RadrootsNip22CommentProjectionError::ParentKindInvalid { tag_index })?;
    if kind == KIND_COMMENT || RadrootsNip22CommentRootKind::parse(kind).is_ok() {
        Ok(kind)
    } else {
        Err(RadrootsNip22CommentProjectionError::ParentKindInvalid { tag_index })
    }
}

fn parse_canonical_kind_token(value: &str) -> Option<u32> {
    if value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || value.len() > 1 && value.starts_with('0')
    {
        return None;
    }
    value.parse().ok()
}

fn parse_required_root_author(
    (tag_index, tag): IndexedTag<'_>,
    diagnostics: &mut Vec<RadrootsNip22CommentDiagnostic>,
) -> Result<RadrootsInboundNip22Participant, RadrootsNip22CommentProjectionError> {
    if !matches!(tag.len(), 2 | 3) {
        return Err(RadrootsNip22CommentProjectionError::RootAuthorInvalid { tag_index });
    }
    let pubkey = PublicKey::from_hex(&tag[1])
        .map_err(|_| RadrootsNip22CommentProjectionError::RootAuthorInvalid { tag_index })?;
    let relay = project_relay(tag_index, tag, 2, RelayRole::RootAuthor, diagnostics);
    Ok(RadrootsInboundNip22Participant {
        tag_index,
        pubkey,
        relay,
        raw_tag: tag.clone(),
    })
}

fn project_participants(
    tags: &[Vec<String>],
    diagnostics: &mut Vec<RadrootsNip22CommentDiagnostic>,
) -> Vec<RadrootsInboundNip22Participant> {
    let mut participants = Vec::new();
    for (tag_index, tag) in find_tags(tags, "p") {
        if !matches!(tag.len(), 2 | 3) {
            diagnostics.push(RadrootsNip22CommentDiagnostic::ParentAuthorShapeIgnored {
                tag_index,
                raw_tag: tag.clone(),
            });
            continue;
        }
        let Ok(pubkey) = PublicKey::from_hex(&tag[1]) else {
            diagnostics.push(RadrootsNip22CommentDiagnostic::ParentAuthorInvalidIgnored {
                tag_index,
                raw_tag: tag.clone(),
            });
            continue;
        };
        let relay = project_relay(tag_index, tag, 2, RelayRole::ParentAuthor, diagnostics);
        if participants
            .iter()
            .any(|participant: &RadrootsInboundNip22Participant| participant.pubkey == pubkey)
        {
            diagnostics.push(
                RadrootsNip22CommentDiagnostic::ParentAuthorDuplicateIgnored {
                    tag_index,
                    raw_tag: tag.clone(),
                },
            );
            continue;
        }
        participants.push(RadrootsInboundNip22Participant {
            tag_index,
            pubkey,
            relay,
            raw_tag: tag.clone(),
        });
    }
    participants
}

fn parse_event_reference(
    (tag_index, tag): IndexedTag<'_>,
    role: ReferenceRole,
    diagnostics: &mut Vec<RadrootsNip22CommentDiagnostic>,
) -> Result<ParsedEventReference, RadrootsNip22CommentProjectionError> {
    if !(2..=4).contains(&tag.len()) {
        return Err(role.reference_shape_error(tag_index));
    }
    let event_id =
        RadrootsEventId::parse(&tag[1]).map_err(|error| role.event_id_error(tag_index, error))?;
    let relay = project_relay(tag_index, tag, 2, role.relay_role(), diagnostics);
    let author_hint = tag
        .get(3)
        .and_then(|value| match PublicKey::from_hex(value) {
            Ok(author) => Some(author),
            Err(_) => {
                diagnostics.push(role.author_hint_diagnostic(tag_index, tag.clone()));
                None
            }
        });
    Ok(ParsedEventReference {
        event_id,
        relay,
        author_hint,
    })
}

fn parse_address_reference(
    (tag_index, tag): IndexedTag<'_>,
    role: ReferenceRole,
    diagnostics: &mut Vec<RadrootsNip22CommentDiagnostic>,
) -> Result<ParsedAddressReference, RadrootsNip22CommentProjectionError> {
    if !matches!(tag.len(), 2 | 3) {
        return Err(role.reference_shape_error(tag_index));
    }
    let canonical_kind = tag[1]
        .split_once(':')
        .and_then(|(kind, _)| parse_canonical_kind_token(kind))
        .ok_or_else(|| role.coordinate_error(tag_index))?;
    let parts = RadrootsAddressableCoordinateParts::parse(&tag[1])
        .map_err(|_| role.coordinate_error(tag_index))?;
    if parts.kind != canonical_kind {
        return Err(role.coordinate_error(tag_index));
    }
    let kind = RadrootsNip22CommentRootKind::parse(parts.kind)
        .map_err(|_| role.coordinate_error(tag_index))?;
    let coordinate = RadrootsAddressableCoordinate::parse(format_coordinate(&parts))
        .map_err(|_| role.coordinate_error(tag_index))?;
    let relay = project_relay(tag_index, tag, 2, role.relay_role(), diagnostics);
    Ok(ParsedAddressReference {
        coordinate,
        author: parts.pubkey,
        kind,
        relay,
    })
}

fn format_coordinate(parts: &RadrootsAddressableCoordinateParts) -> String {
    format!("{}:{}:{}", parts.kind, parts.pubkey, parts.d_tag)
}

fn parse_current_revision(
    (tag_index, tag): IndexedTag<'_>,
    diagnostics: &mut Vec<RadrootsNip22CommentDiagnostic>,
) -> Result<RadrootsInboundNip22CurrentRevision, RadrootsNip22CommentProjectionError> {
    if !matches!(tag.len(), 2 | 3) {
        return Err(RadrootsNip22CommentProjectionError::RevisionShape { tag_index });
    }
    let event_id = RadrootsEventId::parse(&tag[1]).map_err(|error| {
        RadrootsNip22CommentProjectionError::RevisionEventIdInvalid { tag_index, error }
    })?;
    let relay = project_relay(tag_index, tag, 2, RelayRole::Revision, diagnostics);
    Ok(RadrootsInboundNip22CurrentRevision {
        tag_index,
        event_id,
        relay,
        raw_tag: tag.clone(),
    })
}

fn require_parent_cardinality(
    event_tags: &[IndexedTag<'_>],
    address_tags: &[IndexedTag<'_>],
    expected_event_count: usize,
    expected_address_count: usize,
) -> Result<(), RadrootsNip22CommentProjectionError> {
    if event_tags.len() == expected_event_count && address_tags.len() == expected_address_count {
        Ok(())
    } else {
        Err(RadrootsNip22CommentProjectionError::ParentCardinality {
            event_count: event_tags.len(),
            address_count: address_tags.len(),
        })
    }
}

fn select_direct_parent_author(
    mut participants: Vec<RadrootsInboundNip22Participant>,
    expected_author: &PublicKey,
    author_hint: Option<&PublicKey>,
) -> Result<
    (
        RadrootsInboundNip22Participant,
        Vec<RadrootsInboundNip22Participant>,
    ),
    RadrootsNip22CommentProjectionError,
> {
    if participants.is_empty() {
        return Err(RadrootsNip22CommentProjectionError::ParentAuthorMissing);
    }
    if author_hint.is_some_and(|hint| hint != expected_author) {
        return Err(RadrootsNip22CommentProjectionError::ParentAuthorMismatch);
    }
    let Some(selected_index) = participants
        .iter()
        .position(|participant| participant.pubkey() == expected_author)
    else {
        return Err(RadrootsNip22CommentProjectionError::ParentAuthorMismatch);
    };
    let selected = participants.remove(selected_index);
    Ok((selected, participants))
}

fn select_nested_parent_author(
    mut participants: Vec<RadrootsInboundNip22Participant>,
    author_hint: Option<&PublicKey>,
) -> Result<
    (
        RadrootsInboundNip22Participant,
        Vec<RadrootsInboundNip22Participant>,
    ),
    RadrootsNip22CommentProjectionError,
> {
    if participants.is_empty() {
        return Err(RadrootsNip22CommentProjectionError::ParentAuthorMissing);
    }
    let selected_index = if let Some(author_hint) = author_hint {
        participants
            .iter()
            .position(|participant| participant.pubkey() == author_hint)
            .ok_or(RadrootsNip22CommentProjectionError::ParentAuthorMismatch)?
    } else {
        if participants.len() > 1 {
            return Err(RadrootsNip22CommentProjectionError::ParentAuthorAmbiguous);
        }
        0
    };
    let selected = participants.remove(selected_index);
    Ok((selected, participants))
}

fn project_relay(
    tag_index: usize,
    tag: &[String],
    element_index: usize,
    role: RelayRole,
    diagnostics: &mut Vec<RadrootsNip22CommentDiagnostic>,
) -> Option<RadrootsNostrRelayHint> {
    let value = tag.get(element_index)?;
    if value.is_empty() {
        return None;
    }
    match RadrootsNostrRelayHint::parse(value) {
        Ok(relay) => Some(relay),
        Err(_) => {
            diagnostics.push(role.diagnostic(tag_index, tag.to_vec()));
            None
        }
    }
}

fn validate_tag_and_wire_budgets(
    tags: &[Vec<String>],
    content: &str,
    created_at_digits: usize,
) -> Result<(), RadrootsNip22CommentProjectionError> {
    if tags.len() > RADROOTS_NIP22_COMMENT_TAG_MAX_COUNT {
        return Err(RadrootsNip22CommentProjectionError::TagCountExceeded {
            max: RADROOTS_NIP22_COMMENT_TAG_MAX_COUNT,
            actual: tags.len(),
        });
    }
    let tag_element_count = tags
        .iter()
        .fold(0usize, |total, tag| total.saturating_add(tag.len()));
    if tag_element_count > RADROOTS_NIP22_COMMENT_TAG_TOTAL_ELEMENT_MAX_COUNT {
        return Err(
            RadrootsNip22CommentProjectionError::TagElementCountExceeded {
                max: RADROOTS_NIP22_COMMENT_TAG_TOTAL_ELEMENT_MAX_COUNT,
                actual: tag_element_count,
            },
        );
    }
    let mut tag_bytes = 0usize;
    let mut tags_json_bytes = 2usize;
    for (tag_index, tag) in tags.iter().enumerate() {
        if tag_index > 0 {
            tags_json_bytes = tags_json_bytes.saturating_add(1);
        }
        tags_json_bytes = tags_json_bytes.saturating_add(2);
        for (element_index, element) in tag.iter().enumerate() {
            if element.len() > RADROOTS_NIP22_COMMENT_TAG_ELEMENT_MAX_BYTES {
                return Err(RadrootsNip22CommentProjectionError::TagElementTooLarge {
                    max: RADROOTS_NIP22_COMMENT_TAG_ELEMENT_MAX_BYTES,
                    actual: element.len(),
                    tag_index,
                    element_index,
                });
            }
            if element_index > 0 {
                tags_json_bytes = tags_json_bytes.saturating_add(1);
            }
            tags_json_bytes = tags_json_bytes.saturating_add(canonical_json_string_bytes(element));
            tag_bytes = tag_bytes.saturating_add(element.len());
        }
    }
    if tag_bytes > RADROOTS_NIP22_COMMENT_TAG_TOTAL_MAX_BYTES {
        return Err(RadrootsNip22CommentProjectionError::TagBytesExceeded {
            max: RADROOTS_NIP22_COMMENT_TAG_TOTAL_MAX_BYTES,
            actual: tag_bytes,
        });
    }
    let actual = RADROOTS_NIP22_COMMENT_SIGNED_EVENT_FIXED_MAX_BYTES
        .saturating_add(created_at_digits)
        .saturating_add(tags_json_bytes)
        .saturating_add(canonical_json_string_bytes(content));
    if actual > RADROOTS_NIP22_COMMENT_EVENT_WIRE_MAX_BYTES {
        return Err(RadrootsNip22CommentProjectionError::EventWireTooLarge {
            max: RADROOTS_NIP22_COMMENT_EVENT_WIRE_MAX_BYTES,
            actual,
        });
    }
    Ok(())
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

const fn decimal_digits(mut value: u64) -> usize {
    let mut digits = 1usize;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

#[cfg(test)]
mod tests;
