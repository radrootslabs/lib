#[cfg(not(feature = "std"))]
use alloc::{collections::BTreeSet, string::String, vec::Vec};
use core::fmt;
#[cfg(feature = "std")]
use std::collections::BTreeSet;

use radroots_event::{
    ids::{RadrootsEventId, RadrootsIdParseError, RadrootsPublicKey},
    kinds::KIND_POST,
    post::{
        RADROOTS_POST_CONTENT_MAX_BYTES, RADROOTS_POST_EVENT_WIRE_MAX_BYTES,
        RADROOTS_POST_TAG_ELEMENT_MAX_BYTES, RADROOTS_POST_TAG_TOTAL_MAX_BYTES,
    },
    reply::RadrootsNip10RelayHint,
};

use crate::verification::RadrootsSignatureVerifiedEvent;

const RADROOTS_NIP10_REPLY_SIGNED_EVENT_FIXED_BYTES: usize = "{\"id\":\"".len()
    + 64
    + "\",\"pubkey\":\"".len()
    + 64
    + "\",\"created_at\":".len()
    + ",\"kind\":1,\"tags\":".len()
    + ",\"content\":".len()
    + ",\"sig\":\"".len()
    + 128
    + "\"}".len();

#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadrootsNip10ReplyStyle {
    Marked,
    LegacyPositional,
}

/// One ignored interoperability issue encountered while projecting a verified
/// inbound NIP-10 Reply.
///
/// Diagnostics are ordered deterministically: event-reference diagnostics in
/// `e`-tag order, participant diagnostics in `p`-tag order, then reference to
/// participant relationship diagnostics in root/parent/citation order.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsNip10ReplyDiagnostic {
    ReplyAuthorMissing,
    ReferenceRelayIgnored {
        tag_index: usize,
        raw_tag: Vec<String>,
        error: RadrootsIdParseError,
    },
    ReferenceAuthorIgnored {
        tag_index: usize,
        raw_tag: Vec<String>,
        error: RadrootsIdParseError,
    },
    CitationShapeIgnored {
        tag_index: usize,
        raw_tag: Vec<String>,
    },
    CitationEventIdIgnored {
        tag_index: usize,
        raw_tag: Vec<String>,
        error: RadrootsIdParseError,
    },
    CitationMarkerIgnored {
        tag_index: usize,
        raw_tag: Vec<String>,
    },
    ReplyAuthorShapeIgnored {
        tag_index: usize,
        raw_tag: Vec<String>,
    },
    ReplyAuthorIgnored {
        tag_index: usize,
        raw_tag: Vec<String>,
        error: RadrootsIdParseError,
    },
    ReplyAuthorRelayIgnored {
        tag_index: usize,
        raw_tag: Vec<String>,
        error: RadrootsIdParseError,
    },
    ReplyAuthorDuplicateIgnored {
        tag_index: usize,
        raw_tag: Vec<String>,
    },
    ReplyAuthorMismatchIgnored {
        tag_index: usize,
        raw_tag: Vec<String>,
    },
}

impl RadrootsNip10ReplyDiagnostic {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::ReplyAuthorMissing => "reply_author_missing_ignored",
            Self::ReferenceRelayIgnored { .. } => "reply_reference_relay_ignored",
            Self::ReferenceAuthorIgnored { .. } => "reply_reference_author_ignored",
            Self::CitationShapeIgnored { .. } => "reply_citation_shape_ignored",
            Self::CitationEventIdIgnored { .. } => "reply_citation_event_id_ignored",
            Self::CitationMarkerIgnored { .. } => "reply_citation_marker_ignored",
            Self::ReplyAuthorShapeIgnored { .. } => "reply_author_shape_ignored",
            Self::ReplyAuthorIgnored { .. } => "reply_author_invalid_ignored",
            Self::ReplyAuthorRelayIgnored { .. } => "reply_author_relay_ignored",
            Self::ReplyAuthorDuplicateIgnored { .. } => "reply_author_duplicate_ignored",
            Self::ReplyAuthorMismatchIgnored { .. } => "reply_author_mismatch_ignored",
        }
    }

    pub const fn tag_index(&self) -> Option<usize> {
        match self {
            Self::ReplyAuthorMissing => None,
            Self::ReferenceRelayIgnored { tag_index, .. }
            | Self::ReferenceAuthorIgnored { tag_index, .. }
            | Self::CitationShapeIgnored { tag_index, .. }
            | Self::CitationEventIdIgnored { tag_index, .. }
            | Self::CitationMarkerIgnored { tag_index, .. }
            | Self::ReplyAuthorShapeIgnored { tag_index, .. }
            | Self::ReplyAuthorIgnored { tag_index, .. }
            | Self::ReplyAuthorRelayIgnored { tag_index, .. }
            | Self::ReplyAuthorDuplicateIgnored { tag_index, .. }
            | Self::ReplyAuthorMismatchIgnored { tag_index, .. } => Some(*tag_index),
        }
    }

    pub fn raw_tag(&self) -> Option<&[String]> {
        match self {
            Self::ReplyAuthorMissing => None,
            Self::ReferenceRelayIgnored { raw_tag, .. }
            | Self::ReferenceAuthorIgnored { raw_tag, .. }
            | Self::CitationShapeIgnored { raw_tag, .. }
            | Self::CitationEventIdIgnored { raw_tag, .. }
            | Self::CitationMarkerIgnored { raw_tag, .. }
            | Self::ReplyAuthorShapeIgnored { raw_tag, .. }
            | Self::ReplyAuthorIgnored { raw_tag, .. }
            | Self::ReplyAuthorRelayIgnored { raw_tag, .. }
            | Self::ReplyAuthorDuplicateIgnored { raw_tag, .. }
            | Self::ReplyAuthorMismatchIgnored { raw_tag, .. } => Some(raw_tag),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsInboundNip10EventReference {
    tag_index: usize,
    raw_tag: Vec<String>,
    event_id: RadrootsEventId,
    relay: Option<RadrootsNip10RelayHint>,
    author_hint: Option<RadrootsPublicKey>,
}

impl RadrootsInboundNip10EventReference {
    pub const fn tag_index(&self) -> usize {
        self.tag_index
    }

    pub fn raw_tag(&self) -> &[String] {
        &self.raw_tag
    }

    pub const fn event_id(&self) -> &RadrootsEventId {
        &self.event_id
    }

    pub const fn relay(&self) -> Option<&RadrootsNip10RelayHint> {
        self.relay.as_ref()
    }

    pub const fn author_hint(&self) -> Option<&RadrootsPublicKey> {
        self.author_hint.as_ref()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsInboundNip10Participant {
    tag_index: usize,
    raw_tag: Vec<String>,
    pubkey: RadrootsPublicKey,
    relay: Option<RadrootsNip10RelayHint>,
}

impl RadrootsInboundNip10Participant {
    pub const fn tag_index(&self) -> usize {
        self.tag_index
    }

    pub fn raw_tag(&self) -> &[String] {
        &self.raw_tag
    }

    pub const fn pubkey(&self) -> &RadrootsPublicKey {
        &self.pubkey
    }

    pub const fn relay(&self) -> Option<&RadrootsNip10RelayHint> {
        self.relay.as_ref()
    }
}

/// Normalized projection of one structurally valid NIP-10 reply.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadrootsInboundNip10ReplyProjection {
    style: RadrootsNip10ReplyStyle,
    root: RadrootsInboundNip10EventReference,
    parent: Option<RadrootsInboundNip10EventReference>,
    citations: Vec<RadrootsInboundNip10EventReference>,
    participants: Vec<RadrootsInboundNip10Participant>,
    diagnostics: Vec<RadrootsNip10ReplyDiagnostic>,
}

impl RadrootsInboundNip10ReplyProjection {
    pub const fn style(&self) -> RadrootsNip10ReplyStyle {
        self.style
    }

    pub const fn root(&self) -> &RadrootsInboundNip10EventReference {
        &self.root
    }

    pub fn parent(&self) -> &RadrootsInboundNip10EventReference {
        self.parent.as_ref().unwrap_or(&self.root)
    }

    pub const fn reply_reference(&self) -> Option<&RadrootsInboundNip10EventReference> {
        self.parent.as_ref()
    }

    pub const fn is_direct(&self) -> bool {
        self.parent.is_none()
    }

    pub fn citations(&self) -> &[RadrootsInboundNip10EventReference] {
        &self.citations
    }

    pub fn participants(&self) -> &[RadrootsInboundNip10Participant] {
        &self.participants
    }

    pub fn diagnostics(&self) -> &[RadrootsNip10ReplyDiagnostic] {
        &self.diagnostics
    }

    pub const fn contract_id(&self) -> &'static str {
        "radroots.social.reply.v1"
    }
}

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsNip10ReplyProjectionError {
    InvalidKind {
        expected: u32,
        actual: u32,
    },
    ContentTooLarge {
        max: usize,
        actual: usize,
    },
    ReplyMarkerMissing,
    ReplyMarkerCount,
    EventReferenceShape {
        tag_index: usize,
    },
    EventIdInvalid {
        tag_index: usize,
        error: RadrootsIdParseError,
    },
    ReplyReferenceAmbiguous,
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

impl RadrootsNip10ReplyProjectionError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidKind { .. } => "invalid_kind",
            Self::ContentTooLarge { .. } => "reply_content_too_large",
            Self::ReplyMarkerMissing => "reply_marker_missing",
            Self::ReplyMarkerCount => "reply_marker_count",
            Self::EventReferenceShape { .. } => "reply_reference_shape",
            Self::EventIdInvalid { .. } => "reply_event_id_invalid",
            Self::ReplyReferenceAmbiguous => "reply_reference_ambiguous",
            Self::TagElementTooLarge { .. } => "reply_tag_element_too_large",
            Self::TagBytesExceeded { .. } => "reply_tag_bytes_exceeded",
            Self::EventWireTooLarge { .. } => "reply_event_wire_too_large",
        }
    }
}

impl fmt::Display for RadrootsNip10ReplyProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidKind { expected, actual } => {
                write!(
                    formatter,
                    "NIP-10 reply kind must be {expected}, got {actual}"
                )
            }
            Self::ContentTooLarge { max, actual } => {
                write!(
                    formatter,
                    "NIP-10 reply content is {actual} bytes; max is {max}"
                )
            }
            Self::ReplyMarkerMissing => formatter.write_str(
                "NIP-10 reply references require a marked root or valid positional anchors",
            ),
            Self::ReplyMarkerCount => formatter
                .write_str("NIP-10 reply must contain one root and at most one reply marker"),
            Self::EventReferenceShape { tag_index } => {
                write!(formatter, "NIP-10 e tag {tag_index} has an invalid shape")
            }
            Self::EventIdInvalid { tag_index, error } => {
                write!(
                    formatter,
                    "NIP-10 e tag {tag_index} has an invalid event id: {error}"
                )
            }
            Self::ReplyReferenceAmbiguous => {
                formatter.write_str("NIP-10 root and reply references must differ")
            }
            Self::TagElementTooLarge {
                max,
                actual,
                tag_index,
                element_index,
            } => write!(
                formatter,
                "NIP-10 tag {tag_index} element {element_index} is {actual} bytes; max is {max}"
            ),
            Self::TagBytesExceeded { max, actual } => {
                write!(
                    formatter,
                    "NIP-10 reply tag bytes are {actual}; max is {max}"
                )
            }
            Self::EventWireTooLarge { max, actual } => write!(
                formatter,
                "NIP-10 reply compact signed event is {actual} bytes; max is {max}"
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RadrootsNip10ReplyProjectionError {}

/// Projects a signature-and-id verified kind-1 event as a NIP-10 reply.
///
/// Preferred marked references and deprecated positional references are both
/// accepted. This does not prove that referenced events exist or are kind 1.
pub fn project_verified_nip10_reply_event(
    verified_event: &RadrootsSignatureVerifiedEvent,
) -> Result<RadrootsInboundNip10ReplyProjection, RadrootsNip10ReplyProjectionError> {
    let event = verified_event.event();
    project_nip10_reply_parts(
        event.kind_u32(),
        &event.tags_as_vec(),
        event.content(),
        decimal_digits(event.created_at_u64()),
    )
}

fn project_nip10_reply_parts(
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
    created_at_digits: usize,
) -> Result<RadrootsInboundNip10ReplyProjection, RadrootsNip10ReplyProjectionError> {
    if kind != KIND_POST {
        return Err(RadrootsNip10ReplyProjectionError::InvalidKind {
            expected: KIND_POST,
            actual: kind,
        });
    }
    if content.len() > RADROOTS_POST_CONTENT_MAX_BYTES {
        return Err(RadrootsNip10ReplyProjectionError::ContentTooLarge {
            max: RADROOTS_POST_CONTENT_MAX_BYTES,
            actual: content.len(),
        });
    }
    validate_tag_and_wire_budgets(tags, content, created_at_digits)?;

    let e_tags = tags
        .iter()
        .enumerate()
        .filter(|(_, tag)| tag.first().is_some_and(|name| name == "e"))
        .collect::<Vec<_>>();
    if e_tags.is_empty() {
        return Err(RadrootsNip10ReplyProjectionError::ReplyMarkerMissing);
    }
    let has_marked_reference = e_tags
        .iter()
        .any(|(_, tag)| matches!(tag.get(3).map(String::as_str), Some("root" | "reply")));
    let mut diagnostics = Vec::new();
    let (style, root, parent, citations) = if has_marked_reference {
        project_marked_references(&e_tags, &mut diagnostics)?
    } else {
        project_positional_references(&e_tags, &mut diagnostics)?
    };
    let participants = project_participants(tags, &mut diagnostics);
    for reference in core::iter::once(&root)
        .chain(parent.iter())
        .chain(citations.iter())
    {
        if let Some(author) = reference.author_hint()
            && !participants
                .iter()
                .any(|participant| participant.pubkey() == author)
        {
            diagnostics.push(RadrootsNip10ReplyDiagnostic::ReplyAuthorMismatchIgnored {
                tag_index: reference.tag_index,
                raw_tag: reference.raw_tag.clone(),
            });
        }
    }

    Ok(RadrootsInboundNip10ReplyProjection {
        style,
        root,
        parent,
        citations,
        participants,
        diagnostics,
    })
}

type IndexedTag<'a> = (usize, &'a Vec<String>);

fn project_marked_references(
    e_tags: &[IndexedTag<'_>],
    diagnostics: &mut Vec<RadrootsNip10ReplyDiagnostic>,
) -> Result<
    (
        RadrootsNip10ReplyStyle,
        RadrootsInboundNip10EventReference,
        Option<RadrootsInboundNip10EventReference>,
        Vec<RadrootsInboundNip10EventReference>,
    ),
    RadrootsNip10ReplyProjectionError,
> {
    let mut root = None;
    let mut parent = None;
    let mut citations = Vec::new();
    for (tag_index, tag) in e_tags {
        let marker = match tag.get(3).map(String::as_str) {
            Some(marker @ ("root" | "reply")) => marker,
            _ => {
                if let Some(citation) = project_supplemental_reference(*tag_index, tag, diagnostics)
                {
                    citations.push(citation);
                }
                continue;
            }
        };
        if !matches!(tag.len(), 4 | 5) {
            return Err(RadrootsNip10ReplyProjectionError::EventReferenceShape {
                tag_index: *tag_index,
            });
        }
        let reference = parse_event_reference(*tag_index, tag, diagnostics)?;
        match marker {
            "root" if root.is_none() => root = Some(reference),
            "reply" if parent.is_none() => parent = Some(reference),
            _ => return Err(RadrootsNip10ReplyProjectionError::ReplyMarkerCount),
        }
    }
    let Some(root) = root else {
        return Err(RadrootsNip10ReplyProjectionError::ReplyMarkerCount);
    };
    if parent
        .as_ref()
        .is_some_and(|parent| parent.event_id == root.event_id)
    {
        return Err(RadrootsNip10ReplyProjectionError::ReplyReferenceAmbiguous);
    }
    Ok((RadrootsNip10ReplyStyle::Marked, root, parent, citations))
}

fn project_positional_references(
    e_tags: &[IndexedTag<'_>],
    diagnostics: &mut Vec<RadrootsNip10ReplyDiagnostic>,
) -> Result<
    (
        RadrootsNip10ReplyStyle,
        RadrootsInboundNip10EventReference,
        Option<RadrootsInboundNip10EventReference>,
        Vec<RadrootsInboundNip10EventReference>,
    ),
    RadrootsNip10ReplyProjectionError,
> {
    let (root_tag_index, root_tag) = e_tags[0];
    if !is_positional_reference_shape(root_tag) {
        return Err(RadrootsNip10ReplyProjectionError::ReplyMarkerMissing);
    }
    let root = parse_event_reference(root_tag_index, root_tag, diagnostics)?;
    let mut citations = Vec::new();
    for (tag_index, tag) in e_tags.iter().skip(1).take(e_tags.len().saturating_sub(2)) {
        if let Some(citation) = project_supplemental_reference(*tag_index, tag, diagnostics) {
            citations.push(citation);
        }
    }
    let parent = if e_tags.len() > 1 {
        let (parent_tag_index, parent_tag) = e_tags[e_tags.len() - 1];
        if !is_positional_reference_shape(parent_tag) {
            return Err(RadrootsNip10ReplyProjectionError::ReplyMarkerMissing);
        }
        Some(parse_event_reference(
            parent_tag_index,
            parent_tag,
            diagnostics,
        )?)
    } else {
        None
    };
    if parent
        .as_ref()
        .is_some_and(|parent| parent.event_id == root.event_id)
    {
        return Err(RadrootsNip10ReplyProjectionError::ReplyReferenceAmbiguous);
    }
    Ok((
        RadrootsNip10ReplyStyle::LegacyPositional,
        root,
        parent,
        citations,
    ))
}

fn parse_event_reference(
    tag_index: usize,
    tag: &[String],
    diagnostics: &mut Vec<RadrootsNip10ReplyDiagnostic>,
) -> Result<RadrootsInboundNip10EventReference, RadrootsNip10ReplyProjectionError> {
    let event_id = tag
        .get(1)
        .ok_or(RadrootsNip10ReplyProjectionError::EventReferenceShape { tag_index })
        .and_then(|value| {
            RadrootsEventId::parse(value).map_err(|error| {
                RadrootsNip10ReplyProjectionError::EventIdInvalid { tag_index, error }
            })
        })?;
    let relay = match parse_relay_hint(tag.get(2).map(String::as_str)) {
        Ok(relay) => relay,
        Err(error) => {
            diagnostics.push(RadrootsNip10ReplyDiagnostic::ReferenceRelayIgnored {
                tag_index,
                raw_tag: tag.to_vec(),
                error,
            });
            None
        }
    };
    let author_hint = if tag.len() == 5 {
        match RadrootsPublicKey::parse(&tag[4]) {
            Ok(author) => Some(author),
            Err(error) => {
                diagnostics.push(RadrootsNip10ReplyDiagnostic::ReferenceAuthorIgnored {
                    tag_index,
                    raw_tag: tag.to_vec(),
                    error,
                });
                None
            }
        }
    } else {
        None
    };
    Ok(RadrootsInboundNip10EventReference {
        tag_index,
        raw_tag: tag.to_vec(),
        event_id,
        relay,
        author_hint,
    })
}

fn project_supplemental_reference(
    tag_index: usize,
    tag: &[String],
    diagnostics: &mut Vec<RadrootsNip10ReplyDiagnostic>,
) -> Option<RadrootsInboundNip10EventReference> {
    match tag.get(3).map(String::as_str) {
        Some("") if matches!(tag.len(), 4 | 5) => {}
        Some("") => {
            diagnostics.push(RadrootsNip10ReplyDiagnostic::CitationShapeIgnored {
                tag_index,
                raw_tag: tag.to_vec(),
            });
            return None;
        }
        Some(_) => {
            diagnostics.push(RadrootsNip10ReplyDiagnostic::CitationMarkerIgnored {
                tag_index,
                raw_tag: tag.to_vec(),
            });
            return None;
        }
        None if matches!(tag.len(), 2 | 3) => {}
        None => {
            diagnostics.push(RadrootsNip10ReplyDiagnostic::CitationShapeIgnored {
                tag_index,
                raw_tag: tag.to_vec(),
            });
            return None;
        }
    }
    let event_id = match RadrootsEventId::parse(&tag[1]) {
        Ok(event_id) => event_id,
        Err(error) => {
            diagnostics.push(RadrootsNip10ReplyDiagnostic::CitationEventIdIgnored {
                tag_index,
                raw_tag: tag.to_vec(),
                error,
            });
            return None;
        }
    };
    let relay = match parse_relay_hint(tag.get(2).map(String::as_str)) {
        Ok(relay) => relay,
        Err(error) => {
            diagnostics.push(RadrootsNip10ReplyDiagnostic::ReferenceRelayIgnored {
                tag_index,
                raw_tag: tag.to_vec(),
                error,
            });
            None
        }
    };
    let author_hint = if tag.len() == 5 {
        match RadrootsPublicKey::parse(&tag[4]) {
            Ok(author) => Some(author),
            Err(error) => {
                diagnostics.push(RadrootsNip10ReplyDiagnostic::ReferenceAuthorIgnored {
                    tag_index,
                    raw_tag: tag.to_vec(),
                    error,
                });
                None
            }
        }
    } else {
        None
    };
    Some(RadrootsInboundNip10EventReference {
        tag_index,
        raw_tag: tag.to_vec(),
        event_id,
        relay,
        author_hint,
    })
}

fn is_positional_reference_shape(tag: &[String]) -> bool {
    matches!(tag.len(), 2 | 3)
        || matches!(tag.len(), 4 | 5) && tag.get(3).is_some_and(String::is_empty)
}

fn project_participants(
    tags: &[Vec<String>],
    diagnostics: &mut Vec<RadrootsNip10ReplyDiagnostic>,
) -> Vec<RadrootsInboundNip10Participant> {
    let p_tags = tags
        .iter()
        .enumerate()
        .filter(|(_, tag)| tag.first().is_some_and(|name| name == "p"))
        .collect::<Vec<_>>();
    if p_tags.is_empty() {
        diagnostics.push(RadrootsNip10ReplyDiagnostic::ReplyAuthorMissing);
        return Vec::new();
    }

    let mut seen = BTreeSet::new();
    let mut participants = Vec::with_capacity(p_tags.len());
    for (tag_index, tag) in p_tags {
        if !(2..=4).contains(&tag.len()) {
            diagnostics.push(RadrootsNip10ReplyDiagnostic::ReplyAuthorShapeIgnored {
                tag_index,
                raw_tag: tag.clone(),
            });
            if tag.len() < 2 {
                continue;
            }
        }
        let pubkey = match RadrootsPublicKey::parse(&tag[1]) {
            Ok(pubkey) => pubkey,
            Err(error) => {
                diagnostics.push(RadrootsNip10ReplyDiagnostic::ReplyAuthorIgnored {
                    tag_index,
                    raw_tag: tag.clone(),
                    error,
                });
                continue;
            }
        };
        let relay = match parse_relay_hint(tag.get(2).map(String::as_str)) {
            Ok(relay) => relay,
            Err(error) => {
                diagnostics.push(RadrootsNip10ReplyDiagnostic::ReplyAuthorRelayIgnored {
                    tag_index,
                    raw_tag: tag.clone(),
                    error,
                });
                None
            }
        };
        if !seen.insert(pubkey.clone()) {
            diagnostics.push(RadrootsNip10ReplyDiagnostic::ReplyAuthorDuplicateIgnored {
                tag_index,
                raw_tag: tag.clone(),
            });
            continue;
        }
        participants.push(RadrootsInboundNip10Participant {
            tag_index,
            raw_tag: tag.clone(),
            pubkey,
            relay,
        });
    }
    participants
}

fn parse_relay_hint(
    value: Option<&str>,
) -> Result<Option<RadrootsNip10RelayHint>, RadrootsIdParseError> {
    match value {
        None | Some("") => Ok(None),
        Some(value) => RadrootsNip10RelayHint::parse(value).map(Some),
    }
}

fn validate_tag_and_wire_budgets(
    tags: &[Vec<String>],
    content: &str,
    created_at_digits: usize,
) -> Result<(), RadrootsNip10ReplyProjectionError> {
    let mut tag_bytes = 0usize;
    let mut tags_json_bytes = 2usize;
    for (tag_index, tag) in tags.iter().enumerate() {
        if tag_index > 0 {
            tags_json_bytes = tags_json_bytes.saturating_add(1);
        }
        tags_json_bytes = tags_json_bytes.saturating_add(2);
        for (element_index, element) in tag.iter().enumerate() {
            if element.len() > RADROOTS_POST_TAG_ELEMENT_MAX_BYTES {
                return Err(RadrootsNip10ReplyProjectionError::TagElementTooLarge {
                    max: RADROOTS_POST_TAG_ELEMENT_MAX_BYTES,
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
    if tag_bytes > RADROOTS_POST_TAG_TOTAL_MAX_BYTES {
        return Err(RadrootsNip10ReplyProjectionError::TagBytesExceeded {
            max: RADROOTS_POST_TAG_TOTAL_MAX_BYTES,
            actual: tag_bytes,
        });
    }
    let actual = RADROOTS_NIP10_REPLY_SIGNED_EVENT_FIXED_BYTES
        .saturating_add(created_at_digits)
        .saturating_add(tags_json_bytes)
        .saturating_add(canonical_json_string_bytes(content));
    if actual > RADROOTS_POST_EVENT_WIRE_MAX_BYTES {
        return Err(RadrootsNip10ReplyProjectionError::EventWireTooLarge {
            max: RADROOTS_POST_EVENT_WIRE_MAX_BYTES,
            actual,
        });
    }
    Ok(())
}

fn decimal_digits(mut value: u64) -> usize {
    let mut digits = 1usize;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
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
mod tests {
    use super::*;

    #[test]
    fn tolerant_inbound_accepts_blank_content_and_absent_participants() {
        let projection = project_nip10_reply_parts(
            KIND_POST,
            &[vec![
                "e".to_string(),
                "a".repeat(64),
                String::new(),
                "root".to_string(),
            ]],
            "",
            10,
        )
        .expect("optional participants and blank content do not erase a Reply");

        assert!(projection.is_direct());
        assert!(projection.participants().is_empty());
        assert_eq!(
            projection
                .diagnostics()
                .iter()
                .map(RadrootsNip10ReplyDiagnostic::code)
                .collect::<Vec<_>>(),
            vec!["reply_author_missing_ignored"]
        );
        assert_eq!(projection.diagnostics()[0].tag_index(), None);
        assert_eq!(projection.diagnostics()[0].raw_tag(), None);
    }

    #[test]
    fn tolerant_inbound_retains_raw_optional_metadata_and_orders_diagnostics() {
        let root_id = "a".repeat(64);
        let parent_id = "d".repeat(64);
        let participant = "b".repeat(64);
        let parent_author_hint = "c".repeat(64);
        let tags = vec![
            vec![
                "e".to_string(),
                root_id.clone(),
                "https://relay.example".to_string(),
                "root".to_string(),
                "not-a-pubkey".to_string(),
            ],
            vec![
                "e".to_string(),
                parent_id.clone(),
                String::new(),
                "reply".to_string(),
                parent_author_hint,
            ],
            vec!["p".to_string()],
            vec!["p".to_string(), "not-a-pubkey".to_string()],
            vec![
                "p".to_string(),
                participant.clone(),
                "https://relay.example".to_string(),
            ],
            vec!["p".to_string(), participant.clone()],
        ];

        let projection = project_nip10_reply_parts(KIND_POST, &tags, "Reply", 10)
            .expect("malformed optional metadata must not erase a Reply");

        assert!(!projection.is_direct());
        assert_eq!(projection.root().tag_index(), 0);
        assert_eq!(projection.root().raw_tag(), tags[0]);
        assert_eq!(projection.root().event_id().as_str(), root_id);
        assert!(projection.root().relay().is_none());
        assert!(projection.root().author_hint().is_none());
        let parent = projection.reply_reference().expect("reply reference");
        assert_eq!(parent.tag_index(), 1);
        assert_eq!(parent.raw_tag(), tags[1]);
        assert_eq!(parent.event_id().as_str(), parent_id);
        assert_eq!(projection.participants().len(), 1);
        assert_eq!(projection.participants()[0].tag_index(), 4);
        assert_eq!(projection.participants()[0].raw_tag(), tags[4]);
        assert_eq!(projection.participants()[0].pubkey().as_str(), participant);
        assert!(projection.participants()[0].relay().is_none());

        assert_eq!(
            projection
                .diagnostics()
                .iter()
                .map(RadrootsNip10ReplyDiagnostic::code)
                .collect::<Vec<_>>(),
            vec![
                "reply_reference_relay_ignored",
                "reply_reference_author_ignored",
                "reply_author_shape_ignored",
                "reply_author_invalid_ignored",
                "reply_author_relay_ignored",
                "reply_author_duplicate_ignored",
                "reply_author_mismatch_ignored",
            ]
        );
        assert_eq!(
            projection
                .diagnostics()
                .iter()
                .map(RadrootsNip10ReplyDiagnostic::tag_index)
                .collect::<Vec<_>>(),
            vec![
                Some(0),
                Some(0),
                Some(2),
                Some(3),
                Some(4),
                Some(5),
                Some(1),
            ]
        );
        for diagnostic in projection.diagnostics() {
            let Some(tag_index) = diagnostic.tag_index() else {
                continue;
            };
            assert_eq!(diagnostic.raw_tag().expect("source tag"), tags[tag_index]);
        }
    }

    #[test]
    fn inbound_projection_uses_the_canonical_relay_hint_profile() {
        let root_id = "a".repeat(64);
        let root_author = "b".repeat(64);
        for relay in [
            "wss://%65xample.com",
            "wss://127.1",
            "wss://relay.example:01",
            "wss://[2001:0db8::1]",
            "wss://relay.example/%2f",
        ] {
            let root_tag = vec![
                "e".to_string(),
                root_id.clone(),
                relay.to_string(),
                "root".to_string(),
            ];
            let tags = vec![root_tag.clone(), vec!["p".to_string(), root_author.clone()]];
            let projection = project_nip10_reply_parts(KIND_POST, &tags, "Reply", 10)
                .unwrap_or_else(|error| panic!("{relay} must remain advisory: {error}"));

            assert!(projection.root().relay().is_none(), "{relay}");
            assert_eq!(projection.root().raw_tag(), root_tag);
            assert_eq!(
                projection
                    .diagnostics()
                    .iter()
                    .map(RadrootsNip10ReplyDiagnostic::code)
                    .collect::<Vec<_>>(),
                vec!["reply_reference_relay_ignored"],
                "{relay}"
            );
            assert_eq!(
                projection.diagnostics()[0].raw_tag(),
                Some(root_tag.as_slice()),
                "{relay}"
            );
        }

        let tags = vec![
            vec![
                "e".to_string(),
                root_id,
                "wss://[2001:db8::1]:65535/nostr?region=ca-bc".to_string(),
                "root".to_string(),
            ],
            vec![
                "p".to_string(),
                root_author,
                "ws://127.0.0.1:21003".to_string(),
            ],
        ];
        let projection = project_nip10_reply_parts(KIND_POST, &tags, "Reply", 10)
            .expect("canonical reference and participant relays");
        assert_eq!(
            projection.root().relay().expect("root relay").as_str(),
            "wss://[2001:db8::1]:65535/nostr?region=ca-bc"
        );
        assert_eq!(
            projection.participants()[0]
                .relay()
                .expect("participant relay")
                .as_str(),
            "ws://127.0.0.1:21003"
        );
        assert!(projection.diagnostics().is_empty());
    }

    #[test]
    fn inbound_relay_syntax_and_tag_element_budgets_remain_separate() {
        let prefix = "wss://relay.example/";
        let relay = format!(
            "{prefix}{}",
            "a".repeat(RADROOTS_POST_TAG_ELEMENT_MAX_BYTES + 1 - prefix.len())
        );
        assert_eq!(relay.len(), RADROOTS_POST_TAG_ELEMENT_MAX_BYTES + 1);
        RadrootsNip10RelayHint::parse(&relay).expect("relay syntax has no Reply wire budget");

        let tags = vec![
            vec!["e".to_string(), "a".repeat(64), relay, "root".to_string()],
            vec!["p".to_string(), "b".repeat(64)],
        ];
        assert!(matches!(
            project_nip10_reply_parts(KIND_POST, &tags, "Reply", 10),
            Err(RadrootsNip10ReplyProjectionError::TagElementTooLarge {
                max: RADROOTS_POST_TAG_ELEMENT_MAX_BYTES,
                actual,
                tag_index: 0,
                element_index: 2,
            }) if actual == RADROOTS_POST_TAG_ELEMENT_MAX_BYTES + 1
        ));
    }

    #[test]
    fn tolerant_inbound_keeps_reference_ids_and_markers_as_hard_gates() {
        let author = "b".repeat(64);
        for (tag, expected) in [
            (
                vec![
                    "e".to_string(),
                    "not-an-event-id".to_string(),
                    String::new(),
                    "root".to_string(),
                ],
                "reply_event_id_invalid",
            ),
            (
                vec![
                    "e".to_string(),
                    "a".repeat(64),
                    String::new(),
                    "mention".to_string(),
                ],
                "reply_marker_missing",
            ),
        ] {
            let error = project_nip10_reply_parts(
                KIND_POST,
                &[tag, vec!["p".to_string(), author.clone()]],
                "Reply",
                10,
            )
            .unwrap_err();
            assert_eq!(error.code(), expected);
        }
    }

    #[test]
    fn marked_inbound_retains_citations_and_ignores_malformed_supplements() {
        let root_id = "a".repeat(64);
        let citation_id = "c".repeat(64);
        let tags = vec![
            vec!["e".to_string()],
            vec![
                "e".to_string(),
                citation_id.clone(),
                "wss://relay.example".to_string(),
                String::new(),
                "b".repeat(64),
            ],
            vec!["e".to_string(), "not-an-event-id".to_string()],
            vec![
                "e".to_string(),
                "d".repeat(64),
                String::new(),
                "mention".to_string(),
            ],
            vec![
                "e".to_string(),
                root_id.clone(),
                String::new(),
                "root".to_string(),
            ],
        ];

        let projection = project_nip10_reply_parts(KIND_POST, &tags, "Reply", 10)
            .expect("supplemental references must not erase a marked Reply");

        assert_eq!(projection.root().event_id().as_str(), root_id);
        assert_eq!(projection.citations().len(), 1);
        assert_eq!(projection.citations()[0].tag_index(), 1);
        assert_eq!(projection.citations()[0].raw_tag(), tags[1]);
        assert_eq!(projection.citations()[0].event_id().as_str(), citation_id);
        assert_eq!(
            projection.citations()[0]
                .relay()
                .expect("citation relay")
                .as_str(),
            "wss://relay.example"
        );
        assert_eq!(
            projection.citations()[0]
                .author_hint()
                .expect("citation author")
                .as_str(),
            "b".repeat(64)
        );
        assert_eq!(
            projection
                .diagnostics()
                .iter()
                .map(RadrootsNip10ReplyDiagnostic::code)
                .collect::<Vec<_>>(),
            vec![
                "reply_citation_shape_ignored",
                "reply_citation_event_id_ignored",
                "reply_citation_marker_ignored",
                "reply_author_missing_ignored",
                "reply_author_mismatch_ignored",
            ]
        );
        assert_eq!(
            projection
                .diagnostics()
                .iter()
                .map(RadrootsNip10ReplyDiagnostic::tag_index)
                .collect::<Vec<_>>(),
            vec![Some(0), Some(2), Some(3), None, Some(1)]
        );
    }

    #[test]
    fn positional_inbound_accepts_empty_markers_and_tolerates_middle_citations() {
        let root_id = "a".repeat(64);
        let root_author = "b".repeat(64);
        let direct_tags = vec![
            vec![
                "e".to_string(),
                root_id.clone(),
                "wss://root.relay.example".to_string(),
                String::new(),
                root_author.clone(),
            ],
            vec!["p".to_string(), root_author.clone()],
        ];
        let direct = project_nip10_reply_parts(KIND_POST, &direct_tags, "Direct", 10)
            .expect("empty marker and fifth author are valid positional input");
        assert_eq!(direct.style(), RadrootsNip10ReplyStyle::LegacyPositional);
        assert!(direct.is_direct());
        assert_eq!(
            direct
                .root()
                .author_hint()
                .expect("root author hint")
                .as_str(),
            root_author
        );
        assert!(direct.diagnostics().is_empty());

        let parent_id = "c".repeat(64);
        let parent_author = "d".repeat(64);
        let citation_id = "e".repeat(64);
        let citation_author = "f".repeat(64);
        let nested_tags = vec![
            vec![
                "e".to_string(),
                root_id,
                String::new(),
                String::new(),
                root_author.clone(),
            ],
            vec![
                "e".to_string(),
                "1".repeat(64),
                String::new(),
                "mention".to_string(),
            ],
            vec![
                "e".to_string(),
                citation_id.clone(),
                String::new(),
                String::new(),
                citation_author.clone(),
            ],
            vec![
                "e".to_string(),
                parent_id.clone(),
                "https://parent.relay.example".to_string(),
                String::new(),
                parent_author.clone(),
            ],
            vec!["p".to_string(), root_author],
            vec!["p".to_string(), parent_author],
            vec!["p".to_string(), citation_author],
        ];
        let nested = project_nip10_reply_parts(KIND_POST, &nested_tags, "Nested", 10)
            .expect("malformed middle citations must not erase positional anchors");
        assert_eq!(nested.style(), RadrootsNip10ReplyStyle::LegacyPositional);
        assert_eq!(nested.parent().event_id().as_str(), parent_id);
        assert_eq!(nested.citations().len(), 1);
        assert_eq!(nested.citations()[0].tag_index(), 2);
        assert_eq!(nested.citations()[0].event_id().as_str(), citation_id);
        assert_eq!(
            nested
                .diagnostics()
                .iter()
                .map(RadrootsNip10ReplyDiagnostic::code)
                .collect::<Vec<_>>(),
            vec![
                "reply_citation_marker_ignored",
                "reply_reference_relay_ignored",
            ]
        );
        assert_eq!(nested.diagnostics()[0].tag_index(), Some(1));
        assert_eq!(
            nested.diagnostics()[0].raw_tag(),
            Some(nested_tags[1].as_slice())
        );
        assert_eq!(nested.diagnostics()[1].tag_index(), Some(3));
        assert_eq!(
            nested.diagnostics()[1].raw_tag(),
            Some(nested_tags[3].as_slice())
        );
    }
}
